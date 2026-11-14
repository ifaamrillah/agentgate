//! Watcher tiket: orang yang mengikuti tiket menerima notifikasi komentar. Pembuat dan penerima
//! tugas otomatis mengikuti; siapa pun yang bisa melihat tiket bisa ikut atau berhenti.

use loco_rs::model::{TenantEntity, TenantQueryExt};
use sea_orm::{ActiveValue::Set, QueryOrder, entity::prelude::*, sea_query::OnConflict};

pub use super::_entities::ticket_watchers::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

impl TenantEntity for Entity {
    type TenantId = WorkspaceId;

    fn tenant_column() -> Column {
        Column::WorkspaceId
    }
}

standard_behavior!(ActiveModel);

impl Model {
    /// Id user yang mengikuti tiket.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn user_ids<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        ticket_id: Uuid,
    ) -> Result<Vec<Uuid>, DbErr> {
        Ok(Entity::find()
            .in_tenant(workspace_id)
            .filter(Column::TicketId.eq(ticket_id))
            .order_by_asc(Column::CreatedAt)
            .all(db)
            .await?
            .into_iter()
            .map(|w| w.user_id)
            .collect())
    }
}

impl ActiveModel {
    /// Ikuti (idempoten). Bukan anggota workspace → error FK; pemanggil hanya memanggil untuk
    /// anggota.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn watch<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        ticket_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), DbErr> {
        let row = Self {
            id: Set(Uuid::now_v7()),
            workspace_id: Set(workspace_id.get()),
            ticket_id: Set(ticket_id),
            user_id: Set(user_id),
            ..Default::default()
        };
        Entity::insert(row)
            .on_conflict(OnConflict::new().do_nothing().to_owned())
            .exec_without_returning(db)
            .await?;
        Ok(())
    }

    /// # Errors
    ///
    /// Error database.
    pub async fn unwatch<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        ticket_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), DbErr> {
        Entity::delete_many()
            .filter(Column::WorkspaceId.eq(workspace_id.get()))
            .filter(Column::TicketId.eq(ticket_id))
            .filter(Column::UserId.eq(user_id))
            .exec(db)
            .await?;
        Ok(())
    }
}
