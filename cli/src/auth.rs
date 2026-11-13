//! Login CLI: device code flow (RFC 8628) dan refresh token.
//!
//! `POST /oauth/device_authorization` → kode user ditampilkan → user menyetujui di browser →
//! CLI polling `POST /oauth/token` tiap `interval` detik (`slow_down` menambah 5 detik) →
//! token disimpan di `credentials.json` (0600) bersama identitas dari `GET /api/cli/me`.

use std::io::Write;

use chrono::Utc;
use serde::Deserialize;

use crate::{
    api::{self, Api, ApiError, CLIENT_ID},
    config::{self, Credentials, Paths},
    error::{Error, Result},
};

/// Grant device code (RFC 8628 §3.4).
pub const DEVICE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";

#[derive(Debug, Deserialize)]
struct DeviceStart {
    device_code: String,
    user_code: String,
    verification_uri: String,
    #[serde(default)]
    verification_uri_complete: String,
    expires_in: i64,
    interval: i64,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<i64>,
}

/// `GET /api/cli/me`.
#[derive(Debug, Clone, Deserialize)]
pub struct Me {
    #[serde(default)]
    pub user: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub workspace: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub version: String,
}

fn credentials_from(server: &str, t: TokenResponse) -> Credentials {
    Credentials {
        server: server.to_string(),
        access_token: t.access_token,
        refresh_token: t.refresh_token.filter(|r| !r.is_empty()),
        expires_at: t
            .expires_in
            .filter(|s| *s > 0)
            .map(|s| Utc::now() + chrono::Duration::seconds(s)),
        user: String::new(),
        email: String::new(),
        workspace: String::new(),
    }
}

/// Login device flow ke `server`; kredensial disimpan dan dikembalikan.
///
/// # Errors
///
/// Server tidak bisa dihubungi, user menolak, kode kedaluwarsa.
pub async fn login(paths: &Paths, server: &str, out: &mut impl Write) -> Result<Credentials> {
    let server = api::normalize_server(server);
    let http = api::http_client()?;
    let start: DeviceStart = api::json(
        api::send(
            http.post(format!("{server}/oauth/device_authorization"))
                .form(&[
                    ("client_id", CLIENT_ID),
                    ("device_name", &config::device_name()),
                ]),
        )
        .await?,
    )
    .await?;
    let url = if start.verification_uri_complete.is_empty() {
        &start.verification_uri
    } else {
        &start.verification_uri_complete
    };
    let _ = writeln!(
        out,
        "Buka {url}\nlalu masukkan kode: {}\n\nMenunggu persetujuan Anda (berlaku {} menit)…",
        start.user_code,
        (start.expires_in / 60).max(1)
    );
    let _ = out.flush();
    let mut interval = u64::try_from(start.interval.max(1)).unwrap_or(5);
    let deadline = std::time::Instant::now()
        + config::seconds(u64::try_from(start.expires_in.max(1)).unwrap_or(900));
    let token = loop {
        tokio::time::sleep(config::seconds(interval)).await;
        let res = api::send(http.post(format!("{server}/oauth/token")).form(&[
            ("grant_type", DEVICE_GRANT),
            ("client_id", CLIENT_ID),
            ("device_code", start.device_code.as_str()),
        ]))
        .await;
        match res {
            Ok(res) => break api::json::<TokenResponse>(res).await?,
            Err(e) => match e.code() {
                Some("authorization_pending") => {}
                Some("slow_down") => interval += 5,
                Some(code @ ("access_denied" | "expired_token")) => {
                    return Err(Error::Login(code.to_string()));
                }
                _ => return Err(e.into()),
            },
        }
        if std::time::Instant::now() > deadline {
            return Err(Error::Login("expired_token".into()));
        }
    };
    let mut creds = credentials_from(&server, token);
    let me = me(&Api::new(http, &server, &creds.access_token)).await?;
    creds.user = me.user;
    creds.email = me.email;
    creds.workspace = me.workspace;
    creds.save(paths)?;
    Ok(creds)
}

/// # Errors
///
/// Lihat [`ApiError`].
pub async fn me(api: &Api) -> std::result::Result<Me, ApiError> {
    api.get("/api/cli/me").await
}

/// Perbarui token dengan refresh token.
async fn refresh(http: &reqwest::Client, creds: &Credentials) -> Result<Credentials> {
    let Some(rt) = creds.refresh_token.as_deref() else {
        return Err(Error::Api(ApiError::Unauthorized));
    };
    let token: TokenResponse = api::json(
        api::send(http.post(format!("{}/oauth/token", creds.server)).form(&[
            ("grant_type", "refresh_token"),
            ("client_id", CLIENT_ID),
            ("refresh_token", rt),
        ]))
        .await
        .map_err(|e| match e {
            // Refresh token dicabut/kedaluwarsa = harus login ulang.
            ApiError::Rejected(_) => ApiError::Unauthorized,
            other => other,
        })?,
    )
    .await?;
    let mut next = credentials_from(&creds.server, token);
    if next.refresh_token.is_none() {
        next.refresh_token.clone_from(&creds.refresh_token);
    }
    next.user.clone_from(&creds.user);
    next.email.clone_from(&creds.email);
    next.workspace.clone_from(&creds.workspace);
    Ok(next)
}

/// Client API dengan token tersimpan; token yang hampir kedaluwarsa diperbarui dulu.
///
/// # Errors
///
/// Belum login, refresh gagal.
pub async fn session(paths: &Paths) -> Result<(Api, Credentials)> {
    let Some(mut creds) = Credentials::load(paths)? else {
        return Err(Error::NotLoggedIn);
    };
    let http = api::http_client()?;
    if creds.expiring(Utc::now()) && creds.refresh_token.is_some() {
        creds = refresh(&http, &creds).await?;
        creds.save(paths)?;
    }
    Ok((Api::new(http, &creds.server, &creds.access_token), creds))
}

/// Hapus kredensial lokal (token di server dicabut lebih dulu bila bisa).
///
/// # Errors
///
/// Error IO.
pub async fn logout(paths: &Paths) -> Result<bool> {
    if let Some(creds) = Credentials::load(paths)?
        && let Ok(http) = api::http_client()
    {
        // Gagal mencabut (offline) tidak menghalangi logout lokal.
        for token in std::iter::once(&creds.access_token).chain(&creds.refresh_token) {
            let _ = http
                .post(format!("{}/oauth/revoke", creds.server))
                .form(&[("token", token.as_str())])
                .send()
                .await;
        }
    }
    Ok(config::remove(&paths.credentials())?)
}
