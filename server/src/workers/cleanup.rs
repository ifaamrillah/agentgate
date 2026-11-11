//! Job pembersihan harian (logika di `services::cleanup`). Scheduler Loco hanya menjalankan task
//! atau perintah shell, jadi jadwal harian memanggil task `cleanup confirm:yes`
//! (`config/*.yaml`, bagian `scheduler`), yang menjalankan [`run`] yang sama. Worker ini
//! terdaftar di `App::connect_workers` supaya pembersihan juga bisa di-enqueue dari kode
//! (`CleanupWorker::perform_later`) dan diproses di container worker.
//!
//! Idempoten dan tanpa args: dijalankan dua kali (retry, redelivery) hanya menemukan yang
//! belum dibersihkan. Langkah atau item yang gagal dicatat di log; job tetap selesai supaya
//! satu workspace bermasalah tidak membuat seluruh pembersihan diulang terus-menerus.
use loco_rs::{app::AppContext, bgworker::BackgroundWorker};
use serde::{Deserialize, Serialize};

use crate::services::cleanup::{self, CleanupMode, CleanupReport};

/// Args kosong: yang dibersihkan selalu dibaca ulang dari database saat job jalan.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CleanupArgs {}

pub struct CleanupWorker {
    ctx: AppContext,
}

#[async_trait::async_trait]
impl BackgroundWorker<CleanupArgs> for CleanupWorker {
    fn build(ctx: &AppContext) -> Self {
        Self { ctx: ctx.clone() }
    }

    async fn perform(&self, _args: CleanupArgs) -> loco_rs::Result<()> {
        run(&self.ctx).await.map(|_| ())
    }
}

/// Jalankan pembersihan sekarang (bukan dry run).
///
/// # Errors
///
/// Tidak pernah gagal karena satu langkah; `Result` untuk keseragaman dengan job lain.
pub async fn run(ctx: &AppContext) -> loco_rs::Result<CleanupReport> {
    let report = cleanup::run(ctx, CleanupMode::Apply).await;
    tracing::info!(
        failed = report.failed(),
        steps = ?report.steps,
        "cleanup finished"
    );
    Ok(report)
}
