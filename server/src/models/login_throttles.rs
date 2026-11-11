//! Penghitung gagal login per kunci (`email:<lower>` / `ip:<addr>`), tanpa Redis (rules §7).
//! Logika rate limit ada di `services::auth::throttle` (Task 3). Primary key-nya `key`, bukan
//! UUID: satu baris per kunci, di-upsert.

use sea_orm::{ActiveValue::Set, entity::prelude::*};

pub use super::_entities::login_throttles::{ActiveModel, Column, Entity, Model};

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if !insert && self.updated_at.is_unchanged() {
            self.updated_at = Set(chrono::Utc::now().into());
        }
        Ok(self)
    }
}
