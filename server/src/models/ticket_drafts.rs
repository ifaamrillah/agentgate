//! Draft tiket hasil brainstorming (`ag_draft_tickets`): ditinjau manusia di halaman "Draft
//! tiket" project sebelum menjadi tiket sungguhan. Hierarki lewat `parent_id` (epic → story →
//! subtask); `approved` = dicentang untuk dibuat.
use loco_rs::model::{TenantEntity, TenantQueryExt};
use sea_orm::{QueryOrder, entity::prelude::*};

pub use super::_entities::ticket_drafts::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

/// Batas draft per panggilan dan per project.
pub const MAX_PER_CALL: usize = 50;
pub const MAX_PER_PROJECT: u64 = 200;

impl TenantEntity for Entity {
    type TenantId = WorkspaceId;

    fn tenant_column() -> Column {
        Column::WorkspaceId
    }
}

standard_behavior!(ActiveModel);

impl Model {
    /// Estimasi AI draft (`None` = tanpa estimasi / tanpa jam per stage).
    #[must_use]
    pub fn estimate(&self) -> Option<crate::services::drafts::DraftEstimate> {
        serde_json::from_value::<crate::services::drafts::DraftEstimate>(self.estimate.clone())
            .ok()
            .filter(|e| !e.stages.is_empty())
    }

    /// Draft project, urut posisi.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn of_project<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        project_id: Uuid,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .in_tenant(wid)
            .filter(Column::ProjectId.eq(project_id))
            .order_by_asc(Column::Position)
            .order_by_asc(Column::CreatedAt)
            .all(db)
            .await
    }

    /// Jumlah draft project.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn count_in_project<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        project_id: Uuid,
    ) -> Result<u64, DbErr> {
        Entity::find()
            .in_tenant(wid)
            .filter(Column::ProjectId.eq(project_id))
            .count(db)
            .await
    }

    /// # Errors
    ///
    /// Error database.
    pub async fn find_in_project<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        project_id: Uuid,
        id: Uuid,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(id)
            .in_tenant(wid)
            .filter(Column::ProjectId.eq(project_id))
            .one(db)
            .await
    }
}
