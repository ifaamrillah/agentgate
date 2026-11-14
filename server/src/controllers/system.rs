//! Status sistem instance (M12): `GET /system` (ringkasan) dan `GET /system/{panel}`
//! (`services`, `database`, `backups`, `errors`). Hanya admin instance
//! (`settings.system.admin_emails`); selain itu 404 (keberadaannya tidak terlihat).
//! Halaman baca saja: backup dijalankan container `backup` (deploy/), pemulihan lewat CLI server.

use axum::http::StatusCode;
use loco_rs::prelude::*;

use crate::{
    common::settings::Settings,
    controllers::{extractors::CurrentUser, shell::app_shell},
    services::system::{self as svc, Status},
    views::{
        self,
        system::{
            BackupView, FailedView, PANELS, QueueView, StorageRow, human_bytes, human_duration,
            when,
        },
        time::wib_datetime,
        ui::Stat,
    },
};

#[must_use]
pub fn stats(s: &Status) -> Vec<Stat> {
    let uptime = human_duration(s.uptime.num_seconds());
    let queued: i64 = s.queue.iter().map(|q| q.queued).sum();
    let mut v = vec![
        Stat::new("system-stat-uptime", uptime, "activity").tone("success"),
        Stat::new("system-stat-runners", s.runners_online, "device-laptop").of(s.runners_total),
        Stat::new("system-stat-queued", queued, "hourglass").hint(
            "system-stat-queued-hint",
            human_duration(s.oldest_queued_seconds as i64),
        ),
        Stat::new("system-stat-failed", s.failed_jobs.len(), "alert-circle").link("/system/errors"),
    ];
    if !s.failed_jobs.is_empty() {
        v[3] = Stat::new("system-stat-failed", s.failed_jobs.len(), "alert-circle")
            .tone("danger")
            .link("/system/errors");
    }
    v
}

#[must_use]
pub fn queue(s: &Status) -> Vec<QueueView> {
    s.queue
        .iter()
        .map(|q| QueueView {
            name: q.name.clone(),
            queued: q.queued,
            running: q.running,
            failed: q.failed,
        })
        .collect()
}

#[must_use]
pub fn failed(s: &Status) -> Vec<FailedView> {
    s.failed_jobs
        .iter()
        .map(|f| FailedView {
            name: f.name.clone(),
            at: wib_datetime(&f.at),
            attempts: f.attempts,
            error: f.error.clone(),
        })
        .collect()
}

#[must_use]
pub fn backups(s: &Status) -> Vec<BackupView> {
    s.backups
        .iter()
        .map(|b| BackupView {
            at: wib_datetime(&b.started_at),
            ok: b.status == "ok",
            running: b.status == "running",
            size: b.size_bytes.map(human_bytes).unwrap_or_default(),
            duration: b
                .finished_at
                .map(|f| human_duration(f.signed_duration_since(b.started_at).num_seconds()))
                .unwrap_or_default(),
            location: b.location.clone(),
            message: b.message.clone(),
        })
        .collect()
}

#[must_use]
pub fn storage(s: &Status) -> Vec<StorageRow> {
    let u = &s.storage;
    let total = u.total().1.max(1);
    [
        ("system-storage-attachments", u.attachments),
        ("system-storage-evidence", u.evidence),
        ("system-storage-exports", u.exports),
        ("system-storage-imports", u.imports),
        ("system-storage-rule-backups", u.rule_backups),
        ("system-storage-logos", u.logos),
    ]
    .into_iter()
    .map(|(label_key, (files, bytes))| StorageRow {
        label_key,
        files,
        size: human_bytes(bytes),
        pct: bytes * 100 / total,
    })
    .collect()
}

async fn render(
    ctx: &AppContext,
    v: &TeraView,
    user: &crate::models::users::Model,
    panel: &str,
) -> Result<Response> {
    let settings = Settings::from_ctx(ctx)?;
    if !settings.system.is_admin(&user.email) {
        return Err(Error::NotFound);
    }
    if !PANELS.iter().any(|(k, ..)| *k == panel) {
        return Err(Error::NotFound);
    }
    let s = svc::status(ctx).await?;
    let backup_alert = s
        .warnings
        .iter()
        .any(|w| w.starts_with("system-warn-backup"));
    let subnav = views::system::subnav(panel, backup_alert, s.failed_jobs.len());
    let (files, bytes) = s.storage.total();
    let data = serde_json::json!({
        "panel": panel,
        "subnav": subnav,
        "checked_at": wib_datetime(&chrono::Utc::now()),
        "warnings": s.warnings,
        "stats": stats(&s),
        "version": agentgate_core::VERSION,
        "build": option_env!("BUILD_SHA").or(option_env!("GITHUB_SHA")).unwrap_or("dev"),
        "environment": ctx.environment.to_string(),
        "cli_min_version": settings.system.cli_min_version,
        "workspaces": s.workspaces,
        "users": s.users,
        "http_total": s.http_total,
        "http_5xx": s.http_5xx,
        "realtime": {
            "connected": s.realtime_connected,
            "listeners": s.realtime_listeners,
            "received": s.realtime_received,
        },
        "queue": queue(&s),
        "oldest_queued": human_duration(s.oldest_queued_seconds as i64),
        "failed": failed(&s),
        "db": {
            "version": s.pg_version,
            "size": human_bytes(s.db_bytes),
            "connections": s.db_connections,
            "max_connections": s.db_max_connections,
            "pool_in_use": s.pool_in_use,
            "pool_max": s.pool_max,
            "last_migration": s.last_migration,
            "extensions": s.extensions.iter().map(|(n, v)| format!("{n} {v}")).collect::<Vec<_>>().join(", "),
            "tables": s.largest_tables.iter().map(|t| serde_json::json!({"name": t.name, "size": human_bytes(t.bytes)})).collect::<Vec<_>>(),
        },
        "storage": storage(&s),
        "storage_total": { "files": files, "size": human_bytes(bytes) },
        "backups": backups(&s),
        "last_backup_ok": when(s.last_backup_ok.as_ref()),
        "backup_target": settings.system.backup_target,
        "backup_schedule": settings.system.backup_schedule,
        "metrics_enabled": !settings.system.metrics_token.is_empty(),
    });
    let shell = app_shell(ctx, user, None, None).await?.at("system");
    views::tickets::render(v, &shell, "system/page.html", StatusCode::OK, data)
}

#[debug_handler]
async fn summary(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    CurrentUser(user): CurrentUser,
) -> Result<Response> {
    render(&ctx, &v, &user, "summary").await
}

#[debug_handler]
async fn panel(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    CurrentUser(user): CurrentUser,
    Path(panel): Path<String>,
) -> Result<Response> {
    if panel == "summary" {
        return Err(Error::NotFound);
    }
    render(&ctx, &v, &user, &panel).await
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/system")
        .add("/", get(summary))
        .add("/{panel}", get(panel))
}
