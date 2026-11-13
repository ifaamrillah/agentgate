//! Sumber dokumen knowledge di repo (repo + path/prefix): dasar deteksi basi dan panel
//! "Terhubung ke graph".
pub use super::_entities::knowledge_sources::{ActiveModel, Column, Entity, Model};

#[async_trait::async_trait]
impl sea_orm::ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, sea_orm::DbErr>
    where
        C: sea_orm::ConnectionTrait,
    {
        if insert && self.id.is_not_set() {
            self.id = sea_orm::ActiveValue::Set(uuid::Uuid::now_v7());
        }
        Ok(self)
    }
}
