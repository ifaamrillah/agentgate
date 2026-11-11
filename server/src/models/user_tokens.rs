//! Token sekali pakai yang dikirim lewat email (reset password, verifikasi email), satu tabel
//! untuk semua purpose (rules §7). Token mentah 32 byte acak hanya ada di link email;
//! database menyimpan digest SHA-256-nya. Konsumsi memakai claim bersyarat
//! (`used_at IS NULL AND expires_at > now`, `UPDATE … RETURNING`), sehingga dua request
//! bersamaan dengan token yang sama hanya satu yang menang (rules §5).

use chrono::{Duration, Utc};
use sea_orm::{
    ActiveValue::Set, Condition, PaginatorTrait, QueryFilter, TransactionSession, TransactionTrait,
    entity::prelude::*, sea_query::Expr,
};

pub use super::_entities::user_tokens::{ActiveModel, Column, Entity, Model};
use crate::common::crypto;

/// Kegunaan token. Disimpan sebagai string dengan CHECK constraint di migrasi; menambah
/// varian berarti migrasi yang mengganti constraint itu (rules §4, dijaga `tests/models/enums.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum TokenPurpose {
    #[sea_orm(string_value = "password_reset")]
    PasswordReset,
    #[sea_orm(string_value = "email_verification")]
    EmailVerification,
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
            self.updated_at = Set(Utc::now().into());
        }
        Ok(self)
    }
}

impl Model {
    /// Purpose bertipe dari kolom string. Gagal hanya bila CHECK constraint dilewati.
    ///
    /// # Errors
    ///
    /// Nilai kolom bukan varian [`TokenPurpose`].
    pub fn purpose(&self) -> Result<TokenPurpose, DbErr> {
        TokenPurpose::try_from_value(&self.purpose)
    }
}

/// Buat token baru untuk `user_id` yang berlaku `ttl`, dan hapus token lain milik user itu
/// dengan purpose yang sama (permintaan baru membatalkan link lama). Mengembalikan token
/// mentah untuk link email; yang tersimpan hanya digest-nya.
///
/// # Errors
///
/// Error database.
pub async fn issue<C>(
    db: &C,
    user_id: Uuid,
    purpose: TokenPurpose,
    ttl: Duration,
) -> Result<String, DbErr>
where
    C: ConnectionTrait + TransactionTrait,
{
    let raw = crypto::random_token();
    let txn = db.begin().await?;
    Entity::delete_many()
        .filter(Column::UserId.eq(user_id))
        .filter(Column::Purpose.eq(purpose.to_value()))
        .exec(&txn)
        .await?;
    ActiveModel {
        user_id: Set(user_id),
        purpose: Set(purpose.to_value()),
        token_digest: Set(crypto::sha256_hex(&raw)),
        expires_at: Set((Utc::now() + ttl).into()),
        ..Default::default()
    }
    .insert(&txn)
    .await?;
    txn.commit().await?;
    Ok(raw)
}

/// Pakai token: tandai `used_at` bila token cocok, purpose sama, belum dipakai, dan belum
/// kedaluwarsa. Mengembalikan pemilik token, atau `None` untuk token yang tidak berlaku
/// karena alasan apa pun (alasannya sengaja tidak dibedakan).
///
/// # Errors
///
/// Error database.
pub async fn consume<C: ConnectionTrait>(
    db: &C,
    raw: &str,
    purpose: TokenPurpose,
) -> Result<Option<Uuid>, DbErr> {
    let now = Utc::now();
    let claimed = Entity::update_many()
        .col_expr(Column::UsedAt, Expr::value(now))
        .col_expr(Column::UpdatedAt, Expr::value(now))
        .filter(usable(raw, purpose))
        .exec_with_returning(db)
        .await?;
    Ok(match claimed.as_slice() {
        [token] => Some(token.user_id),
        _ => None,
    })
}

/// Apakah token masih bisa dipakai, tanpa mengonsumsinya (untuk `GET` link reset: pemindai
/// link email membuka link tanpa diklik user).
///
/// # Errors
///
/// Error database.
pub async fn peek<C: ConnectionTrait>(
    db: &C,
    raw: &str,
    purpose: TokenPurpose,
) -> Result<bool, DbErr> {
    Ok(peek_owner(db, raw, purpose).await?.is_some())
}

/// Pemilik token yang masih bisa dipakai, tanpa mengonsumsinya. Untuk memeriksa token (dan
/// pemiliknya) sebelum pekerjaan mahal; konsumsi tetap lewat [`consume`].
///
/// # Errors
///
/// Error database.
pub async fn peek_owner<C: ConnectionTrait>(
    db: &C,
    raw: &str,
    purpose: TokenPurpose,
) -> Result<Option<Uuid>, DbErr> {
    Ok(Entity::find()
        .filter(usable(raw, purpose))
        .one(db)
        .await?
        .map(|token| token.user_id))
}

/// Satu definisi "token berlaku", dipakai `consume` dan `peek` supaya tidak bisa menyimpang.
fn usable(raw: &str, purpose: TokenPurpose) -> Condition {
    Condition::all()
        .add(Column::TokenDigest.eq(crypto::sha256_hex(raw)))
        .add(Column::Purpose.eq(purpose.to_value()))
        .add(Column::UsedAt.is_null())
        .add(Column::ExpiresAt.gt(Utc::now()))
}

/// Hitung token yang sudah selesai sebelum `cutoff` (lihat [`delete_finished_before`]) tanpa
/// menghapusnya (dry run job pembersihan).
///
/// # Errors
///
/// Error database.
pub async fn count_finished_before<C: ConnectionTrait>(
    db: &C,
    cutoff: chrono::DateTime<Utc>,
) -> Result<u64, DbErr> {
    Entity::find()
        .filter(finished_before(cutoff))
        .count(db)
        .await
}

/// Hapus token yang kedaluwarsa atau dipakai sebelum `cutoff` (job pembersihan: 7 hari lalu).
/// Token seperti ini tidak bisa dipakai lagi; disimpan sebentar hanya untuk penelusuran.
///
/// # Errors
///
/// Error database.
pub async fn delete_finished_before<C: ConnectionTrait>(
    db: &C,
    cutoff: chrono::DateTime<Utc>,
) -> Result<u64, DbErr> {
    Entity::delete_many()
        .filter(finished_before(cutoff))
        .exec(db)
        .await
        .map(|res| res.rows_affected)
}

fn finished_before(cutoff: chrono::DateTime<Utc>) -> Condition {
    Condition::any()
        .add(Column::ExpiresAt.lt(cutoff))
        .add(Column::UsedAt.lt(cutoff))
}
