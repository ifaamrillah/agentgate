//! Provider OAuth palsu untuk tes request: server Axum kecil di `127.0.0.1:0` yang dijalankan
//! di dalam tes (loopback, bukan jaringan). Melayani authorize (redirect balik dengan `code` dan
//! `state`), token, userinfo, dan emails (gaya GitHub). Setiap tes memilih profil yang
//! dikembalikan lewat [`FakeProvider::set_identity`].

use std::sync::{Arc, Mutex};

use axum::{
    Json, Router,
    extract::{Query, State},
    response::{IntoResponse, Redirect},
    routing::{get, post},
};
use loco_rs::config::{OAuthProviderConfig, OAuthStrategy};
use serde::Deserialize;
use serde_json::{Value, json};

/// Satu-satunya code yang diterima endpoint token.
pub const CODE: &str = "fake-code";

#[derive(Default)]
struct Shared {
    identity: Value,
    emails: Option<Value>,
}

/// Provider palsu yang sedang berjalan. Server berhenti saat runtime tes selesai.
pub struct FakeProvider {
    pub base_url: String,
    shared: Arc<Mutex<Shared>>,
}

impl FakeProvider {
    /// Profil yang dikembalikan endpoint userinfo (bentuk Google: `sub`, `email`,
    /// `email_verified`; bentuk GitHub: `id`, `login`, `email`).
    pub fn set_identity(&self, profile: Value) {
        self.shared.lock().unwrap().identity = profile;
    }

    /// Daftar `/user/emails` gaya GitHub.
    pub fn set_emails(&self, emails: Value) {
        self.shared.lock().unwrap().emails = Some(emails);
    }

    /// Config provider dengan preset `strategy` dan semua endpoint menunjuk server ini.
    #[must_use]
    pub fn provider_config(&self, strategy: OAuthStrategy) -> OAuthProviderConfig {
        let url = |path: &str| Some(format!("{}{path}", self.base_url));
        OAuthProviderConfig {
            strategy: Some(strategy),
            client_id: "fake-client-id".to_string(),
            client_secret: "fake-client-secret".to_string(),
            authorize_url: url("/authorize"),
            token_url: url("/token"),
            userinfo_url: url("/userinfo"),
            emails_url: url("/emails"),
            ..Default::default()
        }
    }
}

/// Jalankan provider palsu di port acak.
pub async fn start() -> FakeProvider {
    let shared = Arc::new(Mutex::new(Shared::default()));
    let app = Router::new()
        .route("/authorize", get(authorize))
        .route("/token", post(token))
        .route("/userinfo", get(userinfo))
        .route("/emails", get(emails))
        .with_state(shared.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    FakeProvider {
        base_url: format!("http://{addr}"),
        shared,
    }
}

#[derive(Deserialize)]
struct AuthorizeParams {
    redirect_uri: String,
    state: String,
}

/// User langsung "menyetujui": kembali ke `redirect_uri` dengan code tetap.
async fn authorize(Query(p): Query<AuthorizeParams>) -> Redirect {
    let mut url = reqwest::Url::parse(&p.redirect_uri).unwrap();
    url.query_pairs_mut()
        .append_pair("code", CODE)
        .append_pair("state", &p.state);
    Redirect::to(url.as_str())
}

async fn token(body: String) -> impl IntoResponse {
    let ok = url_form_has(&body, "code", CODE);
    if ok {
        Json(json!({ "access_token": "fake-access-token", "token_type": "bearer" }))
    } else {
        Json(json!({ "error": "invalid_grant" }))
    }
}

fn url_form_has(body: &str, key: &str, value: &str) -> bool {
    body.split('&').any(|pair| pair == format!("{key}={value}"))
}

async fn userinfo(State(shared): State<Arc<Mutex<Shared>>>) -> Json<Value> {
    Json(shared.lock().unwrap().identity.clone())
}

async fn emails(State(shared): State<Arc<Mutex<Shared>>>) -> impl IntoResponse {
    match shared.lock().unwrap().emails.clone() {
        Some(list) => Json(list).into_response(),
        None => axum::http::StatusCode::NOT_FOUND.into_response(),
    }
}
