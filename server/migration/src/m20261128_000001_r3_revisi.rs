//! R3 revisi (keputusan pemilik produk 2026-11-14, `docs/REVIEW-AKHIR.md` §3.0).
//!
//! - aksi audit `ticket.wip_overridden`: Admin project memindah tiket ke kolom yang batas WIP-nya
//!   sudah penuh (alasan wajib di `changes`).
//!
//! Izin aksi berbahaya per stage disimpan di definisi workflow (`jsonb`), tanpa kolom baru.
//! Kompatibel dengan kode rilis sebelumnya: hanya CHECK yang diperluas.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const AUDIT_ACTIONS: &[&str] = &["ticket.wip_overridden"];

/// `, 'a'::text` (bentuk keluaran `pg_get_constraintdef` untuk `ARRAY[...]`).
fn suffix(values: &[&str]) -> String {
    values
        .iter()
        .map(|v| format!(", ''{v}''::text"))
        .collect::<String>()
}

/// Tambah `values` ke CHECK `<@ ARRAY[...]` constraint `name` di tabel `table`.
fn extend_check(table: &str, name: &str, values: &[&str]) -> String {
    let add = suffix(values);
    format!(
        "DO $$
        DECLARE def text;
        BEGIN
          SELECT pg_get_constraintdef(c.oid) INTO def FROM pg_constraint c
            WHERE c.conname = '{name}' AND c.conrelid = '{table}'::regclass;
          EXECUTE 'ALTER TABLE {table} DROP CONSTRAINT {name}';
          EXECUTE 'ALTER TABLE {table} ADD CONSTRAINT {name} '
            || regexp_replace(def, '\\]', '{add}]');
        END $$"
    )
}

/// Kebalikan [`extend_check`]: buang `values` dari constraint.
fn shrink_check(table: &str, name: &str, values: &[&str]) -> String {
    let add = suffix(values);
    format!(
        "DO $$
        DECLARE def text;
        BEGIN
          SELECT pg_get_constraintdef(c.oid) INTO def FROM pg_constraint c
            WHERE c.conname = '{name}' AND c.conrelid = '{table}'::regclass;
          EXECUTE 'ALTER TABLE {table} DROP CONSTRAINT {name}';
          EXECUTE 'ALTER TABLE {table} ADD CONSTRAINT {name} ' || replace(def, '{add}', '');
        END $$"
    )
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let db = m.get_connection();
        db.execute_unprepared("SET lock_timeout = '5s'").await?;
        db.execute_unprepared(&extend_check(
            "audit_events",
            "audit_events_action_check",
            AUDIT_ACTIONS,
        ))
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let db = m.get_connection();
        db.execute_unprepared("DELETE FROM audit_events WHERE action = 'ticket.wip_overridden'")
            .await?;
        db.execute_unprepared(&shrink_check(
            "audit_events",
            "audit_events_action_check",
            AUDIT_ACTIONS,
        ))
        .await?;
        Ok(())
    }
}
