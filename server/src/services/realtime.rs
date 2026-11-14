//! Realtime lewat `LISTEN/NOTIFY` Postgres (M12; spec "tanpa Redis").
//!
//! Satu [`Hub`] per proses (disimpan di `ctx.shared_store` oleh `App::after_context`) memegang
//! satu koneksi `LISTEN agentgate_jobs` dan meneruskan notifikasi ke kanal `tokio::sync::broadcast`
//! **per workspace**: notifikasi workspace A hanya membangunkan pendengar A. (Satu kanal bersama
//! membuat setiap pendengar menyaring semua notifikasi; uji beban 10.000 pendengar membuat
//! pendengar tertinggal, `docs/benchmarks/2026-11-14-listen-notify.md`.) Pengirim notifikasi adalah trigger database pada `jobs` dan
//! `job_permissions` (migration `m20261125_000001_realtime_notify`), jadi perubahan dari request,
//! worker, task, maupun proses lain ikut membangunkan pendengar.
//!
//! Notifikasi bersifat sementara: saat koneksi pendengar putus, yang terkirim selama putus hilang.
//! Setelah tersambung lagi hub menyiarkan [`Uuid::nil`] ("semua workspace") supaya setiap
//! pendengar membaca ulang keadaannya sekali. Pendengar tetap punya tick cadangan yang jarang
//! (lihat `controllers::runner_api`), sehingga kebenaran tidak bergantung pada notifikasi.
//!
//! Listener baru dijalankan saat [`Hub::subscribe`] pertama dipanggil: task, migrasi, dan tes
//! yang tidak memakai realtime tidak membuka koneksi tambahan.

use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, Once,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

use loco_rs::app::AppContext;
use sea_orm::sqlx::{PgPool, postgres::PgListener};
use tokio::sync::broadcast;
use uuid::Uuid;

/// Kanal `pg_notify` yang dikirim trigger job.
pub const CHANNEL: &str = "agentgate_jobs";
/// Kapasitas buffer siaran per pendengar. Pendengar yang tertinggal lebih dari ini menerima
/// `Lagged` dan membaca ulang semuanya, jadi angka kecil cukup.
const CAPACITY: usize = 256;
/// Jeda sebelum menyambung ulang setelah listener gagal (mis. database restart).
const RETRY: Duration = Duration::from_secs(1);

type Channels = Arc<Mutex<HashMap<Uuid, broadcast::Sender<Uuid>>>>;

/// Siaran id workspace yang job-nya berubah.
pub struct Hub {
    channels: Channels,
    pool: PgPool,
    started: Once,
    connected: Arc<AtomicBool>,
    received: Arc<AtomicU64>,
}

/// Ringkasan hub untuk metrik dan halaman Status sistem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HubStats {
    /// Listener sedang tersambung ke Postgres.
    pub connected: bool,
    /// Jumlah pendengar (koneksi runner) di proses ini.
    pub listeners: usize,
    /// Notifikasi yang diterima sejak proses jalan.
    pub received: u64,
}

impl Hub {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self {
            channels: Channels::default(),
            pool,
            started: Once::new(),
            connected: Arc::new(AtomicBool::new(false)),
            received: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Hub proses ini. `None` hanya bila `after_context` belum memasangnya (mis. context buatan
    /// tangan); pemanggil jatuh ke tick cadangan.
    #[must_use]
    pub fn of(ctx: &AppContext) -> Option<Arc<Self>> {
        ctx.shared_store.get::<Arc<Self>>()
    }

    /// Daftar sebagai pendengar perubahan job `workspace`; menerima id workspace itu, atau
    /// [`Uuid::nil`] setelah listener tersambung ulang (baca ulang semuanya). Menjalankan listener
    /// Postgres pada pemanggilan pertama.
    pub fn subscribe(&self, workspace: Uuid) -> broadcast::Receiver<Uuid> {
        let rx = {
            let mut map = lock(&self.channels);
            map.retain(|_, tx| tx.receiver_count() > 0);
            map.entry(workspace)
                .or_insert_with(|| broadcast::channel(CAPACITY).0)
                .subscribe()
        };
        self.started.call_once(|| {
            tokio::spawn(listen(
                self.pool.clone(),
                self.channels.clone(),
                self.connected.clone(),
                self.received.clone(),
            ));
        });
        rx
    }

    #[must_use]
    pub fn stats(&self) -> HubStats {
        HubStats {
            connected: self.connected.load(Ordering::Relaxed),
            listeners: lock(&self.channels)
                .values()
                .map(broadcast::Sender::receiver_count)
                .sum(),
            received: self.received.load(Ordering::Relaxed),
        }
    }
}

fn lock(channels: &Channels) -> std::sync::MutexGuard<'_, HashMap<Uuid, broadcast::Sender<Uuid>>> {
    // Mutex hanya dipegang untuk operasi map singkat tanpa await; poison berarti panic di
    // tengah operasi itu, isinya tetap konsisten.
    channels
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Teruskan notifikasi ke pendengar workspace-nya; nil ke semua.
fn dispatch(channels: &Channels, workspace: Uuid) {
    let map = lock(channels);
    if workspace.is_nil() {
        for tx in map.values() {
            let _ = tx.send(workspace);
        }
    } else if let Some(tx) = map.get(&workspace) {
        // Tanpa pendengar `send` gagal; itu bukan error.
        let _ = tx.send(workspace);
    }
}

async fn listen(
    pool: PgPool,
    channels: Channels,
    connected: Arc<AtomicBool>,
    received: Arc<AtomicU64>,
) {
    let mut first = true;
    loop {
        if pool.is_closed() {
            connected.store(false, Ordering::Relaxed);
            return;
        }
        let mut listener = match PgListener::connect_with(&pool).await {
            Ok(l) => l,
            Err(err) => {
                tracing::warn!(error = %err, "realtime listener connect failed");
                tokio::time::sleep(RETRY).await;
                continue;
            }
        };
        if let Err(err) = listener.listen(CHANNEL).await {
            tracing::warn!(error = %err, "realtime LISTEN failed");
            tokio::time::sleep(RETRY).await;
            continue;
        }
        connected.store(true, Ordering::Relaxed);
        if !first {
            // Notifikasi selama putus hilang: semua pendengar membaca ulang sekali.
            dispatch(&channels, Uuid::nil());
        }
        first = false;
        loop {
            match listener.try_recv().await {
                Ok(Some(n)) => {
                    received.fetch_add(1, Ordering::Relaxed);
                    if let Ok(id) = Uuid::parse_str(n.payload())
                        && !id.is_nil()
                    {
                        dispatch(&channels, id);
                    }
                }
                Ok(None) => {
                    // Koneksi putus: sambung ulang eksplisit di loop luar, yang menyiarkan nil
                    // setelah LISTEN aktif lagi (bukan sebelumnya, supaya celahnya tertutup).
                    connected.store(false, Ordering::Relaxed);
                    tracing::warn!("realtime listener connection lost, reconnecting");
                    break;
                }
                Err(err) => {
                    connected.store(false, Ordering::Relaxed);
                    if pool.is_closed() {
                        return;
                    }
                    tracing::warn!(error = %err, "realtime listener failed");
                    tokio::time::sleep(RETRY).await;
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_reaches_only_the_workspace_and_nil_reaches_all() {
        let channels = Channels::default();
        let (a, b) = (Uuid::now_v7(), Uuid::now_v7());
        let mut ra = lock(&channels)
            .entry(a)
            .or_insert_with(|| broadcast::channel(4).0)
            .subscribe();
        let mut rb = lock(&channels)
            .entry(b)
            .or_insert_with(|| broadcast::channel(4).0)
            .subscribe();
        dispatch(&channels, a);
        assert_eq!(ra.try_recv().ok(), Some(a));
        assert!(rb.try_recv().is_err());
        dispatch(&channels, Uuid::nil());
        assert_eq!(ra.try_recv().ok(), Some(Uuid::nil()));
        assert_eq!(rb.try_recv().ok(), Some(Uuid::nil()));
    }
}
