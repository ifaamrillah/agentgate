//! GitHub App (R2): cara utama menghubungkan github.com. PAT tetap ada sebagai cadangan (GHES,
//! uji).
//!
//! - JWT App RS256 (`iss` = App ID, umur 9 menit, `iat` mundur 60 detik untuk selisih jam)
//!   ditandatangani kunci privat App dari config ([`app_jwt`]).
//! - Token instalasi (umur 1 jam) diminta dengan JWT dan di-cache per proses sampai 5 menit
//!   sebelum kedaluwarsa ([`installation_token`]). Token tidak pernah disimpan di database/log.
//! - Instalasi baru diverifikasi lewat OAuth user-to-server: `code` dari callback ditukar menjadi
//!   token user, lalu instalasi harus muncul di `GET /user/installations` user itu. Tanpa ini,
//!   siapa pun bisa menautkan instalasi orang lain hanya dengan menebak `installation_id`.
//! - State instalasi = payload bertanda tangan HMAC (workspace, user, kedaluwarsa 15 menit).
use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
    time::Duration,
};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::Sha256;
use uuid::Uuid;

use super::provider::ProviderError;
use crate::common::{crypto, settings::GithubAppSettings};

/// Umur state instalasi.
pub const STATE_MINUTES: i64 = 15;
/// Token instalasi dipakai sampai sisa umurnya kurang dari ini.
pub const TOKEN_MARGIN_SECONDS: i64 = 300;
/// Halaman daftar repo instalasi yang dibaca (100 per halaman).
const REPO_PAGES: u32 = 20;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("github app is not configured")]
    NotConfigured,
    #[error("github app private key is invalid")]
    BadKey,
    #[error(transparent)]
    Provider(#[from] ProviderError),
}

/// DER kunci privat dari PEM (PKCS#1 `RSA PRIVATE KEY` dari GitHub, atau PKCS#8).
fn key_pair(pem: &str) -> Result<ring::signature::RsaKeyPair, AppError> {
    let pem = pem.trim().replace("\\n", "\n");
    let pkcs1 = pem.contains("BEGIN RSA PRIVATE KEY");
    let body: String = pem
        .lines()
        .filter(|l| !l.starts_with("-----"))
        .map(str::trim)
        .collect();
    let der = base64::engine::general_purpose::STANDARD
        .decode(body)
        .map_err(|_| AppError::BadKey)?;
    if pkcs1 {
        ring::signature::RsaKeyPair::from_der(&der).map_err(|_| AppError::BadKey)
    } else {
        ring::signature::RsaKeyPair::from_pkcs8(&der).map_err(|_| AppError::BadKey)
    }
}

/// Kunci privat App bisa dibaca (dipakai validasi boot dan halaman Status sistem).
#[must_use]
pub fn key_ok(app: &GithubAppSettings) -> bool {
    key_pair(&app.private_key).is_ok()
}

/// JWT App (RS256) pada waktu `now` (detik Unix).
///
/// # Errors
///
/// App belum dikonfigurasi, kunci tidak valid.
pub fn app_jwt(app: &GithubAppSettings, now: i64) -> Result<String, AppError> {
    if app.app_id.trim().is_empty() || app.private_key.trim().is_empty() {
        return Err(AppError::NotConfigured);
    }
    let key = key_pair(&app.private_key)?;
    let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"RS256","typ":"JWT"}"#);
    let claims = URL_SAFE_NO_PAD.encode(
        serde_json::json!({ "iat": now - 60, "exp": now + 540, "iss": app.app_id.trim() })
            .to_string(),
    );
    let message = format!("{header}.{claims}");
    let mut sig = vec![0u8; key.public().modulus_len()];
    key.sign(
        &ring::signature::RSA_PKCS1_SHA256,
        &ring::rand::SystemRandom::new(),
        message.as_bytes(),
        &mut sig,
    )
    .map_err(|_| AppError::BadKey)?;
    Ok(format!("{message}.{}", URL_SAFE_NO_PAD.encode(sig)))
}

// ---------------------------------------------------------------------------------------------
// State instalasi
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, Deserialize)]
pub struct InstallState {
    /// Workspace tujuan.
    pub w: Uuid,
    /// User yang memulai.
    pub u: Uuid,
    /// Kedaluwarsa (detik Unix).
    pub e: i64,
}

fn mac(key: &[u8; 32], payload: &str) -> Vec<u8> {
    // HMAC menerima kunci sepanjang apa pun; cabang `Err` tidak terjadi.
    let Ok(mut m) = Hmac::<Sha256>::new_from_slice(key) else {
        return Vec::new();
    };
    m.update(b"github-app-state:");
    m.update(payload.as_bytes());
    m.finalize().into_bytes().to_vec()
}

/// State bertanda tangan untuk URL instalasi.
#[must_use]
pub fn sign_state(key: &[u8; 32], workspace: Uuid, user: Uuid, now: i64) -> String {
    let payload = URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(&InstallState {
            w: workspace,
            u: user,
            e: now + STATE_MINUTES * 60,
        })
        .unwrap_or_default(),
    );
    format!("{payload}.{}", URL_SAFE_NO_PAD.encode(mac(key, &payload)))
}

/// Buka state; `None` bila tanda tangan salah, rusak, atau kedaluwarsa.
#[must_use]
pub fn verify_state(key: &[u8; 32], state: &str, now: i64) -> Option<InstallState> {
    let (payload, sig) = state.split_once('.')?;
    let sig = URL_SAFE_NO_PAD.decode(sig).ok()?;
    if !crypto::ct_eq(&mac(key, payload), &sig) {
        return None;
    }
    let s: InstallState = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload).ok()?).ok()?;
    (s.e >= now).then_some(s)
}

/// URL instalasi App di GitHub.
#[must_use]
pub fn install_url(web_base: &str, app: &GithubAppSettings, state: &str) -> String {
    format!(
        "{}/apps/{}/installations/new?state={state}",
        web_base.trim_end_matches('/'),
        app.slug.trim()
    )
}

// ---------------------------------------------------------------------------------------------
// Klien API App
// ---------------------------------------------------------------------------------------------

/// Instalasi menurut GitHub.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installation {
    pub id: i64,
    pub account: String,
    /// `Organization` atau `User`.
    pub account_type: String,
    pub permissions: serde_json::Value,
    /// `all` atau `selected`.
    pub repository_selection: String,
    pub suspended: bool,
}

#[derive(Deserialize)]
struct GhAccount {
    login: String,
    #[serde(default, rename = "type")]
    kind: String,
}

#[derive(Deserialize)]
struct GhInstallation {
    id: i64,
    account: GhAccount,
    #[serde(default)]
    permissions: serde_json::Value,
    #[serde(default)]
    repository_selection: String,
    #[serde(default)]
    suspended_at: Option<String>,
}

impl From<GhInstallation> for Installation {
    fn from(i: GhInstallation) -> Self {
        Self {
            id: i.id,
            account: i.account.login,
            account_type: i.account.kind,
            permissions: i.permissions,
            repository_selection: i.repository_selection,
            suspended: i.suspended_at.is_some(),
        }
    }
}

/// Bentuk instalasi di payload webhook.
///
/// # Errors
///
/// JSON tidak dikenal.
pub fn installation_from_json(v: &serde_json::Value) -> Result<Installation, ProviderError> {
    serde_json::from_value::<GhInstallation>(v.clone())
        .map(Installation::from)
        .map_err(|e| ProviderError::Decode(e.to_string()))
}

#[derive(Deserialize)]
struct GhToken {
    token: String,
    expires_at: DateTime<Utc>,
}

fn http(timeout: Duration) -> Result<reqwest::Client, ProviderError> {
    reqwest::Client::builder()
        .timeout(timeout)
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("agentgate/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| ProviderError::Network(e.to_string()))
}

async fn send<T: serde::de::DeserializeOwned>(
    req: reqwest::RequestBuilder,
) -> Result<T, ProviderError> {
    let res = req
        .header("accept", "application/vnd.github+json")
        .header("x-github-api-version", "2022-11-28")
        .send()
        .await
        .map_err(|e| ProviderError::Network(e.without_url().to_string()))?;
    let status = res.status();
    if status.is_success() {
        return res
            .json::<T>()
            .await
            .map_err(|e| ProviderError::Decode(e.without_url().to_string()));
    }
    Err(match status.as_u16() {
        401 | 403 => ProviderError::Unauthorized,
        404 => ProviderError::NotFound,
        s => ProviderError::Rejected(format!("HTTP {s}")),
    })
}

/// Klien endpoint App (`/app/…`, `/installation/…`, OAuth user).
pub struct AppClient<'a> {
    pub api_base: &'a str,
    pub web_base: &'a str,
    pub app: &'a GithubAppSettings,
    pub timeout: Duration,
}

impl AppClient<'_> {
    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.api_base.trim_end_matches('/'))
    }

    fn jwt(&self) -> Result<String, AppError> {
        app_jwt(self.app, Utc::now().timestamp())
    }

    /// Instalasi `id` (dengan JWT App).
    ///
    /// # Errors
    ///
    /// [`AppError`].
    pub async fn installation(&self, id: i64) -> Result<Installation, AppError> {
        let inst: GhInstallation = send(
            http(self.timeout)?
                .get(self.url(&format!("/app/installations/{id}")))
                .bearer_auth(self.jwt()?),
        )
        .await?;
        Ok(inst.into())
    }

    async fn new_token(&self, id: i64) -> Result<GhToken, AppError> {
        Ok(send(
            http(self.timeout)?
                .post(self.url(&format!("/app/installations/{id}/access_tokens")))
                .bearer_auth(self.jwt()?),
        )
        .await?)
    }

    /// Repo yang dapat diakses instalasi (`owner/repo`), dengan token instalasi.
    ///
    /// # Errors
    ///
    /// [`ProviderError`].
    pub async fn repositories(&self, token: &str) -> Result<Vec<String>, ProviderError> {
        #[derive(Deserialize)]
        struct Repo {
            full_name: String,
        }
        #[derive(Deserialize)]
        struct Page {
            #[serde(default)]
            total_count: u32,
            repositories: Vec<Repo>,
        }
        let client = http(self.timeout)?;
        let mut out = Vec::new();
        for page in 1..=REPO_PAGES {
            let p: Page = send(
                client
                    .get(self.url("/installation/repositories"))
                    .query(&[("per_page", "100"), ("page", &page.to_string())])
                    .bearer_auth(token),
            )
            .await?;
            let n = p.repositories.len();
            out.extend(p.repositories.into_iter().map(|r| r.full_name));
            if n < 100 || out.len() >= p.total_count as usize {
                break;
            }
        }
        out.sort();
        out.dedup();
        Ok(out)
    }

    /// Tukar `code` OAuth callback instalasi menjadi token user (tidak disimpan).
    ///
    /// # Errors
    ///
    /// `Unauthorized` bila kode ditolak.
    pub async fn exchange_code(&self, code: &str) -> Result<String, AppError> {
        #[derive(Deserialize)]
        struct Res {
            #[serde(default)]
            access_token: String,
        }
        let res: Res = send(
            http(self.timeout)?
                .post(format!(
                    "{}/login/oauth/access_token",
                    self.web_base.trim_end_matches('/')
                ))
                .header("accept", "application/json")
                .form(&[
                    ("client_id", self.app.client_id.trim()),
                    ("client_secret", self.app.client_secret.trim()),
                    ("code", code),
                ]),
        )
        .await?;
        if res.access_token.is_empty() {
            return Err(ProviderError::Unauthorized.into());
        }
        Ok(res.access_token)
    }

    /// Id instalasi App yang dapat diakses user pemilik `user_token`.
    ///
    /// # Errors
    ///
    /// [`ProviderError`].
    pub async fn user_installations(&self, user_token: &str) -> Result<Vec<i64>, ProviderError> {
        #[derive(Deserialize)]
        struct Item {
            id: i64,
        }
        #[derive(Deserialize)]
        struct Res {
            installations: Vec<Item>,
        }
        let res: Res = send(
            http(self.timeout)?
                .get(self.url("/user/installations"))
                .query(&[("per_page", "100")])
                .bearer_auth(user_token),
        )
        .await?;
        Ok(res.installations.into_iter().map(|i| i.id).collect())
    }
}

// ---------------------------------------------------------------------------------------------
// Cache token instalasi
// ---------------------------------------------------------------------------------------------

type TokenCache = HashMap<(String, i64), (String, DateTime<Utc>)>;

static TOKENS: LazyLock<Mutex<TokenCache>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn cached(api_base: &str, id: i64, now: DateTime<Utc>) -> Option<String> {
    let cache = TOKENS.lock().ok()?;
    cache
        .get(&(api_base.to_string(), id))
        .filter(|(_, exp)| (*exp - now).num_seconds() > TOKEN_MARGIN_SECONDS)
        .map(|(t, _)| t.clone())
}

/// Lupakan token instalasi (instalasi dihapus/ditangguhkan, atau token ditolak).
pub fn forget(api_base: &str, id: i64) {
    if let Ok(mut cache) = TOKENS.lock() {
        cache.remove(&(api_base.to_string(), id));
    }
}

/// Token instalasi dari cache, atau minta baru dengan JWT App.
///
/// # Errors
///
/// [`AppError`].
pub async fn installation_token(client: &AppClient<'_>, id: i64) -> Result<String, AppError> {
    let now = Utc::now();
    if let Some(t) = cached(client.api_base, id, now) {
        return Ok(t);
    }
    let fresh = client.new_token(id).await?;
    if let Ok(mut cache) = TOKENS.lock() {
        // Cache kecil (satu baris per instalasi aktif); buang yang sudah lewat supaya tidak
        // tumbuh tanpa batas.
        cache.retain(|_, (_, exp)| *exp > now);
        cache.insert(
            (client.api_base.to_string(), id),
            (fresh.token.clone(), fresh.expires_at),
        );
    }
    Ok(fresh.token)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn state_round_trip_tamper_and_expiry() {
        let key = [7u8; 32];
        let (w, u) = (Uuid::now_v7(), Uuid::now_v7());
        let s = sign_state(&key, w, u, 1_000);
        let st = verify_state(&key, &s, 1_000).unwrap();
        assert_eq!((st.w, st.u), (w, u));
        assert!(verify_state(&[8u8; 32], &s, 1_000).is_none());
        assert!(verify_state(&key, &s, 1_000 + STATE_MINUTES * 60 + 1).is_none());
        let (p, sig) = s.split_once('.').unwrap();
        assert!(verify_state(&key, &format!("{p}x.{sig}"), 1_000).is_none());
        assert!(verify_state(&key, "rusak", 1_000).is_none());
    }

    #[test]
    fn jwt_requires_configuration_and_valid_key() {
        let mut app = GithubAppSettings::default();
        assert!(matches!(app_jwt(&app, 0), Err(AppError::NotConfigured)));
        app.app_id = "1".into();
        app.private_key =
            "-----BEGIN RSA PRIVATE KEY-----\nbukan\n-----END RSA PRIVATE KEY-----".into();
        assert!(matches!(app_jwt(&app, 0), Err(AppError::BadKey)));
    }
}
