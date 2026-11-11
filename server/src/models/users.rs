//! User aplikasi. Email dinormalisasi (`trim().to_lowercase()`) di sini dan unik lewat
//! expression index `lower(email)`; keunikan ditegakkan database, bukan SELECT lebih dulu
//! (rules §5). `password_hash` NULL berarti user hanya login lewat OAuth. Hash dibuat
//! pemanggil lewat `services::auth::password::hash`: model tidak boleh memanggil services
//! (arah layer §3), jadi `NewUser` menerima PHC string, bukan password mentah.

use loco_rs::validation::ModelValidationErrors;
use sea_orm::{ActiveValue::Set, IntoActiveModel, SqlErr, entity::prelude::*};
use validator::{ValidateEmail, ValidationError, ValidationErrors};

pub use super::_entities::users::{ActiveModel, Column, Entity, Model};
use crate::common::text;

pub const EMAIL_INVALID: &str = "email-invalid";
pub const EMAIL_TAKEN: &str = "email-taken";
/// Kode error field milik model ini (kode nama ada di `common::text`); masing-masing punya
/// pesan di Fluent (dijaga tes i18n).
pub const ERROR_CODES: &[&str] = &[EMAIL_INVALID, EMAIL_TAKEN];

/// Data untuk [`ActiveModel::create`].
#[derive(Debug)]
pub struct NewUser {
    pub email: String,
    pub name: String,
    /// PHC string dari `password::hash`; `None` untuk user OAuth.
    pub password_hash: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum UserError {
    /// Email (case-insensitive) sudah dipakai user lain.
    #[error("email already taken")]
    EmailTaken,
    /// Validasi field (`email`, `name`) gagal; berisi kode error per field.
    #[error("invalid user: {0}")]
    Invalid(ModelValidationErrors),
    #[error(transparent)]
    Db(#[from] DbErr),
}

impl From<UserError> for loco_rs::Error {
    fn from(err: UserError) -> Self {
        match err {
            UserError::EmailTaken => {
                let mut errors = ValidationErrors::new();
                errors.add("email", ValidationError::new(EMAIL_TAKEN));
                Self::Validation(errors.into())
            }
            UserError::Invalid(errors) => Self::Validation(errors),
            UserError::Db(e) => Self::DB(e),
        }
    }
}

/// Normalisasi email satu-satunya: dipakai saat simpan dan saat mencari.
#[must_use]
pub fn normalize_email(email: &str) -> String {
    email.trim().to_lowercase()
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
    /// User dengan email ini (setelah normalisasi), aktif atau tidak.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn find_by_email<C: ConnectionTrait>(
        db: &C,
        email: &str,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find()
            .filter(Column::Email.eq(normalize_email(email)))
            .one(db)
            .await
    }

    /// User dengan id ini, aktif atau tidak.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn find_by_id<C: ConnectionTrait>(db: &C, id: Uuid) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(id).one(db).await
    }

    /// Ganti PHC string tanpa menyentuh `password_changed_at`: dipakai rehash saat login,
    /// yang bukan penggantian password oleh user.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn rehash_password<C: ConnectionTrait>(
        self,
        db: &C,
        password_hash: String,
    ) -> Result<Self, DbErr> {
        let mut user = self.into_active_model();
        user.password_hash = Set(Some(password_hash));
        user.update(db).await
    }

    /// Boleh login dan memakai sesi: belum dinonaktifkan.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.disabled_at.is_none()
    }
}

impl ActiveModel {
    /// Buat user setelah menormalisasi dan memvalidasi email dan nama.
    ///
    /// # Errors
    ///
    /// [`UserError::Invalid`] untuk field tidak valid, [`UserError::EmailTaken`] bila unique
    /// index `lower(email)` menolak, [`UserError::Db`] untuk error database lain.
    pub async fn create<C: ConnectionTrait>(db: &C, new: NewUser) -> Result<Model, UserError> {
        let email = normalize_email(&new.email);
        let name = text::normalize_name(&new.name);
        validate(&email, &name)?;
        let user = Self {
            email: Set(email),
            name: Set(name),
            password_hash: Set(new.password_hash),
            ..Default::default()
        };
        user.insert(db).await.map_err(|e| {
            if matches!(e.sql_err(), Some(SqlErr::UniqueConstraintViolation(_))) {
                UserError::EmailTaken
            } else {
                UserError::Db(e)
            }
        })
    }
}

fn validate(email: &str, name: &str) -> Result<(), UserError> {
    let mut errors = ValidationErrors::new();
    if !email.validate_email() {
        errors.add("email", ValidationError::new(EMAIL_INVALID));
    }
    if let Err(e) = text::validate_name("user", name) {
        errors.add("name", e);
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(UserError::Invalid(errors.into()))
    }
}
