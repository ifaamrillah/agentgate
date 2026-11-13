//! Client HTTP ke API CLI/runner server (`/api/cli/…`, `/api/runner/…`, `/oauth/…`).
//!
//! Server membalas error domain sebagai `{"error": "<kode>"}` dengan status 401/404/422; OAuth
//! memakai `{"error", "error_description"}` dengan 400. Keduanya dipetakan sekali ke
//! [`ApiError`].

use std::time::Duration;

use reqwest::{Method, RequestBuilder, StatusCode};
use serde::{Serialize, de::DeserializeOwned};
use uuid::Uuid;

/// Header id runner untuk endpoint job (sama dengan server).
pub const RUNNER_HEADER: &str = "x-agentgate-runner";
/// Client OAuth publik bawaan CLI.
pub const CLIENT_ID: &str = "agentgate-cli";

/// Error memanggil server.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// Token tidak ada, kedaluwarsa, atau dicabut.
    #[error("unauthorized")]
    Unauthorized,
    #[error("not found ({0})")]
    NotFound(String),
    /// Ditolak validasi domain (422) atau OAuth (400); isinya kode error server.
    #[error("rejected: {0}")]
    Rejected(String),
    #[error("unexpected status {status}: {code}")]
    Status { status: u16, code: String },
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("invalid response: {0}")]
    Decode(String),
}

impl ApiError {
    /// Kode error server (`job-not-holder`, `authorization_pending`, …) bila ada.
    #[must_use]
    pub fn code(&self) -> Option<&str> {
        match self {
            Self::NotFound(c) | Self::Rejected(c) | Self::Status { code: c, .. } => Some(c),
            _ => None,
        }
    }
}

pub type Result<T> = std::result::Result<T, ApiError>;

/// Client server dengan token bearer.
#[derive(Debug, Clone)]
pub struct Api {
    http: reqwest::Client,
    base: String,
    token: String,
    runner: Option<Uuid>,
}

/// HTTP client bersama (timeout wajar; `user-agent` berisi versi CLI).
///
/// # Errors
///
/// Gagal membuat client TLS.
pub fn http_client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .user_agent(format!("agentgate-cli/{}", agentgate_core::VERSION))
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        .build()?)
}

/// Normalisasi URL server: tanpa `/` di akhir, skema bawaan `https://`.
#[must_use]
pub fn normalize_server(url: &str) -> String {
    let url = url.trim().trim_end_matches('/');
    if url.starts_with("http://") || url.starts_with("https://") {
        url.to_string()
    } else {
        format!("https://{url}")
    }
}

async fn error_from(res: reqwest::Response) -> ApiError {
    let status = res.status();
    let code = res
        .json::<serde_json::Value>()
        .await
        .ok()
        .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_string))
        .unwrap_or_default();
    match status {
        StatusCode::UNAUTHORIZED => ApiError::Unauthorized,
        StatusCode::NOT_FOUND => ApiError::NotFound(code),
        StatusCode::UNPROCESSABLE_ENTITY | StatusCode::BAD_REQUEST => ApiError::Rejected(code),
        s => ApiError::Status {
            status: s.as_u16(),
            code,
        },
    }
}

/// Kirim request; status bukan 2xx → [`ApiError`].
///
/// # Errors
///
/// Lihat [`ApiError`].
pub async fn send(req: RequestBuilder) -> Result<reqwest::Response> {
    let res = req.send().await?;
    if res.status().is_success() {
        Ok(res)
    } else {
        Err(error_from(res).await)
    }
}

/// Body JSON respons.
///
/// # Errors
///
/// Body bukan JSON bentuk `T`.
pub async fn json<T: DeserializeOwned>(res: reqwest::Response) -> Result<T> {
    let bytes = res.bytes().await?;
    serde_json::from_slice(&bytes).map_err(|e| ApiError::Decode(e.to_string()))
}

impl Api {
    #[must_use]
    pub fn new(http: reqwest::Client, base: &str, token: &str) -> Self {
        Self {
            http,
            base: normalize_server(base),
            token: token.to_string(),
            runner: None,
        }
    }

    /// Salinan dengan header runner (endpoint `/api/runner/jobs/…`).
    #[must_use]
    pub fn with_runner(&self, runner: Uuid) -> Self {
        Self {
            runner: Some(runner),
            ..self.clone()
        }
    }

    #[must_use]
    pub fn base(&self) -> &str {
        &self.base
    }

    #[must_use]
    pub fn token(&self) -> &str {
        &self.token
    }

    fn request(&self, method: Method, path: &str) -> RequestBuilder {
        let mut req = self
            .http
            .request(method, format!("{}{path}", self.base))
            .bearer_auth(&self.token);
        if let Some(r) = self.runner {
            req = req.header(RUNNER_HEADER, r.to_string());
        }
        req
    }

    /// `GET` → JSON.
    ///
    /// # Errors
    ///
    /// Lihat [`ApiError`].
    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        json(send(self.request(Method::GET, path)).await?).await
    }

    /// `POST` JSON → JSON.
    ///
    /// # Errors
    ///
    /// Lihat [`ApiError`].
    pub async fn post<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        json(send(self.request(Method::POST, path).json(body)).await?).await
    }

    /// `POST` JSON, isi respons diabaikan (204/201).
    ///
    /// # Errors
    ///
    /// Lihat [`ApiError`].
    pub async fn post_unit<B: Serialize + ?Sized>(&self, path: &str, body: &B) -> Result<()> {
        send(self.request(Method::POST, path).json(body)).await?;
        Ok(())
    }

    /// `POST` tanpa body, isi respons diabaikan.
    ///
    /// # Errors
    ///
    /// Lihat [`ApiError`].
    pub async fn post_empty(&self, path: &str) -> Result<()> {
        send(self.request(Method::POST, path)).await?;
        Ok(())
    }

    /// URL WebSocket (`ws://`/`wss://`) untuk `path`.
    #[must_use]
    pub fn ws_url(&self, path: &str) -> String {
        let base = if let Some(rest) = self.base.strip_prefix("https://") {
            format!("wss://{rest}")
        } else if let Some(rest) = self.base.strip_prefix("http://") {
            format!("ws://{rest}")
        } else {
            self.base.clone()
        };
        format!("{base}{path}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_urls_are_normalized() {
        assert_eq!(normalize_server("ag.example/"), "https://ag.example");
        assert_eq!(
            normalize_server(" http://localhost:5150/ "),
            "http://localhost:5150"
        );
        let http = http_client().unwrap();
        assert_eq!(
            Api::new(http.clone(), "http://x:1", "t").ws_url("/api/runner/ws"),
            "ws://x:1/api/runner/ws"
        );
        assert_eq!(Api::new(http, "https://x", "t").ws_url("/ws"), "wss://x/ws");
    }
}
