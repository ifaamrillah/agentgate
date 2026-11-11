//! Identitas login eksternal (Google, GitHub) yang ditautkan ke user. Satu identitas per
//! `(provider, provider_uid)`; logika tautan ada di `services::auth::oauth` (Task 7).

use sea_orm::{ActiveValue::Set, entity::prelude::*, sea_query::OnConflict};

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
    /// Identitas `(provider, provider_uid)`, bila sudah ditautkan.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn find_by_provider_uid<C: ConnectionTrait>(
        db: &C,
        provider: IdentityProvider,
        provider_uid: &str,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find()
            .filter(Column::Provider.eq(provider.to_value()))
            .filter(Column::ProviderUid.eq(provider_uid))
            .one(db)
            .await
    }

    /// Tautkan `(provider, provider_uid)` ke `user_id` lewat insert bersyarat
    /// (`ON CONFLICT DO NOTHING` pada unique `(provider, provider_uid)`): callback bersamaan
    /// tidak membuat dua baris, dan yang kalah tidak gagal. Mengembalikan baris yang berlaku,
    /// yang bisa milik user lain bila callback lain menautkannya lebih dulu.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn link<C: ConnectionTrait>(
        db: &C,
        user_id: Uuid,
        provider: IdentityProvider,
        provider_uid: &str,
        email: Option<&str>,
    ) -> Result<Self, DbErr> {
        // `Entity::insert` tidak memanggil `before_save`: id diisi di sini.
        let row = ActiveModel {
            id: Set(Uuid::now_v7()),
            user_id: Set(user_id),
            provider: Set(provider.to_value()),
            provider_uid: Set(provider_uid.to_string()),
            email: Set(email.map(str::to_string)),
            ..Default::default()
        };
        Entity::insert(row)
            .on_conflict(
                OnConflict::columns([Column::Provider, Column::ProviderUid])
                    .do_nothing()
                    .to_owned(),
            )
            .exec_without_returning(db)
            .await?;
        Self::find_by_provider_uid(db, provider, provider_uid)
            .await?
            .ok_or_else(|| DbErr::RecordNotFound("identity after insert".to_string()))
    }

    /// Provider bertipe dari kolom string. Gagal hanya bila CHECK constraint dilewati.
    ///
    /// # Errors
    ///
    /// Nilai kolom bukan varian [`IdentityProvider`].
    pub fn provider(&self) -> Result<IdentityProvider, DbErr> {
        IdentityProvider::try_from_value(&self.provider)
    }
}
