//! Uji beban `LISTEN/NOTIFY` (M12, spec "uji beban LISTEN/NOTIFY").
//!
//! Mensimulasikan `PROCS` proses web (masing-masing satu [`Hub`] = satu koneksi LISTEN) dengan
//! total `LISTENERS` koneksi runner, lalu `WRITERS` penulis paralel mengirim `NOTIFIES` notifikasi
//! (masing-masing dalam transaksi sendiri, seperti trigger job) ke `WORKSPACES` workspace acak.
//! Mengukur latensi commit → diterima pendengar workspace itu (p50/p95/p99/max) dan pesan yang
//! tertinggal (`Lagged`).
//!
//! ```sh
//! docker exec -e DATABASE_URL=postgres://agentgate:agentgate@db:5432/agentgate_dev \
//!   agentgate-rs-main-app-1 cargo run --release -p agentgate-server --example listen_notify_load -- \
//!   4 2000 500 5000 16
//! ```
//! Argumen: PROCS LISTENERS WORKSPACES NOTIFIES WRITERS (default 4 1000 200 2000 8).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use agentgate_server::services::realtime::Hub;
use sea_orm::sqlx::{Executor, postgres::PgPoolOptions};
use tokio::sync::broadcast::error::RecvError;
use uuid::Uuid;

fn arg(i: usize, default: usize) -> usize {
    std::env::args()
        .nth(i)
        .and_then(|a| a.parse().ok())
        .unwrap_or(default)
}

fn pct(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let i = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[i]
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let procs = arg(1, 4);
    let listeners = arg(2, 1000);
    let workspaces = arg(3, 200);
    let notifies = arg(4, 2000);
    let writers = arg(5, 8);
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");

    let ws: Vec<Uuid> = (0..workspaces).map(|_| Uuid::now_v7()).collect();
    // Waktu kirim per (workspace, urutan) supaya latensi diukur dari sebelum commit.
    let sent: Arc<Mutex<HashMap<Uuid, Vec<Instant>>>> = Arc::default();
    let lat: Arc<Mutex<Vec<f64>>> = Arc::default();
    let lagged: Arc<Mutex<u64>> = Arc::default();

    let mut hubs = Vec::new();
    for _ in 0..procs {
        let pool = PgPoolOptions::new()
            .max_connections(2)
            .connect(&url)
            .await
            .unwrap();
        hubs.push(Arc::new(Hub::new(pool)));
    }
    let mut tasks = Vec::new();
    for i in 0..listeners {
        let hub = hubs[i % procs].clone();
        let mine = ws[i % workspaces];
        let mut rx = hub.subscribe(mine);
        let sent = sent.clone();
        let lat = lat.clone();
        let lagged = lagged.clone();
        tasks.push(tokio::spawn(async move {
            loop {
                match tokio::time::timeout(Duration::from_secs(5), rx.recv()).await {
                    Ok(Ok(w)) => {
                        let now = Instant::now();
                        let at = sent.lock().unwrap().get(&w).and_then(|v| v.last().copied());
                        if let Some(at) = at {
                            lat.lock()
                                .unwrap()
                                .push(now.duration_since(at).as_secs_f64() * 1000.0);
                        }
                    }
                    Ok(Err(RecvError::Lagged(n))) => *lagged.lock().unwrap() += n,
                    Ok(Err(RecvError::Closed)) | Err(_) => return,
                }
            }
        }));
    }
    // Tunggu semua hub tersambung.
    for _ in 0..100 {
        if hubs.iter().all(|h| h.stats().connected) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let writer_pool = PgPoolOptions::new()
        .max_connections(u32::try_from(writers).unwrap())
        .connect(&url)
        .await
        .unwrap();
    let started = Instant::now();
    let per = notifies / writers;
    let mut ws_tasks = Vec::new();
    for w in 0..writers {
        let pool = writer_pool.clone();
        let ws = ws.clone();
        let sent = sent.clone();
        ws_tasks.push(tokio::spawn(async move {
            for n in 0..per {
                let target = ws[(w * per + n) % ws.len()];
                sent.lock()
                    .unwrap()
                    .entry(target)
                    .or_default()
                    .push(Instant::now());
                let mut tx = pool.begin().await.unwrap();
                tx.execute(
                    sea_orm::sqlx::query("SELECT pg_notify('agentgate_jobs', $1)")
                        .bind(target.to_string()),
                )
                .await
                .unwrap();
                tx.commit().await.unwrap();
            }
        }));
    }
    for t in ws_tasks {
        t.await.unwrap();
    }
    let send_secs = started.elapsed().as_secs_f64();
    // Biarkan pesan terakhir tiba.
    tokio::time::sleep(Duration::from_secs(2)).await;
    let received: u64 = hubs.iter().map(|h| h.stats().received).sum();
    drop(hubs);
    for t in &tasks {
        t.abort();
    }

    let mut l = lat.lock().unwrap().clone();
    l.sort_by(f64::total_cmp);
    let expected = (per * writers) as f64 * listeners as f64 / workspaces as f64;
    println!(
        "procs={procs} listeners={listeners} workspaces={workspaces} notifies={} writers={writers}",
        per * writers
    );
    println!(
        "kirim: {:.2} dtk ({:.0} notif/dtk); diterima hub: {received} (harapan {})",
        send_secs,
        (per * writers) as f64 / send_secs,
        per * writers * procs
    );
    println!(
        "pengiriman ke pendengar: {} (≈ harapan {:.0}); lagged: {}",
        l.len(),
        expected,
        lagged.lock().unwrap()
    );
    println!(
        "latensi ms: p50 {:.1} · p95 {:.1} · p99 {:.1} · max {:.1}",
        pct(&l, 0.50),
        pct(&l, 0.95),
        pct(&l, 0.99),
        l.last().copied().unwrap_or(0.0)
    );
}
