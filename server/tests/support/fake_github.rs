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

fn authorized(headers: &HeaderMap) -> bool {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v == format!("Bearer {TOKEN}"))
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
    let authed = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v == format!("Bearer {TOKEN}"));
    if !authed {
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
