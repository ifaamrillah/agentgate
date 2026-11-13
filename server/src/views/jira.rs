//! Pengaturan → Jira (M11): baris log sinkron dan kunci teks untuk template
//! `integrations/jira.html`. Teks lewat kunci Fluent (`ui-jira.ftl`).
use serde::Serialize;
use uuid::Uuid;

use crate::models::jira_sync_logs::{self, LogArgs};

pub const FLASH_SAVED: &str = "jira-flash-saved";
pub const FLASH_SYNCING: &str = "jira-flash-syncing";
pub const FLASH_PAUSED: &str = "jira-flash-paused";
pub const FLASH_RESUMED: &str = "jira-flash-resumed";
pub const FLASH_FULL: &str = "jira-flash-full";
pub const FLASH_DISCONNECTED: &str = "jira-flash-disconnected";

/// Kunci pesan log yang ditulis `services::jira::sync` (kontrak i18n).
pub const LOG_MESSAGES: &[&str] = &[
    "jira-log-run-failed",
    "jira-log-owner-missing",
    "jira-log-push-failed",
    "jira-log-issue-created",
    "jira-log-issue-updated",
    "jira-log-stage-unmapped",
    "jira-log-status-pushed",
    "jira-log-transition-failed",
    "jira-log-comment-failed",
    "jira-log-comment-pushed",
    "jira-log-comment-pulled",
    "jira-log-search-failed",
    "jira-log-ticket-created",
    "jira-log-ticket-updated",
    "jira-log-pull-failed",
    "jira-log-stage-moved",
    "jira-log-gate-rejected",
    "jira-log-gate-rejected-kept",
    "jira-log-ticket-imported",
    "jira-log-user-unmapped",
    "jira-log-stage-gated",
    "jira-log-import-failed",
    "jira-log-attachment-skipped",
    "jira-log-attachment-failed",
];

/// Kunci alasan yang ditulis sinkron selain kode error.
pub const REASONS: &[&str] = &["jira-reason-unmapped", "jira-reason-no-access"];

/// Kunci teks yang dipakai dari Rust (kontrak i18n).
#[must_use]
pub fn label_keys() -> Vec<String> {
    [
        FLASH_SAVED,
        FLASH_SYNCING,
        FLASH_PAUSED,
        FLASH_RESUMED,
        FLASH_FULL,
        FLASH_DISCONNECTED,
        "jira-panel-overview",
        "jira-panel-overview-desc",
        "jira-panel-conn",
        "jira-panel-conn-desc",
        "jira-panel-mode",
        "jira-panel-mode-desc",
        "jira-panel-status",
        "jira-panel-status-desc",
        "jira-panel-fields",
        "jira-panel-fields-desc",
        "jira-panel-log",
        "jira-panel-log-desc",
        "jira-col-time",
        "jira-col-result",
        "jira-col-ticket",
        "jira-col-message",
        "jira-filter-result",
        "jira-filter-direction",
        "jira-result-ok",
        "jira-result-gate",
        "jira-result-fail",
        "jira-dir-in",
        "jira-dir-out",
        "jira-log-search",
        "jira-stat-tickets",
        "jira-stat-tickets-hint",
        "jira-stat-changes",
        "jira-stat-changes-hint",
        "jira-stat-gate",
        "jira-stat-gate-hint",
        "jira-env-cloud",
        "jira-env-data_center",
        "jira-mode-once",
        "jira-mode-mirror",
        "jira-mode-two_way",
    ]
    .iter()
    .map(|k| (*k).to_string())
    .chain(LOG_MESSAGES.iter().map(|k| (*k).to_string()))
    .chain(REASONS.iter().map(|k| (*k).to_string()))
    .chain(
        crate::models::jira_connections::FieldTarget::BUILTIN
            .iter()
            .map(|t| format!("jira-target-{}", t.as_str())),
    )
    .collect()
}

/// Satu baris Log sinkron.
#[derive(Debug, Clone, Serialize)]
pub struct LogRow {
    pub id: Uuid,
    pub at: String,
    pub direction: String,
    pub result: String,
    /// `SR-311 → KSR-24` (masuk) / `KSR-24 → SR-311` (keluar), `↛` bila ditolak gate.
    pub subject: String,
    pub ticket_href: Option<String>,
    pub message: String,
    pub args: LogArgs,
}

impl LogRow {
    #[must_use]
    pub fn of(l: &jira_sync_logs::Model, slug: &str) -> Self {
        let arrow = if l.result == "gate" { "↛" } else { "→" };
        let subject = match (l.issue_key.as_str(), l.ticket_code.as_str()) {
            ("", "") => String::new(),
            (issue, "") => issue.to_string(),
            ("", code) => code.to_string(),
            (issue, code) if l.direction == "in" => format!("{issue} {arrow} {code}"),
            (issue, code) => format!("{code} {arrow} {issue}"),
        };
        Self {
            id: l.id,
            at: super::time::wib_datetime(&l.created_at),
            direction: l.direction.clone(),
            result: l.result.clone(),
            subject,
            ticket_href: (!l.ticket_code.is_empty() && l.ticket_id.is_some())
                .then(|| format!("/w/{slug}/t/{}", l.ticket_code)),
            message: l.message.clone(),
            args: translated(l),
        }
    }
}

/// Argumen log; daftar target field (`title, labels`) diterjemahkan ke nama field.
fn translated(l: &jira_sync_logs::Model) -> LogArgs {
    let mut args = l.args();
    if matches!(
        l.message.as_str(),
        "jira-log-issue-updated" | "jira-log-ticket-updated"
    ) {
        args.detail = args
            .detail
            .split(',')
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(|t| crate::common::i18n::text(&format!("jira-target-{t}"), &[]))
            .collect::<Vec<_>>()
            .join(", ");
    }
    args
}

/// Satu baris unduhan JSONL.
#[derive(Debug, Serialize)]
pub struct LogLine<'a> {
    pub at: String,
    pub direction: &'a str,
    pub result: &'a str,
    pub ticket: &'a str,
    pub issue: &'a str,
    pub message: &'a str,
    pub args: serde_json::Value,
}

impl<'a> LogLine<'a> {
    #[must_use]
    pub fn of(l: &'a jira_sync_logs::Model) -> Self {
        Self {
            at: l.created_at.to_rfc3339(),
            direction: &l.direction,
            result: &l.result,
            ticket: &l.ticket_code,
            issue: &l.issue_key,
            message: &l.message,
            args: l.args.clone(),
        }
    }
}
