//! Rate limit API publik per token (M11): jendela tetap per menit, satu baris per token,
//! di-upsert atomik di Postgres (tanpa Redis, seperti `login_throttles`). Primary key-nya
//! `token_id`, bukan UUID baru: satu baris per token.

use chrono::{DateTime, Duration, DurationRound, Utc};
use sea_orm::{DbBackend, FromQueryResult, Statement, entity::prelude::*};

pub use super::_entities::api_rate_limits::{ActiveModel, Column, Entity, Model};

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if !insert && self.updated_at.is_unchanged() {
            self.updated_at = sea_orm::ActiveValue::Set(Utc::now().into());
        }
        Ok(self)
    }
}

#[derive(Debug, FromQueryResult)]
struct Count {
    requests: i32,
}

/// Hitung satu request token pada `now` dan kembalikan jumlah request di menit ini (termasuk
/// request ini). Jendela berganti di awal setiap menit (UTC).
///
/// # Errors
///
/// Error database.
pub async fn hit<C: ConnectionTrait>(
    db: &C,
    token_id: Uuid,
    now: DateTime<Utc>,
) -> Result<i32, DbErr> {
    let window = now.duration_trunc(Duration::minutes(1)).unwrap_or(now);
    let row = Count::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO api_rate_limits (token_id, window_started_at, requests)
         VALUES ($1, $2, 1)
         ON CONFLICT (token_id) DO UPDATE SET
           requests = CASE WHEN api_rate_limits.window_started_at = EXCLUDED.window_started_at
                      THEN api_rate_limits.requests + 1 ELSE 1 END,
           window_started_at = EXCLUDED.window_started_at,
           updated_at = now()
         RETURNING requests",
        [token_id.into(), DateTimeWithTimeZone::from(window).into()],
    ))
    .one(db)
    .await?;
    Ok(row.map_or(1, |r| r.requests))
}

/// Hapus baris yang jendelanya sudah lewat lebih dari satu jam (job pembersihan).
///
/// # Errors
///
/// Error database.
pub async fn delete_stale<C: ConnectionTrait>(
    db: &C,
    now: DateTime<Utc>,
    dry_run: bool,
) -> Result<u64, DbErr> {
    use sea_orm::PaginatorTrait;
    let cutoff = DateTimeWithTimeZone::from(now - Duration::hours(1));
    if dry_run {
        return Entity::find()
            .filter(Column::WindowStartedAt.lt(cutoff))
            .count(db)
            .await;
    }
    Ok(Entity::delete_many()
        .filter(Column::WindowStartedAt.lt(cutoff))
        .exec(db)
        .await?
        .rows_affected)
}
