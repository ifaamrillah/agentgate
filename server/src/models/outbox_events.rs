//! Outbox event domain (M11): satu baris per kejadian yang boleh dilihat integrasi luar
//! (webhook keluar sekarang, connector Jira nanti). Ditulis lewat [`publish`] **di transaksi yang
//! sama** dengan perubahannya, dari titik pusat yang sudah ada (riwayat tiket
//! `ticket_events::record`, komentar `ticket_comments::post`, sprint, rilis, job). Perubahan
//! yang di-rollback tidak meninggalkan event; event tidak pernah hilang karena enqueue gagal.
//!
//! [`publish`] juga menulis baris `webhook_deliveries` (pending) untuk setiap webhook aktif yang
//! berlangganan event itu dan project-nya. Pengirimannya efek samping "wajib terjadi" (rules
//! §5): dikerjakan worker setelah commit (`services::webhooks::dispatch`), dan scheduler menyapu
//! yang tertahan. Id delivery baru dicatat di [`pending`] supaya layer HTTP bisa meng-enqueue
//! worker setelah respons (setelah commit) tanpa menyentuh setiap controller.
//!
//! Konsumen lain membaca event berurutan lewat [`Model::since`] (keyset `(created_at, id)`).

use std::{cell::RefCell, future::Future};

use sea_orm::{
    ActiveValue::Set, Iterable, QueryOrder, QuerySelect, entity::prelude::*, sea_query::Expr,
};
use serde::Serialize;

pub use super::_entities::outbox_events::{ActiveModel, Column, Entity, Model};
use super::{
    projects, ticket_events::EventKind, tickets, users, webhook_deliveries, webhooks,
    workspaces::{self, WorkspaceId},
};

standard_behavior!(ActiveModel);

/// Event yang bisa dilanggan webhook (CHECK `outbox_events.event`, `webhooks.events`, dan
/// `webhook_deliveries.event`; `ping` hanya untuk delivery uji).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, DeriveActiveEnum, Serialize, PartialOrd, Ord,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum WebhookEvent {
    #[sea_orm(string_value = "ticket.created")]
    #[serde(rename = "ticket.created")]
    TicketCreated,
    #[sea_orm(string_value = "ticket.updated")]
    #[serde(rename = "ticket.updated")]
    TicketUpdated,
    #[sea_orm(string_value = "ticket.stage_changed")]
    #[serde(rename = "ticket.stage_changed")]
    TicketStageChanged,
    #[sea_orm(string_value = "ticket.deleted")]
    #[serde(rename = "ticket.deleted")]
    TicketDeleted,
    #[sea_orm(string_value = "comment.created")]
    #[serde(rename = "comment.created")]
    CommentCreated,
    #[sea_orm(string_value = "sprint.started")]
    #[serde(rename = "sprint.started")]
    SprintStarted,
    #[sea_orm(string_value = "sprint.completed")]
    #[serde(rename = "sprint.completed")]
    SprintCompleted,
    #[sea_orm(string_value = "release.published")]
    #[serde(rename = "release.published")]
    ReleasePublished,
    #[sea_orm(string_value = "gate.rejected")]
    #[serde(rename = "gate.rejected")]
    GateRejected,
    #[sea_orm(string_value = "job.failed")]
    #[serde(rename = "job.failed")]
    JobFailed,
    /// Event uji (tombol "Kirim event uji"); bukan event domain, tidak pernah di outbox.
    #[sea_orm(string_value = "ping")]
    #[serde(rename = "ping")]
    Ping,
}

impl WebhookEvent {
    /// Event domain yang bisa dipilih di webhook (tanpa `ping`).
    #[must_use]
    pub fn subscribable() -> Vec<Self> {
        Self::iter().filter(|e| *e != Self::Ping).collect()
    }

    /// Nilai string (`ticket.created`).
    #[must_use]
    pub fn as_str(self) -> String {
        self.to_value()
    }

    /// Event dari string; tidak dikenal → `None`.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::try_from_value(&value.to_string()).ok()
    }

    /// Event webhook dari jenis riwayat tiket; `None` = tidak diteruskan ke luar.
    #[must_use]
    pub fn of_ticket_event(kind: EventKind) -> Option<Self> {
        Some(match kind {
            EventKind::Created => Self::TicketCreated,
            EventKind::StageChanged
            | EventKind::StageJumped
            | EventKind::TransitionForced
            | EventKind::WorkflowMigrated => Self::TicketStageChanged,
            EventKind::Trashed => Self::TicketDeleted,
            EventKind::Rejected => Self::GateRejected,
            EventKind::Updated
            | EventKind::StatusChanged
            | EventKind::Linked
            | EventKind::Unlinked
            | EventKind::AttachmentAdded
            | EventKind::AttachmentRemoved
            | EventKind::Restored
            | EventKind::ReviewRequested
            | EventKind::Approved
            | EventKind::DecisionNeeded
            | EventKind::Decided
            | EventKind::ArtifactSaved
            | EventKind::PeopleChanged => Self::TicketUpdated,
        })
    }
}

/// Data event baru untuk [`publish`]. `payload` adalah isi `data` amplop webhook.
#[derive(Debug, Clone)]
pub struct NewEvent {
    pub workspace_id: WorkspaceId,
    pub project_id: Option<Uuid>,
    pub event: WebhookEvent,
    pub subject_id: Uuid,
    pub data: serde_json::Value,
}

tokio::task_local! {
    static PENDING: RefCell<Vec<Uuid>>;
}

/// Jalankan `fut` sambil mengumpulkan id delivery yang ditulis [`publish`] di dalamnya
/// (dipakai layer HTTP `controllers::middleware::webhook_dispatch`). Di luar scope ini
/// (worker, task) id tidak dikumpulkan; sapuan scheduler yang mengirimnya.
pub async fn collect_pending<F: Future>(fut: F) -> (F::Output, Vec<Uuid>) {
    PENDING
        .scope(RefCell::new(Vec::new()), async move {
            let out = fut.await;
            let ids = PENDING.with(|p| p.take());
            (out, ids)
        })
        .await
}

/// Catat delivery yang baru ditulis untuk dikirim setelah request selesai (setelah commit).
pub(crate) fn note_pending(ids: &[Uuid]) {
    // Di luar scope `collect_pending`, `try_with` gagal: diabaikan (sapuan scheduler).
    let _ = PENDING.try_with(|p| p.borrow_mut().extend_from_slice(ids));
}

/// Amplop JSON yang dikirim ke webhook dan disimpan di `payload`.
#[derive(Debug, Serialize)]
struct Envelope<'a> {
    id: Uuid,
    event: WebhookEvent,
    created_at: DateTimeWithTimeZone,
    workspace: serde_json::Value,
    project: serde_json::Value,
    data: &'a serde_json::Value,
}

/// Tulis event ke outbox dan delivery pending untuk setiap webhook aktif yang cocok. Dipanggil
/// dengan transaksi perubahan domainnya.
///
/// # Errors
///
/// Error database (pemanggil membatalkan transaksinya).
pub async fn publish<C: ConnectionTrait>(db: &C, new: NewEvent) -> Result<Model, DbErr> {
    let workspace = workspaces::Entity::find_by_id(new.workspace_id.get())
        .one(db)
        .await?;
    let project = match new.project_id {
        Some(pid) => projects::Model::find_in(db, new.workspace_id, pid).await?,
        None => None,
    };
    let id = Uuid::now_v7();
    let created_at: DateTimeWithTimeZone = chrono::Utc::now().into();
    let envelope = Envelope {
        id,
        event: new.event,
        created_at,
        workspace: workspace
            .as_ref()
            .map_or(serde_json::Value::Null, |w| {
                serde_json::json!({ "slug": w.slug, "name": w.name })
            }),
        project: project.as_ref().map_or(serde_json::Value::Null, |p| {
            serde_json::json!({ "key": p.key, "name": p.name })
        }),
        data: &new.data,
    };
    let payload = serde_json::to_value(&envelope).unwrap_or_default();
    let row = ActiveModel {
        id: Set(id),
        workspace_id: Set(new.workspace_id.get()),
        project_id: Set(new.project_id),
        event: Set(new.event.to_value()),
        subject_id: Set(new.subject_id),
        payload: Set(payload.clone()),
        created_at: Set(created_at),
        updated_at: Set(created_at),
    }
    .insert(db)
    .await?;
    let hooks = webhooks::Model::subscribed(db, new.workspace_id, new.event, new.project_id).await?;
    if hooks.is_empty() {
        return Ok(row);
    }
    let body = payload.to_string();
    for hook in &hooks {
        webhook_deliveries::ActiveModel::enqueue(
            db,
            new.workspace_id,
            hook.id,
            Some(row.id),
            new.event,
            body.clone(),
        )
        .await?;
    }
    Ok(row)
}

/// Ringkasan tiket untuk payload (bentuk sama dengan `GET /api/v1/tickets/{code}` ringkas).
#[must_use]
pub fn ticket_json(t: &tickets::Model, project_key: &str) -> serde_json::Value {
    serde_json::json!({
        "id": t.id,
        "code": t.code,
        "title": t.title,
        "project": project_key,
        "status": t.status,
        "priority": t.priority,
        "stage": t.stage_key,
        "story_points": t.story_points,
        "labels": t.labels,
        "assignee_id": t.assignee_id,
        "updated_at": t.updated_at,
    })
}

/// Event tiket dari riwayat (`ticket_events::record`): dipetakan lewat
/// [`WebhookEvent::of_ticket_event`]; tiket dibaca ulang di transaksi yang sama (keadaan
/// setelah perubahan).
///
/// # Errors
///
/// Error database.
pub async fn publish_ticket<C: ConnectionTrait>(
    db: &C,
    workspace_id: WorkspaceId,
    ticket_id: Uuid,
    actor_id: Option<Uuid>,
    kind: EventKind,
    changes: &serde_json::Value,
) -> Result<(), DbErr> {
    let Some(event) = WebhookEvent::of_ticket_event(kind) else {
        return Ok(());
    };
    let Some(ticket) = tickets::Model::find_in(db, workspace_id, ticket_id).await? else {
        return Ok(());
    };
    let key = projects::Model::find_in(db, workspace_id, ticket.project_id)
        .await?
        .map(|p| p.key)
        .unwrap_or_default();
    let actor = actor_json(db, actor_id).await?;
    publish(
        db,
        NewEvent {
            workspace_id,
            project_id: Some(ticket.project_id),
            event,
            subject_id: ticket.id,
            data: serde_json::json!({
                "ticket": ticket_json(&ticket, &key),
                "change": kind,
                "changes": changes,
                "actor": actor,
            }),
        },
    )
    .await?;
    Ok(())
}

/// Event `comment.created` (dari `ticket_comments::post`, transaksi pemanggil).
///
/// # Errors
///
/// Error database.
pub async fn publish_comment<C: ConnectionTrait>(
    db: &C,
    workspace_id: WorkspaceId,
    comment: &super::ticket_comments::Model,
) -> Result<(), DbErr> {
    let Some(ticket) = tickets::Model::find_in(db, workspace_id, comment.ticket_id).await? else {
        return Ok(());
    };
    let key = projects::Model::find_in(db, workspace_id, ticket.project_id)
        .await?
        .map(|p| p.key)
        .unwrap_or_default();
    let actor = actor_json(db, comment.author_id).await?;
    publish(
        db,
        NewEvent {
            workspace_id,
            project_id: Some(ticket.project_id),
            event: WebhookEvent::CommentCreated,
            subject_id: comment.id,
            data: serde_json::json!({
                "ticket": ticket_json(&ticket, &key),
                "comment": { "id": comment.id, "body": comment.body, "created_at": comment.created_at },
                "actor": actor,
            }),
        },
    )
    .await?;
    Ok(())
}

/// Event sprint (`sprint.started` / `sprint.completed`), transaksi pemanggil.
///
/// # Errors
///
/// Error database.
pub async fn publish_sprint<C: ConnectionTrait>(
    db: &C,
    workspace_id: WorkspaceId,
    event: WebhookEvent,
    sprint: &super::sprints::Model,
    actor_id: Option<Uuid>,
) -> Result<(), DbErr> {
    let actor = actor_json(db, actor_id).await?;
    publish(
        db,
        NewEvent {
            workspace_id,
            project_id: Some(sprint.project_id),
            event,
            subject_id: sprint.id,
            data: serde_json::json!({
                "sprint": {
                    "id": sprint.id, "name": sprint.name, "goal": sprint.goal,
                    "state": sprint.state, "starts_on": sprint.starts_on, "ends_on": sprint.ends_on,
                    "committed_points": sprint.committed_points,
                    "completed_points": sprint.completed_points,
                },
                "actor": actor,
            }),
        },
    )
    .await?;
    Ok(())
}

/// Event `release.published` (rilis ditandai dirilis), transaksi pemanggil.
///
/// # Errors
///
/// Error database.
pub async fn publish_release<C: ConnectionTrait>(
    db: &C,
    workspace_id: WorkspaceId,
    release: &super::releases::Model,
    actor_id: Option<Uuid>,
) -> Result<(), DbErr> {
    let actor = actor_json(db, actor_id).await?;
    publish(
        db,
        NewEvent {
            workspace_id,
            project_id: Some(release.project_id),
            event: WebhookEvent::ReleasePublished,
            subject_id: release.id,
            data: serde_json::json!({
                "release": {
                    "id": release.id, "name": release.name, "status": release.status,
                    "released_at": release.released_at, "notes": release.notes,
                },
                "actor": actor,
            }),
        },
    )
    .await?;
    Ok(())
}

/// Event `job.failed` (job runner selesai gagal).
///
/// # Errors
///
/// Error database.
pub async fn publish_job_failed<C: ConnectionTrait>(
    db: &C,
    workspace_id: WorkspaceId,
    job: &super::jobs::Model,
) -> Result<(), DbErr> {
    let ticket = match job.ticket_id {
        Some(id) => tickets::Model::find_in(db, workspace_id, id).await?,
        None => None,
    };
    publish(
        db,
        NewEvent {
            workspace_id,
            project_id: Some(job.project_id),
            event: WebhookEvent::JobFailed,
            subject_id: job.id,
            data: serde_json::json!({
                "job": {
                    "id": job.id, "kind": job.kind, "persona": job.persona,
                    "status": job.status, "summary": job.summary, "finished_at": job.finished_at,
                },
                "ticket": ticket.map(|t| serde_json::json!({ "id": t.id, "code": t.code, "title": t.title })),
            }),
        },
    )
    .await?;
    Ok(())
}

/// `{id, name}` pelaku, atau `null` (sistem/agent).
///
/// # Errors
///
/// Error database.
pub async fn actor_json<C: ConnectionTrait>(
    db: &C,
    actor_id: Option<Uuid>,
) -> Result<serde_json::Value, DbErr> {
    Ok(match actor_id {
        Some(id) => users::Entity::find_by_id(id)
            .one(db)
            .await?
            .map_or(serde_json::Value::Null, |u| {
                serde_json::json!({ "id": u.id, "name": u.name })
            }),
        None => serde_json::Value::Null,
    })
}

impl Model {
    /// # Errors
    ///
    /// Nilai kolom bukan varian [`WebhookEvent`].
    pub fn event(&self) -> Result<WebhookEvent, DbErr> {
        WebhookEvent::try_from_value(&self.event)
    }

    /// Event workspace setelah posisi `after` (`(created_at, id)` event terakhir yang sudah
    /// diproses; `None` = dari awal), terlama dulu. Untuk konsumen berurutan (mirror Jira).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn since<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        after: Option<(DateTimeWithTimeZone, Uuid)>,
        limit: u64,
    ) -> Result<Vec<Self>, DbErr> {
        let mut q = Entity::find().filter(Column::WorkspaceId.eq(workspace_id.get()));
        if let Some((at, id)) = after {
            q = q.filter(Expr::cust_with_values(
                "(outbox_events.created_at, outbox_events.id) > ($1, $2)",
                [sea_orm::Value::from(at), sea_orm::Value::from(id)],
            ));
        }
        q.order_by_asc(Column::CreatedAt)
            .order_by_asc(Column::Id)
            .limit(limit)
            .all(db)
            .await
    }
}

/// Hapus event outbox lebih tua dari [`webhook_deliveries::RETENTION_DAYS`] hari (job
/// pembersihan). Delivery yang merujuknya memakai `ON DELETE SET NULL`.
///
/// # Errors
///
/// Error database.
pub async fn delete_expired<C: ConnectionTrait>(
    db: &C,
    now: chrono::DateTime<chrono::Utc>,
    dry_run: bool,
) -> Result<u64, DbErr> {
    let cutoff =
        DateTimeWithTimeZone::from(now - chrono::Duration::days(webhook_deliveries::RETENTION_DAYS));
    let filter = Column::CreatedAt.lt(cutoff);
    if dry_run {
        return Entity::find().filter(filter).count(db).await;
    }
    Ok(Entity::delete_many().filter(filter).exec(db).await?.rows_affected)
}
