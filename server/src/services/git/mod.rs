//! Git & PR (M6): koneksi provider per workspace, tautan repo + webhook, PR tiket, webhook
//! status PR/CI, dan merge otomatis.
//!
//! - [`provider`]: abstraksi [`provider::GitProvider`] + implementasi GitHub.
//! - [`connections`]: hubungkan/putus provider (`ManageIntegrations`), tautkan repo.
//! - [`pulls`]: buka PR dari runner, upsert PR dari provider, tautan PR → tiket, merge.
//! - [`webhook`]: verifikasi tanda tangan dan event GitHub.

pub mod connections;
pub mod provider;
pub mod pulls;
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
pub const ERROR_CODES: &[&str] = &[
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

/// Provider untuk satu koneksi.
///
/// # Errors
///
/// Token tidak bisa dibuka (`git-token-invalid`), client HTTP gagal dibuat.
pub fn provider_for(
    conn: &git_connections::Model,
    settings: &Settings,
) -> Result<Box<dyn provider::GitProvider>, TicketFailure> {
    let token = conn
        .token(&key(settings)?)
        .map_err(|_| invalid("token", TOKEN_INVALID))?;
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
