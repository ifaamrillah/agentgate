//! Mesin sinkron Jira (M11). Satu putaran ([`run`]) untuk satu koneksi, di-claim lebih dulu
//! (`jira_connections::ActiveModel::claim_sync`) sehingga worker dan scheduler tidak berjalan
//! bersamaan untuk koneksi yang sama. Dipanggil `workers::jira_sync` (setelah request yang
//! menulis outbox, tombol Sinkron sekarang, Simpan & sinkron) dan task `jira_sync` (scheduler).
//!
//! - **Impor sekali** ([`import`]): semua issue project → tiket (kode Jira = alias), komentar,
//!   lampiran ≤ 25 MB, sprint, penerima lewat peta user, stage lewat peta status tanpa melewati
//!   gate (stage yang didahului gate → stage awal + log). Issue yang sudah tertaut (atau aliasnya
//!   sudah ada) tidak dibuat ulang.
//! - **Mirror** ([`push_outbox`]): event outbox sejak cursor → issue dibuat/diubah, status
//!   ditransisikan, komentar ditambahkan. Field yang sama dengan snapshot tautan tidak dikirim,
//!   komentar yang bertaut tidak dikirim ulang: perubahan yang datang dari Jira tidak memantul.
//!   Putaran pertama dan "Sinkron ulang semua" merekonsiliasi semua tiket ([`reconcile`]) dan
//!   menimpa nilai di Jira.
//! - **Dua arah** ([`pull`]): mirror + JQL `updated >= -Nm`. Field digabung tiga arah (perubahan
//!   terakhir menang); status Jira → `stages::move_to` lewat gate sebagai pemilik koneksi,
//!   ditolak → log "Ditolak gate" dan status Jira dikembalikan ke status stage sekarang.
//!
//! Gangguan sementara (token ditolak, Jira tidak terjangkau) menghentikan putaran tanpa
//! memajukan cursor/watermark; putaran berikutnya mengulang. Penolakan per item (400, 404)
//! dicatat lalu dilewati.

use axum::body::Bytes;
use chrono::{DateTime, Utc};
use loco_rs::storage::Storage;
use sea_orm::{
    ActiveEnum, ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, DbErr,
    EntityTrait, IntoActiveModel, QueryFilter, QueryOrder, TransactionTrait,
};
use uuid::Uuid;

use super::{
    api::{JiraApi, JiraError, JiraIssue},
    issues::{self, SYNCED},
};
use crate::{
    common::settings::Settings,
    models::{
        components, jira_comment_links::{self, CommentOrigin},
        jira_connections::{self, FieldMap, FieldTarget, StatusMap, SyncMode, SyncStats, UserMap},
        jira_links::{self, NewLink, Snapshot},
        jira_sync_logs::{self, Direction, LogArgs, NewLog, SyncResult},
        notifications::{self, NewNotification, NotificationKind},
        outbox_events::{self, WebhookEvent},
        projects, releases, roles, sprints, ticket_codes, ticket_comments, ticket_types, tickets,
        users, workspace_members,
        workspaces::{self, WorkspaceId},
    },
    services::{
        planning::sprints as sprint_svc,
        tickets::{
            TicketFailure, attachments, comments, create::{self, TicketForm},
            stages::{self, Target}, update,
        },
        workspaces::Acting,
    },
};

/// Jenis putaran.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunKind {
    /// Dorong outbox saja (setelah request yang mengubah tiket).
    Push,
    /// Putaran biasa: impor (mode sekali), dorong, dan tarik (dua arah).
    Cycle,
    /// Sinkron ulang semua: rekonsiliasi penuh (menimpa Jira), lalu tarik.
    Full,
}

/// Hasil putaran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Done,
    /// Dijeda, belum siap, atau mode tidak cocok dengan jenis putaran.
    Skipped,
    /// Putaran lain sedang berjalan.
    Busy,
    /// Gangguan sementara; dicatat di `last_error`.
    Failed,
}

/// Event outbox per halaman.
const OUTBOX_BATCH: u64 = 100;

struct Cx<'a> {
    db: &'a DatabaseConnection,
    storage: &'a Storage,
    api: &'a dyn JiraApi,
    conn: jira_connections::Model,
    wid: WorkspaceId,
    project: projects::Model,
    workspace: workspaces::Model,
    owner: users::Model,
    role: roles::Model,
    statuses: StatusMap,
    fields: FieldMap,
    users: UserMap,
    types: Vec<ticket_types::Model>,
    stats: SyncStats,
}

impl Cx<'_> {
    fn acting(&self) -> Acting<'_> {
        Acting {
            workspace: &self.workspace,
            user: &self.owner,
            role: &self.role,
            request_id: None,
        }
    }

    async fn log(&self, log: NewLog) -> Result<(), DbErr> {
        jira_sync_logs::ActiveModel::write(self.db, self.wid, self.conn.id, log).await?;
        Ok(())
    }

    fn type_key(&self, type_id: Uuid) -> &str {
        self.types
            .iter()
            .find(|t| t.id == type_id)
            .map_or("task", |t| t.key.as_str())
    }
}

/// Jalankan satu putaran untuk koneksi `connection_id` dengan klien dari koneksi tersimpan.
///
/// # Errors
///
/// Error database.
pub async fn run(
    db: &DatabaseConnection,
    storage: &Storage,
    settings: &Settings,
    connection_id: Uuid,
    kind: RunKind,
) -> Result<Outcome, DbErr> {
    let Some(conn) = jira_connections::Entity::find_by_id(connection_id).one(db).await? else {
        return Ok(Outcome::Skipped);
    };
    if conn.is_paused() || !conn.is_ready() {
        return Ok(Outcome::Skipped);
    }
    match super::client_for(&conn, settings).await {
        Ok(api) => run_with(db, storage, &api, conn.id, kind, Utc::now()).await,
        Err(code) => {
            fail(db, &conn, code).await?;
            Ok(Outcome::Failed)
        }
    }
}

/// [`run`] dengan klien `api` yang diberikan (tes memakai double atau server palsu).
///
/// # Errors
///
/// Error database.
pub async fn run_with(
    db: &DatabaseConnection,
    storage: &Storage,
    api: &dyn JiraApi,
    connection_id: Uuid,
    kind: RunKind,
    now: DateTime<Utc>,
) -> Result<Outcome, DbErr> {
    let Some(conn) = jira_connections::Entity::find_by_id(connection_id).one(db).await? else {
        return Ok(Outcome::Skipped);
    };
    if conn.is_paused() || !conn.is_ready() {
        return Ok(Outcome::Skipped);
    }
    let mode = conn.mode();
    if kind == RunKind::Push && !mode.pushes() {
        return Ok(Outcome::Skipped);
    }
    if mode == SyncMode::Once && conn.imported_at.is_some() && kind != RunKind::Full {
        return Ok(Outcome::Skipped);
    }
    if !jira_connections::ActiveModel::claim_sync(db, conn.id, now).await? {
        return Ok(Outcome::Busy);
    }
    let result = cycle(db, storage, api, &conn, kind, now).await;
    jira_connections::ActiveModel::release_sync(db, conn.id).await?;
    match result {
        Ok(Ok(stats)) => {
            let links = jira_links::Model::count_of(db, conn.id).await?;
            let mut row = jira_connections::Entity::find_by_id(conn.id)
                .one(db)
                .await?
                .map(IntoActiveModel::into_active_model);
            if let Some(row) = row.as_mut() {
                let mut stats = stats;
                stats.tickets = links;
                row.stats = Set(jira_connections::ActiveModel::json(&stats));
                row.last_sync_at = Set(Some(now.into()));
                row.last_error = Set(String::new());
                if mode == SyncMode::Once {
                    row.imported_at = Set(Some(now.into()));
                }
                row.clone().update(db).await?;
            }
            Ok(Outcome::Done)
        }
        Ok(Err(e)) => {
            fail(db, &conn, super::error_code(&e)).await?;
            Ok(Outcome::Failed)
        }
        Err(e) => Err(e),
    }
}

/// Catat gangguan: `last_error`, log, dan notifikasi `jira_failed` ke pemilik koneksi saat error
/// baru muncul (bukan setiap putaran yang gagal berturut-turut).
async fn fail(db: &DatabaseConnection, conn: &jira_connections::Model, code: &str) -> Result<(), DbErr> {
    let Some(ws) = workspaces::Model::find_by_id(db, conn.workspace_id).await? else {
        return Ok(());
    };
    let wid = ws.workspace_id();
    let first = conn.last_error.is_empty();
    jira_connections::Entity::update_many()
        .col_expr(
            jira_connections::Column::LastError,
            sea_orm::sea_query::Expr::value(code.to_string()),
        )
        .filter(jira_connections::Column::Id.eq(conn.id))
        .exec(db)
        .await?;
    jira_sync_logs::ActiveModel::write(
        db,
        wid,
        conn.id,
        NewLog::new(Direction::Out, SyncResult::Fail, "jira-log-run-failed").args(LogArgs {
            reason: code.to_string(),
            ..LogArgs::default()
        }),
    )
    .await?;
    if first && let Some(owner) = conn.owner_id {
        notifications::ActiveModel::notify(
            db,
            wid,
            NewNotification {
                user_id: owner,
                actor_id: None,
                kind: NotificationKind::JiraFailed,
                ticket_id: None,
                comment_id: None,
                project_id: None,
            },
        )
        .await?;
    }
    tracing::warn!(connection_id = %conn.id, code, "jira sync failed");
    Ok(())
}

type Step = Result<SyncStats, JiraError>;

async fn context<'a>(
    db: &'a DatabaseConnection,
    storage: &'a Storage,
    api: &'a dyn JiraApi,
    conn: &jira_connections::Model,
) -> Result<Option<Cx<'a>>, DbErr> {
    let Some(workspace) = workspaces::Model::find_by_id(db, conn.workspace_id).await? else {
        return Ok(None);
    };
    let wid = workspace.workspace_id();
    let Some(project_id) = conn.project_id else {
        return Ok(None);
    };
    let Some(project) = projects::Model::find_in(db, wid, project_id).await? else {
        return Ok(None);
    };
    let Some(owner_id) = conn.owner_id else {
        return Ok(None);
    };
    let Some(owner) = users::Model::find_by_id(db, owner_id).await? else {
        return Ok(None);
    };
    let Some((_, role)) = workspace_members::Model::find_with_role(db, wid, owner_id).await? else {
        return Ok(None);
    };
    Ok(Some(Cx {
        db,
        storage,
        api,
        conn: conn.clone(),
        wid,
        project,
        workspace,
        owner,
        role,
        statuses: conn.status_map(),
        fields: conn.field_map(),
        users: conn.user_map(),
        types: ticket_types::Model::all_in(db, wid).await?,
        stats: conn.stats(),
    }))
}

async fn cycle(
    db: &DatabaseConnection,
    storage: &Storage,
    api: &dyn JiraApi,
    conn: &jira_connections::Model,
    kind: RunKind,
    now: DateTime<Utc>,
) -> Result<Step, DbErr> {
    let Some(mut cx) = context(db, storage, api, conn).await? else {
        if let Some(ws) = workspaces::Model::find_by_id(db, conn.workspace_id).await? {
            jira_sync_logs::ActiveModel::write(
                db,
                ws.workspace_id(),
                conn.id,
                NewLog::new(Direction::Out, SyncResult::Fail, "jira-log-owner-missing"),
            )
            .await?;
        }
        return Ok(Ok(conn.stats()));
    };
    let mode = conn.mode();
    if mode == SyncMode::Once {
        if let Err(e) = import(&mut cx).await? {
            return Ok(Err(e));
        }
        return Ok(Ok(cx.stats));
    }
    let full = kind == RunKind::Full
        || conn.outbox_cursor().is_none()
        || conn.full_sync_requested_at.is_some();
    // Dua arah: tarik dulu supaya perubahan Jira yang lebih baru tidak ditimpa rekonsiliasi.
    if mode == SyncMode::TwoWay && kind != RunKind::Push && !full {
        if let Err(e) = pull(&mut cx, now).await? {
            return Ok(Err(e));
        }
    }
    let pushed = if full {
        reconcile(&mut cx).await?
    } else {
        push_outbox(&mut cx).await?
    };
    if let Err(e) = pushed {
        return Ok(Err(e));
    }
    if full && mode == SyncMode::TwoWay {
        // Setelah menimpa Jira, watermark tarik dimulai dari sekarang.
        jira_connections::Entity::update_many()
            .col_expr(
                jira_connections::Column::PulledAt,
                sea_orm::sea_query::Expr::value(DateTimeWithTz::from(now)),
            )
            .filter(jira_connections::Column::Id.eq(conn.id))
            .exec(db)
            .await?;
        if conn.pulled_at.is_none() {
            // Putaran pertama dua arah: issue Jira yang belum tertaut ikut ditarik.
            if let Err(e) = pull_all(&mut cx).await? {
                return Ok(Err(e));
            }
        }
    }
    if full {
        jira_connections::Entity::update_many()
            .col_expr(
                jira_connections::Column::FullSyncRequestedAt,
                sea_orm::sea_query::Expr::value(Option::<DateTimeWithTz>::None),
            )
            .filter(jira_connections::Column::Id.eq(conn.id))
            .exec(db)
            .await?;
    }
    Ok(Ok(cx.stats))
}

type DateTimeWithTz = sea_orm::prelude::DateTimeWithTimeZone;

/// Hasil langkah per item: `Err` hanya untuk gangguan sementara (hentikan putaran).
type ItemResult = Result<Result<(), JiraError>, DbErr>;

/// Penolakan per item → log gagal dan lanjut; gangguan sementara → dikembalikan.
async fn soft(cx: &Cx<'_>, e: JiraError, log: NewLog) -> ItemResult {
    if e.is_transient() {
        return Ok(Err(e));
    }
    let detail = match &e {
        JiraError::Rejected(m) => m.clone(),
        other => other.to_string(),
    };
    cx.log(log.detail(&detail)).await?;
    Ok(Ok(()))
}

macro_rules! jira {
    ($cx:expr, $call:expr, $log:expr) => {
        match $call.await {
            Ok(v) => v,
            Err(e) => return soft($cx, e, $log).await,
        }
    };
}

// ---------------------------------------------------------------------------------------------
// Mirror: Agent Gate → Jira

async fn latest_event(
    cx: &Cx<'_>,
) -> Result<Option<outbox_events::Model>, DbErr> {
    outbox_events::Entity::find()
        .filter(outbox_events::Column::WorkspaceId.eq(cx.wid.get()))
        .order_by_desc(outbox_events::Column::CreatedAt)
        .order_by_desc(outbox_events::Column::Id)
        .one(cx.db)
        .await
}

/// Rekonsiliasi penuh: semua tiket project didorong dengan nilai Jira sekarang sebagai dasar
/// (perubahan di Jira ditimpa), komentar yang belum tertaut ditambahkan.
async fn reconcile(cx: &mut Cx<'_>) -> Result<Result<(), JiraError>, DbErr> {
    if let Some(last) = latest_event(cx).await? {
        jira_connections::ActiveModel::advance_cursor(cx.db, cx.conn.id, last.created_at, last.id)
            .await?;
    } else {
        // Belum ada event: cursor awal = sekarang (id nol tidak pernah dipakai uuidv7).
        jira_connections::ActiveModel::advance_cursor(
            cx.db,
            cx.conn.id,
            Utc::now().into(),
            Uuid::nil(),
        )
        .await?;
    }
    let tickets = tickets::Entity::find()
        .filter(tickets::Column::WorkspaceId.eq(cx.wid.get()))
        .filter(tickets::Column::ProjectId.eq(cx.project.id))
        .filter(tickets::Column::DeletedAt.is_null())
        .order_by_asc(tickets::Column::CreatedAt)
        .order_by_asc(tickets::Column::Id)
        .all(cx.db)
        .await?;
    for t in tickets {
        if let Err(e) = push_ticket(cx, &t, true).await? {
            return Ok(Err(e));
        }
        for c in ticket_comments::Model::of_ticket(cx.db, cx.wid, t.id).await? {
            if let Err(e) = push_comment(cx, &c).await? {
                return Ok(Err(e));
            }
        }
    }
    Ok(Ok(()))
}

/// Event outbox sejak cursor; cursor maju per event.
async fn push_outbox(cx: &mut Cx<'_>) -> Result<Result<(), JiraError>, DbErr> {
    loop {
        let conn = jira_connections::Entity::find_by_id(cx.conn.id)
            .one(cx.db)
            .await?
            .ok_or_else(|| DbErr::RecordNotFound("jira connection".into()))?;
        let events =
            outbox_events::Model::since(cx.db, cx.wid, conn.outbox_cursor(), OUTBOX_BATCH).await?;
        if events.is_empty() {
            return Ok(Ok(()));
        }
        for e in &events {
            if e.project_id == Some(cx.project.id)
                && let Err(err) = push_event(cx, e).await?
            {
                return Ok(Err(err));
            }
            jira_connections::ActiveModel::advance_cursor(cx.db, cx.conn.id, e.created_at, e.id)
                .await?;
        }
    }
}

async fn push_event(cx: &mut Cx<'_>, e: &outbox_events::Model) -> ItemResult {
    match e.event() {
        Ok(
            WebhookEvent::TicketCreated
            | WebhookEvent::TicketUpdated
            | WebhookEvent::TicketStageChanged,
        ) => {
            let Some(t) = tickets::Model::find_in(cx.db, cx.wid, e.subject_id).await? else {
                return Ok(Ok(()));
            };
            if t.is_trashed() || t.project_id != cx.project.id {
                return Ok(Ok(()));
            }
            push_ticket(cx, &t, false).await
        }
        Ok(WebhookEvent::CommentCreated) => {
            let Some(c) = ticket_comments::Entity::find_by_id(e.subject_id)
                .one(cx.db)
                .await?
            else {
                return Ok(Ok(()));
            };
            push_comment(cx, &c).await
        }
        _ => Ok(Ok(())),
    }
}

fn mapped_targets(fm: &FieldMap) -> Vec<FieldTarget> {
    SYNCED.into_iter().filter(|t| fm.maps(*t)).collect()
}

fn names(targets: &[FieldTarget]) -> String {
    targets
        .iter()
        .map(|t| t.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Dorong tiket: buat issue bila belum tertaut, ubah field yang berbeda dari dasar, transisikan
/// status bila stage berbeda. `force` = dasar adalah nilai Jira sekarang (rekonsiliasi).
async fn push_ticket(cx: &mut Cx<'_>, t: &tickets::Model, force: bool) -> ItemResult {
    let snap = issues::of_ticket(t);
    let env = cx.api.env();
    let out = || NewLog::new(Direction::Out, SyncResult::Fail, "jira-log-push-failed").ticket(t.id, &t.code);
    let link = match jira_links::Model::of_ticket(cx.db, cx.conn.id, t.id).await? {
        Some(link) => link,
        None => {
            let mut fields = issues::jira_fields(env, &cx.fields, &snap, &mapped_targets(&cx.fields));
            fields["project"] = serde_json::json!({ "key": cx.conn.jira_project_key });
            fields["issuetype"] =
                serde_json::json!({ "name": issues::type_name(cx.type_key(t.type_id)) });
            if fields.get("summary").is_none() {
                fields["summary"] = serde_json::json!(t.title);
            }
            let created = jira!(cx, cx.api.create_issue(fields), out());
            let link = jira_links::ActiveModel::link(
                cx.db,
                NewLink {
                    workspace_id: cx.wid,
                    connection_id: cx.conn.id,
                    ticket_id: t.id,
                    issue_id: created.id.clone(),
                    issue_key: created.key.clone(),
                    jira_status_id: String::new(),
                    snapshot: snap.clone(),
                },
            )
            .await?;
            cx.log(
                NewLog::new(Direction::Out, SyncResult::Ok, "jira-log-issue-created")
                    .ticket(t.id, &t.code)
                    .issue(&created.key),
            )
            .await?;
            return push_status(cx, t, link, None).await;
        }
    };
    let (base, jira_status) = if force {
        let issue = jira!(
            cx,
            cx.api.issue(&link.issue_key, &issues::wanted(&cx.fields)),
            out().issue(&link.issue_key)
        );
        (issues::of_issue(&issue, &cx.fields), Some(issue.status_id))
    } else {
        (link.snapshot(), None)
    };
    let changed = issues::differing(&cx.fields, &base, &snap);
    let link = if changed.is_empty() {
        link
    } else {
        jira!(
            cx,
            cx.api.update_issue(
                &link.issue_key,
                issues::jira_fields(env, &cx.fields, &snap, &changed)
            ),
            out().issue(&link.issue_key)
        );
        cx.log(
            NewLog::new(Direction::Out, SyncResult::Ok, "jira-log-issue-updated")
                .ticket(t.id, &t.code)
                .issue(&link.issue_key)
                .detail(&names(&changed)),
        )
        .await?;
        let status = link.jira_status_id.clone();
        jira_links::ActiveModel::agree(cx.db, link, snap, &status).await?
    };
    push_status(cx, t, link, jira_status).await
}

/// Transisi Jira ke status yang dipetakan ke stage tiket. `actual` = status Jira sekarang bila
/// diketahui (rekonsiliasi), selain itu status terakhir di tautan.
async fn push_status(
    cx: &mut Cx<'_>,
    t: &tickets::Model,
    link: jira_links::Model,
    actual: Option<String>,
) -> ItemResult {
    let current = actual.unwrap_or_else(|| link.jira_status_id.clone());
    if cx.statuses.stage_of(&current) == Some(t.stage_key.as_str()) {
        if current != link.jira_status_id {
            let snap = link.snapshot();
            jira_links::ActiveModel::agree(cx.db, link, snap, &current).await?;
        }
        return Ok(Ok(()));
    }
    let Some(target) = cx.statuses.status_for(&t.stage_key, &current).cloned() else {
        cx.log(
            NewLog::new(Direction::Out, SyncResult::Fail, "jira-log-stage-unmapped")
                .ticket(t.id, &t.code)
                .issue(&link.issue_key)
                .args(LogArgs {
                    stage: t.stage_key.clone(),
                    ..LogArgs::default()
                }),
        )
        .await?;
        return Ok(Ok(()));
    };
    if !move_jira(cx, &link, &target.id, t).await? .is_ok_and(|moved| moved) {
        return Ok(Ok(()));
    }
    cx.log(
        NewLog::new(Direction::Out, SyncResult::Ok, "jira-log-status-pushed")
            .ticket(t.id, &t.code)
            .issue(&link.issue_key)
            .args(LogArgs {
                status: target.name.clone(),
                ..LogArgs::default()
            }),
    )
    .await?;
    let snap = link.snapshot();
    jira_links::ActiveModel::agree(cx.db, link, snap, &target.id).await?;
    Ok(Ok(()))
}

/// Jalankan transisi Jira ke status `to`. `Ok(Ok(false))` = tidak ada transisi yang menuju ke
/// sana (dicatat). Gangguan sementara → `Ok(Err)`.
async fn move_jira(
    cx: &Cx<'_>,
    link: &jira_links::Model,
    to: &str,
    t: &tickets::Model,
) -> Result<Result<bool, JiraError>, DbErr> {
    let fail = || {
        NewLog::new(Direction::Out, SyncResult::Fail, "jira-log-transition-failed")
            .ticket(t.id, &t.code)
            .issue(&link.issue_key)
            .args(LogArgs {
                status: cx.statuses.name_of(to),
                ..LogArgs::default()
            })
    };
    let transitions = match cx.api.transitions(&link.issue_key).await {
        Ok(list) => list,
        Err(e) => return soft(cx, e, fail()).await.map(|r| r.map(|()| false)),
    };
    let Some(tr) = transitions.iter().find(|tr| tr.to_id == to) else {
        cx.log(fail()).await?;
        return Ok(Ok(false));
    };
    match cx.api.transition(&link.issue_key, &tr.id).await {
        Ok(()) => Ok(Ok(true)),
        Err(e) => soft(cx, e, fail()).await.map(|r| r.map(|()| false)),
    }
}

/// Dorong komentar Agent Gate yang belum tertaut.
async fn push_comment(cx: &mut Cx<'_>, c: &ticket_comments::Model) -> ItemResult {
    if jira_comment_links::Model::of_comment(cx.db, c.id).await?.is_some() {
        return Ok(Ok(()));
    }
    let Some(t) = tickets::Model::find_in(cx.db, cx.wid, c.ticket_id).await? else {
        return Ok(Ok(()));
    };
    if t.project_id != cx.project.id || t.is_trashed() {
        return Ok(Ok(()));
    }
    let link = match jira_links::Model::of_ticket(cx.db, cx.conn.id, t.id).await? {
        Some(l) => l,
        None => {
            if let Err(e) = push_ticket(cx, &t, false).await? {
                return Ok(Err(e));
            }
            match jira_links::Model::of_ticket(cx.db, cx.conn.id, t.id).await? {
                Some(l) => l,
                None => return Ok(Ok(())),
            }
        }
    };
    let author = match c.author_id {
        Some(id) => users::Model::find_by_id(cx.db, id).await?.map(|u| u.name),
        None => None,
    }
    .unwrap_or_else(|| "Agent Gate".to_string());
    let body = format!("**{author}** (Agent Gate):\n\n{}", c.body);
    let added = jira!(
        cx,
        cx.api.add_comment(&link.issue_key, &body),
        NewLog::new(Direction::Out, SyncResult::Fail, "jira-log-comment-failed")
            .ticket(t.id, &t.code)
            .issue(&link.issue_key)
    );
    jira_comment_links::ActiveModel::record(
        cx.db,
        cx.wid,
        cx.conn.id,
        c.id,
        &added.id,
        CommentOrigin::AgentGate,
    )
    .await?;
    cx.log(
        NewLog::new(Direction::Out, SyncResult::Ok, "jira-log-comment-pushed")
            .ticket(t.id, &t.code)
            .issue(&link.issue_key),
    )
    .await?;
    Ok(Ok(()))
}

// ---------------------------------------------------------------------------------------------
// Jira → Agent Gate

/// Semua issue hasil `jql` (berhalaman).
async fn each_issue(
    cx: &mut Cx<'_>,
    jql: &str,
    mut f: impl for<'b> FnMut(&'b mut Cx<'_>, JiraIssue) -> std::pin::Pin<Box<dyn std::future::Future<Output = ItemResult> + Send + 'b>>,
) -> ItemResult {
    let fields = issues::wanted(&cx.fields);
    let mut next: Option<String> = None;
    loop {
        let page = match cx.api.search(jql, &fields, next.as_deref()).await {
            Ok(p) => p,
            Err(e) => {
                return soft(
                    cx,
                    e,
                    NewLog::new(Direction::In, SyncResult::Fail, "jira-log-search-failed"),
                )
                .await;
            }
        };
        for issue in page.issues {
            if let Err(e) = f(cx, issue).await? {
                return Ok(Err(e));
            }
        }
        match page.next {
            Some(n) => next = Some(n),
            None => return Ok(Ok(())),
        }
    }
}

fn project_jql(key: &str) -> String {
    let safe: String = key
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    format!("project = \"{safe}\"")
}

/// Tarik perubahan sejak watermark (`updated >= -Nm`, relatif supaya tidak bergantung zona
/// waktu profil Jira; tumpang tindih 2 menit, issue yang sudah sama dilewati).
async fn pull(cx: &mut Cx<'_>, now: DateTime<Utc>) -> Result<Result<(), JiraError>, DbErr> {
    let since = cx.conn.pulled_at.map(|t| {
        let t: DateTime<Utc> = t.into();
        (now - t).num_minutes().max(0) + 2
    });
    let jql = match since {
        Some(m) => format!(
            "{} AND updated >= -{m}m ORDER BY updated ASC",
            project_jql(&cx.conn.jira_project_key)
        ),
        None => format!("{} ORDER BY updated ASC", project_jql(&cx.conn.jira_project_key)),
    };
    if let Err(e) = each_issue(cx, &jql, |cx, issue| Box::pin(pull_issue(cx, issue))).await? {
        return Ok(Err(e));
    }
    jira_connections::Entity::update_many()
        .col_expr(
            jira_connections::Column::PulledAt,
            sea_orm::sea_query::Expr::value(DateTimeWithTz::from(now)),
        )
        .filter(jira_connections::Column::Id.eq(cx.conn.id))
        .exec(cx.db)
        .await?;
    Ok(Ok(()))
}

/// Tarik semua issue project (putaran pertama dua arah).
async fn pull_all(cx: &mut Cx<'_>) -> Result<Result<(), JiraError>, DbErr> {
    let jql = format!("{} ORDER BY created ASC", project_jql(&cx.conn.jira_project_key));
    each_issue(cx, &jql, |cx, issue| Box::pin(pull_issue(cx, issue))).await
}

async fn pull_issue(cx: &mut Cx<'_>, issue: JiraIssue) -> ItemResult {
    let jira = issues::of_issue(&issue, &cx.fields);
    let Some(link) = jira_links::Model::of_issue(cx.db, cx.conn.id, &issue.id).await? else {
        // Issue baru di Jira → tiket baru di stage awal.
        let Some(t) = create_ticket(cx, &issue, false).await? else {
            return Ok(Ok(()));
        };
        cx.log(
            NewLog::new(Direction::In, SyncResult::Ok, "jira-log-ticket-created")
                .ticket(t.id, &t.code)
                .issue(&issue.key),
        )
        .await?;
        let link = jira_links::Model::of_ticket(cx.db, cx.conn.id, t.id)
            .await?
            .ok_or_else(|| DbErr::RecordNotFound("jira link".into()))?;
        if let Err(e) = pull_status(cx, &issue, link, &t).await? {
            return Ok(Err(e));
        }
        return pull_comments(cx, &t, &issue.key).await;
    };
    let Some(t) = tickets::Model::find_in(cx.db, cx.wid, link.ticket_id).await? else {
        return Ok(Ok(()));
    };
    if t.is_trashed() {
        return Ok(Ok(()));
    }
    let base = link.snapshot();
    let ag = issues::of_ticket(&t);
    let ticket_at: DateTime<Utc> = t.updated_at.into();
    let jira_newer = issue.updated.is_some_and(|u| u > ticket_at);
    let merge = issues::merge(&cx.fields, &base, &ag, &jira, jira_newer);
    let mut t = t;
    if !merge.to_ticket.is_empty() {
        match apply(cx, &t, &merge.merged, &merge.to_ticket).await? {
            Ok(updated) => {
                cx.log(
                    NewLog::new(Direction::In, SyncResult::Ok, "jira-log-ticket-updated")
                        .ticket(t.id, &t.code)
                        .issue(&issue.key)
                        .detail(&names(&merge.to_ticket)),
                )
                .await?;
                t = updated;
            }
            Err(code) => {
                cx.log(
                    NewLog::new(Direction::In, SyncResult::Fail, "jira-log-pull-failed")
                        .ticket(t.id, &t.code)
                        .issue(&issue.key)
                        .args(LogArgs {
                            reason: code,
                            ..LogArgs::default()
                        }),
                )
                .await?;
                return Ok(Ok(()));
            }
        }
    }
    if !merge.to_jira.is_empty() {
        let env = cx.api.env();
        jira!(
            cx,
            cx.api.update_issue(
                &link.issue_key,
                issues::jira_fields(env, &cx.fields, &merge.merged, &merge.to_jira)
            ),
            NewLog::new(Direction::Out, SyncResult::Fail, "jira-log-push-failed")
                .ticket(t.id, &t.code)
                .issue(&link.issue_key)
        );
        cx.log(
            NewLog::new(Direction::Out, SyncResult::Ok, "jira-log-issue-updated")
                .ticket(t.id, &t.code)
                .issue(&link.issue_key)
                .detail(&names(&merge.to_jira)),
        )
        .await?;
    }
    // Snapshot = nilai tiket setelah ditulis (normalisasi Agent Gate ikut tercatat).
    let agreed = if merge.to_ticket.is_empty() {
        merge.merged.clone()
    } else {
        issues::of_ticket(&t)
    };
    let status = link.jira_status_id.clone();
    let link = jira_links::ActiveModel::agree(cx.db, link, agreed, &status).await?;
    if let Err(e) = pull_status(cx, &issue, link, &t).await? {
        return Ok(Err(e));
    }
    pull_comments(cx, &t, &issue.key).await
}

/// Status Jira berubah → pindah stage lewat gate; ditolak → kembalikan status Jira.
async fn pull_status(
    cx: &mut Cx<'_>,
    issue: &JiraIssue,
    link: jira_links::Model,
    t: &tickets::Model,
) -> ItemResult {
    if issue.status_id.is_empty() || issue.status_id == link.jira_status_id {
        return Ok(Ok(()));
    }
    let wanted = cx.statuses.stage_of(&issue.status_id).map(str::to_string);
    if wanted.as_deref() == Some(t.stage_key.as_str()) {
        let snap = link.snapshot();
        jira_links::ActiveModel::agree(cx.db, link, snap, &issue.status_id).await?;
        return Ok(Ok(()));
    }
    let rejected: Option<String> = match &wanted {
        None => Some("jira-reason-unmapped".to_string()),
        Some(stage) => {
            let acting = cx.acting();
            match stages::move_to(cx.db, &acting, t.id, Target::Stage(stage.clone()), None).await
            {
                Ok(moved) => {
                    cx.log(
                        NewLog::new(Direction::In, SyncResult::Ok, "jira-log-stage-moved")
                            .ticket(t.id, &t.code)
                            .issue(&issue.key)
                            .args(LogArgs {
                                status: issue.status_name.clone(),
                                stage: moved.stage_key.clone(),
                                ..LogArgs::default()
                            }),
                    )
                    .await?;
                    let snap = link.snapshot();
                    jira_links::ActiveModel::agree(cx.db, link, snap, &issue.status_id).await?;
                    return Ok(Ok(()));
                }
                Err(TicketFailure::Invalid(errors)) => Some(
                    errors
                        .errors
                        .values()
                        .find_map(|es| es.first().map(|e| e.code.to_string()))
                        .unwrap_or_else(|| stages::GATES_UNMET.to_string()),
                ),
                Err(TicketFailure::NotFound) => Some("jira-reason-no-access".to_string()),
                Err(TicketFailure::Db(e)) => return Err(e),
            }
        }
    };
    let reason = rejected.unwrap_or_default();
    // Kembalikan status Jira ke status stage sekarang (status/stage selalu ikut Agent Gate).
    let back = cx.statuses.status_for(&t.stage_key, &link.jira_status_id).cloned();
    let mut back_name = String::new();
    if let Some(b) = &back {
        match move_jira(cx, &link, &b.id, t).await? {
            Ok(true) => back_name.clone_from(&b.name),
            Ok(false) => {}
            Err(e) => return Ok(Err(e)),
        }
    }
    cx.log(
        NewLog::new(
            Direction::In,
            if wanted.is_some() { SyncResult::Gate } else { SyncResult::Fail },
            if back_name.is_empty() {
                "jira-log-gate-rejected-kept"
            } else {
                "jira-log-gate-rejected"
            },
        )
        .ticket(t.id, &t.code)
        .issue(&issue.key)
        .args(LogArgs {
            status: issue.status_name.clone(),
            reason,
            back: back_name.clone(),
            ..LogArgs::default()
        }),
    )
    .await?;
    let status = back
        .filter(|_| !back_name.is_empty())
        .map_or_else(|| issue.status_id.clone(), |b| b.id);
    let snap = link.snapshot();
    jira_links::ActiveModel::agree(cx.db, link, snap, &status).await?;
    Ok(Ok(()))
}

/// Komentar Jira yang belum tertaut → komentar tiket (atas nama pemilik koneksi, penulis Jira
/// disebut di baris pertama).
async fn pull_comments(cx: &mut Cx<'_>, t: &tickets::Model, key: &str) -> ItemResult {
    let list = jira!(
        cx,
        cx.api.comments(key),
        NewLog::new(Direction::In, SyncResult::Fail, "jira-log-comment-failed")
            .ticket(t.id, &t.code)
            .issue(key)
    );
    for c in list {
        if c.id.is_empty()
            || jira_comment_links::Model::of_jira(cx.db, cx.conn.id, &c.id)
                .await?
                .is_some()
        {
            continue;
        }
        let author = if c.author.name.is_empty() {
            "Jira".to_string()
        } else {
            c.author.name.clone()
        };
        let body = format!("**{author}** (Jira):\n\n{}", c.body.trim());
        let acting = cx.acting();
        match comments::post(cx.db, &acting, t.id, &body).await {
            Ok(comment) => {
                jira_comment_links::ActiveModel::record(
                    cx.db,
                    cx.wid,
                    cx.conn.id,
                    comment.id,
                    &c.id,
                    CommentOrigin::Jira,
                )
                .await?;
                cx.stats.comments += 1;
                cx.log(
                    NewLog::new(Direction::In, SyncResult::Ok, "jira-log-comment-pulled")
                        .ticket(t.id, &t.code)
                        .issue(key),
                )
                .await?;
            }
            Err(TicketFailure::Db(e)) => return Err(e),
            Err(_) => {
                cx.log(
                    NewLog::new(Direction::In, SyncResult::Fail, "jira-log-comment-failed")
                        .ticket(t.id, &t.code)
                        .issue(key),
                )
                .await?;
            }
        }
    }
    Ok(Ok(()))
}

/// Form tiket dari keadaan tiket sekarang (bentuk sama dengan form detail).
fn form_of(t: &tickets::Model, parent: Option<&str>) -> TicketForm {
    TicketForm {
        input: tickets::TicketInput {
            title: t.title.clone(),
            description: t.description.clone(),
            status: t.status.clone(),
            priority: t.priority.clone(),
            labels: t.labels.join(", "),
            due_on: t.due_on.map(|d| d.to_string()).unwrap_or_default(),
            story_points: t.story_points.map(|p| p.to_string()).unwrap_or_default(),
        },
        type_id: t.type_id.to_string(),
        assignee: t.assignee_id.map(|id| id.to_string()).unwrap_or_default(),
        parent: parent.unwrap_or_default().to_string(),
        custom: t.custom_values().0,
        stage: String::new(),
        sprint: Some(t.sprint_id.map(|id| id.to_string()).unwrap_or_default()),
        release: Some(t.release_id.map(|id| id.to_string()).unwrap_or_default()),
        component: Some(t.component_id.map(|id| id.to_string()).unwrap_or_default()),
    }
}

fn put_snapshot(form: &mut TicketForm, s: &Snapshot, targets: &[FieldTarget]) {
    for t in targets {
        match t {
            FieldTarget::Title => form.input.title.clone_from(&s.title),
            FieldTarget::Description => form.input.description.clone_from(&s.description),
            FieldTarget::Labels => form.input.labels = s.labels.join(", "),
            FieldTarget::Priority => form.input.priority.clone_from(&s.priority),
            FieldTarget::DueOn => form.input.due_on = s.due_on.clone().unwrap_or_default(),
            FieldTarget::StoryPoints => {
                form.input.story_points = s.story_points.map(|p| p.to_string()).unwrap_or_default();
            }
            _ => {}
        }
    }
}

/// Tulis nilai Jira ke tiket lewat service ubah tiket. `Err(kode)` = ditolak validasi.
async fn apply(
    cx: &Cx<'_>,
    t: &tickets::Model,
    s: &Snapshot,
    targets: &[FieldTarget],
) -> Result<Result<tickets::Model, String>, DbErr> {
    let parent = match t.parent_id {
        Some(id) => tickets::Model::find_in(cx.db, cx.wid, id).await?.map(|p| p.code),
        None => None,
    };
    let mut form = form_of(t, parent.as_deref());
    put_snapshot(&mut form, s, targets);
    let acting = cx.acting();
    match update::run(cx.db, &acting, t.id, &form).await {
        Ok(t) => Ok(Ok(t)),
        Err(TicketFailure::Db(e)) => Err(e),
        Err(e) => Ok(Err(first_code(&e))),
    }
}

fn first_code(e: &TicketFailure) -> String {
    match e {
        TicketFailure::Invalid(errors) => errors
            .errors
            .values()
            .find_map(|es| es.first().map(|e| e.code.to_string()))
            .unwrap_or_default(),
        TicketFailure::NotFound => "jira-reason-no-access".into(),
        TicketFailure::Db(_) => String::new(),
    }
}

// ---------------------------------------------------------------------------------------------
// Impor sekali

async fn import(cx: &mut Cx<'_>) -> Result<Result<(), JiraError>, DbErr> {
    let jql = format!("{} ORDER BY created ASC", project_jql(&cx.conn.jira_project_key));
    each_issue(cx, &jql, |cx, issue| Box::pin(import_issue(cx, issue))).await
}

async fn import_issue(cx: &mut Cx<'_>, issue: JiraIssue) -> ItemResult {
    if jira_links::Model::of_issue(cx.db, cx.conn.id, &issue.id)
        .await?
        .is_some()
    {
        return Ok(Ok(()));
    }
    let Some(t) = create_ticket(cx, &issue, true).await? else {
        return Ok(Ok(()));
    };
    cx.log(
        NewLog::new(Direction::In, SyncResult::Ok, "jira-log-ticket-imported")
            .ticket(t.id, &t.code)
            .issue(&issue.key),
    )
    .await?;
    if let Err(e) = pull_comments(cx, &t, &issue.key).await? {
        return Ok(Err(e));
    }
    import_attachments(cx, &t, &issue).await
}

/// Buat (atau tautkan ulang lewat alias) tiket dari issue. `mapped_stage` = pakai peta status
/// (impor); `false` = stage awal (issue baru dari sinkron dua arah). `None` = dilewati (log).
async fn create_ticket(
    cx: &mut Cx<'_>,
    issue: &JiraIssue,
    mapped_stage: bool,
) -> Result<Option<tickets::Model>, DbErr> {
    // Alias sudah ada (koneksi diputus lalu dihubungkan lagi): tautkan, jangan buat ulang.
    if let Some(code) = ticket_codes::Model::find(cx.db, cx.wid, &issue.key).await?
        && let Some(t) = tickets::Model::find_in(cx.db, cx.wid, code.ticket_id).await?
        && t.project_id == cx.project.id
    {
        if jira_links::Model::of_ticket(cx.db, cx.conn.id, t.id).await?.is_none() {
            jira_links::ActiveModel::link(cx.db, new_link(cx, &t, issue)).await?;
        }
        return Ok(None);
    }
    let snap = issues::of_issue(issue, &cx.fields);
    let mut form = TicketForm {
        input: tickets::TicketInput {
            title: snap.title.clone(),
            description: snap.description.clone(),
            status: String::new(),
            priority: snap.priority.clone(),
            labels: snap.labels.join(", "),
            due_on: snap.due_on.clone().unwrap_or_default(),
            story_points: snap.story_points.map(|p| p.to_string()).unwrap_or_default(),
        },
        type_id: cx
            .types
            .iter()
            .find(|t| t.key == issues::type_key(issue))
            .map(|t| t.id.to_string())
            .unwrap_or_default(),
        ..TicketForm::default()
    };
    // Penerima lewat peta user; tidak dipetakan → kosong + log.
    if let Some(account) = issues::assignee(issue, cx.api.env()) {
        match cx.users.user_of(&account) {
            Some(id) => form.assignee = id.to_string(),
            None => {
                cx.log(
                    NewLog::new(Direction::In, SyncResult::Fail, "jira-log-user-unmapped")
                        .issue(&issue.key)
                        .args(LogArgs {
                            email: issues::assignee_label(issue),
                            ..LogArgs::default()
                        }),
                )
                .await?;
            }
        }
    }
    if mapped_stage {
        planning_fields(cx, issue, &mut form).await?;
        form.stage = cx
            .statuses
            .stage_of(&issue.status_id)
            .unwrap_or_default()
            .to_string();
    }
    let mut created = insert_ticket(cx, &form, &issue.key).await?;
    if let Err(code) = &created
        && !form.stage.is_empty()
        && code == stages::JUMP_ADMIN_ONLY
    {
        // Stage tujuan didahului gate: masuk stage awal, tanpa melewati gate.
        cx.log(
            NewLog::new(Direction::In, SyncResult::Gate, "jira-log-stage-gated")
                .issue(&issue.key)
                .args(LogArgs {
                    status: issue.status_name.clone(),
                    stage: form.stage.clone(),
                    ..LogArgs::default()
                }),
        )
        .await?;
        form.stage = String::new();
        created = insert_ticket(cx, &form, &issue.key).await?;
    }
    if let Err(code) = &created
        && (code == tickets::ASSIGNEE_INVALID || code.starts_with("planning-"))
    {
        form.assignee = String::new();
        form.sprint = None;
        created = insert_ticket(cx, &form, &issue.key).await?;
    }
    match created {
        Ok(t) => {
            cx.stats.tickets += 1;
            let mut link = new_link(cx, &t, issue);
            if !mapped_stage {
                // Status Jira diproses setelahnya (pull_status) supaya gate tetap ditegakkan.
                link.jira_status_id = String::new();
            }
            jira_links::ActiveModel::link(cx.db, link).await?;
            Ok(Some(t))
        }
        Err(code) => {
            cx.log(
                NewLog::new(Direction::In, SyncResult::Fail, "jira-log-import-failed")
                    .issue(&issue.key)
                    .args(LogArgs {
                        reason: code,
                        ..LogArgs::default()
                    }),
            )
            .await?;
            Ok(None)
        }
    }
}

fn new_link(cx: &Cx<'_>, t: &tickets::Model, issue: &JiraIssue) -> NewLink {
    NewLink {
        workspace_id: cx.wid,
        connection_id: cx.conn.id,
        ticket_id: t.id,
        issue_id: issue.id.clone(),
        issue_key: issue.key.clone(),
        jira_status_id: issue.status_id.clone(),
        snapshot: issues::of_ticket(t),
    }
}

/// Sprint (dibuat bila belum ada), Fix Version/s dan Components (dicocokkan lewat nama),
/// field custom.
async fn planning_fields(
    cx: &mut Cx<'_>,
    issue: &JiraIssue,
    form: &mut TicketForm,
) -> Result<(), DbErr> {
    if let Some(sp) = issues::sprint(issue, &cx.fields) {
        let existing = sprints::Model::of_project(cx.db, cx.wid, cx.project.id)
            .await?
            .into_iter()
            .find(|s| s.name.eq_ignore_ascii_case(&sp.name));
        let id = match existing {
            Some(s) => Some(s.id),
            None => {
                let today = Utc::now().date_naive();
                let starts = sp.starts_on.clone().unwrap_or_else(|| today.to_string());
                let ends = sp.ends_on.clone().unwrap_or_else(|| {
                    chrono::NaiveDate::parse_from_str(&starts, "%Y-%m-%d")
                        .unwrap_or(today)
                        .checked_add_signed(chrono::Duration::days(sprint_svc::DEFAULT_DAYS))
                        .unwrap_or(today)
                        .to_string()
                });
                let acting = cx.acting();
                match sprint_svc::create(
                    cx.db,
                    &acting,
                    cx.project.id,
                    &sprint_svc::SprintInput {
                        name: sp.name.clone(),
                        goal: String::new(),
                        starts_on: starts,
                        ends_on: ends,
                        capacity: String::new(),
                    },
                )
                .await
                {
                    Ok(s) => {
                        cx.stats.sprints += 1;
                        Some(s.id)
                    }
                    Err(_) => None,
                }
            }
        };
        form.sprint = id.map(|id| id.to_string());
    }
    if let Some(name) = issues::first_name(issue, &cx.fields, FieldTarget::Release) {
        form.release = releases::Model::of_project(cx.db, cx.wid, cx.project.id)
            .await?
            .into_iter()
            .find(|r| r.name.eq_ignore_ascii_case(&name))
            .map(|r| r.id.to_string());
    }
    if let Some(name) = issues::first_name(issue, &cx.fields, FieldTarget::Component) {
        form.component = components::Model::of_project(cx.db, cx.wid, cx.project.id)
            .await?
            .into_iter()
            .find(|c| c.name.eq_ignore_ascii_case(&name))
            .map(|c| c.id.to_string());
    }
    for f in &cx.fields.fields {
        if let FieldTarget::Custom(id) = f.target
            && let Some(v) = issues::custom_value(issue, &cx.fields, id)
        {
            form.custom.insert(id, v);
        }
    }
    Ok(())
}

/// Buat tiket + alias kode Jira di satu transaksi; notifikasi per tiket dihapus (impor massal
/// tidak membanjiri kotak masuk). `Err(kode)` = ditolak validasi.
async fn insert_ticket(
    cx: &Cx<'_>,
    form: &TicketForm,
    alias: &str,
) -> Result<Result<tickets::Model, String>, DbErr> {
    let acting = cx.acting();
    let txn = cx.db.begin().await?;
    let ticket = match create::run_in(&txn, &acting, cx.project.id, form).await {
        Ok(t) => t,
        Err(TicketFailure::Db(e)) => return Err(e),
        Err(e) => return Ok(Err(first_code(&e))),
    };
    if !ticket_codes::ActiveModel::claim(&txn, cx.wid, ticket.id, alias).await? {
        tracing::info!(alias, "jira alias already used; ticket kept without alias");
    }
    notifications::Entity::delete_many()
        .filter(notifications::Column::WorkspaceId.eq(cx.wid.get()))
        .filter(notifications::Column::TicketId.eq(ticket.id))
        .exec(&txn)
        .await?;
    txn.commit().await?;
    Ok(Ok(ticket))
}

/// Lampiran ≤ 25 MB diunduh ke storage; lebih besar dilewati + log.
async fn import_attachments(cx: &mut Cx<'_>, t: &tickets::Model, issue: &JiraIssue) -> ItemResult {
    for a in issues::attachments(issue) {
        let mb = |n: u64| format!("{:.0}", n as f64 / 1024.0 / 1024.0);
        if usize::try_from(a.size).unwrap_or(usize::MAX) > super::MAX_ATTACHMENT_BYTES {
            cx.log(
                NewLog::new(Direction::In, SyncResult::Fail, "jira-log-attachment-skipped")
                    .ticket(t.id, &t.code)
                    .issue(&issue.key)
                    .args(LogArgs {
                        detail: a.filename.clone(),
                        size: mb(a.size),
                        ..LogArgs::default()
                    }),
            )
            .await?;
            continue;
        }
        let fail = NewLog::new(Direction::In, SyncResult::Fail, "jira-log-attachment-failed")
            .ticket(t.id, &t.code)
            .issue(&issue.key)
            .args(LogArgs {
                detail: a.filename.clone(),
                ..LogArgs::default()
            });
        let data = match cx.api.download(&a.url, super::MAX_ATTACHMENT_BYTES).await {
            Ok(d) => d,
            Err(e) => {
                if let Err(e) = soft(cx, e, fail).await? {
                    return Ok(Err(e));
                }
                continue;
            }
        };
        let acting = cx.acting();
        let mime = (!a.mime.is_empty()).then_some(a.mime.as_str());
        match attachments::upload_limited(
            cx.db,
            cx.storage,
            &acting,
            t.id,
            &a.filename,
            mime,
            Bytes::from(data),
            super::MAX_ATTACHMENT_BYTES,
        )
        .await
        {
            Ok(_) => cx.stats.attachments += 1,
            Err(TicketFailure::Db(e)) if !e.to_string().contains("storage") => return Err(e),
            Err(_) => cx.log(fail).await?,
        }
    }
    Ok(Ok(()))
}

/// Id koneksi yang perlu didorong setelah request yang menulis outbox workspace `wid`.
///
/// # Errors
///
/// Error database.
pub async fn pushable<C: sea_orm::ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
) -> Result<Option<Uuid>, DbErr> {
    Ok(jira_connections::Model::of_workspace(db, wid)
        .await?
        .filter(|c| c.mode().pushes() && !c.is_paused() && c.is_ready())
        .map(|c| c.id))
}
