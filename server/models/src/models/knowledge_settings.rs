//! Pengaturan update otomatis knowledge per workspace. Baris dibuat saat pertama disimpan;
//! sebelum itu [`Model::defaults`] yang berlaku.
use sea_orm::{ConnectionTrait, DbErr, EntityTrait};

pub use super::_entities::knowledge_settings::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

impl sea_orm::ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Nilai bawaan workspace yang belum pernah menyimpan pengaturan.
    #[must_use]
    pub fn defaults(wid: WorkspaceId) -> Self {
        Self {
            workspace_id: wid.get(),
            on_done: true,
            on_merge: true,
            audit: true,
            audit_day: 1,
            default_depth: "standar".into(),
            repo_sync: true,
            last_audit_at: None,
            updated_by: None,
            updated_at: chrono::Utc::now().into(),
        }
    }

    /// Pengaturan workspace (atau bawaan).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn of<C: ConnectionTrait>(db: &C, wid: WorkspaceId) -> Result<Self, DbErr> {
        Ok(Entity::find_by_id(wid.get())
            .one(db)
            .await?
            .unwrap_or_else(|| Self::defaults(wid)))
    }
}
