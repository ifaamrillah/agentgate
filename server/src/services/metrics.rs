//! Metrik minimal (rules §10 "Logging dan observability", M12) dalam format teks Prometheus,
//! tanpa dependensi baru:
//!
//! - `agentgate_http_requests_total{method,route,status}` dan histogram
//!   `agentgate_http_request_duration_seconds` per method + route (pola route Axum, bukan URL
//!   asli: tanpa id dan token, kardinalitas terbatas);
//! - `agentgate_queue_jobs{name,status}` dan `agentgate_queue_oldest_queued_seconds` dari
//!   tabel queue Loco (`pg_loco_queue`);
//! - `agentgate_db_pool_connections{state}` dan `agentgate_db_pool_max`;
//! - `agentgate_login_failures_total` (pakai `rate(...[1m])` untuk login gagal per menit);
//! - `agentgate_realtime_listeners`, `agentgate_realtime_connected`,
//!   `agentgate_realtime_notifications_total`;
//! - `agentgate_build_info{version}` dan `agentgate_process_start_time_seconds`.
//!
//! Alert pertama yang disarankan (rules §10): rasio 5xx, umur job antre tertua, pool habis.
//! Contoh aturan di `docs/admin/operasi.md`.

use std::{
    collections::BTreeMap,
    fmt::Write as _,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use loco_rs::app::AppContext;
use sea_orm::{ConnectionTrait, DbBackend, DbErr, Statement};

/// Batas atas bucket histogram durasi (detik).
const BUCKETS: [f64; 11] = [
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

static LOGIN_FAILURES: AtomicU64 = AtomicU64::new(0);
static STARTED: OnceLock<SystemTime> = OnceLock::new();

/// Catat satu login password yang ditolak atau terkunci.
pub fn login_failed() {
    LOGIN_FAILURES.fetch_add(1, Ordering::Relaxed);
}

/// Detik sejak epoch saat proses mulai (diisi saat registry pertama dibuat).
#[must_use]
pub fn started_at() -> SystemTime {
    *STARTED.get_or_init(SystemTime::now)
}

#[derive(Debug, Default, Clone)]
struct Series {
    count: u64,
    sum: f64,
    buckets: [u64; BUCKETS.len()],
}

/// Penghitung request HTTP per proses. Disimpan di `ctx.shared_store` (`App::after_context`).
#[derive(Debug, Default)]
pub struct Registry {
    /// (method, route) → status → jumlah.
    statuses: Mutex<BTreeMap<(String, String), BTreeMap<u16, u64>>>,
    /// (method, route) → durasi.
    durations: Mutex<BTreeMap<(String, String), Series>>,
}

impl Registry {
    #[must_use]
    pub fn new() -> Self {
        started_at();
        Self::default()
    }

    /// Registry proses ini, bila terpasang.
    #[must_use]
    pub fn of(ctx: &AppContext) -> Option<std::sync::Arc<Self>> {
        ctx.shared_store.get::<std::sync::Arc<Self>>()
    }

    /// Catat satu request selesai.
    pub fn observe(&self, method: &str, route: &str, status: u16, elapsed: Duration) {
        let key = (method.to_string(), route.to_string());
        {
            let mut s = lock(&self.statuses);
            *s.entry(key.clone()).or_default().entry(status).or_default() += 1;
        }
        let secs = elapsed.as_secs_f64();
        let mut d = lock(&self.durations);
        let series = d.entry(key).or_default();
        series.count += 1;
        series.sum += secs;
        for (i, upper) in BUCKETS.iter().enumerate() {
            if secs <= *upper {
                series.buckets[i] += 1;
            }
        }
    }

    /// Jumlah request (semua route) dengan status `status`; untuk tes dan Status sistem.
    #[must_use]
    pub fn count_status(&self, predicate: impl Fn(u16) -> bool) -> u64 {
        lock(&self.statuses)
            .values()
            .flat_map(|m| m.iter())
            .filter(|(s, _)| predicate(**s))
            .map(|(_, n)| *n)
            .sum()
    }

    fn render_http(&self, out: &mut String) {
        out.push_str("# HELP agentgate_http_requests_total Request HTTP per route dan status.\n");
        out.push_str("# TYPE agentgate_http_requests_total counter\n");
        for ((method, route), statuses) in lock(&self.statuses).iter() {
            for (status, n) in statuses {
                let _ = writeln!(
                    out,
                    "agentgate_http_requests_total{{method=\"{}\",route=\"{}\",status=\"{status}\"}} {n}",
                    esc(method),
                    esc(route)
                );
            }
        }
        out.push_str(
            "# HELP agentgate_http_request_duration_seconds Durasi request HTTP per route.\n",
        );
        out.push_str("# TYPE agentgate_http_request_duration_seconds histogram\n");
        for ((method, route), s) in lock(&self.durations).iter() {
            let labels = format!("method=\"{}\",route=\"{}\"", esc(method), esc(route));
            for (i, upper) in BUCKETS.iter().enumerate() {
                let _ = writeln!(
                    out,
                    "agentgate_http_request_duration_seconds_bucket{{{labels},le=\"{upper}\"}} {}",
                    s.buckets[i]
                );
            }
            let _ = writeln!(
                out,
                "agentgate_http_request_duration_seconds_bucket{{{labels},le=\"+Inf\"}} {}",
                s.count
            );
            let _ = writeln!(
                out,
                "agentgate_http_request_duration_seconds_sum{{{labels}}} {}",
                s.sum
            );
            let _ = writeln!(
                out,
                "agentgate_http_request_duration_seconds_count{{{labels}}} {}",
                s.count
            );
        }
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Escape nilai label Prometheus.
fn esc(v: &str) -> String {
    v.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

/// Keadaan antrean job Loco: jumlah per (nama, status) dan umur job `queued` tertua yang sudah
/// jatuh tempo (detik; 0 bila kosong).
///
/// # Errors
///
/// Error database.
pub async fn queue_stats<C: ConnectionTrait>(
    db: &C,
) -> Result<(Vec<(String, String, i64)>, f64), DbErr> {
    // Tabel queue dibuat Loco saat worker queue Postgres pertama jalan; tes (job inline) tidak
    // punya tabelnya.
    let present = db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT to_regclass('pg_loco_queue') IS NOT NULL AS present",
        ))
        .await?
        .and_then(|r| r.try_get::<bool>("", "present").ok())
        .unwrap_or(false);
    if !present {
        return Ok((Vec::new(), 0.0));
    }
    let rows = db
        .query_all_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT name, status, count(*) AS n FROM pg_loco_queue GROUP BY 1, 2 ORDER BY 1, 2",
        ))
        .await?;
    let counts = rows
        .iter()
        .filter_map(|r| {
            Some((
                r.try_get::<String>("", "name").ok()?,
                r.try_get::<String>("", "status").ok()?,
                r.try_get::<i64>("", "n").ok()?,
            ))
        })
        .collect();
    let oldest = db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT COALESCE(EXTRACT(EPOCH FROM now() - min(run_at)), 0)::float8 AS age
             FROM pg_loco_queue WHERE status = 'queued' AND run_at <= now()",
        ))
        .await?
        .and_then(|r| r.try_get::<f64>("", "age").ok())
        .unwrap_or(0.0);
    Ok((counts, oldest.max(0.0)))
}

/// Seluruh metrik dalam format teks Prometheus 0.0.4.
///
/// # Errors
///
/// Error database (antrean).
pub async fn render(ctx: &AppContext) -> Result<String, DbErr> {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# TYPE agentgate_build_info gauge\nagentgate_build_info{{version=\"{}\"}} 1",
        esc(agentgate_core::VERSION)
    );
    let start = started_at()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64());
    let _ = writeln!(
        out,
        "# TYPE agentgate_process_start_time_seconds gauge\nagentgate_process_start_time_seconds {start}"
    );
    if let Some(reg) = Registry::of(ctx) {
        reg.render_http(&mut out);
    }

    let (counts, oldest) = queue_stats(&ctx.db).await?;
    out.push_str("# HELP agentgate_queue_jobs Job antrean Loco per nama dan status.\n");
    out.push_str("# TYPE agentgate_queue_jobs gauge\n");
    for (name, status, n) in counts {
        let _ = writeln!(
            out,
            "agentgate_queue_jobs{{name=\"{}\",status=\"{}\"}} {n}",
            esc(&name),
            esc(&status)
        );
    }
    let _ = writeln!(
        out,
        "# HELP agentgate_queue_oldest_queued_seconds Umur job antre tertua yang sudah jatuh tempo.\n# TYPE agentgate_queue_oldest_queued_seconds gauge\nagentgate_queue_oldest_queued_seconds {oldest}"
    );

    let pool = ctx.db.get_postgres_connection_pool();
    let size = pool.size();
    let idle = u32::try_from(pool.num_idle()).unwrap_or(u32::MAX);
    out.push_str("# HELP agentgate_db_pool_connections Koneksi pool database proses ini.\n");
    out.push_str("# TYPE agentgate_db_pool_connections gauge\n");
    let _ = writeln!(
        out,
        "agentgate_db_pool_connections{{state=\"idle\"}} {idle}\nagentgate_db_pool_connections{{state=\"in_use\"}} {}",
        size.saturating_sub(idle)
    );
    let _ = writeln!(
        out,
        "# TYPE agentgate_db_pool_max gauge\nagentgate_db_pool_max {}",
        pool.options().get_max_connections()
    );

    let _ = writeln!(
        out,
        "# HELP agentgate_login_failures_total Login password ditolak atau terkunci.\n# TYPE agentgate_login_failures_total counter\nagentgate_login_failures_total {}",
        LOGIN_FAILURES.load(Ordering::Relaxed)
    );

    if let Some(hub) = crate::services::realtime::Hub::of(ctx) {
        let st = hub.stats();
        let _ = writeln!(
            out,
            "# TYPE agentgate_realtime_listeners gauge\nagentgate_realtime_listeners {}\n# TYPE agentgate_realtime_connected gauge\nagentgate_realtime_connected {}\n# TYPE agentgate_realtime_notifications_total counter\nagentgate_realtime_notifications_total {}",
            st.listeners,
            u8::from(st.connected),
            st.received
        );
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn histogram_buckets_are_cumulative() {
        let reg = Registry::new();
        reg.observe("GET", "/w/{slug}", 200, Duration::from_millis(30));
        reg.observe("GET", "/w/{slug}", 500, Duration::from_millis(3));
        let mut out = String::new();
        reg.render_http(&mut out);
        assert!(out.contains(
            "agentgate_http_requests_total{method=\"GET\",route=\"/w/{slug}\",status=\"500\"} 1"
        ));
        assert!(out.contains("le=\"0.005\"} 1"));
        assert!(out.contains("le=\"0.05\"} 2"));
        assert!(out.contains("le=\"+Inf\"} 2"));
        assert_eq!(reg.count_status(|s| s >= 500), 1);
    }

    #[test]
    fn label_values_are_escaped() {
        assert_eq!(esc("a\"b\\c"), "a\\\"b\\\\c");
    }
}
