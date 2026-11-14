//! GitHub REST palsu untuk tes M6: server Axum di `127.0.0.1:0`, dipasang di `/` (github.com)
//! dan `/api/v3` (GitHub Enterprise Server). Token valid: [`TOKEN`]. Semua permintaan dicatat.

use std::sync::{Arc, Mutex};

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use serde_json::{Value, json};

pub const TOKEN: &str = "ghp_valid_test_token";
pub const LOGIN: &str = "pt-contoh-bot";

/// GitHub App palsu (R2).
pub const APP_ID: &str = "4242";
pub const APP_SLUG: &str = "agentgate-tes";
pub const APP_CLIENT_ID: &str = "Iv1.agentgatetes";
pub const APP_CLIENT_SECRET: &str = "rahasia-klien-app";
pub const APP_WEBHOOK_SECRET: &str = "rahasia-webhook-app";
/// Kunci RSA khusus tes (bukan rahasia); kunci publiknya memverifikasi JWT App.
pub const APP_KEY_PEM: &str = include_str!("github_app_test_key.pem");
/// Kode OAuth callback instalasi yang diterima, dan token user hasil tukarnya.
pub const APP_CODE: &str = "kode-instalasi-ok";
pub const APP_USER_TOKEN: &str = "ghu_user_test_token";
/// Instalasi milik user [`APP_USER_TOKEN`].
pub const INSTALLATION: i64 = 7701;

#[derive(Debug, Clone)]
pub struct Hook {
    pub repo: String,
    pub url: String,
    pub secret: String,
    pub events: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Pull {
    pub repo: String,
    pub number: i64,
    pub title: String,
    pub body: String,
    pub head: String,
    pub base: String,
    pub sha: String,
    pub merged: bool,
}

impl Pull {
    pub fn json(&self) -> Value {
        json!({
            "number": self.number,
            "html_url": format!("https://github.com/{}/pull/{}", self.repo, self.number),
            "title": self.title,
            "state": if self.merged { "closed" } else { "open" },
            "draft": false,
            "merged": self.merged,
            "merged_at": if self.merged { json!("2026-11-13T03:00:00Z") } else { Value::Null },
            "head": { "ref": self.head, "sha": self.sha },
            "base": { "ref": self.base, "sha": "base000" },
            "additions": 42,
            "deletions": 7,
            "user": { "login": LOGIN },
        })
    }
}

#[derive(Default)]
pub struct Inner {
    pub repos: Vec<String>,
    pub hooks: Vec<Hook>,
    pub pulls: Vec<Pull>,
    pub merges: Vec<(String, i64)>,
    /// `merge_method` setiap permintaan merge (urutan sama dengan `merges`).
    pub merge_methods: Vec<String>,
    /// Merge berikutnya ditolak 405.
    pub refuse_merge: bool,
    pub requests: Vec<String>,
    /// Arsip `.tar.gz` per repo (`/repos/{o}/{r}/tarball/{ref}` → redirect → `/codeload/...`).
    pub tarballs: std::collections::HashMap<String, Vec<u8>>,
    /// Repo yang diberi akses ke instalasi App.
    pub app_repos: Vec<String>,
    /// Instalasi App ditangguhkan.
    pub app_suspended: bool,
    /// Jumlah token instalasi yang diterbitkan.
    pub app_tokens_issued: u32,
    /// JWT App yang ditolak (tanda tangan/klaim salah).
    pub app_jwt_rejected: u32,
    /// Ref yang dibuat lewat `POST /git/refs`: (repo, ref) → sha.
    pub refs: Vec<(String, String, String)>,
    /// File yang ditulis lewat Contents API: (repo, branch, path, isi, pesan commit).
    pub files: Vec<(String, String, String, String, String)>,
}

type Shared = Arc<Mutex<Inner>>;

pub struct FakeGithub {
    pub base_url: String,
    pub state: Shared,
}

impl FakeGithub {
    pub fn add_repo(&self, full_name: &str) {
        self.state.lock().unwrap().repos.push(full_name.into());
    }

    /// Isi repo untuk unduhan tarball (index code graph oleh server).
    pub fn set_files(&self, full_name: &str, files: &[(&str, &[u8])]) {
        let top = full_name.replace('/', "-");
        self.state.lock().unwrap().tarballs.insert(
            full_name.into(),
            super::fake_codeload::tarball(&format!("{top}-abc"), files),
        );
    }

    pub fn set_app_repos(&self, repos: &[&str]) {
        self.state.lock().unwrap().app_repos = repos.iter().map(|r| (*r).to_string()).collect();
    }

    pub fn app_tokens_issued(&self) -> u32 {
        self.state.lock().unwrap().app_tokens_issued
    }

    pub fn requests(&self) -> Vec<String> {
        self.state.lock().unwrap().requests.clone()
    }

    /// Ref yang dibuat (repo, ref, sha).
    pub fn refs(&self) -> Vec<(String, String, String)> {
        self.state.lock().unwrap().refs.clone()
    }

    /// File yang ditulis (repo, branch, path, isi, pesan).
    pub fn files(&self) -> Vec<(String, String, String, String, String)> {
        self.state.lock().unwrap().files.clone()
    }

    pub fn hooks(&self) -> Vec<Hook> {
        self.state.lock().unwrap().hooks.clone()
    }

    pub fn pulls(&self) -> Vec<Pull> {
        self.state.lock().unwrap().pulls.clone()
    }

    pub fn merges(&self) -> Vec<(String, i64)> {
        self.state.lock().unwrap().merges.clone()
    }

    #[must_use]
    pub fn merge_methods(&self) -> Vec<String> {
        self.state.lock().unwrap().merge_methods.clone()
    }
}

/// PAT tes atau token instalasi App yang diterbitkan fake ini.
fn authorized(headers: &HeaderMap) -> bool {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .is_some_and(|t| t == TOKEN || t.starts_with(&format!("ghs_inst_{INSTALLATION}_")))
}

fn bearer(headers: &HeaderMap) -> String {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or_default()
        .to_string()
}

/// JWT App valid: RS256 dengan kunci tes, `iss` = App ID, belum kedaluwarsa, umur ≤ 10 menit.
fn app_jwt_ok(token: &str) -> bool {
    use base64::{
        Engine,
        engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
    };
    let mut parts = token.split('.');
    let (Some(h), Some(c), Some(sig), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    let body: String = APP_KEY_PEM
        .lines()
        .filter(|l| !l.starts_with("-----"))
        .collect();
    let der = STANDARD.decode(body).unwrap();
    let pair = ring::signature::RsaKeyPair::from_der(&der).unwrap();
    let public = ring::signature::UnparsedPublicKey::new(
        &ring::signature::RSA_PKCS1_2048_8192_SHA256,
        pair.public().as_ref().to_vec(),
    );
    let Ok(sig) = URL_SAFE_NO_PAD.decode(sig) else {
        return false;
    };
    if public.verify(format!("{h}.{c}").as_bytes(), &sig).is_err() {
        return false;
    }
    let header: Value = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(h).unwrap()).unwrap();
    let claims: Value = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(c).unwrap()).unwrap();
    let now = chrono::Utc::now().timestamp();
    let (iat, exp) = (
        claims["iat"].as_i64().unwrap_or(0),
        claims["exp"].as_i64().unwrap_or(0),
    );
    header["alg"] == "RS256"
        && claims["iss"] == APP_ID
        && iat <= now
        && exp > now
        && exp - iat <= 600
}

fn installation_json(st: &Inner, id: i64) -> Value {
    json!({
        "id": id,
        "account": { "login": "pt-contoh", "type": "Organization" },
        "permissions": { "contents": "write", "pull_requests": "write", "checks": "read", "metadata": "read" },
        "repository_selection": "selected",
        "suspended_at": if st.app_suspended { json!("2026-11-14T00:00:00Z") } else { Value::Null },
    })
}

async fn app_installation(
    State(s): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Response {
    let mut st = s.lock().unwrap();
    st.requests.push(format!("GET /app/installations/{id}"));
    if !app_jwt_ok(&bearer(&headers)) {
        st.app_jwt_rejected += 1;
        return unauthorized();
    }
    if id != INSTALLATION {
        return StatusCode::NOT_FOUND.into_response();
    }
    Json(installation_json(&st, id)).into_response()
}

async fn app_token(State(s): State<Shared>, headers: HeaderMap, Path(id): Path<i64>) -> Response {
    let mut st = s.lock().unwrap();
    st.requests
        .push(format!("POST /app/installations/{id}/access_tokens"));
    if !app_jwt_ok(&bearer(&headers)) {
        st.app_jwt_rejected += 1;
        return unauthorized();
    }
    if id != INSTALLATION {
        return StatusCode::NOT_FOUND.into_response();
    }
    if st.app_suspended {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "message": "suspended" })),
        )
            .into_response();
    }
    st.app_tokens_issued += 1;
    let expires = chrono::Utc::now() + chrono::Duration::hours(1);
    (
        StatusCode::CREATED,
        Json(json!({
            "token": format!("ghs_inst_{id}_{}", st.app_tokens_issued),
            "expires_at": expires.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        })),
    )
        .into_response()
}

async fn installation_repos(State(s): State<Shared>, headers: HeaderMap) -> Response {
    if !authorized(&headers) || bearer(&headers) == TOKEN {
        return unauthorized();
    }
    let st = s.lock().unwrap();
    let repos: Vec<Value> = st
        .app_repos
        .iter()
        .map(|r| json!({ "full_name": r }))
        .collect();
    Json(json!({ "total_count": repos.len(), "repositories": repos })).into_response()
}

async fn user_installations(headers: HeaderMap) -> Response {
    if bearer(&headers) != APP_USER_TOKEN {
        return unauthorized();
    }
    Json(json!({ "total_count": 1, "installations": [ { "id": INSTALLATION } ] })).into_response()
}

async fn oauth_token(
    axum::Form(form): axum::Form<std::collections::HashMap<String, String>>,
) -> Response {
    let ok = form.get("client_id").map(String::as_str) == Some(APP_CLIENT_ID)
        && form.get("client_secret").map(String::as_str) == Some(APP_CLIENT_SECRET)
        && form.get("code").map(String::as_str) == Some(APP_CODE);
    if ok {
        Json(json!({ "access_token": APP_USER_TOKEN, "token_type": "bearer" })).into_response()
    } else {
        // GitHub menjawab 200 dengan field error untuk kode yang salah.
        Json(json!({ "error": "bad_verification_code" })).into_response()
    }
}

fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({ "message": "Bad credentials" })),
    )
        .into_response()
}

fn has_repo(s: &Inner, full: &str) -> bool {
    s.repos.iter().any(|r| r == full)
}

async fn user(State(s): State<Shared>, headers: HeaderMap) -> Response {
    s.lock().unwrap().requests.push("GET /user".into());
    if !authorized(&headers) {
        return unauthorized();
    }
    Json(json!({ "login": LOGIN })).into_response()
}

async fn repo(
    State(s): State<Shared>,
    headers: HeaderMap,
    Path((o, r)): Path<(String, String)>,
) -> Response {
    if !authorized(&headers) {
        return unauthorized();
    }
    let full = format!("{o}/{r}");
    if !has_repo(&s.lock().unwrap(), &full) {
        return StatusCode::NOT_FOUND.into_response();
    }
    Json(json!({
        "full_name": full, "default_branch": "main",
        "html_url": format!("https://github.com/{full}"),
    }))
    .into_response()
}

async fn create_hook(
    State(s): State<Shared>,
    headers: HeaderMap,
    Path((o, r)): Path<(String, String)>,
    Json(body): Json<Value>,
) -> Response {
    if !authorized(&headers) {
        return unauthorized();
    }
    let full = format!("{o}/{r}");
    let mut st = s.lock().unwrap();
    st.requests.push(format!("POST /repos/{full}/hooks"));
    if !has_repo(&st, &full) {
        return StatusCode::NOT_FOUND.into_response();
    }
    st.hooks.push(Hook {
        repo: full,
        url: body["config"]["url"].as_str().unwrap_or_default().into(),
        secret: body["config"]["secret"].as_str().unwrap_or_default().into(),
        events: body["events"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|e| e.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
    });
    let id = 9000 + i64::try_from(st.hooks.len()).unwrap();
    (StatusCode::CREATED, Json(json!({ "id": id }))).into_response()
}

#[derive(serde::Deserialize)]
struct PullQuery {
    #[serde(default)]
    head: String,
}

async fn list_pulls(
    State(s): State<Shared>,
    headers: HeaderMap,
    Path((o, r)): Path<(String, String)>,
    Query(q): Query<PullQuery>,
) -> Response {
    if !authorized(&headers) {
        return unauthorized();
    }
    let full = format!("{o}/{r}");
    let head = q.head.split_once(':').map_or(q.head.as_str(), |(_, b)| b);
    let st = s.lock().unwrap();
    let out: Vec<Value> = st
        .pulls
        .iter()
        .filter(|p| p.repo == full && !p.merged && (head.is_empty() || p.head == head))
        .map(Pull::json)
        .collect();
    Json(out).into_response()
}

async fn create_pull(
    State(s): State<Shared>,
    headers: HeaderMap,
    Path((o, r)): Path<(String, String)>,
    Json(body): Json<Value>,
) -> Response {
    if !authorized(&headers) {
        return unauthorized();
    }
    let full = format!("{o}/{r}");
    let mut st = s.lock().unwrap();
    st.requests.push(format!("POST /repos/{full}/pulls"));
    if !has_repo(&st, &full) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let number = 140 + i64::try_from(st.pulls.len()).unwrap();
    let pull = Pull {
        repo: full,
        number,
        title: body["title"].as_str().unwrap_or_default().into(),
        body: body["body"].as_str().unwrap_or_default().into(),
        head: body["head"].as_str().unwrap_or_default().into(),
        base: body["base"].as_str().unwrap_or_default().into(),
        sha: format!("sha{number}"),
        merged: false,
    };
    let out = pull.json();
    st.pulls.push(pull);
    (StatusCode::CREATED, Json(out)).into_response()
}

async fn get_pull(
    State(s): State<Shared>,
    headers: HeaderMap,
    Path((o, r, n)): Path<(String, String, i64)>,
) -> Response {
    if !authorized(&headers) {
        return unauthorized();
    }
    let full = format!("{o}/{r}");
    let st = s.lock().unwrap();
    match st.pulls.iter().find(|p| p.repo == full && p.number == n) {
        Some(p) => Json(p.json()).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn merge_pull(
    State(s): State<Shared>,
    headers: HeaderMap,
    Path((o, r, n)): Path<(String, String, i64)>,
    Json(body): Json<Value>,
) -> Response {
    if !authorized(&headers) {
        return unauthorized();
    }
    let full = format!("{o}/{r}");
    let mut st = s.lock().unwrap();
    if st.refuse_merge {
        return (
            StatusCode::METHOD_NOT_ALLOWED,
            Json(json!({ "message": "Pull Request is not mergeable" })),
        )
            .into_response();
    }
    let Some(p) = st
        .pulls
        .iter_mut()
        .find(|p| p.repo == full && p.number == n)
    else {
        return StatusCode::NOT_FOUND.into_response();
    };
    p.merged = true;
    st.merges.push((full, n));
    st.merge_methods.push(
        body["merge_method"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
    );
    Json(json!({ "merged": true, "sha": "mergedsha" })).into_response()
}

async fn tarball_redirect(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path((o, r, _git_ref)): Path<(String, String, String)>,
) -> Response {
    let st = state.lock().unwrap();
    if !authorized(&headers) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if !st.tarballs.contains_key(&format!("{o}/{r}")) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let host = headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    (
        StatusCode::FOUND,
        [("location", format!("http://{host}/codeload/{o}/{r}"))],
    )
        .into_response()
}

async fn codeload(State(state): State<Shared>, Path((o, r)): Path<(String, String)>) -> Response {
    match state.lock().unwrap().tarballs.get(&format!("{o}/{r}")) {
        Some(b) => (StatusCode::OK, b.clone()).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// HEAD branch: `head-<branch>` untuk branch utama, atau ref yang sudah dibuat.
async fn get_branch_ref(
    State(s): State<Shared>,
    headers: HeaderMap,
    Path((o, r, branch)): Path<(String, String, String)>,
) -> Response {
    if !authorized(&headers) {
        return unauthorized();
    }
    let full = format!("{o}/{r}");
    let st = s.lock().unwrap();
    if !has_repo(&st, &full) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let name = format!("refs/heads/{branch}");
    let sha = st
        .refs
        .iter()
        .find(|(repo, n, _)| *repo == full && *n == name)
        .map(|(_, _, sha)| sha.clone())
        .or_else(|| {
            (branch == "main").then(|| "a1b2c3d4e5f60718293a4b5c6d7e8f9012345678".to_string())
        });
    match sha {
        Some(sha) => {
            Json(json!({ "ref": name, "object": { "sha": sha, "type": "commit" } })).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn create_git_ref(
    State(s): State<Shared>,
    headers: HeaderMap,
    Path((o, r)): Path<(String, String)>,
    Json(body): Json<Value>,
) -> Response {
    if !authorized(&headers) {
        return unauthorized();
    }
    let full = format!("{o}/{r}");
    let mut st = s.lock().unwrap();
    st.requests.push(format!("POST /repos/{full}/git/refs"));
    if !has_repo(&st, &full) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let name = body["ref"].as_str().unwrap_or_default().to_string();
    let sha = body["sha"].as_str().unwrap_or_default().to_string();
    if st
        .refs
        .iter()
        .any(|(repo, n, _)| *repo == full && *n == name)
    {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "message": "Reference already exists" })),
        )
            .into_response();
    }
    st.refs.push((full, name.clone(), sha.clone()));
    (
        StatusCode::CREATED,
        Json(json!({ "ref": name, "object": { "sha": sha } })),
    )
        .into_response()
}

#[derive(serde::Deserialize)]
struct RefQuery {
    #[serde(default, rename = "ref")]
    git_ref: String,
}

async fn get_contents(
    State(s): State<Shared>,
    headers: HeaderMap,
    Path((o, r, path)): Path<(String, String, String)>,
    Query(q): Query<RefQuery>,
) -> Response {
    if !authorized(&headers) {
        return unauthorized();
    }
    let full = format!("{o}/{r}");
    let st = s.lock().unwrap();
    match st
        .files
        .iter()
        .rev()
        .find(|(repo, b, p, ..)| *repo == full && *b == q.git_ref && *p == path)
    {
        Some((.., content, _)) => {
            Json(json!({ "path": path, "sha": format!("blob{}", content.len()) })).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn put_contents(
    State(s): State<Shared>,
    headers: HeaderMap,
    Path((o, r, path)): Path<(String, String, String)>,
    Json(body): Json<Value>,
) -> Response {
    use base64::Engine;
    if !authorized(&headers) {
        return unauthorized();
    }
    let full = format!("{o}/{r}");
    let mut st = s.lock().unwrap();
    st.requests
        .push(format!("PUT /repos/{full}/contents/{path}"));
    let branch = body["branch"].as_str().unwrap_or_default().to_string();
    let content = base64::engine::general_purpose::STANDARD
        .decode(body["content"].as_str().unwrap_or_default())
        .map(|b| String::from_utf8_lossy(&b).to_string())
        .unwrap_or_default();
    let exists = st
        .files
        .iter()
        .rev()
        .find(|(repo, b, p, ..)| *repo == full && *b == branch && *p == path)
        .map(|(.., c, _)| format!("blob{}", c.len()));
    // Seperti GitHub: menimpa file butuh sha blob lama yang cocok.
    if exists.as_deref() != body["sha"].as_str() {
        return (
            StatusCode::CONFLICT,
            Json(json!({ "message": "sha does not match" })),
        )
            .into_response();
    }
    let message = body["message"].as_str().unwrap_or_default().to_string();
    st.files.push((full, branch, path, content, message));
    (
        StatusCode::CREATED,
        Json(json!({ "content": {}, "commit": { "sha": "c0ffee" } })),
    )
        .into_response()
}

fn api(state: Shared) -> Router {
    Router::new()
        .route("/user", get(user))
        .route("/repos/{o}/{r}", get(repo))
        .route("/repos/{o}/{r}/hooks", post(create_hook))
        .route("/repos/{o}/{r}/pulls", get(list_pulls).post(create_pull))
        .route("/repos/{o}/{r}/pulls/{n}", get(get_pull))
        .route("/repos/{o}/{r}/pulls/{n}/merge", put(merge_pull))
        .route("/repos/{o}/{r}/tarball/{git_ref}", get(tarball_redirect))
        .route("/codeload/{o}/{r}", get(codeload))
        .route(
            "/repos/{o}/{r}/git/ref/heads/{*branch}",
            get(get_branch_ref),
        )
        .route("/repos/{o}/{r}/git/refs", post(create_git_ref))
        .route(
            "/repos/{o}/{r}/contents/{*path}",
            get(get_contents).put(put_contents),
        )
        .route("/app/installations/{id}", get(app_installation))
        .route("/app/installations/{id}/access_tokens", post(app_token))
        .route("/installation/repositories", get(installation_repos))
        .route("/user/installations", get(user_installations))
        .route("/login/oauth/access_token", post(oauth_token))
        .with_state(state)
}

pub async fn start() -> FakeGithub {
    let state: Shared = Arc::default();
    let app = api(state.clone()).nest("/api/v3", api(state.clone()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    FakeGithub {
        base_url: format!("http://{addr}"),
        state,
    }
}
