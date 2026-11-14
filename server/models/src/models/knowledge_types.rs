//! Jenis dokumen custom per workspace (template + instruksi AI), tambahan katalog bawaan.
use sea_orm::{ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

pub use super::_entities::knowledge_types::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

standard_behavior!(ActiveModel);

impl Model {
    /// Berlaku untuk project `project_id` (daftar kosong = semua project).
    #[must_use]
    pub fn applies_to(&self, project_id: Uuid) -> bool {
        self.projects.is_empty() || self.projects.contains(&project_id)
    }

    /// Jenis custom aktif di workspace, urut nama.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn active_in<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::WorkspaceId.eq(wid.get()))
            .filter(Column::ArchivedAt.is_null())
            .order_by_asc(Column::Name)
            .all(db)
            .await
    }

    /// Jenis berdasarkan id (aktif atau diarsipkan).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn find_in<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        id: Uuid,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(id)
            .filter(Column::WorkspaceId.eq(wid.get()))
            .one(db)
            .await
    }
}
