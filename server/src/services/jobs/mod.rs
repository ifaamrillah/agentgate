//! Job AI (spec "Runner & eksekusi job"): satu stage × satu tiket, atau `@agent` dari komentar,
//! dijalankan di runner pribadi executor.
//!
//! **Model klaim.** Job stage dibuat saat tiket masuk stage ber-persona dengan mode otomatis
//! atau interaktif ([`on_stage_entered`], dipanggil `stages::enter_stage` sehingga semua jalur
//! masuk stage tercakup); job terbuka lama milik tiket dibatalkan. Executor stage mendapat
//! notifikasi. Executor pertama yang klik **Ambil** mendapat lock (`UPDATE … WHERE status =
//! 'queued'`), lalu runner miliknya menjalankan job. Stage tanpa executor boleh diambil siapa
//! pun yang memegang `EditProject`. Executor bisa melepas job (kembali ke antrean); executor
//! pemegang lock dan `AdministerProject` bisa membatalkan.
//!
//! Sisi runner (register, start, log, evidence, selesai) ada di [`runner`].

pub mod evidence;
pub mod runner;

use agentgate_core::{
    runner::protocol::{JobKind, JobMode, JobStatus, PermissionStatus},
    workflow::{Mode, Stage},
};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr,
    EntityTrait, IntoActiveModel, QueryFilter, QueryOrder, TransactionTrait, sea_query::Expr,
};
use uuid::Uuid;

use super::{
    audit::{self, AuditAction, AuditEntry, AuditSubject},
    tickets::{TicketFailure, access_in},
    workspaces::Acting,
};
use crate::models::{
    job_logs, job_permissions, jobs,
    notifications::{self, NewNotification, NotificationKind},
    permission::Permission,
    projects, runner_prefs, runners, ticket_stage_people,
    tickets::{self, TicketPriority},
    workspaces::WorkspaceId,
};

pub const NOT_QUEUED: &str = "job-not-queued";
pub const NOT_EXECUTOR: &str = "job-not-executor";
pub const NOT_HOLDER: &str = "job-not-holder";
pub const CLOSED: &str = "job-closed";
pub const RUNNER_INVALID: &str = "job-runner-invalid";
pub const NOT_QUOTA: &str = "job-not-quota";
pub const PERMISSION_CLOSED: &str = "job-permission-closed";
pub const ACTION_BLOCKED: &str = "job-action-blocked";
pub const RUNNER_BUSY: &str = "job-runner-busy";
pub const ERROR_CODES: &[&str] = &[
    NOT_QUEUED,
    NOT_EXECUTOR,
    NOT_HOLDER,
    CLOSED,
    RUNNER_INVALID,
    NOT_QUOTA,
    PERMISSION_CLOSED,
    ACTION_BLOCKED,
    RUNNER_BUSY,
];

fn invalid(code: &'static str) -> TicketFailure {
    TicketFailure::field("job", code)
}

fn now() -> chrono::DateTime<chrono::FixedOffset> {
    Utc::now().into()
}

/// Tulis satu baris log job (runner atau server).
///
/// # Errors
///
/// Error database.
pub async fn log<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    job_id: Uuid,
    stream: agentgate_core::runner::protocol::LogStream,
    body: &str,
) -> Result<(), DbErr> {
    job_logs::ActiveModel {
        workspace_id: Set(wid.get()),
        job_id: Set(job_id),
        stream: Set(stream.as_str().to_string()),
        body: Set(body.chars().take(runner::LOG_LINE_MAX).collect()),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(())
}

async fn notify_users<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    actor: Option<Uuid>,
    users: impl IntoIterator<Item = Uuid>,
    kind: NotificationKind,
    ticket_id: Uuid,
) -> Result<(), DbErr> {
    for user_id in users {
        if Some(user_id) == actor {
            continue;
        }
        notifications::ActiveModel::notify(
            db,
            wid,
            NewNotification {
                user_id,
                actor_id: actor,
                kind,
                ticket_id: Some(ticket_id),
                comment_id: None,
            },
        )
        .await?;
    }
    Ok(())
}

/// Batalkan job stage terbuka milik tiket (tiket pindah stage / dibatalkan).
///
/// # Errors
///
/// Error database.
pub async fn cancel_open_stage_jobs<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    ticket_id: Uuid,
) -> Result<(), DbErr> {
    jobs::Entity::update_many()
        .col_expr(
            jobs::Column::Status,
            Expr::value(JobStatus::Cancelled.as_str()),
        )
        .col_expr(jobs::Column::FinishedAt, Expr::value(now()))
        .col_expr(jobs::Column::UpdatedAt, Expr::value(now()))
        .filter(jobs::Column::WorkspaceId.eq(wid.get()))
        .filter(jobs::Column::TicketId.eq(ticket_id))
        .filter(jobs::Column::Kind.eq(JobKind::Stage.as_str()))
        .filter(jobs::Column::Status.is_in(jobs::OPEN.iter().copied()))
        .exec(db)
        .await?;
    Ok(())
}

/// Tiket baru saja masuk `stage` (aturan versi aktif): batalkan job stage lama, lalu umumkan job
/// baru bila stage dikerjakan AI (persona + mode otomatis/interaktif). Mengembalikan job baru.
///
/// # Errors
///
/// Error database.
pub async fn on_stage_entered<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    ticket: &tickets::Model,
    stage: &Stage,
) -> Result<Option<jobs::Model>, DbErr> {
    cancel_open_stage_jobs(db, wid, ticket.id).await?;
    let (Some(persona), mode) = (stage.persona.as_ref(), stage.mode) else {
        return Ok(None);
    };
    let mode = match mode {
        Mode::Auto => JobMode::Auto,
        Mode::Interactive => JobMode::Interactive,
        Mode::Manual => return Ok(None),
    };
    let job = jobs::ActiveModel {
        workspace_id: Set(wid.get()),
        project_id: Set(ticket.project_id),
        ticket_id: Set(ticket.id),
        kind: Set(JobKind::Stage.as_str().into()),
        mode: Set(mode.as_str().into()),
        stage_key: Set(stage.key.clone()),
        persona: Set(persona.clone()),
        status: Set(JobStatus::Queued.as_str().into()),
        steps: Set(serde_json::json!([])),
        ..Default::default()
    }
    .insert(db)
    .await?;
    let people = ticket_stage_people::Model::of_stage(db, wid, ticket.id, &stage.key).await?;
    notify_users(
        db,
        wid,
        None,
        people.executors,
        NotificationKind::JobQueued,
        ticket.id,
    )
    .await?;
    Ok(Some(job))
}

/// Job di workspace yang tiketnya masih aktif.
///
/// # Errors
///
/// Error database.
pub async fn find<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    id: Uuid,
) -> Result<Option<jobs::Model>, DbErr> {
    jobs::Entity::find_by_id(id)
        .filter(jobs::Column::WorkspaceId.eq(wid.get()))
        .one(db)
        .await
}

/// Job terbaru tiket (terbuka dulu), untuk kartu job di halaman tiket.
///
/// # Errors
///
/// Error database.
pub async fn latest_of_ticket<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    ticket_id: Uuid,
) -> Result<Option<jobs::Model>, DbErr> {
    jobs::Entity::find()
        .filter(jobs::Column::WorkspaceId.eq(wid.get()))
        .filter(jobs::Column::TicketId.eq(ticket_id))
        .filter(jobs::Column::Kind.eq(JobKind::Stage.as_str()))
        .order_by_desc(jobs::Column::CreatedAt)
        .one(db)
        .await
}

/// Job yang bisa dilihat pelaku: tiketnya ada di project yang bisa ia buka.
///
/// # Errors
///
/// `NotFound`, error database.
pub async fn visible(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
) -> Result<(jobs::Model, tickets::Model), TicketFailure> {
    let wid = acting.workspace_id();
    let job = find(db, wid, id).await?.ok_or(TicketFailure::NotFound)?;
    let ticket = tickets::Model::find_in(db, wid, job.ticket_id)
        .await?
        .filter(|t| !t.is_trashed())
        .ok_or(TicketFailure::NotFound)?;
    access_in(db, acting, ticket.project_id, Permission::ViewProject).await?;
    Ok((job, ticket))
}

/// Pelaku executor stage job (atau stage tanpa executor dan pelaku memegang `EditProject`).
async fn is_executor<C: ConnectionTrait>(
    db: &C,
    acting: &Acting<'_>,
    job: &jobs::Model,
) -> Result<bool, TicketFailure> {
    let wid = acting.workspace_id();
    if job.kind() == JobKind::Comment {
        return Ok(job.requested_by == Some(acting.user.id));
    }
    let people =
        ticket_stage_people::Model::of_stage(db, wid, job.ticket_id, &job.stage_key).await?;
    if people.executors.is_empty() {
        return Ok(
            access_in(db, acting, job.project_id, Permission::EditProject)
                .await
                .is_ok(),
        );
    }
    Ok(people.executors.contains(&acting.user.id))
}

/// Pelaku boleh mengambil job ini (untuk tombol Ambil).
///
/// # Errors
///
/// Error database.
pub async fn can_claim<C: ConnectionTrait>(
    db: &C,
    acting: &Acting<'_>,
    job: &jobs::Model,
) -> Result<bool, DbErr> {
    if job.status() != JobStatus::Queued {
        return Ok(false);
    }
    match is_executor(db, acting, job).await {
        Ok(b) => Ok(b),
        Err(TicketFailure::Db(e)) => Err(e),
        Err(_) => Ok(false),
    }
}

/// Pelaku memegang `AdministerProject` di project job.
pub async fn is_project_admin<C: ConnectionTrait>(
    db: &C,
    acting: &Acting<'_>,
    job: &jobs::Model,
) -> bool {
    access_in(db, acting, job.project_id, Permission::AdministerProject)
        .await
        .is_ok()
}

/// Runner aktif milik pelaku di workspace ini.
async fn own_runner<C: ConnectionTrait>(
    db: &C,
    acting: &Acting<'_>,
    runner_id: Uuid,
) -> Result<runners::Model, TicketFailure> {
    runners::Entity::find_by_id(runner_id)
        .filter(runners::Column::WorkspaceId.eq(acting.workspace_id().get()))
        .filter(runners::Column::UserId.eq(acting.user.id))
        .filter(runners::Column::RevokedAt.is_null())
        .one(db)
        .await?
        .ok_or_else(|| invalid(RUNNER_INVALID))
}

/// Ambil job: executor pertama mendapat lock. `runner` = runner tujuan (opsional; tanpa runner,
/// runner pelaku yang pertama online menjalankannya).
///
/// # Errors
///
/// `job-not-queued`, `job-not-executor`, `job-runner-invalid`, `NotFound`, error database.
pub async fn claim(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
    runner: Option<Uuid>,
) -> Result<jobs::Model, TicketFailure> {
    let (job, _) = visible(db, acting, id).await?;
    if job.status() != JobStatus::Queued {
        return Err(invalid(NOT_QUEUED));
    }
    if !is_executor(db, acting, &job).await? {
        return Err(invalid(NOT_EXECUTOR));
    }
    if let Some(r) = runner {
        own_runner(db, acting, r).await?;
    }
    let txn = db.begin().await?;
    let updated = jobs::Entity::update_many()
        .col_expr(
            jobs::Column::Status,
            Expr::value(JobStatus::Claimed.as_str()),
        )
        .col_expr(jobs::Column::ClaimedBy, Expr::value(acting.user.id))
        .col_expr(jobs::Column::RunnerId, Expr::value(runner))
        .col_expr(jobs::Column::ClaimedAt, Expr::value(now()))
        .col_expr(jobs::Column::UpdatedAt, Expr::value(now()))
        .filter(jobs::Column::Id.eq(job.id))
        .filter(jobs::Column::Status.eq(JobStatus::Queued.as_str()))
        .exec(&txn)
        .await?;
    if updated.rows_affected != 1 {
        return Err(invalid(NOT_QUEUED));
    }
    log(
        &txn,
        acting.workspace_id(),
        job.id,
        agentgate_core::runner::protocol::LogStream::Info,
        &format!("Diambil oleh {}", acting.user.name),
    )
    .await?;
    txn.commit().await?;
    find(db, acting.workspace_id(), id)
        .await?
        .ok_or(TicketFailure::NotFound)
}

/// Lepas job ke antrean (pemegang lock atau Admin project). `note` untuk executor berikutnya.
///
/// # Errors
///
/// `job-not-holder`, `job-closed`, `NotFound`, error database.
pub async fn release(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
    note: &str,
) -> Result<jobs::Model, TicketFailure> {
    let (job, _) = visible(db, acting, id).await?;
    let status = job.status();
    if !status.is_open() || status == JobStatus::Queued {
        return Err(invalid(CLOSED));
    }
    if job.claimed_by != Some(acting.user.id) && !is_project_admin(db, acting, &job).await {
        return Err(invalid(NOT_HOLDER));
    }
    let wid = acting.workspace_id();
    let txn = db.begin().await?;
    let mut m = job.clone().into_active_model();
    m.status = Set(JobStatus::Queued.as_str().into());
    m.claimed_by = Set(None);
    m.runner_id = Set(None);
    m.claimed_at = Set(None);
    m.quota_reset_at = Set(None);
    let job = m.update(&txn).await?;
    let mut line = format!("Dilepas ke antrean oleh {}", acting.user.name);
    let note = note.trim();
    if !note.is_empty() {
        line.push_str(" · ");
        line.extend(note.chars().take(500));
    }
    log(
        &txn,
        wid,
        job.id,
        agentgate_core::runner::protocol::LogStream::Warn,
        &line,
    )
    .await?;
    if job.kind() == JobKind::Stage {
        let people =
            ticket_stage_people::Model::of_stage(&txn, wid, job.ticket_id, &job.stage_key).await?;
        notify_users(
            &txn,
            wid,
            Some(acting.user.id),
            people.executors,
            NotificationKind::JobQueued,
            job.ticket_id,
        )
        .await?;
    }
    txn.commit().await?;
    Ok(job)
}

/// Batalkan job: pemegang lock (atau executor bila belum diambil) dan Admin project. Proses
/// client di runner dihentikan lewat event `job_cancelled`.
///
/// # Errors
///
/// `job-not-holder`, `job-closed`, `NotFound`, error database.
pub async fn cancel(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
) -> Result<jobs::Model, TicketFailure> {
    let (job, ticket) = visible(db, acting, id).await?;
    if !job.status().is_open() {
        return Err(invalid(CLOSED));
    }
    let allowed = match job.claimed_by {
        Some(holder) => holder == acting.user.id,
        None => is_executor(db, acting, &job).await?,
    } || is_project_admin(db, acting, &job).await;
    if !allowed {
        return Err(invalid(NOT_HOLDER));
    }
    let wid = acting.workspace_id();
    let txn = db.begin().await?;
    let mut m = job.clone().into_active_model();
    m.status = Set(JobStatus::Cancelled.as_str().into());
    m.finished_at = Set(Some(now()));
    let job = m.update(&txn).await?;
    log(
        &txn,
        wid,
        job.id,
        agentgate_core::runner::protocol::LogStream::Fail,
        &format!("Dibatalkan oleh {}", acting.user.name),
    )
    .await?;
    audit::record(
        &txn,
        AuditEntry {
            workspace_id: wid,
            actor_id: Some(acting.user.id),
            action: AuditAction::JobCancelled,
            subject: (AuditSubject::Job, job.id),
            changes: serde_json::json!({ "ticket": ticket.code, "stage": job.stage_key }),
            request_id: acting.request_id.clone(),
        },
    )
    .await?;
    txn.commit().await?;
    Ok(job)
}

/// Pilihan saat kuota AI habis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaChoice {
    /// Tunggu limit reset, lanjut di runner yang sama.
    Wait,
    /// Lepas ke antrean klaim.
    Release,
}

/// Executor memutuskan job yang berhenti karena kuota habis.
///
/// # Errors
///
/// `job-not-quota`, `job-not-holder`, `NotFound`, error database.
pub async fn decide_quota(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
    choice: QuotaChoice,
) -> Result<jobs::Model, TicketFailure> {
    let (job, _) = visible(db, acting, id).await?;
    if job.status() != JobStatus::Quota {
        return Err(invalid(NOT_QUOTA));
    }
    if job.claimed_by != Some(acting.user.id) && !is_project_admin(db, acting, &job).await {
        return Err(invalid(NOT_HOLDER));
    }
    match choice {
        QuotaChoice::Release => release(db, acting, id, "").await,
        QuotaChoice::Wait => {
            // Kembali ke "diambil": runner yang sama menjalankannya lagi setelah reset.
            let mut m = job.into_active_model();
            m.status = Set(JobStatus::Claimed.as_str().into());
            let job = m.update(db).await?;
            log(
                db,
                acting.workspace_id(),
                job.id,
                agentgate_core::runner::protocol::LogStream::Info,
                "Menunggu limit reset, lanjut di runner yang sama",
            )
            .await?;
            Ok(job)
        }
    }
}

/// Jawab permintaan izin aksi berbahaya (pemegang lock job atau Admin project).
///
/// # Errors
///
/// `job-permission-closed`, `job-not-holder`, `NotFound`, error database.
pub async fn decide_permission(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    permission_id: Uuid,
    status: PermissionStatus,
) -> Result<job_permissions::Model, TicketFailure> {
    let wid = acting.workspace_id();
    let perm = job_permissions::Entity::find_by_id(permission_id)
        .filter(job_permissions::Column::WorkspaceId.eq(wid.get()))
        .one(db)
        .await?
        .ok_or(TicketFailure::NotFound)?;
    let (job, ticket) = visible(db, acting, perm.job_id).await?;
    if perm.status() != PermissionStatus::Pending || status == PermissionStatus::Pending {
        return Err(invalid(PERMISSION_CLOSED));
    }
    if job.claimed_by != Some(acting.user.id) && !is_project_admin(db, acting, &job).await {
        return Err(invalid(NOT_HOLDER));
    }
    let txn = db.begin().await?;
    let mut m = perm.into_active_model();
    m.status = Set(status.as_str().into());
    m.decided_by = Set(Some(acting.user.id));
    m.decided_at = Set(Some(now()));
    let perm = m.update(&txn).await?;
    if job.status() == JobStatus::Paused {
        let mut j = job.clone().into_active_model();
        j.status = Set(JobStatus::Running.as_str().into());
        j.update(&txn).await?;
    }
    let verdict = match status {
        PermissionStatus::Denied => "ditolak",
        PermissionStatus::AllowedTicket => "diizinkan untuk tiket ini",
        _ => "diizinkan sekali",
    };
    log(
        &txn,
        wid,
        job.id,
        if status == PermissionStatus::Denied {
            agentgate_core::runner::protocol::LogStream::Fail
        } else {
            agentgate_core::runner::protocol::LogStream::Ok
        },
        &format!("Izin {} {verdict} oleh {}", perm.command, acting.user.name),
    )
    .await?;
    audit::record(
        &txn,
        AuditEntry {
            workspace_id: wid,
            actor_id: Some(acting.user.id),
            action: AuditAction::JobPermissionDecided,
            subject: (AuditSubject::Job, job.id),
            changes: serde_json::json!({
                "ticket": ticket.code,
                "action": perm.action,
                "command": perm.command,
                "status": status.as_str(),
            }),
            request_id: acting.request_id.clone(),
        },
    )
    .await?;
    txn.commit().await?;
    Ok(perm)
}

/// Satu baris antrean job (halaman Runner & CLI).
#[derive(Debug, Clone)]
pub struct QueueRow {
    pub job: jobs::Model,
    pub ticket: tickets::Model,
    pub project: projects::Model,
    pub priority: u8,
    pub can_claim: bool,
}

/// Job terbuka di project yang bisa dibuka pelaku, urut prioritas tiket lalu umur.
///
/// # Errors
///
/// Error database.
pub async fn queue(db: &DatabaseConnection, acting: &Acting<'_>) -> Result<Vec<QueueRow>, DbErr> {
    let wid = acting.workspace_id();
    let projects = projects::Model::accessible_for(db, &acting.accessor()).await?;
    if projects.is_empty() {
        return Ok(Vec::new());
    }
    let open = jobs::Entity::find()
        .filter(jobs::Column::WorkspaceId.eq(wid.get()))
        .filter(jobs::Column::Status.is_in(jobs::OPEN.iter().copied()))
        .filter(jobs::Column::ProjectId.is_in(projects.iter().map(|p| p.id)))
        .order_by_asc(jobs::Column::CreatedAt)
        .all(db)
        .await?;
    let mut rows = Vec::with_capacity(open.len());
    for job in open {
        let Some(ticket) = tickets::Model::find_in(db, wid, job.ticket_id)
            .await?
            .filter(|t| !t.is_trashed())
        else {
            continue;
        };
        let Some(project) = projects.iter().find(|p| p.id == job.project_id).cloned() else {
            continue;
        };
        let priority = ticket.priority().map_or(3, TicketPriority::rank);
        let can_claim = can_claim(db, acting, &job).await?;
        rows.push(QueueRow {
            job,
            ticket,
            project,
            priority,
            can_claim,
        });
    }
    rows.sort_by(|a, b| {
        a.priority
            .cmp(&b.priority)
            .then(a.job.created_at.cmp(&b.job.created_at))
    });
    Ok(rows)
}

/// Permintaan izin yang menunggu jawaban pelaku (job yang ia pegang, atau semua job di project
/// yang ia administrasikan).
///
/// # Errors
///
/// Error database.
pub async fn pending_permissions(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
) -> Result<Vec<(job_permissions::Model, jobs::Model, tickets::Model)>, DbErr> {
    let wid = acting.workspace_id();
    let pending = job_permissions::Entity::find()
        .filter(job_permissions::Column::WorkspaceId.eq(wid.get()))
        .filter(job_permissions::Column::Status.eq(PermissionStatus::Pending.as_str()))
        .order_by_asc(job_permissions::Column::CreatedAt)
        .all(db)
        .await?;
    let mut out = Vec::new();
    for p in pending {
        let Some(job) = find(db, wid, p.job_id).await? else {
            continue;
        };
        if job.claimed_by != Some(acting.user.id) && !is_project_admin(db, acting, &job).await {
            continue;
        }
        let Some(ticket) = tickets::Model::find_in(db, wid, job.ticket_id).await? else {
            continue;
        };
        out.push((p, job, ticket));
    }
    Ok(out)
}

/// Runner milik `user` di workspace (yang belum dicabut), terbaru dulu.
///
/// # Errors
///
/// Error database.
pub async fn runners_of<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    user: Option<Uuid>,
) -> Result<Vec<runners::Model>, DbErr> {
    let mut q = runners::Entity::find()
        .filter(runners::Column::WorkspaceId.eq(wid.get()))
        .filter(runners::Column::RevokedAt.is_null());
    if let Some(u) = user {
        q = q.filter(runners::Column::UserId.eq(u));
    }
    q.order_by_desc(runners::Column::LastSeenAt).all(db).await
}

/// Simpan pengaturan runner pelaku.
///
/// # Errors
///
/// Error database.
pub async fn save_prefs(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    prefs: &runner_prefs::Prefs,
) -> Result<(), DbErr> {
    let wid = acting.workspace_id();
    let existing = runner_prefs::Entity::find()
        .filter(runner_prefs::Column::WorkspaceId.eq(wid.get()))
        .filter(runner_prefs::Column::UserId.eq(acting.user.id))
        .one(db)
        .await?;
    let mut m = existing.map_or_else(
        || runner_prefs::ActiveModel {
            workspace_id: Set(wid.get()),
            user_id: Set(acting.user.id),
            ..Default::default()
        },
        IntoActiveModel::into_active_model,
    );
    m.auto_claim = Set(prefs.auto_claim);
    m.max_parallel = Set(prefs.max_parallel);
    m.default_client = Set(prefs.default_client.clone());
    m.quota_policy = Set(sea_orm::ActiveEnum::to_value(&prefs.quota_policy));
    m.terminals = Set(prefs.terminals.clone());
    m.save(db).await?;
    Ok(())
}

/// Cabut runner milik pelaku (atau runner siapa pun bagi Owner/Admin workspace). Job yang sedang
/// dipegang runner itu kembali ke antrean.
///
/// # Errors
///
/// `NotFound`, error database.
pub async fn revoke_runner(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    runner_id: Uuid,
) -> Result<(), TicketFailure> {
    let wid = acting.workspace_id();
    let runner = runners::Entity::find_by_id(runner_id)
        .filter(runners::Column::WorkspaceId.eq(wid.get()))
        .filter(runners::Column::RevokedAt.is_null())
        .one(db)
        .await?
        .ok_or(TicketFailure::NotFound)?;
    let admin = crate::models::project_access::is_admin_everywhere(acting.role);
    if runner.user_id != acting.user.id && !admin {
        return Err(TicketFailure::NotFound);
    }
    let txn = db.begin().await?;
    jobs::Entity::update_many()
        .col_expr(
            jobs::Column::Status,
            Expr::value(JobStatus::Queued.as_str()),
        )
        .col_expr(jobs::Column::RunnerId, Expr::value(Option::<Uuid>::None))
        .col_expr(jobs::Column::ClaimedBy, Expr::value(Option::<Uuid>::None))
        .col_expr(jobs::Column::UpdatedAt, Expr::value(now()))
        .filter(jobs::Column::RunnerId.eq(runner.id))
        .filter(jobs::Column::Status.is_in(jobs::OPEN.iter().copied()))
        .exec(&txn)
        .await?;
    let name = runner.name.clone();
    let mut m = runner.into_active_model();
    m.revoked_at = Set(Some(now()));
    let runner = m.update(&txn).await?;
    audit::record(
        &txn,
        AuditEntry {
            workspace_id: wid,
            actor_id: Some(acting.user.id),
            action: AuditAction::RunnerRevoked,
            subject: (AuditSubject::Runner, runner.id),
            changes: serde_json::json!({ "name": name }),
            request_id: acting.request_id.clone(),
        },
    )
    .await?;
    txn.commit().await?;
    Ok(())
}

/// Kunci persona yang disebut `@agent-<kunci>` di teks (unik, urut kemunculan, maks. 3).
#[must_use]
pub fn agent_mentions(body: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (i, _) in body.match_indices("@agent-") {
        if i > 0
            && body[..i]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric())
        {
            continue;
        }
        let key: String = body[i + "@agent-".len()..]
            .chars()
            .take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-')
            .collect();
        let key = key.trim_end_matches('-').to_string();
        if !key.is_empty() && !out.contains(&key) {
            out.push(key);
        }
        if out.len() == 3 {
            break;
        }
    }
    out
}

/// Komentar `@agent-<persona>` (penulis memegang `EditProject`, jadi Member ke atas): job
/// `comment` langsung terkunci ke penulis, dijalankan runner penulis. Persona tidak dikenal
/// diabaikan.
///
/// # Errors
///
/// Error database.
pub async fn on_agent_mentions<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    ticket: &tickets::Model,
    comment: &crate::models::ticket_comments::Model,
) -> Result<Vec<jobs::Model>, DbErr> {
    let Some(author) = comment.author_id else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for key in agent_mentions(&comment.body) {
        if crate::models::personas::Model::find_active(db, wid, &key)
            .await?
            .is_none()
        {
            continue;
        }
        let job = jobs::ActiveModel {
            workspace_id: Set(wid.get()),
            project_id: Set(ticket.project_id),
            ticket_id: Set(ticket.id),
            kind: Set(JobKind::Comment.as_str().into()),
            mode: Set(JobMode::Auto.as_str().into()),
            stage_key: Set(ticket.stage_key.clone()),
            persona: Set(key),
            status: Set(JobStatus::Claimed.as_str().into()),
            comment_id: Set(Some(comment.id)),
            prompt_note: Set(comment.body.chars().take(4_000).collect()),
            requested_by: Set(Some(author)),
            claimed_by: Set(Some(author)),
            claimed_at: Set(Some(now())),
            steps: Set(serde_json::json!([])),
            ..Default::default()
        }
        .insert(db)
        .await?;
        out.push(job);
    }
    Ok(out)
}

/// Job `@agent` per komentar tiket (untuk penanda di komentar).
///
/// # Errors
///
/// Error database.
pub async fn of_comments<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    ticket_id: Uuid,
) -> Result<Vec<jobs::Model>, DbErr> {
    jobs::Entity::find()
        .filter(jobs::Column::WorkspaceId.eq(wid.get()))
        .filter(jobs::Column::TicketId.eq(ticket_id))
        .filter(jobs::Column::Kind.eq(JobKind::Comment.as_str()))
        .order_by_asc(jobs::Column::CreatedAt)
        .all(db)
        .await
}

/// Log job, `limit` baris terakhir (urut lama → baru).
///
/// # Errors
///
/// Error database.
pub async fn logs_of<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    job_id: Uuid,
    limit: u64,
) -> Result<Vec<job_logs::Model>, DbErr> {
    use sea_orm::QuerySelect;
    let mut rows = job_logs::Entity::find()
        .filter(job_logs::Column::WorkspaceId.eq(wid.get()))
        .filter(job_logs::Column::JobId.eq(job_id))
        .order_by_desc(job_logs::Column::Id)
        .limit(limit)
        .all(db)
        .await?;
    rows.reverse();
    Ok(rows)
}

/// Permintaan izin satu job, terbaru dulu.
///
/// # Errors
///
/// Error database.
pub async fn permissions_of_job<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    job_id: Uuid,
) -> Result<Vec<job_permissions::Model>, DbErr> {
    job_permissions::Entity::find()
        .filter(job_permissions::Column::WorkspaceId.eq(wid.get()))
        .filter(job_permissions::Column::JobId.eq(job_id))
        .order_by_desc(job_permissions::Column::CreatedAt)
        .all(db)
        .await
}

/// Job yang sedang berjalan di runner-runner ini (`runner_id`, job).
///
/// # Errors
///
/// Error database.
pub async fn running_on<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    runner_ids: &[Uuid],
) -> Result<Vec<jobs::Model>, DbErr> {
    if runner_ids.is_empty() {
        return Ok(Vec::new());
    }
    jobs::Entity::find()
        .filter(jobs::Column::WorkspaceId.eq(wid.get()))
        .filter(jobs::Column::RunnerId.is_in(runner_ids.iter().copied()))
        .filter(
            jobs::Column::Status.is_in([JobStatus::Running.as_str(), JobStatus::Paused.as_str()]),
        )
        .all(db)
        .await
}

#[cfg(test)]
mod tests {
    use super::agent_mentions;

    #[test]
    fn finds_agent_mentions() {
        assert_eq!(agent_mentions("@agent-qa tolong cek ulang"), ["qa"]);
        assert_eq!(
            agent_mentions("hai @agent-dev, lalu @agent-qa. @agent-dev lagi"),
            ["dev", "qa"]
        );
        assert!(agent_mentions("email x@agent-qa.com").is_empty());
        assert!(agent_mentions("@agent- kosong").is_empty());
    }
}
