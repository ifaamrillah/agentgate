//! Kode pemulihan 2FA, disimpan sebagai digest; logikanya ada di `services::auth::totp` (Task 6).

use sea_orm::{ActiveValue::Set, entity::prelude::*};

pub use super::_entities::recovery_codes::{ActiveModel, Column, Entity, Model};

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if insert && self.id.is_not_set() {
            self.id = Set(Uuid::now_v7());
        }
        if !insert && self.updated_at.is_unchanged() {
            self.updated_at = Set(chrono::Utc::now().into());
        }
        Ok(self)
    }
}
