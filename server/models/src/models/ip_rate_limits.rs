//! Rate limit per IP untuk endpoint publik tanpa login (M12): jendela tetap per `bucket`, satu
//! baris per (bucket, IP), di-upsert atomik di Postgres (pola sama dengan `api_rate_limits`).
//! Tabel tingkat instance tanpa entity SeaORM: hanya dua query, keduanya di sini.

use std::net::IpAddr;

use chrono::{DateTime, Duration, DurationRound, Utc};
use sea_orm::{ConnectionTrait, DbBackend, DbErr, Statement, prelude::DateTimeWithTimeZone};

/// Bucket pendaftaran client OAuth dinamis (`/oauth/register`).
pub const OAUTH_REGISTER: &str = "oauth_register";

/// Hitung satu request `ip` di `bucket` pada `now` dan kembalikan jumlah request di jendela ini
/// (termasuk request ini). Jendela berganti di kelipatan `window` (UTC).
///
/// # Errors
///
/// Error database.
pub async fn hit<C: ConnectionTrait>(
    db: &C,
    bucket: &str,
    ip: IpAddr,
    window: Duration,
    now: DateTime<Utc>,
) -> Result<i32, DbErr> {
    let start = now.duration_trunc(window).unwrap_or(now);
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO ip_rate_limits (bucket, ip, window_started_at, requests)
             VALUES ($1, $2::inet, $3, 1)
             ON CONFLICT (bucket, ip) DO UPDATE SET
               requests = CASE WHEN ip_rate_limits.window_started_at = EXCLUDED.window_started_at
                          THEN ip_rate_limits.requests + 1 ELSE 1 END,
               window_started_at = EXCLUDED.window_started_at,
               updated_at = now()
             RETURNING requests",
            [
                bucket.into(),
                ip.to_string().into(),
                DateTimeWithTimeZone::from(start).into(),
            ],
        ))
        .await?;
    Ok(row
        .and_then(|r| r.try_get::<i32>("", "requests").ok())
        .unwrap_or(1))
}

/// Hapus baris yang jendelanya mulai lebih dari sehari lalu (job pembersihan).
///
/// # Errors
///
/// Error database.
pub async fn delete_stale<C: ConnectionTrait>(
    db: &C,
    now: DateTime<Utc>,
    dry_run: bool,
) -> Result<u64, DbErr> {
    let cutoff = DateTimeWithTimeZone::from(now - Duration::days(1));
    if dry_run {
        let row = db
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT count(*) AS n FROM ip_rate_limits WHERE window_started_at < $1",
                [cutoff.into()],
            ))
            .await?;
        return Ok(row
            .and_then(|r| r.try_get::<i64>("", "n").ok())
            .and_then(|n| u64::try_from(n).ok())
            .unwrap_or(0));
    }
    Ok(db
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "DELETE FROM ip_rate_limits WHERE window_started_at < $1",
            [cutoff.into()],
        ))
        .await?
        .rows_affected())
}
