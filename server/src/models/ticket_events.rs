//! Riwayat aktivitas tiket (spec "Riwayat aktivitas"): ditulis di transaksi yang sama dengan
//! perubahannya oleh `services::tickets`, append-only. `changes` berisi field yang berubah
//! (`{"title": {"from": …, "to": …}}`) atau data kejadian (relasi, lampiran).

use loco_rs::model::{TenantEntity, TenantQueryExt};
use sea_orm::{ActiveValue::Set, QueryOrder, entity::prelude::*};
use serde::Serialize;

pub use super::_entities::ticket_events::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    #[sea_orm(string_value = "created")]
    Created,
    #[sea_orm(string_value = "updated")]
    Updated,
    #[sea_orm(string_value = "status_changed")]
    StatusChanged,
    #[sea_orm(string_value = "linked")]
    Linked,
    #[sea_orm(string_value = "unlinked")]
    Unlinked,
    #[sea_orm(string_value = "attachment_added")]
    AttachmentAdded,
    #[sea_orm(string_value = "attachment_removed")]
    AttachmentRemoved,
    #[sea_orm(string_value = "trashed")]
    Trashed,
    #[sea_orm(string_value = "restored")]
    Restored,
}

impl TenantEntity for Entity {
    type TenantId = WorkspaceId;

    fn tenant_column() -> Column {
        Column::WorkspaceId
    }
}

standard_behavior!(ActiveModel, append_only);

impl Model {
    /// # Errors
    ///
    /// Nilai kolom bukan varian [`EventKind`].
    pub fn kind(&self) -> Result<EventKind, DbErr> {
        EventKind::try_from_value(&self.kind)
    }

    /// Riwayat tiket, terlama dulu.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn of_ticket<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        ticket_id: Uuid,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .in_tenant(workspace_id)
            .filter(Column::TicketId.eq(ticket_id))
            .order_by_asc(Column::CreatedAt)
            .order_by_asc(Column::Id)
            .all(db)
            .await
    }
}

impl ActiveModel {
    /// # Errors
    ///
    /// Error database.
    pub async fn record<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        ticket_id: Uuid,
        actor_id: Option<Uuid>,
        kind: EventKind,
        changes: serde_json::Value,
    ) -> Result<Model, DbErr> {
        Self {
            workspace_id: Set(workspace_id.get()),
            ticket_id: Set(ticket_id),
            actor_id: Set(actor_id),
            kind: Set(kind.to_value()),
            changes: Set(changes),
            ..Default::default()
        }
        .insert(db)
        .await
    }
}
