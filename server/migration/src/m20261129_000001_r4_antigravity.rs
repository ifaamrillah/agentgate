//! R4 revisi (keputusan pemilik produk 2026-11-14, `docs/REVIEW-AKHIR.md` §3.0): Gemini CLI
//! diganti Antigravity CLI. Kunci client `gemini` dipetakan ke `antigravity`:
//!
//! - `runners.client` dan `runner_prefs.default_client`;
//! - kunci `gemini` di `personas.models`. Model katalog Gemini CLI lama (`gemini-2.5-pro`,
//!   `gemini-2.5-flash`) bukan slug Antigravity (headless menolak model tak dikenal), jadi
//!   dikosongkan; model lain yang diketik Admin dipertahankan. Entri yang menjadi kosong dibuang.
//!   Bila persona sudah punya kunci `antigravity`, kunci `gemini` dibuang saja.
//!
//! `jobs.client` tidak diubah: riwayat job mencatat client yang benar-benar menjalankannya.
//! Pernyataan SQL idempoten (aman dijalankan ulang) dan dipakai tes.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Model katalog Gemini CLI lama.
const OLD_MODELS: &str = "('gemini-2.5-pro', 'gemini-2.5-flash')";

/// Pernyataan `up`.
#[must_use]
pub fn up_sql() -> Vec<String> {
    vec![
        "UPDATE runners SET client = 'antigravity' WHERE client = 'gemini'".into(),
        "UPDATE runner_prefs SET default_client = 'antigravity' WHERE default_client = 'gemini'"
            .into(),
        format!(
            "UPDATE personas SET models = CASE
               WHEN models ? 'antigravity' THEN models - 'gemini'
               ELSE (models - 'gemini') || jsonb_build_object('antigravity',
                 CASE WHEN models -> 'gemini' ->> 'model' IN {OLD_MODELS}
                   THEN (models -> 'gemini') - 'model'
                   ELSE models -> 'gemini' END)
             END
             WHERE models ? 'gemini'"
        ),
        "UPDATE personas SET models = models - 'antigravity' \
         WHERE models -> 'antigravity' = '{}'::jsonb"
            .into(),
    ]
}

/// Pernyataan `down` (effort dibuang: Gemini CLI tidak punya pilihan effort).
#[must_use]
pub fn down_sql() -> Vec<String> {
    vec![
        "UPDATE runners SET client = 'gemini' WHERE client = 'antigravity'".into(),
        "UPDATE runner_prefs SET default_client = 'gemini' WHERE default_client = 'antigravity'"
            .into(),
        "UPDATE personas SET models = CASE
           WHEN models ? 'gemini' THEN models - 'antigravity'
           ELSE (models - 'antigravity')
             || jsonb_build_object('gemini', (models -> 'antigravity') - 'effort')
         END
         WHERE models ? 'antigravity'"
            .into(),
        "UPDATE personas SET models = models - 'gemini' WHERE models -> 'gemini' = '{}'::jsonb"
            .into(),
    ]
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let db = m.get_connection();
        for sql in up_sql() {
            db.execute_unprepared(&sql).await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let db = m.get_connection();
        for sql in down_sql() {
            db.execute_unprepared(&sql).await?;
        }
        Ok(())
    }
}
