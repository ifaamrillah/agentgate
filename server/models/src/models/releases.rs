//! Rilis/versi project (M9, Fix version): tiket menaut lewat `tickets.release_id`; release notes
//! Markdown berversi (`release_note_versions`).
use sea_orm::{
    ActiveEnum, ColumnTrait, ConnectionTrait, DbErr, DeriveActiveEnum, EntityTrait, EnumIter,
    QueryFilter, QueryOrder,
};
use uuid::Uuid;

pub use super::_entities::releases::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

standard_behavior!(ActiveModel);

pub const NAME_MAX_LEN: usize = 60;
pub const DESCRIPTION_MAX_LEN: usize = 300;
pub const NOTES_MAX_LEN: usize = 100_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum ReleaseStatus {
    /// Rencana (belum ada pekerjaan berjalan).
    #[sea_orm(string_value = "planned")]
    Planned,
    /// Sedang dikerjakan, belum dirilis.
    #[sea_orm(string_value = "unreleased")]
    Unreleased,
    #[sea_orm(string_value = "released")]
    Released,
    #[sea_orm(string_value = "archived")]
    Archived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum NotesSource {
    #[sea_orm(string_value = "")]
    None,
    #[sea_orm(string_value = "ai")]
    Ai,
    #[sea_orm(string_value = "manual")]
    Manual,
}

impl Model {
    #[must_use]
    pub fn status(&self) -> ReleaseStatus {
        ReleaseStatus::try_from_value(&self.status).unwrap_or(ReleaseStatus::Planned)
    }

    #[must_use]
    pub fn notes_source(&self) -> NotesSource {
        NotesSource::try_from_value(&self.notes_source).unwrap_or(NotesSource::None)
    }

    /// Rilis di workspace berdasarkan id.
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

    /// Semua rilis project (target terbaru dulu, tanpa tanggal di atas).
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
            .filter(Column::WorkspaceId.eq(wid.get()))
            .filter(Column::ProjectId.eq(project_id))
            .order_by_desc(Column::TargetOn)
            .order_by_desc(Column::CreatedAt)
            .order_by_desc(Column::Id)
            .all(db)
            .await
    }
}
