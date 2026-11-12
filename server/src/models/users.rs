//! User aplikasi. Email dinormalisasi (`trim().to_lowercase()`) di sini dan unik lewat
//! expression index `lower(email)`; keunikan ditegakkan database, bukan SELECT lebih dulu
//! (rules §5). `password_hash` NULL berarti user hanya login lewat OAuth. Hash dibuat
//! pemanggil lewat `services::auth::password::hash`: model tidak boleh memanggil services
//! (arah layer §3), jadi `NewUser` menerima PHC string, bukan password mentah.

use loco_rs::validation::ModelValidationErrors;
use sea_orm::{
    ActiveValue::Set, Condition, IntoActiveModel, QuerySelect, SqlErr, entity::prelude::*,
    sea_query::Expr,
};
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
    /// Simpan tampilan project yang disematkan user di bar tab (urut). Kunci divalidasi
    /// pemanggil (`views::nav::PROJECT_VIEWS`); kosong = bawaan.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn set_project_views<C: ConnectionTrait>(
        self,
        db: &C,
        views: Vec<String>,
    ) -> Result<Self, DbErr> {
        let mut user = self.into_active_model();
        user.project_views = Set(views);
        user.update(db).await
    }

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

    /// Nama user `ids` (satu query), mis. pelaku di halaman audit. Id yang tidak ada (user
    /// sudah dihapus) tidak ada di hasil.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn names_by_id<C: ConnectionTrait>(
        db: &C,
        ids: &[Uuid],
    ) -> Result<std::collections::HashMap<Uuid, String>, DbErr> {
        if ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }
        let rows: Vec<(Uuid, String)> = Entity::find()
            .select_only()
            .column(Column::Id)
            .column(Column::Name)
            .filter(Column::Id.is_in(ids.iter().copied()))
            .into_tuple()
            .all(db)
            .await?;
        Ok(rows.into_iter().collect())
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

    /// Password baru dipilih user (ganti password): simpan PHC string dan
    /// `password_changed_at`.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn change_password<C: ConnectionTrait>(
        self,
        db: &C,
        password_hash: String,
    ) -> Result<Self, DbErr> {
        let mut user = self.into_active_model();
        user.password_hash = Set(Some(password_hash));
        user.password_changed_at = Set(Some(chrono::Utc::now().into()));
        user.update(db).await
    }

    /// Password baru lewat link reset: seperti [`Self::change_password`], dan
    /// `email_verified_at` diisi bila masih kosong, karena user terbukti memegang inbox-nya.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn reset_password<C: ConnectionTrait>(
        self,
        db: &C,
        password_hash: String,
    ) -> Result<Self, DbErr> {
        let now: DateTimeWithTimeZone = chrono::Utc::now().into();
        let verified = self.email_verified_at.unwrap_or(now);
        let mut user = self.into_active_model();
        user.password_hash = Set(Some(password_hash));
        user.password_changed_at = Set(Some(now));
        user.email_verified_at = Set(Some(verified));
        user.update(db).await
    }

    /// Tandai email terverifikasi. Waktu verifikasi pertama dipertahankan bila sudah ada.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn mark_email_verified<C: ConnectionTrait>(self, db: &C) -> Result<Self, DbErr> {
        if self.email_verified_at.is_some() {
            return Ok(self);
        }
        let mut user = self.into_active_model();
        user.email_verified_at = Set(Some(chrono::Utc::now().into()));
        user.update(db).await
    }

    /// Email dijamin provider OAuth (`email_verified`) saat identitas ditautkan: tandai
    /// terverifikasi. Bila sebelumnya belum terverifikasi, password lokal dibuang: password itu
    /// bisa dibuat orang lain yang mendaftarkan email ini lebih dulu tanpa memegang inbox-nya
    /// (pre-hijacking). Pemilik email bisa membuat password lewat lupa password.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn verify_email_from_provider<C: ConnectionTrait>(
        self,
        db: &C,
    ) -> Result<Self, DbErr> {
        if self.email_verified_at.is_some() {
            return Ok(self);
        }
        let mut user = self.into_active_model();
        user.email_verified_at = Set(Some(chrono::Utc::now().into()));
        user.password_hash = Set(None);
        user.update(db).await
    }

    /// Ganti nama tampilan setelah normalisasi dan validasi yang sama dengan [`ActiveModel::create`].
    ///
    /// # Errors
    ///
    /// [`UserError::Invalid`] untuk nama tidak valid (field `name`), [`UserError::Db`] untuk
    /// error database.
    pub async fn rename<C: ConnectionTrait>(self, db: &C, name: &str) -> Result<Self, UserError> {
        let name = text::normalize_name(name);
        if let Err(e) = text::validate_name("user", &name) {
            let mut errors = ValidationErrors::new();
            errors.add("name", e);
            return Err(UserError::Invalid(errors.into()));
        }
        let mut user = self.into_active_model();
        user.name = Set(name);
        Ok(user.update(db).await?)
    }

    /// Kunci baris user ini (`SELECT … FOR UPDATE`) sampai transaksi `db` selesai, dan
    /// kembalikan nilainya yang terbaru. Untuk perubahan yang harus berurutan per user
    /// (aktifkan 2FA, buat ulang kode pemulihan): request kedua menunggu yang pertama commit,
    /// lalu melihat hasilnya.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn lock_for_update<C: ConnectionTrait>(
        db: &C,
        id: Uuid,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(id).lock_exclusive().one(db).await
    }

    /// Aktifkan 2FA dengan secret terenkripsi `secret_encrypted`, hanya bila belum aktif
    /// (`UPDATE` bersyarat). `step` adalah langkah kode konfirmasi, supaya kode itu tidak bisa
    /// dipakai lagi untuk login. `false`: 2FA sudah aktif (mis. klik ganda).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn enable_totp<C: ConnectionTrait>(
        &self,
        db: &C,
        secret_encrypted: Vec<u8>,
        step: i64,
    ) -> Result<bool, DbErr> {
        let now = chrono::Utc::now();
        let res = Entity::update_many()
            .col_expr(Column::TotpSecretEncrypted, Expr::value(secret_encrypted))
            .col_expr(Column::TotpEnabledAt, Expr::value(now))
            .col_expr(Column::TotpLastStep, Expr::value(step))
            .col_expr(Column::UpdatedAt, Expr::value(now))
            .filter(Column::Id.eq(self.id))
            .filter(Column::TotpEnabledAt.is_null())
            .exec(db)
            .await?;
        Ok(res.rows_affected == 1)
    }

    /// Matikan 2FA: secret, waktu aktif, dan langkah terakhir dikosongkan.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn disable_totp<C: ConnectionTrait>(self, db: &C) -> Result<Self, DbErr> {
        let mut user = self.into_active_model();
        user.totp_secret_encrypted = Set(None);
        user.totp_enabled_at = Set(None);
        user.totp_last_step = Set(None);
        user.update(db).await
    }

    /// Catat `step` sebagai langkah TOTP terakhir yang diterima, hanya bila lebih baru dari
    /// yang tersimpan: satu `UPDATE` bersyarat, sehingga kode yang sama tidak bisa dipakai dua
    /// kali, juga oleh dua request bersamaan. `false` berarti langkah itu sudah (atau
    /// langkah yang lebih baru sudah) dipakai.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn claim_totp_step<C: ConnectionTrait>(
        &self,
        db: &C,
        step: i64,
    ) -> Result<bool, DbErr> {
        let res = Entity::update_many()
            .col_expr(Column::TotpLastStep, Expr::value(step))
            .col_expr(Column::UpdatedAt, Expr::value(chrono::Utc::now()))
            .filter(Column::Id.eq(self.id))
            .filter(Column::TotpEnabledAt.is_not_null())
            .filter(
                Condition::any()
                    .add(Column::TotpLastStep.is_null())
                    .add(Column::TotpLastStep.lt(step)),
            )
            .exec(db)
            .await?;
        Ok(res.rows_affected == 1)
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

/// Validasi email dan nama untuk user baru tanpa menyimpan apa pun, setelah normalisasi yang
/// sama dengan [`ActiveModel::create`]. Untuk form yang memvalidasi semua field sekaligus
/// (mis. bersama aturan password) sebelum pekerjaan mahal seperti hashing.
///
/// # Errors
///
/// Kode error per field (`email`, `name`).
pub fn check_new(email: &str, name: &str) -> Result<(), ModelValidationErrors> {
    match validate(&normalize_email(email), &text::normalize_name(name)) {
        Err(UserError::Invalid(errors)) => Err(errors),
        _ => Ok(()),
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
