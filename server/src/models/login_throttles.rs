//! Penghitung gagal login per kunci (`email:<lower>` / `ip:<addr>`), tanpa Redis (rules §7).
//! Logika rate limit ada di `services::auth::throttle` (Task 3). Primary key-nya `key`, bukan
//! UUID: satu baris per kunci, di-upsert.

use chrono::{DateTime, Duration, Utc};
use sea_orm::{ActiveValue::Set, Condition, PaginatorTrait, entity::prelude::*};

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

/// Hitung baris basi (lihat [`delete_stale`]) tanpa menghapusnya (dry run job pembersihan).
///
/// # Errors
///
/// Error database.
pub async fn count_stale<C: ConnectionTrait>(
    db: &C,
    window: Duration,
    now: DateTime<Utc>,
) -> Result<u64, DbErr> {
    Entity::find().filter(stale(window, now)).count(db).await
}

/// Hapus baris basi: tidak terkunci (atau kuncinya sudah lewat) dan jendela hitungnya
/// (`window`, `settings.auth.login_window_minutes`) sudah lewat. Baris seperti ini tidak lagi
/// memengaruhi rate limit (gagal berikutnya memulai jendela baru), jadi aman dibuang.
///
/// # Errors
///
/// Error database.
pub async fn delete_stale<C: ConnectionTrait>(
    db: &C,
    window: Duration,
    now: DateTime<Utc>,
) -> Result<u64, DbErr> {
    Entity::delete_many()
        .filter(stale(window, now))
        .exec(db)
        .await
        .map(|res| res.rows_affected)
}

fn stale(window: Duration, now: DateTime<Utc>) -> Condition {
    Condition::all()
        .add(Column::WindowStartedAt.lt(now - window))
        .add(
            Condition::any()
                .add(Column::LockedUntil.is_null())
                .add(Column::LockedUntil.lte(now)),
        )
}
