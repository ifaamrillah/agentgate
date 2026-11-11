//! Kode pemulihan 2FA. Database hanya menyimpan digest SHA-256 bentuk ternormalisasinya;
//! pembuatan dan normalisasi kode ada di `services::auth::totp`.

use chrono::Utc;
use sea_orm::{ActiveValue::Set, entity::prelude::*, sea_query::Expr};

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

/// Ganti semua kode user dengan `digests`. Pemanggil menjalankannya di dalam transaksi
/// bersama perubahan lain (aktifkan 2FA), sehingga user tidak pernah tanpa kode.
///
/// # Errors
///
/// Error database.
pub async fn replace_for_user<C: ConnectionTrait>(
    db: &C,
    user_id: Uuid,
    digests: &[String],
) -> Result<(), DbErr> {
    delete_for_user(db, user_id).await?;
    let rows = digests.iter().map(|digest| ActiveModel {
        id: Set(Uuid::now_v7()),
        user_id: Set(user_id),
        code_digest: Set(digest.clone()),
        ..Default::default()
    });
    Entity::insert_many(rows).exec(db).await?;
    Ok(())
}

/// Hapus semua kode user (2FA dimatikan).
///
/// # Errors
///
/// Error database.
pub async fn delete_for_user<C: ConnectionTrait>(db: &C, user_id: Uuid) -> Result<(), DbErr> {
    Entity::delete_many()
        .filter(Column::UserId.eq(user_id))
        .exec(db)
        .await?;
    Ok(())
}

/// Pakai kode dengan digest `digest`: claim bersyarat (`used_at IS NULL`), sehingga dua
/// request bersamaan dengan kode yang sama tidak bisa sama-sama berhasil.
///
/// # Errors
///
/// Error database.
pub async fn claim<C: ConnectionTrait>(db: &C, user_id: Uuid, digest: &str) -> Result<bool, DbErr> {
    let now = Utc::now();
    let res = Entity::update_many()
        .col_expr(Column::UsedAt, Expr::value(now))
        .col_expr(Column::UpdatedAt, Expr::value(now))
        .filter(Column::UserId.eq(user_id))
        .filter(Column::CodeDigest.eq(digest))
        .filter(Column::UsedAt.is_null())
        .exec(db)
        .await?;
    Ok(res.rows_affected == 1)
}

/// Jumlah kode user yang belum dipakai.
///
/// # Errors
///
/// Error database.
pub async fn count_unused<C: ConnectionTrait>(db: &C, user_id: Uuid) -> Result<u64, DbErr> {
    Entity::find()
        .filter(Column::UserId.eq(user_id))
        .filter(Column::UsedAt.is_null())
        .count(db)
        .await
}
