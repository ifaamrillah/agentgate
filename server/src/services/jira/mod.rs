//! Connector Jira (M11, spec "Integrasi Jira & API publik"): satu koneksi per workspace, tiga
//! mode.
//!
//! - [`api`]: trait [`api::JiraApi`] + klien reqwest Cloud (REST v3) / Data Center (REST v2).
//! - [`adf`]: deskripsi/komentar ADF ↔ teks.
//! - [`mapping`]: saran pemetaan status ↔ stage, field ↔ field, user ↔ user (email).
//! - [`connections`]: simpan/uji koneksi, project, mode, pemetaan, jeda, putus (audit).
//! - [`issues`]: nilai field issue ↔ tiket ([`crate::models::jira_links::Snapshot`]).
//! - [`sync`]: impor sekali, mirror Agent Gate → Jira (outbox), tarik dua arah (JQL), dijalankan
//!   worker `JiraSyncWorker` dan task `jira_sync` (scheduler).
//!
//! Status/stage selalu ikut Agent Gate (gate ditegakkan di sini); field lain ikut perubahan
//! terakhir.

pub mod adf;
pub mod api;
pub mod connections;
pub mod issues;
pub mod mapping;
pub mod sync;

use std::time::Duration;

use crate::{
    common::settings::Settings,
    models::jira_connections::{self, JiraEnv},
};

pub const URL_INVALID: &str = "jira-url-invalid";
pub const URL_FORBIDDEN: &str = "jira-url-forbidden";
pub const EMAIL_REQUIRED: &str = "jira-email-required";
pub const TOKEN_REQUIRED: &str = "jira-token-required";
pub const TOKEN_INVALID: &str = "jira-token-invalid";
pub const UNREACHABLE: &str = "jira-unreachable";
pub const REJECTED: &str = "jira-rejected";
pub const ENV_INVALID: &str = "jira-env-invalid";
pub const MODE_INVALID: &str = "jira-mode-invalid";
pub const PROJECT_INVALID: &str = "jira-project-invalid";
pub const JIRA_PROJECT_INVALID: &str = "jira-jira-project-invalid";
pub const NOT_CONNECTED: &str = "jira-not-connected";
pub const STAGE_INVALID: &str = "jira-stage-invalid";
pub const FIELD_INVALID: &str = "jira-field-invalid";
pub const USER_INVALID: &str = "jira-user-invalid";
/// Kode error yang dipakai dari Rust (kontrak i18n).
pub const ERROR_CODES: &[&str] = &[
    URL_INVALID,
    URL_FORBIDDEN,
    EMAIL_REQUIRED,
    TOKEN_REQUIRED,
    TOKEN_INVALID,
    UNREACHABLE,
    REJECTED,
    ENV_INVALID,
    MODE_INVALID,
    PROJECT_INVALID,
    JIRA_PROJECT_INVALID,
    NOT_CONNECTED,
    STAGE_INVALID,
    FIELD_INVALID,
    USER_INVALID,
];

/// Lampiran Jira terbesar yang diimpor (spec: ≤ 25 MB; lebih besar dilewati + log).
pub const MAX_ATTACHMENT_BYTES: usize = 25 * 1024 * 1024;

/// Kunci enkripsi token (`ENCRYPTION_KEY`, sudah divalidasi saat boot).
///
/// # Errors
///
/// Kunci tidak valid.
pub fn key(settings: &Settings) -> Result<[u8; 32], &'static str> {
    settings
        .auth
        .encryption_key_bytes()
        .map_err(|_| TOKEN_INVALID)
}

/// Kredensial dari isian.
#[must_use]
pub fn auth(env: JiraEnv, email: &str, token: &str) -> api::Auth {
    match env {
        JiraEnv::Cloud => api::Auth::Basic {
            email: email.to_string(),
            token: token.to_string(),
        },
        JiraEnv::DataCenter => api::Auth::Bearer(token.to_string()),
    }
}

/// Klien HTTP untuk alamat + kredensial.
///
/// # Errors
///
/// Kode Fluent ([`URL_INVALID`], [`URL_FORBIDDEN`], [`UNREACHABLE`]).
pub async fn client(
    settings: &Settings,
    base_url: &str,
    env: JiraEnv,
    auth: api::Auth,
) -> Result<api::HttpJira, &'static str> {
    let timeout = Duration::from_secs(settings.jira.timeout_seconds.max(1));
    api::HttpJira::connect(base_url, env, auth, timeout, settings.jira.allow_loopback)
        .await
        .map_err(|e| error_code(&e))
}

/// Klien untuk koneksi tersimpan.
///
/// # Errors
///
/// Token tidak bisa dibuka, URL ditolak, DNS gagal.
pub async fn client_for(
    conn: &jira_connections::Model,
    settings: &Settings,
) -> Result<api::HttpJira, &'static str> {
    let token = conn
        .token(&key(settings)?)
        .map_err(|_| TOKEN_INVALID)?;
    client(settings, &conn.base_url, conn.env(), auth(conn.env(), &conn.email, &token)).await
}

/// Error Jira → kode Fluent.
#[must_use]
pub fn error_code(e: &api::JiraError) -> &'static str {
    match e {
        api::JiraError::Unauthorized => TOKEN_INVALID,
        api::JiraError::Rejected(m) if m == "url_forbidden" => URL_FORBIDDEN,
        api::JiraError::Rejected(m) if m == "url_invalid" => URL_INVALID,
        api::JiraError::NotFound | api::JiraError::Rejected(_) => REJECTED,
        api::JiraError::Network(_) | api::JiraError::Decode(_) => UNREACHABLE,
    }
}

/// Validasi URL site/server Jira (penjaga SSRF webhook: https, http hanya ke loopback bila
/// `settings.jira.allow_loopback`). Kembalikan URL tanpa query dan `/` di akhir (path konteks
/// Data Center seperti `/jira` dipertahankan).
///
/// # Errors
///
/// [`URL_INVALID`] atau [`URL_FORBIDDEN`].
pub fn webhooks_url(raw: &str, settings: &Settings) -> Result<String, &'static str> {
    let mut url = crate::services::webhooks::url_guard::validate(raw, settings.jira.allow_loopback)
        .map_err(|code| {
            if code == crate::services::webhooks::url_guard::URL_FORBIDDEN {
                URL_FORBIDDEN
            } else {
                URL_INVALID
            }
        })?;
    url.set_query(None);
    Ok(url.as_str().trim_end_matches('/').to_string())
}
