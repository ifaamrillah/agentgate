//! Pengaturan → API & webhook (M11): baris token, webhook, dan log pengiriman untuk template
//! `integrations/api.html`. Teks lewat kunci Fluent (`ui-api.ftl`).
use std::collections::HashMap;

use chrono::Utc;
use serde::Serialize;
use uuid::Uuid;

use super::runner::{Age, age};
use crate::models::{
    api_tokens::{self, ApiScope},
    outbox_events::WebhookEvent,
    webhook_deliveries::{self, DeliveryStatus},
    webhooks,
};

pub const FLASH_TOKEN_REVOKED: &str = "api-flash-token-revoked";
pub const FLASH_HOOK_CREATED: &str = "api-flash-hook-created";
pub const FLASH_HOOK_UPDATED: &str = "api-flash-hook-updated";
pub const FLASH_HOOK_DELETED: &str = "api-flash-hook-deleted";
pub const FLASH_HOOK_ON: &str = "api-flash-hook-on";
pub const FLASH_HOOK_OFF: &str = "api-flash-hook-off";
pub const FLASH_PING: &str = "api-flash-ping";
pub const FLASH_RESENT: &str = "api-flash-resent";

/// Kunci teks yang dipakai dari Rust (kontrak i18n).
#[must_use]
pub fn label_keys() -> Vec<String> {
    let mut keys: Vec<String> = [
        FLASH_TOKEN_REVOKED,
        FLASH_HOOK_CREATED,
        FLASH_HOOK_UPDATED,
        FLASH_HOOK_DELETED,
        FLASH_HOOK_ON,
        FLASH_HOOK_OFF,
        FLASH_PING,
        FLASH_RESENT,
        "api-panel-tokens",
        "api-panel-tokens-desc",
        "api-panel-hooks",
        "api-panel-hooks-desc",
        "api-panel-log",
        "api-panel-log-desc",
        "api-panel-docs",
        "api-panel-docs-desc",
        "api-col-name",
        "api-col-scopes",
        "api-col-used",
        "api-col-expires",
        "api-col-actions",
        "api-col-target",
        "api-col-last",
        "api-col-active",
        "api-col-time",
        "api-col-event",
        "api-col-status",
        "api-col-duration",
        "api-filter-creator",
        "api-filter-result",
        "api-result-ok",
        "api-result-fail",
        "api-result-pending",
        "api-tokens-search",
        "api-log-search",
        "api-expiry-30",
        "api-expiry-90",
        "api-expiry-365",
        "api-expiry-never",
    ]
    .iter()
    .map(|k| (*k).to_string())
    .collect();
    keys.extend(<ApiScope as sea_orm::Iterable>::iter().map(ApiScope::label_key));
    keys.extend(DELIVERY_ERRORS.iter().map(|c| error_key(c)));
    keys.push(error_key(""));
    keys
}

/// Pilihan cakupan di dialog Token baru.
#[derive(Debug, Serialize)]
pub struct ScopeOption {
    pub value: String,
    pub label_key: String,
    pub checked: bool,
}

#[must_use]
pub fn scope_options(checked: &[String]) -> Vec<ScopeOption> {
    use sea_orm::Iterable;
    ApiScope::iter()
        .map(|s| ScopeOption {
            value: s.as_str(),
            label_key: s.label_key(),
            checked: checked.contains(&s.as_str()),
        })
        .collect()
}

/// Satu baris token.
#[derive(Debug, Serialize)]
pub struct TokenRow {
    pub id: Uuid,
    pub name: String,
    pub prefix: String,
    pub scopes: Vec<String>,
    pub creator: String,
    pub creator_initials: String,
    pub project: String,
    pub used: Option<Age>,
    /// `dd/mm/yyyy` WIB; `None` = tanpa batas.
    pub expires: Option<String>,
    /// Kedaluwarsa dalam 14 hari (atau sudah lewat).
    pub expiring: bool,
    pub expired: bool,
    pub revoke_action: String,
}

impl TokenRow {
    #[must_use]
    pub fn of(t: &api_tokens::Model, creator: &str, project: &str, base: &str) -> Self {
        let now = Utc::now();
        let expires = t
            .expires_at
            .map(|e| super::time::wib_datetime(&e)[..10].to_string());
        let left = t.expires_at.map(|e| e.with_timezone(&Utc) - now);
        Self {
            id: t.id,
            name: t.name.clone(),
            prefix: t.prefix.clone(),
            scopes: t.scopes.clone(),
            creator: creator.to_string(),
            creator_initials: initials(creator),
            project: project.to_string(),
            used: t.last_used_at.map(|at| age(&at, now)),
            expires,
            expiring: left.is_some_and(|d| d.num_days() < 14),
            expired: left.is_some_and(|d| d.num_seconds() <= 0),
            revoke_action: format!("{base}/tokens/{}/revoke", t.id),
        }
    }
}

fn initials(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase()
}

/// Satu baris webhook.
#[derive(Debug, Serialize)]
pub struct HookRow {
    pub id: Uuid,
    pub url: String,
    pub host: String,
    pub events: Vec<String>,
    pub projects: Vec<String>,
    pub project_keys: Vec<String>,
    /// Nilai awal combo project di dialog ubah (`["all"]` bila tanpa filter).
    pub form_projects: Vec<String>,
    pub active: bool,
    /// `ok` / `fail` / `off` / `none`.
    pub state: &'static str,
    pub last_status: Option<i32>,
    pub last: Option<Age>,
    pub failures: i32,
    pub update_action: String,
    pub toggle_action: String,
    pub ping_action: String,
    pub rotate_action: String,
    pub delete_action: String,
    pub log_href: String,
}

impl HookRow {
    #[must_use]
    pub fn of(
        h: &webhooks::Model,
        project_names: &HashMap<Uuid, (String, String)>,
        base: &str,
    ) -> Self {
        let state = if !h.is_active() {
            "off"
        } else if h.consecutive_failures > 0 {
            "fail"
        } else if h.last_delivery_at.is_some() {
            "ok"
        } else {
            "none"
        };
        let mut events = h.events.clone();
        events.sort();
        let project_keys: Vec<String> = h
            .project_ids
            .iter()
            .filter_map(|p| project_names.get(p).map(|(k, _)| k.clone()))
            .collect();
        Self {
            id: h.id,
            url: h.url.clone(),
            host: h.host(),
            events,
            projects: h
                .project_ids
                .iter()
                .filter_map(|p| project_names.get(p).map(|(_, n)| n.clone()))
                .collect(),
            project_keys: project_keys.clone(),
            form_projects: if project_keys.is_empty() {
                vec!["all".to_string()]
            } else {
                project_keys
            },
            active: h.is_active(),
            state,
            last_status: h.last_status,
            last: h.last_delivery_at.map(|at| age(&at, Utc::now())),
            failures: h.consecutive_failures,
            update_action: format!("{base}/hooks/{}", h.id),
            toggle_action: format!("{base}/hooks/{}/toggle", h.id),
            ping_action: format!("{base}/hooks/{}/ping", h.id),
            rotate_action: format!("{base}/hooks/{}/rotate", h.id),
            delete_action: format!("{base}/hooks/{}/delete", h.id),
            log_href: format!("{base}/log?hook={}", h.id),
        }
    }
}

/// Satu baris log pengiriman (+ isi dialog detail).
#[derive(Debug, Serialize)]
pub struct DeliveryRow {
    pub id: Uuid,
    /// Judul dialog detail: `event · status`.
    pub title: String,
    pub at: String,
    pub event: String,
    pub host: String,
    pub url: String,
    /// `ok` / `fail` / `pending`.
    pub result: &'static str,
    pub status: Option<i32>,
    pub duration_ms: Option<i32>,
    pub attempts: i32,
    pub error: String,
    /// Kunci Fluent penjelasan `error` (`api-error-*`).
    pub error_key: String,
    pub payload: String,
    pub response: String,
    pub resend_action: String,
}

impl DeliveryRow {
    #[must_use]
    pub fn of(d: &webhook_deliveries::Model, hook: Option<&webhooks::Model>, base: &str) -> Self {
        let result = match d.status() {
            Ok(DeliveryStatus::Succeeded) => "ok",
            Ok(DeliveryStatus::Failed) => "fail",
            _ => "pending",
        };
        let payload = serde_json::from_str::<serde_json::Value>(&d.body)
            .ok()
            .and_then(|v| serde_json::to_string_pretty(&v).ok())
            .unwrap_or_else(|| d.body.clone());
        Self {
            id: d.id,
            title: format!(
                "{} · {}",
                d.event,
                d.response_status
                    .map_or_else(|| "–".to_string(), |s| s.to_string())
            ),
            at: super::time::wib_datetime(&d.created_at),
            event: d.event.clone(),
            host: hook.map(webhooks::Model::host).unwrap_or_default(),
            url: hook.map(|h| h.url.clone()).unwrap_or_default(),
            result,
            status: d.response_status,
            duration_ms: d.duration_ms,
            attempts: d.attempts,
            error: d.error.clone(),
            error_key: error_key(&d.error),
            payload,
            response: d.response_body.clone(),
            resend_action: format!("{base}/log/{}/resend", d.id),
        }
    }
}

/// Kode galat pengiriman (`dispatch`, `url_guard`) yang punya penjelasan.
pub const DELIVERY_ERRORS: &[&str] = &[
    "timeout",
    "connect_failed",
    "request_failed",
    "client_error",
    "secret_unavailable",
    "webhook_disabled",
    "url_invalid",
    "url_forbidden",
    "dns_failed",
];

/// `api-error-<kode>`; kode tak dikenal → `api-error-other`.
#[must_use]
pub fn error_key(code: &str) -> String {
    if DELIVERY_ERRORS.contains(&code) {
        format!("api-error-{}", code.replace('_', "-"))
    } else {
        "api-error-other".to_string()
    }
}

/// Opsi event di combo Webhook baru (dikelompokkan per subjek).
#[must_use]
pub fn event_options() -> Vec<super::ui::ComboOption> {
    WebhookEvent::subscribable()
        .into_iter()
        .map(|e| {
            let v = e.as_str();
            let mut o = super::ui::ComboOption::new(&v, &v);
            o.group = v.split('.').next().unwrap_or_default().to_string();
            o
        })
        .collect()
}
