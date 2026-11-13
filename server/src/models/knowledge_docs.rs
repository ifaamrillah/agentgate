//! Dokumen knowledge (M8). `project_id` NULL = dokumen workspace (standar lintas project).
//! Status tampilan: basi (`stale_reason`), ada usulan (usulan pending), atau terbaru.
use sea_orm::{ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

pub use super::_entities::knowledge_docs::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

standard_behavior!(ActiveModel);

/// Panjang maksimal judul (karakter), sama dengan CHECK migrasi.
pub const TITLE_MAX: usize = 160;
/// Panjang maksimal isi dokumen (karakter).
pub const BODY_MAX: usize = 200_000;

impl Model {
    #[must_use]
    pub fn is_stale(&self) -> bool {
        !self.stale_reason.is_empty()
    }

    #[must_use]
    pub fn is_workspace_doc(&self) -> bool {
        self.project_id.is_none()
    }

    /// Path dokumen di repo.
    #[must_use]
    pub fn repo_path(&self) -> String {
        format!(".agentgate/knowledge/{}.md", self.slug)
    }

    /// Dokumen aktif project, urut kelompok lalu judul.
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
            .filter(Column::ArchivedAt.is_null())
            .order_by_asc(Column::GroupKey)
            .order_by_asc(Column::Title)
            .all(db)
            .await
    }

    /// Dokumen workspace aktif (semua project), urut judul.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn of_workspace<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::WorkspaceId.eq(wid.get()))
            .filter(Column::ProjectId.is_null())
            .filter(Column::ArchivedAt.is_null())
            .order_by_asc(Column::Title)
            .all(db)
            .await
    }

    /// Dokumen project berdasarkan slug (termasuk yang diarsipkan).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn by_slug<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        project_id: Option<Uuid>,
        slug: &str,
    ) -> Result<Option<Self>, DbErr> {
        let q = Entity::find()
            .filter(Column::WorkspaceId.eq(wid.get()))
            .filter(Column::Slug.eq(slug));
        let q = match project_id {
            Some(p) => q.filter(Column::ProjectId.eq(p)),
            None => q.filter(Column::ProjectId.is_null()),
        };
        q.one(db).await
    }

    /// Dokumen di workspace berdasarkan id.
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
