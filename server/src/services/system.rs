//! Status sistem instance (M12, halaman `/system` untuk admin instance). Semua angka dibaca
//! langsung dari Postgres (katalog + tabel aplikasi) dan dari proses ini (hub realtime,
//! registry metrik); tidak ada data lintas tenant yang ditampilkan selain angka agregat.

use chrono::{DateTime, Duration, Utc};
use loco_rs::app::AppContext;
use sea_orm::{ConnectionTrait, DbBackend, DbErr, QueryResult, Statement};

use crate::services::{metrics, realtime::Hub};

/// Backup dianggap basi bila backup sukses terakhir lebih tua dari ini (jadwal harian + 2 jam).
pub const BACKUP_STALE_HOURS: i64 = 26;

#[derive(Debug, Clone)]
pub struct QueueRow {
    pub name: String,
    pub queued: i64,
    pub running: i64,
    pub failed: i64,
}

#[derive(Debug, Clone)]
pub struct FailedJob {
    pub name: String,
    pub at: DateTime<Utc>,
    pub attempts: i32,
    pub error: String,
}

#[derive(Debug, Clone)]
pub struct Backup {
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub status: String,
    pub size_bytes: Option<i64>,
    pub location: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct TableSize {
    pub name: String,
    pub bytes: i64,
}

#[derive(Debug, Clone, Default)]
pub struct StorageUse {
    pub attachments: (i64, i64),
    pub evidence: (i64, i64),
    pub exports: (i64, i64),
    pub imports: (i64, i64),
    pub rule_backups: (i64, i64),
    pub logos: (i64, i64),
}

impl StorageUse {
    /// (jumlah berkas, byte) total.
    #[must_use]
    pub fn total(&self) -> (i64, i64) {
        [
            self.attachments,
            self.evidence,
            self.exports,
            self.imports,
            self.rule_backups,
            self.logos,
        ]
        .iter()
        .fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1))
    }
}

/// Seluruh status untuk halaman. Field `warnings` = kunci Fluent hal yang perlu perhatian.
#[derive(Debug, Clone)]
pub struct Status {
    pub uptime: Duration,
    pub runners_online: i64,
    pub runners_total: i64,
    pub workspaces: i64,
    pub users: i64,
    pub queue: Vec<QueueRow>,
    pub oldest_queued_seconds: f64,
    pub failed_jobs: Vec<FailedJob>,
    pub http_5xx: u64,
    pub http_total: u64,
    pub realtime_connected: bool,
    pub realtime_listeners: usize,
    pub realtime_received: u64,
    pub pg_version: String,
    pub db_bytes: i64,
    pub db_connections: i64,
    pub db_max_connections: i64,
    pub pool_in_use: u32,
    pub pool_max: u32,
    pub last_migration: String,
    pub extensions: Vec<(String, String)>,
    pub largest_tables: Vec<TableSize>,
    pub storage: StorageUse,
    pub backups: Vec<Backup>,
    pub last_backup_ok: Option<DateTime<Utc>>,
    pub warnings: Vec<&'static str>,
}

fn stmt(sql: &str) -> Statement {
    Statement::from_string(DbBackend::Postgres, sql)
}

async fn one<C: ConnectionTrait>(db: &C, sql: &str) -> Result<Option<QueryResult>, DbErr> {
    db.query_one_raw(stmt(sql)).await
}

async fn scalar_i64<C: ConnectionTrait>(db: &C, sql: &str) -> Result<i64, DbErr> {
    Ok(one(db, sql)
        .await?
        .and_then(|r| r.try_get::<i64>("", "n").ok())
        .unwrap_or(0))
}

async fn table_exists<C: ConnectionTrait>(db: &C, table: &str) -> Result<bool, DbErr> {
    Ok(db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT to_regclass($1) IS NOT NULL AS present",
            [table.into()],
        ))
        .await?
        .and_then(|r| r.try_get::<bool>("", "present").ok())
        .unwrap_or(false))
}

async fn pair<C: ConnectionTrait>(db: &C, sql: &str) -> Result<(i64, i64), DbErr> {
    Ok(one(db, sql)
        .await?
        .map(|r| {
            (
                r.try_get::<i64>("", "n").unwrap_or(0),
                r.try_get::<i64>("", "b").unwrap_or(0),
            )
        })
        .unwrap_or((0, 0)))
}

/// Ringkasan berkas di storage per kategori (dari metadata di database, bukan listing bucket).
///
/// # Errors
///
/// Error database.
pub async fn storage_use<C: ConnectionTrait>(db: &C) -> Result<StorageUse, DbErr> {
    Ok(StorageUse {
        attachments: pair(
            db,
            "SELECT count(*) AS n, COALESCE(sum(size), 0)::int8 AS b FROM ticket_attachments",
        )
        .await?,
        evidence: pair(
            db,
            "SELECT count(*) AS n, COALESCE(sum(size_bytes), 0)::int8 AS b FROM job_evidence",
        )
        .await?,
        exports: pair(
            db,
            "SELECT count(*) AS n, COALESCE(sum(size_bytes), 0)::int8 AS b FROM data_exports WHERE storage_key IS NOT NULL",
        )
        .await?,
        imports: pair(
            db,
            "SELECT count(*) AS n, COALESCE(sum(size_bytes), 0)::int8 AS b FROM ticket_imports WHERE source_key IS NOT NULL",
        )
        .await?,
        rule_backups: pair(
            db,
            "SELECT count(*) AS n, COALESCE(sum(size), 0)::int8 AS b FROM rule_backups",
        )
        .await?,
        logos: pair(
            db,
            "SELECT count(*) AS n, COALESCE(sum(logo_size), 0)::int8 AS b FROM projects WHERE logo_key IS NOT NULL",
        )
        .await?,
    })
}

/// Riwayat backup terbaru (maks. `limit`).
///
/// # Errors
///
/// Error database.
pub async fn backups<C: ConnectionTrait>(db: &C, limit: u64) -> Result<Vec<Backup>, DbErr> {
    let rows = db
        .query_all_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT started_at, finished_at, status, size_bytes, location, message
             FROM system_backups ORDER BY started_at DESC LIMIT $1",
            [i64::try_from(limit).unwrap_or(30).into()],
        ))
        .await?;
    Ok(rows
        .iter()
        .filter_map(|r| {
            Some(Backup {
                started_at: r.try_get::<DateTime<Utc>>("", "started_at").ok()?,
                finished_at: r.try_get::<Option<DateTime<Utc>>>("", "finished_at").ok()?,
                status: r.try_get::<String>("", "status").ok()?,
                size_bytes: r.try_get::<Option<i64>>("", "size_bytes").ok()?,
                location: r.try_get::<String>("", "location").unwrap_or_default(),
                message: r.try_get::<String>("", "message").unwrap_or_default(),
            })
        })
        .collect())
}

async fn queue<C: ConnectionTrait>(db: &C) -> Result<(Vec<QueueRow>, Vec<FailedJob>), DbErr> {
    if !table_exists(db, "pg_loco_queue").await? {
        return Ok((Vec::new(), Vec::new()));
    }
    let rows = db
        .query_all_raw(stmt(
            "SELECT name,
                    count(*) FILTER (WHERE status = 'queued') AS queued,
                    count(*) FILTER (WHERE status = 'processing') AS running,
                    count(*) FILTER (WHERE status = 'failed' AND updated_at > now() - interval '24 hours') AS failed
             FROM pg_loco_queue GROUP BY name ORDER BY name",
        ))
        .await?;
    let queue = rows
        .iter()
        .filter_map(|r| {
            Some(QueueRow {
                name: r.try_get::<String>("", "name").ok()?,
                queued: r.try_get::<i64>("", "queued").unwrap_or(0),
                running: r.try_get::<i64>("", "running").unwrap_or(0),
                failed: r.try_get::<i64>("", "failed").unwrap_or(0),
            })
        })
        .collect();
    // Pesan error worker (Loco menyimpannya di task_data.error); dipotong, tanpa args job
    // (args bisa membawa token, rules §10).
    let rows = db
        .query_all_raw(stmt(
            "SELECT name, updated_at, attempts,
                    left(COALESCE(task_data->>'error', ''), 300) AS error
             FROM pg_loco_queue
             WHERE status = 'failed' AND updated_at > now() - interval '24 hours'
             ORDER BY updated_at DESC LIMIT 50",
        ))
        .await?;
    let failed = rows
        .iter()
        .filter_map(|r| {
            Some(FailedJob {
                name: r.try_get::<String>("", "name").ok()?,
                at: r.try_get::<DateTime<Utc>>("", "updated_at").ok()?,
                attempts: r.try_get::<i32>("", "attempts").unwrap_or(0),
                error: r.try_get::<String>("", "error").unwrap_or_default(),
            })
        })
        .collect();
    Ok((queue, failed))
}

/// Kumpulkan status instance.
///
/// # Errors
///
/// Error database.
pub async fn status(ctx: &AppContext) -> Result<Status, DbErr> {
    let db = &ctx.db;
    let uptime = Utc::now().signed_duration_since(DateTime::<Utc>::from(metrics::started_at()));
    // Konstanta angka (bukan input user), sama dengan `runners::Model::is_online`.
    let runners_online = scalar_i64(
        db,
        &format!(
            "SELECT count(*) AS n FROM runners WHERE revoked_at IS NULL \
             AND last_seen_at > now() - interval '{} seconds'",
            crate::models::runners::ONLINE_SECONDS
        ),
    )
    .await?;
    let runners_total = scalar_i64(
        db,
        "SELECT count(*) AS n FROM runners WHERE revoked_at IS NULL",
    )
    .await?;
    let workspaces = scalar_i64(db, "SELECT count(*) AS n FROM workspaces").await?;
    let users = scalar_i64(db, "SELECT count(*) AS n FROM users").await?;
    let (queue, failed_jobs) = queue(db).await?;
    let (_, oldest_queued_seconds) = metrics::queue_stats(db).await?;
    let (http_5xx, http_total) = metrics::Registry::of(ctx).map_or((0, 0), |r| {
        (r.count_status(|s| s >= 500), r.count_status(|_| true))
    });
    let hub = Hub::of(ctx).map(|h| h.stats());
    let pg_version = one(db, "SELECT current_setting('server_version') AS v")
        .await?
        .and_then(|r| r.try_get::<String>("", "v").ok())
        .unwrap_or_default();
    let db_bytes = scalar_i64(db, "SELECT pg_database_size(current_database()) AS n").await?;
    let db_connections = scalar_i64(
        db,
        "SELECT count(*) AS n FROM pg_stat_activity WHERE datname = current_database()",
    )
    .await?;
    let db_max_connections =
        scalar_i64(db, "SELECT current_setting('max_connections')::int8 AS n").await?;
    let pool = db.get_postgres_connection_pool();
    let idle = u32::try_from(pool.num_idle()).unwrap_or(u32::MAX);
    let last_migration = one(
        db,
        "SELECT version AS v FROM seaql_migrations ORDER BY version DESC LIMIT 1",
    )
    .await?
    .and_then(|r| r.try_get::<String>("", "v").ok())
    .unwrap_or_default();
    let extensions = db
        .query_all_raw(stmt(
            "SELECT extname AS n, extversion AS v FROM pg_extension WHERE extname <> 'plpgsql' ORDER BY 1",
        ))
        .await?
        .iter()
        .filter_map(|r| {
            Some((
                r.try_get::<String>("", "n").ok()?,
                r.try_get::<String>("", "v").ok()?,
            ))
        })
        .collect();
    let largest_tables = db
        .query_all_raw(stmt(
            "SELECT c.relname AS n, pg_total_relation_size(c.oid) AS b
             FROM pg_class c JOIN pg_namespace s ON s.oid = c.relnamespace
             WHERE c.relkind = 'r' AND s.nspname = 'public'
             ORDER BY 2 DESC LIMIT 5",
        ))
        .await?
        .iter()
        .filter_map(|r| {
            Some(TableSize {
                name: r.try_get::<String>("", "n").ok()?,
                bytes: r.try_get::<i64>("", "b").ok()?,
            })
        })
        .collect();
    let storage = storage_use(db).await?;
    let backups = backups(db, 30).await?;
    let last_backup_ok = backups
        .iter()
        .find(|b| b.status == "ok")
        .map(|b| b.finished_at.unwrap_or(b.started_at));

    let mut warnings = Vec::new();
    let stale = last_backup_ok
        .is_none_or(|t| Utc::now().signed_duration_since(t) > Duration::hours(BACKUP_STALE_HOURS));
    if stale {
        warnings.push("system-warn-backup");
    }
    if backups.first().is_some_and(|b| b.status == "failed") {
        warnings.push("system-warn-backup-failed");
    }
    if !failed_jobs.is_empty() {
        warnings.push("system-warn-jobs-failed");
    }
    if oldest_queued_seconds > 600.0 {
        warnings.push("system-warn-queue-old");
    }
    if hub.is_some_and(|h| h.listeners > 0 && !h.connected) {
        warnings.push("system-warn-realtime");
    }
    if http_5xx > 0 {
        warnings.push("system-warn-5xx");
    }
    Ok(Status {
        uptime,
        runners_online,
        runners_total,
        workspaces,
        users,
        queue,
        oldest_queued_seconds,
        failed_jobs,
        http_5xx,
        http_total,
        realtime_connected: hub.is_some_and(|h| h.connected),
        realtime_listeners: hub.map_or(0, |h| h.listeners),
        realtime_received: hub.map_or(0, |h| h.received),
        pg_version,
        db_bytes,
        db_connections,
        db_max_connections,
        pool_in_use: pool.size().saturating_sub(idle),
        pool_max: pool.options().get_max_connections(),
        last_migration,
        extensions,
        largest_tables,
        storage,
        backups,
        last_backup_ok,
        warnings,
    })
}
