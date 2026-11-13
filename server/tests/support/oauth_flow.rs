//! Alur OAuth 2.1 lengkap untuk tes (DCR → consent → token), dipakai tes OAuth dan MCP.

use base64::Engine;
use loco_rs::{TestServer, auth::cookies::Cookie};
use sha2::{Digest, Sha256};

pub const REDIRECT: &str = "http://127.0.0.1:53682/callback";
pub const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";

#[must_use]
pub fn challenge(verifier: &str) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// Daftarkan client publik (DCR), kembalikan `client_id`.
pub async fn register_client(request: &TestServer, name: &str) -> String {
    let res = request
        .post("/oauth/register")
        .json(&serde_json::json!({
            "client_name": name,
            "redirect_uris": [REDIRECT],
            "token_endpoint_auth_method": "none",
            "grant_types": ["authorization_code", "refresh_token"],
        }))
        .await;
    assert_eq!(res.status_code(), 201, "{}", res.text());
    res.json::<serde_json::Value>()["client_id"]
        .as_str()
        .unwrap()
        .to_string()
}

/// Parameter otorisasi standar (PKCE S256 dari [`VERIFIER`]).
#[must_use]
pub fn authorize_params(client_id: &str) -> Vec<(&'static str, String)> {
    vec![
        ("response_type", "code".into()),
        ("client_id", client_id.into()),
        ("redirect_uri", REDIRECT.into()),
        ("code_challenge", challenge(VERIFIER)),
        ("code_challenge_method", "S256".into()),
        ("state", "xyz".into()),
        ("scope", "mcp".into()),
    ]
}

/// Setujui di halaman consent untuk workspace `workspace_id`, kembalikan kode otorisasi.
pub async fn approve(
    request: &TestServer,
    cookie: &Cookie<'static>,
    client_id: &str,
    workspace_id: &str,
) -> String {
    let mut form = authorize_params(client_id);
    form.push(("decision", "allow".into()));
    form.push(("workspace", workspace_id.into()));
    let res = request
        .post("/oauth/authorize")
        .add_cookie(cookie.clone())
        .form(&form)
        .await;
    assert_eq!(res.status_code(), 303, "{}", res.text());
    let location = res.header("location").to_str().unwrap().to_string();
    assert!(location.starts_with(REDIRECT), "{location}");
    assert!(location.contains("state=xyz"), "{location}");
    query_param(&location, "code").expect("code")
}

/// Tukar kode → token (JSON respons).
pub async fn exchange(request: &TestServer, client_id: &str, code: &str) -> serde_json::Value {
    let res = request
        .post("/oauth/token")
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", REDIRECT),
            ("client_id", client_id),
            ("code_verifier", VERIFIER),
        ])
        .await;
    assert_eq!(res.status_code(), 200, "{}", res.text());
    res.json()
}

/// Alur lengkap: client baru → consent → token. Kembalikan (client_id, access_token, refresh).
pub async fn connect(
    request: &TestServer,
    cookie: &Cookie<'static>,
    workspace_id: &str,
) -> (String, String, String) {
    let client_id = register_client(request, "Claude Code").await;
    let code = approve(request, cookie, &client_id, workspace_id).await;
    let t = exchange(request, &client_id, &code).await;
    (
        client_id,
        t["access_token"].as_str().unwrap().to_string(),
        t["refresh_token"].as_str().unwrap().to_string(),
    )
}

#[must_use]
pub fn query_param(url: &str, name: &str) -> Option<String> {
    let query = url.split_once('?')?.1;
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k == name).then(|| percent_decode(v))
    })
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16)
        {
            out.push(b);
            i += 3;
            continue;
        }
        out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Login CLI lewat device code flow untuk user `cookie` di workspace `workspace_id`.
/// Kembalikan access token.
pub async fn device_login(
    request: &TestServer,
    cookie: &Cookie<'static>,
    workspace_id: &str,
) -> String {
    let start = request
        .post("/oauth/device_authorization")
        .form(&[
            ("client_id", "agentgate-cli"),
            ("device_name", "Laptop Uji"),
        ])
        .await;
    assert_eq!(start.status_code(), 200, "{}", start.text());
    let start: serde_json::Value = start.json();
    let res = request
        .post("/device/decide")
        .add_cookie(cookie.clone())
        .form(&[
            ("user_code", start["user_code"].as_str().unwrap()),
            ("workspace", workspace_id),
            ("decision", "allow"),
        ])
        .await;
    assert_eq!(res.status_code(), 303, "{}", res.text());
    let token = request
        .post("/oauth/token")
        .form(&[
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("client_id", "agentgate-cli"),
            ("device_code", start["device_code"].as_str().unwrap()),
        ])
        .await;
    assert_eq!(token.status_code(), 200, "{}", token.text());
    token.json::<serde_json::Value>()["access_token"]
        .as_str()
        .unwrap()
        .to_string()
}
