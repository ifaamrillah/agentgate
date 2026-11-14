//! M12 Rilis: tabel tingkat instance (bukan per workspace).
//!
//! - `ip_rate_limits`: penghitung jendela tetap per (bucket, IP) untuk endpoint publik tanpa
//!   login (mis. pendaftaran client OAuth dinamis `/oauth/register`). Di Postgres seperti
//!   `login_throttles`, tanpa Redis.
//! - `system_backups`: riwayat backup Postgres. Ditulis skrip backup (`deploy/backup/backup.sh`,
//!   lewat `psql`) di container terpisah, dibaca halaman Status sistem. Id default `uuidv7()`
//!   (bawaan Postgres 18) karena penulisnya bukan aplikasi.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &[&str] = &[
    "CREATE TABLE ip_rate_limits (
        bucket text NOT NULL,
        ip inet NOT NULL,
        window_started_at timestamptz NOT NULL,
        requests integer NOT NULL DEFAULT 0,
        updated_at timestamptz NOT NULL DEFAULT now(),
        PRIMARY KEY (bucket, ip)
    )",
    "CREATE INDEX ip_rate_limits_window_idx ON ip_rate_limits (window_started_at)",
    "CREATE TABLE system_backups (
        id uuid PRIMARY KEY DEFAULT uuidv7(),
        started_at timestamptz NOT NULL DEFAULT now(),
        finished_at timestamptz,
        status text NOT NULL DEFAULT 'running'
            CONSTRAINT system_backups_status_check CHECK (status IN ('running', 'ok', 'failed')),
        size_bytes bigint,
        location text NOT NULL DEFAULT '',
        message text NOT NULL DEFAULT ''
    )",
    "CREATE INDEX system_backups_started_idx ON system_backups (started_at DESC)",
];

const DOWN: &[&str] = &[
    "DROP TABLE IF EXISTS system_backups",
    "DROP TABLE IF EXISTS ip_rate_limits",
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let db = m.get_connection();
        for sql in UP {
            db.execute_unprepared(sql).await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let db = m.get_connection();
        for sql in DOWN {
            db.execute_unprepared(sql).await?;
        }
        Ok(())
    }
}
