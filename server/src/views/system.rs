//! Data halaman Status sistem (`/system/{panel}`, M12) dan halaman instal CLI publik
//! (`/install`). Angka mentah dari `services::system::Status` diformat di sini.

use chrono::{DateTime, Utc};
use loco_rs::prelude::*;
use serde::Serialize;

use super::{nav::NavItem, time::wib_datetime};

/// Panel Status sistem: key (path), label Fluent, ikon.
pub const PANELS: &[(&str, &str, &str)] = &[
    ("summary", "system-panel-summary", "heart-rate-monitor"),
    ("services", "system-panel-services", "server"),
    ("database", "system-panel-database", "database"),
    ("backups", "system-panel-backups", "cloud"),
    ("errors", "system-panel-errors", "alert-circle"),
];

/// `1,2 GB` gaya Indonesia (koma desimal, basis 1024).
#[must_use]
pub fn human_bytes(bytes: i64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    #[allow(clippy::cast_precision_loss)]
    let mut v = bytes.max(0) as f64;
    let mut unit = 0;
    while v >= 1024.0 && unit < UNITS.len() - 1 {
        v /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} B", bytes.max(0))
    } else {
        format!("{v:.1} {}", UNITS[unit]).replace('.', ",")
    }
}

/// Durasi ringkas: `3 hari 4 jam`, `2 jam 5 mnt`, `40 dtk`.
#[must_use]
pub fn human_duration(secs: i64) -> String {
    let secs = secs.max(0);
    let (d, h, m) = (secs / 86_400, (secs % 86_400) / 3600, (secs % 3600) / 60);
    if d > 0 {
        format!("{d} hari {h} jam")
    } else if h > 0 {
        format!("{h} jam {m} mnt")
    } else if m > 0 {
        format!("{m} mnt")
    } else {
        format!("{secs} dtk")
    }
}

#[must_use]
pub fn subnav(current: &str, backup_alert: bool, errors: usize) -> Vec<NavItem> {
    PANELS
        .iter()
        .map(|(key, label, icon)| {
            let href = if *key == "summary" {
                "/system".to_string()
            } else {
                format!("/system/{key}")
            };
            let mut item = NavItem::link(key, label, icon, &href);
            item.current = *key == current;
            match *key {
                "backups" if backup_alert => item.counted(1, true),
                "errors" if errors > 0 => item.counted(errors as u64, true),
                _ => item,
            }
        })
        .collect()
}

#[derive(Debug, Serialize)]
pub struct QueueView {
    pub name: String,
    pub queued: i64,
    pub running: i64,
    pub failed: i64,
}

#[derive(Debug, Serialize)]
pub struct FailedView {
    pub name: String,
    pub at: String,
    pub attempts: i32,
    pub error: String,
}

#[derive(Debug, Serialize)]
pub struct BackupView {
    pub at: String,
    pub ok: bool,
    pub running: bool,
    pub size: String,
    pub duration: String,
    pub location: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct StorageRow {
    pub label_key: &'static str,
    pub files: i64,
    pub size: String,
    /// Persen dari total (0–100) untuk bar.
    pub pct: i64,
}

/// Waktu WIB atau `""`.
#[must_use]
pub fn when(at: Option<&DateTime<Utc>>) -> String {
    at.map(wib_datetime).unwrap_or_default()
}

/// Halaman instal CLI publik. `released` = `cli_release_url` terisi (installer tersedia).
///
/// # Errors
///
/// Error render template.
pub fn install(
    v: &impl ViewRenderer,
    base: &str,
    released: bool,
    min_version: &str,
) -> Result<Response> {
    format::render().view(
        v,
        "install/page.html",
        data!({
            "base": base,
            "released": released,
            "min_version": min_version,
            "version": agentgate_core::VERSION,
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_and_durations_read_naturally() {
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(1536), "1,5 KB");
        assert_eq!(human_bytes(1_181_116_006), "1,1 GB");
        assert_eq!(human_duration(40), "40 dtk");
        assert_eq!(human_duration(7_500), "2 jam 5 mnt");
        assert_eq!(human_duration(3 * 86_400 + 4 * 3600), "3 hari 4 jam");
    }
}
