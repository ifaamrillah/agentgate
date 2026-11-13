//! Notifikasi kotak masuk (spec: notifikasi hanya di dalam aplikasi). Ditulis di transaksi yang
//! sama dengan kejadiannya (bukan efek samping keluar proses). Tandai dibaca hanya lewat POST.

use loco_rs::model::{TenantEntity, TenantQueryExt};
use sea_orm::{ActiveValue::Set, QueryOrder, QuerySelect, entity::prelude::*};
use serde::Serialize;

pub use super::_entities::notifications::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum NotificationKind {
    #[sea_orm(string_value = "mentioned")]
    Mentioned,
    #[sea_orm(string_value = "assigned")]
    Assigned,
    #[sea_orm(string_value = "commented")]
    Commented,
    /// Tiket menunggu review approver stage.
    #[sea_orm(string_value = "review_requested")]
    ReviewRequested,
    #[sea_orm(string_value = "approved")]
    Approved,
    #[sea_orm(string_value = "rejected")]
    Rejected,
    /// Putaran gagal mencapai batas; perlu keputusan manusia.
    #[sea_orm(string_value = "decision_needed")]
    DecisionNeeded,
    /// Agent bertanya lewat `ag_ask` (executor dan approver stage).
    #[sea_orm(string_value = "agent_question")]
    AgentQuestion,
    /// Job stage diumumkan ke executor (M5).
    #[sea_orm(string_value = "job_queued")]
    JobQueued,
    /// Job dijeda menunggu izin aksi berbahaya.
    #[sea_orm(string_value = "job_permission")]
    JobPermission,
    /// Kuota AI executor habis.
    #[sea_orm(string_value = "job_quota")]
    JobQuota,
    /// Job gagal.
    #[sea_orm(string_value = "job_failed")]
    JobFailed,
    /// Terminal stage interaktif gagal dibuka; jalankan `/ag-…` sendiri.
    #[sea_orm(string_value = "job_terminal")]
    JobTerminal,
    /// PR dibuka untuk tiket (M6).
    #[sea_orm(string_value = "pr_opened")]
    PrOpened,
    /// CI PR tiket gagal.
    #[sea_orm(string_value = "ci_failed")]
    CiFailed,
    #[sea_orm(string_value = "pr_merged")]
    PrMerged,
    /// Merge otomatis gagal; merge manual di provider.
    #[sea_orm(string_value = "merge_failed")]
    MergeFailed,
    /// Usulan knowledge baru menunggu persetujuan pemilik knowledge (M8, `project_id`).
    #[sea_orm(string_value = "knowledge_proposal")]
    KnowledgeProposal,
    /// Job knowledge (scan/update) diumumkan ke pemilik knowledge.
    #[sea_orm(string_value = "knowledge_job")]
    KnowledgeJob,
    /// Usulan knowledge yang diajukan pengguna ini ditolak.
    #[sea_orm(string_value = "knowledge_rejected")]
    KnowledgeRejected,
    /// Webhook keluar dinonaktifkan otomatis setelah gagal beruntun (M11; tanpa tiket/project).
    #[sea_orm(string_value = "webhook_disabled")]
    WebhookDisabled,
}

impl TenantEntity for Entity {
    type TenantId = WorkspaceId;

    fn tenant_column() -> Column {
        Column::WorkspaceId
    }
}

standard_behavior!(ActiveModel);

/// Satu notifikasi baru.
#[derive(Debug, Clone)]
pub struct NewNotification {
    pub user_id: Uuid,
    pub actor_id: Option<Uuid>,
    pub kind: NotificationKind,
    pub ticket_id: Option<Uuid>,
    pub comment_id: Option<Uuid>,
    /// Project untuk notifikasi tanpa tiket (knowledge, M8).
    pub project_id: Option<Uuid>,
}

impl Model {
    /// # Errors
    ///
    /// Nilai kolom bukan varian [`NotificationKind`].
    pub fn kind(&self) -> Result<NotificationKind, DbErr> {
        NotificationKind::try_from_value(&self.kind)
    }

    /// Satu halaman kotak masuk user, terbaru dulu; `unread_only` = belum dibaca saja.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn inbox<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        user_id: Uuid,
        kinds: &[NotificationKind],
        unread_only: bool,
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<Self>, u64), DbErr> {
        let mut select = Entity::find()
            .in_tenant(workspace_id)
            .filter(Column::UserId.eq(user_id));
        if unread_only {
            select = select.filter(Column::ReadAt.is_null());
        }
        if !kinds.is_empty() {
            select = select.filter(Column::Kind.is_in(kinds.iter().map(|k| k.to_value())));
        }
        let total = select.clone().count(db).await?;
        let rows = select
            .order_by_desc(Column::CreatedAt)
            .order_by_desc(Column::Id)
            .offset(offset)
            .limit(limit)
            .all(db)
            .await?;
        Ok((rows, total))
    }

    /// Jumlah belum dibaca (badge sidebar).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn unread_count<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        user_id: Uuid,
    ) -> Result<u64, DbErr> {
        Entity::find()
            .in_tenant(workspace_id)
            .filter(Column::UserId.eq(user_id))
            .filter(Column::ReadAt.is_null())
            .count(db)
            .await
    }

    /// Tandai dibaca: satu (`Some(id)`) atau semua milik user. Notifikasi orang lain tidak
    /// tersentuh.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn mark_read<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        user_id: Uuid,
        id: Option<Uuid>,
    ) -> Result<u64, DbErr> {
        let mut update = Entity::update_many()
            .col_expr(Column::ReadAt, Expr::current_timestamp())
            .filter(Column::WorkspaceId.eq(workspace_id.get()))
            .filter(Column::UserId.eq(user_id))
            .filter(Column::ReadAt.is_null());
        if let Some(id) = id {
            update = update.filter(Column::Id.eq(id));
        }
        Ok(update.exec(db).await?.rows_affected)
    }
}

impl ActiveModel {
    /// # Errors
    ///
    /// Error database.
    pub async fn notify<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        new: NewNotification,
    ) -> Result<Model, DbErr> {
        Self {
            workspace_id: Set(workspace_id.get()),
            user_id: Set(new.user_id),
            actor_id: Set(new.actor_id),
            kind: Set(new.kind.to_value()),
            ticket_id: Set(new.ticket_id),
            comment_id: Set(new.comment_id),
            project_id: Set(new.project_id),
            ..Default::default()
        }
        .insert(db)
        .await
    }
}
