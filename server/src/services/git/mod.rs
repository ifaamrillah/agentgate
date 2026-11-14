//! Git & PR (M6): koneksi provider per workspace, tautan repo + webhook, PR tiket, webhook
//! status PR/CI, dan merge otomatis.
//!
//! - [`provider`]: abstraksi [`provider::GitProvider`] + implementasi GitHub.
//! - [`connections`]: hubungkan/putus provider (`ManageIntegrations`), tautkan repo.
//! - [`pulls`]: buka PR dari runner, upsert PR dari provider, tautan PR → tiket, merge.
//! - [`webhook`]: verifikasi tanda tangan dan event GitHub.

pub mod app;
pub mod connections;
pub mod provider;
pub mod pulls;
pub mod settings;
pub mod tags;
pub mod webhook;

use std::time::Duration;

use crate::{
    common::settings::Settings,
    models::git_connections::{self, GitProviderKind},
    services::tickets::TicketFailure,
};

pub const TOKEN_INVALID: &str = "git-token-invalid";
pub const UNREACHABLE: &str = "git-unreachable";
pub const URL_INVALID: &str = "git-url-invalid";
pub const NOT_CONNECTED: &str = "git-not-connected";
pub const PROVIDER_REJECTED: &str = "git-provider-rejected";
pub const REPO_NOT_FOUND: &str = "git-repo-not-found";
/// Tombol Merge ditekan untuk PR yang belum siap (review belum di-approve, CI belum hijau,
/// sudah di-merge, atau mode merge otomatis).
pub const MERGE_NOT_READY: &str = "git-merge-not-ready";
/// GitHub App belum dikonfigurasi di instance, atau kuncinya tidak valid.
pub const APP_NOT_CONFIGURED: &str = "git-app-not-configured";
/// Callback instalasi App tidak valid (state kedaluwarsa/rusak, instalasi bukan milik user).
pub const APP_INSTALL_INVALID: &str = "git-app-install-invalid";
/// Instalasi App ditangguhkan pemilik akun GitHub.
pub const APP_SUSPENDED: &str = "git-app-suspended";
/// Repo tidak termasuk repo yang diberi akses ke App.
pub const APP_REPO_NOT_GRANTED: &str = "git-app-repo-not-granted";
pub const ERROR_CODES: &[&str] = &[
    tags::TAG_INVALID,
    tags::SHA_INVALID,
    tags::TAG_EXISTS,
    APP_NOT_CONFIGURED,
    APP_INSTALL_INVALID,
    APP_SUSPENDED,
    APP_REPO_NOT_GRANTED,
    MERGE_NOT_READY,
    TOKEN_INVALID,
    UNREACHABLE,
    URL_INVALID,
    NOT_CONNECTED,
    PROVIDER_REJECTED,
    REPO_NOT_FOUND,
];

fn invalid(field: &'static str, code: &'static str) -> TicketFailure {
    TicketFailure::field(field, code)
}

/// Kunci enkripsi token/secret (`ENCRYPTION_KEY`, sudah divalidasi saat boot).
///
/// # Errors
///
/// Kunci tidak valid (tidak terjadi setelah boot).
pub fn key(settings: &Settings) -> Result<[u8; 32], TicketFailure> {
    settings
        .auth
        .encryption_key_bytes()
        .map_err(|_| invalid("token", TOKEN_INVALID))
}

/// Klien App untuk koneksi (basis API/web koneksi + config App instance).
#[must_use]
pub fn app_client<'a>(
    conn: &'a git_connections::Model,
    settings: &'a Settings,
) -> app::AppClient<'a> {
    app::AppClient {
        api_base: &conn.api_base,
        web_base: &conn.web_base,
        app: &settings.git.app,
        timeout: Duration::from_secs(settings.git.timeout_seconds.max(1)),
    }
}

/// Token akses koneksi: PAT tersimpan (dibuka dengan `ENCRYPTION_KEY`), atau token instalasi
/// GitHub App (di-cache sampai hampir kedaluwarsa).
///
/// # Errors
///
/// `git-token-invalid`, `git-app-not-configured`, `git-app-suspended`, `git-unreachable`.
pub async fn access_token(
    conn: &git_connections::Model,
    settings: &Settings,
) -> Result<String, TicketFailure> {
    if !conn.is_app() {
        return conn
            .token(&key(settings)?)
            .map_err(|_| invalid("token", TOKEN_INVALID));
    }
    if conn.suspended_at.is_some() {
        return Err(invalid("token", APP_SUSPENDED));
    }
    let id = conn
        .installation_id
        .ok_or_else(|| invalid("token", APP_NOT_CONFIGURED))?;
    app::installation_token(&app_client(conn, settings), id)
        .await
        .map_err(|e| invalid("token", app_code(&e)))
}

/// Error App → kode Fluent.
#[must_use]
pub fn app_code(e: &app::AppError) -> &'static str {
    match e {
        app::AppError::NotConfigured | app::AppError::BadKey => APP_NOT_CONFIGURED,
        app::AppError::Provider(p) => provider_code(p),
    }
}

/// Provider untuk satu koneksi (PAT atau token instalasi App).
///
/// # Errors
///
/// Token tidak bisa dibuka/diminta, client HTTP gagal dibuat.
pub async fn provider_for(
    conn: &git_connections::Model,
    settings: &Settings,
) -> Result<Box<dyn provider::GitProvider>, TicketFailure> {
    let token = access_token(conn, settings).await?;
    let timeout = Duration::from_secs(settings.git.timeout_seconds.max(1));
    match conn.kind() {
        GitProviderKind::Github | GitProviderKind::GithubEnterprise => Ok(Box::new(
            provider::GithubProvider::new(&conn.api_base, &token, timeout)
                .map_err(|_| invalid("token", UNREACHABLE))?,
        )),
    }
}

/// Error provider → kode Fluent.
#[must_use]
pub fn provider_code(e: &provider::ProviderError) -> &'static str {
    match e {
        provider::ProviderError::Unauthorized => TOKEN_INVALID,
        provider::ProviderError::NotFound => REPO_NOT_FOUND,
        provider::ProviderError::Rejected(_) => PROVIDER_REJECTED,
        provider::ProviderError::Network(_) | provider::ProviderError::Decode(_) => UNREACHABLE,
    }
}
