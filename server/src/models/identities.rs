//! Identitas login eksternal (Google, GitHub) yang ditautkan ke user. Satu identitas per
//! `(provider, provider_uid)`; logika tautan ada di `services::auth::oauth` (Task 7).

use sea_orm::{ActiveValue::Set, entity::prelude::*};

pub use super::_entities::identities::{ActiveModel, Column, Entity, Model};

/// Provider OAuth. Disimpan sebagai string dengan CHECK constraint (rules §4, dijaga
/// `tests/models/enums.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum IdentityProvider {
    #[sea_orm(string_value = "google")]
    Google,
    #[sea_orm(string_value = "github")]
    Github,
}

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

impl Model {
    /// Provider bertipe dari kolom string. Gagal hanya bila CHECK constraint dilewati.
    ///
    /// # Errors
    ///
    /// Nilai kolom bukan varian [`IdentityProvider`].
    pub fn provider(&self) -> Result<IdentityProvider, DbErr> {
        IdentityProvider::try_from_value(&self.provider)
    }
}
