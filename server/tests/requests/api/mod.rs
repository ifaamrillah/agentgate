//! M11 API & webhook: REST `/api/v1` dengan token API (`rest`), halaman Pengaturan → API &
//! webhook (`ui`), dan pengiriman webhook ke penerima palsu lokal (`delivery`).

mod delivery;
mod rest;
mod ui;

use axum_test::TestResponse;
use loco_rs::{TestServer, auth::cookies::Cookie};
use serde_json::Value;

/// Buat token lewat dialog Token baru dan ambil token mentah dari respons (tampil sekali).
pub async fn new_token(
    request: &TestServer,
    cookie: &Cookie<'static>,
    scopes: &[&str],
    project: &str,
) -> String {
    let mut form: Vec<(&str, &str)> = vec![("name", "Integrasi uji"), ("expiry", "90")];
    form.extend(scopes.iter().map(|s| ("scopes", *s)));
    form.push(("project", project));
    let res = request
        .post("/w/acme/settings/api/tokens")
        .add_cookie(cookie.clone())
        .form(&form)
        .await;
    assert_eq!(res.status_code(), 200, "{}", res.text());
    raw_secret(&res.text(), "ag_pat_")
}

/// Nilai rahasia terpanjang berawalan `prefix` di HTML (token/secret mentah, bukan prefiks
/// tampilan `ag_pat_xxxx••••`).
pub fn raw_secret(html: &str, prefix: &str) -> String {
    html.match_indices(prefix)
        .map(|(i, _)| {
            html[i..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                .collect::<String>()
        })
        .max_by_key(String::len)
        .filter(|s| s.len() > prefix.len() + 20)
        .unwrap_or_else(|| panic!("{prefix}… tidak ada di respons"))
}

pub async fn get(request: &TestServer, token: &str, path: &str) -> TestResponse {
    request
        .get(path)
        .add_header("authorization", format!("Bearer {token}"))
        .await
}

pub async fn send(
    request: &TestServer,
    token: &str,
    method: axum::http::Method,
    path: &str,
    body: &Value,
) -> TestResponse {
    request
        .method(method, path)
        .add_header("authorization", format!("Bearer {token}"))
        .json(body)
        .await
}

pub fn json(res: &TestResponse) -> Value {
    serde_json::from_str(&res.text()).unwrap_or_else(|_| panic!("bukan JSON: {}", res.text()))
}
