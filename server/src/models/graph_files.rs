//! File repo yang di-index (M7): isi untuk `ag_snippet` dan fakta tree-sitter untuk build ulang.
//! File dilewati disimpan tanpa isi beserta alasannya.
use agentgate_graph::FileFacts;
use sea_orm::{QuerySelect, entity::prelude::*};

pub use super::_entities::graph_files::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

standard_behavior!(ActiveModel);

impl Model {
    /// Fakta tree-sitter (`None` bila file dilewati atau data lama tidak terbaca).
    #[must_use]
    pub fn facts(&self) -> Option<FileFacts> {
        if !self.skip_reason.is_empty() {
            return None;
        }
        serde_json::from_value(self.facts.clone()).ok()
    }

    /// `(path, sha)` semua file repo (manifest inkremental).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn manifest<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        repo: Uuid,
    ) -> Result<Vec<(Uuid, String, String)>, DbErr> {
        Entity::find()
            .select_only()
            .column(Column::Id)
            .column(Column::Path)
            .column(Column::Sha)
            .filter(Column::WorkspaceId.eq(wid.get()))
            .filter(Column::RepoId.eq(repo))
            .into_tuple()
            .all(db)
            .await
    }

    /// Satu file repo menurut path.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn by_path<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        repo: Uuid,
        path: &str,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find()
            .filter(Column::WorkspaceId.eq(wid.get()))
            .filter(Column::RepoId.eq(repo))
            .filter(Column::Path.eq(path))
            .one(db)
            .await
    }
}
