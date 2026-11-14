//! M12 Realtime lewat `LISTEN/NOTIFY` (spec: "realtime lewat LISTEN/NOTIFY Postgres, tanpa Redis").
//!
//! Trigger `AFTER INSERT OR UPDATE` pada `jobs` dan `job_permissions` mengirim
//! `pg_notify('agentgate_jobs', workspace_id)`. Notifikasi Postgres baru terkirim saat transaksi
//! commit dan notifikasi identik dalam satu transaksi digabung, jadi satu transaksi yang mengubah
//! banyak job di satu workspace hanya membangunkan pendengar sekali. Trigger di database (bukan di
//! kode) supaya perubahan dari worker, task, dan scheduler ikut terkirim tanpa jalur khusus.
//!
//! Payload hanya id workspace (36 byte; batas payload Postgres 8000 byte): pendengar membaca ulang
//! keadaan dari tabel, sehingga notifikasi yang hilang saat koneksi pendengar putus cukup ditutup
//! dengan satu pembacaan ulang.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &[&str] = &[
    "CREATE OR REPLACE FUNCTION agentgate_notify_jobs() RETURNS trigger
       LANGUAGE plpgsql AS $$
     BEGIN
       PERFORM pg_notify('agentgate_jobs', NEW.workspace_id::text);
       RETURN NULL;
     END $$",
    "CREATE TRIGGER jobs_notify AFTER INSERT OR UPDATE ON jobs
       FOR EACH ROW EXECUTE FUNCTION agentgate_notify_jobs()",
    "CREATE TRIGGER job_permissions_notify AFTER INSERT OR UPDATE ON job_permissions
       FOR EACH ROW EXECUTE FUNCTION agentgate_notify_jobs()",
];

const DOWN: &[&str] = &[
    "DROP TRIGGER IF EXISTS job_permissions_notify ON job_permissions",
    "DROP TRIGGER IF EXISTS jobs_notify ON jobs",
    "DROP FUNCTION IF EXISTS agentgate_notify_jobs()",
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
