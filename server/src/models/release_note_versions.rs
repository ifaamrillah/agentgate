//! Versi release notes (M9): setiap simpan (manusia atau AI) = versi baru.
use sea_orm::{
    ActiveEnum, ColumnTrait, ConnectionTrait, DbErr, DeriveActiveEnum, EntityTrait, EnumIter,
    QueryFilter, QueryOrder, QuerySelect,
};
use uuid::Uuid;

pub use super::_entities::release_note_versions::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

#[async_trait::async_trait]
impl sea_orm::ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if insert && self.id.is_not_set() {
            self.id = sea_orm::ActiveValue::Set(Uuid::now_v7());
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum VersionSource {
    #[sea_orm(string_value = "ai")]
    Ai,
    #[sea_orm(string_value = "manual")]
    Manual,
}

impl Model {
    #[must_use]
    pub fn source(&self) -> VersionSource {
        VersionSource::try_from_value(&self.source).unwrap_or(VersionSource::Manual)
    }

    /// Versi terbaru dulu.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn of_release<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        release_id: Uuid,
        limit: u64,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::WorkspaceId.eq(wid.get()))
            .filter(Column::ReleaseId.eq(release_id))
            .order_by_desc(Column::Number)
            .limit(limit)
            .all(db)
            .await
    }
}
