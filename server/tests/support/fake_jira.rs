//! Jira palsu untuk tes M11: server Axum di `127.0.0.1:0` yang melayani REST v3 (Cloud) dan v2
//! (Data Center) dengan state di memori: project `SR`, status To Do / In Progress / Done,
//! field standar + Story Points + Sprint, user, issue (field, status, komentar, lampiran).
//! Kredensial valid: [`EMAIL`] + [`TOKEN`] (Basic) atau Bearer [`TOKEN`]. Semua permintaan
//! dicatat. JQL yang dipahami: `project = X` dan `updated >= -Nm`. Tanpa jaringan nyata.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use axum::{
    Json, Router,
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::get,
};
use base64::Engine;
use chrono::{DateTime, Duration, Utc};
use serde_json::{Value, json};

pub const EMAIL: &str = "olivia@sevima-retail.id";
pub const TOKEN: &str = "jira_api_token_valid";
pub const ACCOUNT: &str = "acc-olivia";

pub const TODO: &str = "10000";
pub const IN_PROGRESS: &str = "10001";
pub const DONE: &str = "10002";

#[derive(Debug, Clone)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub body: Value,
}

#[derive(Debug, Clone)]
pub struct Issue {
    pub id: String,
    pub key: String,
    pub fields: serde_json::Map<String, Value>,
    pub status_id: String,
    pub updated: DateTime<Utc>,
    pub comments: Vec<Value>,
}

pub struct Inner {
    pub requests: Vec<Request>,
    pub statuses: Vec<(String, String, String)>,
    pub users: Vec<(String, String, String)>,
    pub issues: Vec<Issue>,
    pub files: HashMap<String, Vec<u8>>,
    pub next_id: u64,
    pub next_comment: u64,
    pub base: String,
}

type Shared = Arc<Mutex<Inner>>;

pub struct FakeJira {
    /// `http://127.0.0.1:<port>`.
    pub url: String,
    state: Shared,
}

fn stamp(t: DateTime<Utc>) -> String {
    t.format("%Y-%m-%dT%H:%M:%S%.3f+0000").to_string()
}

impl Inner {
    fn status(&self, id: &str) -> Value {
        let (id, name, cat) = self
            .statuses
            .iter()
            .find(|s| s.0 == id)
            .cloned()
            .unwrap_or_else(|| (id.to_string(), id.to_string(), "new".into()));
        json!({ "id": id, "name": name, "statusCategory": { "key": cat } })
    }

    fn issue_json(&self, i: &Issue) -> Value {
        let mut fields = i.fields.clone();
        fields.insert("status".into(), self.status(&i.status_id));
        fields.insert("updated".into(), json!(stamp(i.updated)));
        json!({ "id": i.id, "key": i.key, "fields": fields })
    }

    fn find(&mut self, key: &str) -> Option<&mut Issue> {
        self.issues.iter_mut().find(|i| i.key == key || i.id == key)
    }

    fn user(&self, account: &str, v3: bool) -> Value {
        let (acc, name, email) = self
            .users
            .iter()
            .find(|u| u.0 == account)
            .cloned()
            .unwrap_or_else(|| (account.to_string(), account.to_string(), String::new()));
        if v3 {
            json!({ "accountId": acc, "displayName": name, "emailAddress": email })
        } else {
            json!({ "name": acc, "key": acc, "displayName": name, "emailAddress": email })
        }
    }
}

impl FakeJira {
    pub fn requests(&self) -> Vec<Request> {
        self.state.lock().unwrap().requests.clone()
    }

    /// Permintaan yang mengubah data (POST/PUT, tanpa pencarian).
    pub fn writes(&self) -> Vec<Request> {
        self.requests()
            .into_iter()
            .filter(|r| r.method != "GET" && !r.path.contains("/search"))
            .collect()
    }

    pub fn clear_requests(&self) {
        self.state.lock().unwrap().requests.clear();
    }

    pub fn issues(&self) -> Vec<Issue> {
        self.state.lock().unwrap().issues.clone()
    }

    pub fn issue(&self, key: &str) -> Issue {
        self.issues()
            .into_iter()
            .find(|i| i.key == key)
            .unwrap_or_else(|| panic!("issue {key} tidak ada"))
    }

    /// Issue baru di project SR (Cloud: deskripsi ADF dari teks). Kembalikan kuncinya.
    pub fn add_issue(&self, summary: &str, status: &str, extra: Value) -> String {
        let mut s = self.state.lock().unwrap();
        s.next_id += 1;
        let n = s.next_id;
        let mut fields = serde_json::Map::new();
        fields.insert("summary".into(), json!(summary));
        fields.insert("issuetype".into(), json!({ "name": "Story" }));
        fields.insert("labels".into(), json!([]));
        fields.insert("priority".into(), json!({ "name": "Medium" }));
        if let Value::Object(o) = extra {
            fields.extend(o);
        }
        let key = format!("SR-{}", 300 + n);
        s.issues.push(Issue {
            id: format!("{}", 20_000 + n),
            key: key.clone(),
            fields,
            status_id: status.to_string(),
            updated: Utc::now(),
            comments: Vec::new(),
        });
        key
    }

    /// Ubah field issue di sisi Jira (`updated` = sekarang + `skew_secs`).
    pub fn edit(&self, key: &str, fields: Value, skew_secs: i64) {
        let mut s = self.state.lock().unwrap();
        let i = s.find(key).expect("issue");
        if let Value::Object(o) = fields {
            i.fields.extend(o);
        }
        i.updated = Utc::now() + Duration::seconds(skew_secs);
    }

    pub fn set_status(&self, key: &str, status: &str) {
        let mut s = self.state.lock().unwrap();
        let i = s.find(key).expect("issue");
        i.status_id = status.to_string();
        i.updated = Utc::now();
    }

    pub fn add_comment(&self, key: &str, author: &str, text: &str) {
        let mut s = self.state.lock().unwrap();
        s.next_comment += 1;
        let id = format!("{}", 50_000 + s.next_comment);
        let author = s.user(author, true);
        let i = s.find(key).expect("issue");
        i.comments.push(json!({
            "id": id, "author": author,
            "body": { "type": "doc", "version": 1, "content": [
                { "type": "paragraph", "content": [{ "type": "text", "text": text }] }] },
        }));
        i.updated = Utc::now();
    }

    /// Lampiran: `size` = ukuran yang dilaporkan Jira; isi disajikan dari `data`.
    pub fn add_attachment(&self, key: &str, filename: &str, data: &[u8], size: u64) {
        let mut s = self.state.lock().unwrap();
        s.next_id += 1;
        let id = format!("{}", 90_000 + s.next_id);
        let url = format!("{}/secure/attachment/{id}/{filename}", s.base);
        s.files.insert(id.clone(), data.to_vec());
        let i = s.find(key).expect("issue");
        let list = i
            .fields
            .entry("attachment")
            .or_insert_with(|| json!([]));
        list.as_array_mut().unwrap().push(json!({
            "id": id, "filename": filename, "size": size, "mimeType": "text/plain",
            "content": url,
        }));
    }

    pub fn add_user(&self, account: &str, name: &str, email: &str) {
        self.state
            .lock()
            .unwrap()
            .users
            .push((account.into(), name.into(), email.into()));
    }

    /// Status tambahan di project.
    pub fn add_status(&self, id: &str, name: &str, category: &str) {
        self.state
            .lock()
            .unwrap()
            .statuses
            .push((id.into(), name.into(), category.into()));
    }
}

fn authorized(headers: &HeaderMap) -> bool {
    let Some(raw) = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
    else {
        return false;
    };
    if let Some(t) = raw.strip_prefix("Bearer ") {
        return t == TOKEN;
    }
    if let Some(b) = raw.strip_prefix("Basic ") {
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(b)
            .ok()
            .and_then(|d| String::from_utf8(d).ok())
            .unwrap_or_default();
        return decoded == format!("{EMAIL}:{TOKEN}");
    }
    false
}

fn unauthorized() -> Response {
    (StatusCode::UNAUTHORIZED, Json(json!({ "errorMessages": ["unauthorized"] }))).into_response()
}

async fn record(state: &Shared, method: &Method, uri: &Uri, body: &Bytes) {
    let body = serde_json::from_slice(body).unwrap_or(Value::Null);
    state.lock().unwrap().requests.push(Request {
        method: method.to_string(),
        path: uri.path().to_string(),
        body,
    });
}

async fn handle(
    State(state): State<Shared>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    Path(rest): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    body: Bytes,
) -> Response {
    record(&state, &method, &uri, &body).await;
    if !authorized(&headers) {
        return unauthorized();
    }
    let input: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    let mut parts: Vec<&str> = rest.split('/').collect();
    let version = parts.remove(0);
    let v3 = version == "3";
    let mut s = state.lock().unwrap();
    match (method.as_str(), parts.as_slice()) {
        ("GET", ["myself"]) => Json(s.user(ACCOUNT, v3)).into_response(),
        ("GET", ["project", "search"]) => Json(json!({
            "values": [{ "id": "10100", "key": "SR", "name": "Sevima Retail POS" },
                       { "id": "10101", "key": "GD", "name": "Gudang" }],
            "isLast": true,
        }))
        .into_response(),
        ("GET", ["project"]) => Json(json!([
            { "id": "10100", "key": "SR", "name": "Sevima Retail POS" },
            { "id": "10101", "key": "GD", "name": "Gudang" },
        ]))
        .into_response(),
        ("GET", ["project", _key, "statuses"]) => {
            let statuses: Vec<Value> = s.statuses.iter().map(|st| s.status(&st.0)).collect();
            Json(json!([{ "id": "1", "name": "Story", "statuses": statuses }])).into_response()
        }
        ("GET", ["field"]) => Json(json!([
            { "id": "summary", "name": "Summary", "custom": false },
            { "id": "description", "name": "Description", "custom": false },
            { "id": "labels", "name": "Labels", "custom": false },
            { "id": "priority", "name": "Priority", "custom": false },
            { "id": "duedate", "name": "Due date", "custom": false },
            { "id": "fixVersions", "name": "Fix Version/s", "custom": false },
            { "id": "components", "name": "Components", "custom": false },
            { "id": "customfield_10016", "name": "Story Points", "custom": true },
            { "id": "customfield_10020", "name": "Sprint", "custom": true },
            { "id": "customfield_10099", "name": "Tim", "custom": true },
        ]))
        .into_response(),
        ("GET", ["user", "assignable", "search"]) => {
            let users: Vec<Value> = s.users.iter().map(|u| s.user(&u.0, v3)).collect();
            Json(Value::Array(users)).into_response()
        }
        ("POST", ["search", "jql"] | ["search"]) => {
            let jql = input["jql"].as_str().unwrap_or_default().to_string();
            let since = jql.split("updated >= -").nth(1).and_then(|r| {
                r.trim_end_matches(|c: char| !c.is_ascii_digit())
                    .split('m')
                    .next()
                    .and_then(|n| n.trim().parse::<i64>().ok())
            });
            let cutoff = since.map(|m| Utc::now() - Duration::minutes(m));
            let issues: Vec<Value> = s
                .issues
                .iter()
                .filter(|i| cutoff.is_none_or(|c| i.updated >= c))
                .map(|i| s.issue_json(i))
                .collect();
            let total = issues.len();
            Json(json!({ "issues": issues, "isLast": true, "total": total, "startAt": 0 }))
                .into_response()
        }
        ("GET", ["issue", key]) => match s.issues.iter().find(|i| i.key == *key || i.id == *key) {
            Some(i) => Json(s.issue_json(i)).into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        },
        ("POST", ["issue"]) => {
            let fields = input["fields"].as_object().cloned().unwrap_or_default();
            if fields.get("summary").and_then(Value::as_str).unwrap_or_default().is_empty() {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "errors": { "summary": "You must specify a summary." } })),
                )
                    .into_response();
            }
            s.next_id += 1;
            let n = s.next_id;
            let key = format!("SR-{}", 300 + n);
            let id = format!("{}", 20_000 + n);
            let mut fields = fields;
            fields.remove("project");
            let status = s.statuses.first().map(|st| st.0.clone()).unwrap_or_default();
            s.issues.push(Issue {
                id: id.clone(),
                key: key.clone(),
                fields,
                status_id: status,
                updated: Utc::now(),
                comments: Vec::new(),
            });
            (StatusCode::CREATED, Json(json!({ "id": id, "key": key }))).into_response()
        }
        ("PUT", ["issue", key]) => {
            let Some(i) = s.find(key) else {
                return StatusCode::NOT_FOUND.into_response();
            };
            if let Some(o) = input["fields"].as_object() {
                i.fields.extend(o.clone());
            }
            i.updated = Utc::now();
            StatusCode::NO_CONTENT.into_response()
        }
        ("GET", ["issue", key, "transitions"]) => {
            if !s.issues.iter().any(|i| i.key == *key) {
                return StatusCode::NOT_FOUND.into_response();
            }
            let list: Vec<Value> = s
                .statuses
                .iter()
                .map(|st| json!({ "id": format!("t{}", st.0), "name": st.1, "to": s.status(&st.0) }))
                .collect();
            Json(json!({ "transitions": list })).into_response()
        }
        ("POST", ["issue", key, "transitions"]) => {
            let to = input["transition"]["id"]
                .as_str()
                .unwrap_or_default()
                .trim_start_matches('t')
                .to_string();
            let Some(i) = s.find(key) else {
                return StatusCode::NOT_FOUND.into_response();
            };
            i.status_id = to;
            i.updated = Utc::now();
            StatusCode::NO_CONTENT.into_response()
        }
        ("GET", ["issue", key, "comment"]) => match s.issues.iter().find(|i| i.key == *key) {
            Some(i) => {
                let comments: Vec<Value> = i
                    .comments
                    .iter()
                    .map(|c| {
                        let mut c = c.clone();
                        if !v3 {
                            c["body"] = json!(text_of(&c["body"]));
                        }
                        c
                    })
                    .collect();
                let total = comments.len();
                Json(json!({ "comments": comments, "total": total, "startAt": 0 })).into_response()
            }
            None => StatusCode::NOT_FOUND.into_response(),
        },
        ("POST", ["issue", key, "comment"]) => {
            s.next_comment += 1;
            let id = format!("{}", 50_000 + s.next_comment);
            let author = s.user(ACCOUNT, v3);
            let Some(i) = s.find(key) else {
                return StatusCode::NOT_FOUND.into_response();
            };
            let c = json!({ "id": id, "author": author, "body": input["body"] });
            i.comments.push(c.clone());
            i.updated = Utc::now();
            (StatusCode::CREATED, Json(c)).into_response()
        }
        _ => {
            drop(s);
            let _ = query;
            (StatusCode::NOT_FOUND, Json(json!({ "errorMessages": ["no route"] }))).into_response()
        }
    }
}

/// Teks polos dari isi komentar ADF (tampilan v2).
pub fn text_of(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Object(o) => o
            .get("content")
            .and_then(Value::as_array)
            .map(|a| a.iter().map(text_of).collect::<Vec<_>>().join(""))
            .or_else(|| o.get("text").and_then(Value::as_str).map(str::to_string))
            .unwrap_or_default(),
        _ => String::new(),
    }
}

async fn attachment(
    State(state): State<Shared>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    Path((id, _name)): Path<(String, String)>,
) -> Response {
    record(&state, &method, &uri, &Bytes::new()).await;
    if !authorized(&headers) {
        return unauthorized();
    }
    match state.lock().unwrap().files.get(&id) {
        Some(data) => data.clone().into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

pub async fn start() -> FakeJira {
    let state: Shared = Arc::new(Mutex::new(Inner {
        requests: Vec::new(),
        statuses: vec![
            (TODO.into(), "To Do".into(), "new".into()),
            (IN_PROGRESS.into(), "In Progress".into(), "indeterminate".into()),
            (DONE.into(), "Done".into(), "done".into()),
        ],
        users: vec![(ACCOUNT.into(), "Olivia Owner".into(), EMAIL.into())],
        issues: Vec::new(),
        files: HashMap::new(),
        next_id: 0,
        next_comment: 0,
        base: String::new(),
    }));
    let app = Router::new()
        .route(
            "/rest/api/{*rest}",
            get(handle).post(handle).put(handle),
        )
        .route("/secure/attachment/{id}/{name}", get(attachment))
        .with_state(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let url = format!("http://{addr}");
    state.lock().unwrap().base.clone_from(&url);
    FakeJira { url, state }
}
