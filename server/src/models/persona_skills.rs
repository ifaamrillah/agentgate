//! Skill pack yang dipasang ke persona. Hanya skill pack `approved` yang boleh dipasang
//! (`services::personas::update`); skill pack yang kembali ke draf/ditolak dilepas.
use loco_rs::model::TenantEntity;
use sea_orm::{QueryOrder, entity::prelude::*};
use uuid::Uuid;

pub use super::_entities::persona_skills::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

impl TenantEntity for Entity {
    type TenantId = WorkspaceId;

    fn tenant_column() -> Column {
        Column::WorkspaceId
    }
}

standard_behavior!(ActiveModel);

impl Model {
    /// Skill pack terpasang di persona, urut dipasang.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn of_persona<C: ConnectionTrait>(
        db: &C,
        persona_id: Uuid,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::PersonaId.eq(persona_id))
            .order_by_asc(Column::CreatedAt)
            .order_by_asc(Column::Id)
            .all(db)
            .await
    }

    /// Semua pemasangan di workspace (hitungan skill per persona / persona per skill).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn all_in<C: ConnectionTrait>(db: &C, wid: WorkspaceId) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::WorkspaceId.eq(wid.get()))
            .order_by_asc(Column::CreatedAt)
            .all(db)
            .await
    }
}
