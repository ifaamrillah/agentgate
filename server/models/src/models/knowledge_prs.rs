//! PR knowledge (R2): setelah usulan knowledge di-approve, server menulis dokumen ke
//! `.agentgate/knowledge/<slug>.md` di branch `agentgate/knowledge-<slug>` repo tertaut project
//! lalu membuka PR (tidak pernah push ke branch utama). Status PR dibaca dari `pull_requests`
//! yang diperbarui webhook.
use sea_orm::{QueryOrder, entity::prelude::*};

pub use super::_entities::knowledge_prs::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

standard_behavior!(ActiveModel);

/// Keadaan pembuatan PR. Dua tempat harus sepakat: CHECK `knowledge_prs.status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, serde::Serialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum KnowledgePrStatus {
    /// Menunggu worker.
    #[sea_orm(string_value = "queued")]
    Queued,
    /// PR terbuka (atau commit ditambahkan ke PR terbuka).
    #[sea_orm(string_value = "open")]
    Open,
    #[sea_orm(string_value = "failed")]
    Failed,
    /// Project tanpa repo tertaut provider: tidak ada PR (bukan galat).
    #[sea_orm(string_value = "skipped")]
    Skipped,
}

impl Model {
    #[must_use]
    pub fn status(&self) -> KnowledgePrStatus {
        KnowledgePrStatus::try_from_value(&self.status).unwrap_or(KnowledgePrStatus::Failed)
    }

    /// PR knowledge dokumen, terbaru dulu.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn of_doc<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        doc: Uuid,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::WorkspaceId.eq(wid.get()))
            .filter(Column::DocId.eq(doc))
            .order_by_desc(Column::CreatedAt)
            .all(db)
            .await
    }
}
