//! Klien REST Jira (M11). [`JiraApi`] adalah semua operasi yang dipakai connector; [`HttpJira`]
//! mengimplementasikannya dengan reqwest untuk dua jenis Jira:
//!
//! - **Cloud** (`REST v3`, Basic `email:api_token`): deskripsi/komentar berupa ADF
//!   ([`super::adf`]); pencarian lewat `POST /search/jql` (paginasi `nextPageToken`).
//! - **Data Center** (`REST v2`, Bearer PAT): deskripsi/komentar teks wiki (dibaca apa adanya);
//!   pencarian `POST /search` (paginasi `startAt`).
//!
//! Alamat Jira divalidasi penjaga SSRF yang sama dengan webhook keluar
//! (`webhooks::url_guard`), di-resolve sekali saat klien dibuat, lalu klien dipaku ke alamat
//! itu (redirect tidak diikuti). Lampiran hanya diunduh dari host yang sama dengan URL Jira
//! (kredensial tidak dikirim ke host lain). Token tidak pernah masuk log maupun pesan error.

use std::time::Duration;

use chrono::{DateTime, Utc};
use serde_json::{Value, json};

use super::adf;
use crate::{
    models::jira_connections::{JiraEnv, StatusCategory},
    services::webhooks::url_guard,
};

/// User Jira.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct JiraUser {
    /// `accountId` (Cloud) atau `name` (Data Center).
    pub account: String,
    pub name: String,
    pub email: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JiraProject {
    pub id: String,
    pub key: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JiraStatus {
    pub id: String,
    pub name: String,
    pub category: StatusCategory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JiraField {
    pub id: String,
    pub name: String,
    pub custom: bool,
}

/// Issue: id, kunci, status, waktu ubah, dan field mentah (dibaca lewat helper di
/// `super::fields`).
#[derive(Debug, Clone, PartialEq)]
pub struct JiraIssue {
    pub id: String,
    pub key: String,
    pub status_id: String,
    pub status_name: String,
    pub updated: Option<DateTime<Utc>>,
    pub fields: Value,
}

/// Satu halaman hasil JQL. `next` = penanda halaman berikutnya (opaque).
#[derive(Debug, Clone, Default)]
pub struct SearchPage {
    pub issues: Vec<JiraIssue>,
    pub next: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transition {
    pub id: String,
    pub to_id: String,
    pub to_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JiraComment {
    pub id: String,
    pub author: JiraUser,
    /// Isi sebagai teks (ADF dikonversi).
    pub body: String,
}

/// Issue baru / perubahan: field Jira (`summary`, `description` sudah dalam bentuk sisi Jira).
#[derive(Debug, Clone, Default)]
pub struct IssueRef {
    pub id: String,
    pub key: String,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum JiraError {
    /// Kredensial ditolak (401).
    #[error("jira rejected the credentials")]
    Unauthorized,
    #[error("not found on jira")]
    NotFound,
    /// Ditolak validasi/izin Jira (400, 403, 409, 422) atau batas lokal (lampiran terlalu besar).
    #[error("jira rejected the request: {0}")]
    Rejected(String),
    /// Jaringan, timeout, 429, 5xx: boleh diulang.
    #[error("jira unreachable: {0}")]
    Network(String),
    #[error("unexpected jira response: {0}")]
    Decode(String),
}

impl JiraError {
    /// Gangguan sementara: sinkron berhenti dan diulang nanti (cursor tidak maju).
    #[must_use]
    pub fn is_transient(&self) -> bool {
        matches!(self, Self::Network(_) | Self::Unauthorized)
    }
}

/// Operasi Jira yang dipakai connector.
#[async_trait::async_trait]
pub trait JiraApi: Send + Sync {
    /// Jenis Jira (bentuk deskripsi/komentar dan akun user).
    fn env(&self) -> JiraEnv;
    async fn myself(&self) -> Result<JiraUser, JiraError>;
    async fn projects(&self) -> Result<Vec<JiraProject>, JiraError>;
    /// Status semua tipe issue project (tanpa duplikat).
    async fn statuses(&self, project_key: &str) -> Result<Vec<JiraStatus>, JiraError>;
    async fn fields(&self) -> Result<Vec<JiraField>, JiraError>;
    /// User yang bisa ditugaskan di project (best effort).
    async fn users(&self, project_key: &str) -> Result<Vec<JiraUser>, JiraError>;
    async fn search(
        &self,
        jql: &str,
        fields: &[String],
        next: Option<&str>,
    ) -> Result<SearchPage, JiraError>;
    async fn issue(&self, key: &str, fields: &[String]) -> Result<JiraIssue, JiraError>;
    async fn create_issue(&self, fields: Value) -> Result<IssueRef, JiraError>;
    async fn update_issue(&self, key: &str, fields: Value) -> Result<(), JiraError>;
    async fn transitions(&self, key: &str) -> Result<Vec<Transition>, JiraError>;
    async fn transition(&self, key: &str, transition_id: &str) -> Result<(), JiraError>;
    async fn comments(&self, key: &str) -> Result<Vec<JiraComment>, JiraError>;
    async fn add_comment(&self, key: &str, body: &str) -> Result<JiraComment, JiraError>;
    /// Unduh lampiran (`content` URL dari issue), maksimal `max_bytes`.
    async fn download(&self, url: &str, max_bytes: usize) -> Result<Vec<u8>, JiraError>;
}

/// Isi teks → bentuk field deskripsi/komentar sisi Jira.
#[must_use]
pub fn body_out(env: JiraEnv, text: &str) -> Value {
    match env {
        JiraEnv::Cloud => adf::from_text(text),
        JiraEnv::DataCenter => Value::String(text.to_string()),
    }
}

/// Field deskripsi/komentar sisi Jira → teks.
#[must_use]
pub fn body_in(value: &Value) -> String {
    adf::to_text(value)
}

/// Waktu Jira (`2026-11-14T10:00:00.000+0000` atau RFC 3339).
#[must_use]
pub fn parse_time(raw: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(raw)
        .or_else(|_| DateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M:%S%.f%z"))
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

/// Kredensial.
#[derive(Clone)]
pub enum Auth {
    /// Cloud: email + API token.
    Basic { email: String, token: String },
    /// Data Center: Personal Access Token.
    Bearer(String),
}

/// Jira lewat HTTP.
pub struct HttpJira {
    env: JiraEnv,
    /// Basis REST tanpa `/` di akhir (`https://acme.atlassian.net/rest/api/3`).
    api: String,
    host: String,
    auth: Auth,
    http: reqwest::Client,
    download_timeout: Duration,
}

impl std::fmt::Debug for HttpJira {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpJira")
            .field("env", &self.env)
            .field("api", &self.api)
            .field("auth", &"[redacted]")
            .finish_non_exhaustive()
    }
}

fn net(e: &reqwest::Error) -> JiraError {
    if e.is_timeout() {
        JiraError::Network("timeout".into())
    } else {
        JiraError::Network(e.to_string().replace(|c: char| c.is_control(), " "))
    }
}

fn s(v: &Value, ptr: &str) -> String {
    v.pointer(ptr)
        .and_then(|x| match x {
            Value::String(s) => Some(s.clone()),
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        })
        .unwrap_or_default()
}

fn user_of(env: JiraEnv, v: &Value) -> JiraUser {
    JiraUser {
        account: match env {
            JiraEnv::Cloud => s(v, "/accountId"),
            JiraEnv::DataCenter => {
                let name = s(v, "/name");
                if name.is_empty() { s(v, "/key") } else { name }
            }
        },
        name: s(v, "/displayName"),
        email: s(v, "/emailAddress").trim().to_lowercase(),
    }
}

fn issue_of(v: &Value) -> JiraIssue {
    JiraIssue {
        id: s(v, "/id"),
        key: s(v, "/key"),
        status_id: s(v, "/fields/status/id"),
        status_name: s(v, "/fields/status/name"),
        updated: parse_time(&s(v, "/fields/updated")),
        fields: v.get("fields").cloned().unwrap_or(Value::Null),
    }
}

impl HttpJira {
    /// Klien untuk `base_url` (URL site/server Jira). Alamat di-resolve dan diperiksa penjaga
    /// SSRF di sini.
    ///
    /// # Errors
    ///
    /// URL ditolak (`Rejected("url_invalid"|"url_forbidden")`), DNS gagal (`Network`).
    pub async fn connect(
        base_url: &str,
        env: JiraEnv,
        auth: Auth,
        timeout: Duration,
        allow_loopback: bool,
    ) -> Result<Self, JiraError> {
        let base = base_url.trim().trim_end_matches('/').to_string();
        let (host, addr) = url_guard::resolve(&base, allow_loopback)
            .await
            .map_err(|code| match code {
                "dns_failed" => JiraError::Network(code.into()),
                other => JiraError::Rejected(other.into()),
            })?;
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(timeout)
            .resolve(&host, addr)
            .user_agent("AgentGate-Jira/1")
            .build()
            .map_err(|e| net(&e))?;
        let version = match env {
            JiraEnv::Cloud => "3",
            JiraEnv::DataCenter => "2",
        };
        Ok(Self {
            env,
            api: format!("{base}/rest/api/{version}"),
            host,
            auth,
            http,
            download_timeout: timeout * 4,
        })
    }

    fn authed(&self, rb: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        let rb = rb.header("Accept", "application/json");
        match &self.auth {
            Auth::Basic { email, token } => rb.basic_auth(email, Some(token)),
            Auth::Bearer(t) => rb.bearer_auth(t),
        }
    }

    async fn send(&self, rb: reqwest::RequestBuilder) -> Result<Option<Value>, JiraError> {
        let res = self.authed(rb).send().await.map_err(|e| net(&e))?;
        let status = res.status().as_u16();
        let text = res.text().await.map_err(|e| net(&e))?;
        match status {
            200..=299 => {
                if text.trim().is_empty() {
                    Ok(None)
                } else {
                    serde_json::from_str(&text)
                        .map(Some)
                        .map_err(|e| JiraError::Decode(e.to_string()))
                }
            }
            401 => Err(JiraError::Unauthorized),
            404 => Err(JiraError::NotFound),
            429 | 500..=599 => Err(JiraError::Network(format!("status {status}"))),
            _ => Err(JiraError::Rejected(error_message(status, &text))),
        }
    }

    async fn get(&self, path: &str, query: &[(&str, String)]) -> Result<Value, JiraError> {
        self.send(self.http.get(format!("{}{path}", self.api)).query(query))
            .await?
            .ok_or_else(|| JiraError::Decode("empty body".into()))
    }

    async fn post(&self, path: &str, body: &Value) -> Result<Option<Value>, JiraError> {
        self.send(self.http.post(format!("{}{path}", self.api)).json(body))
            .await
    }

    async fn put(&self, path: &str, body: &Value) -> Result<Option<Value>, JiraError> {
        self.send(self.http.put(format!("{}{path}", self.api)).json(body))
            .await
    }
}

/// Pesan error Jira (`errorMessages` + `errors`), dipotong.
fn error_message(status: u16, body: &str) -> String {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let mut parts: Vec<String> = v
        .get("errorMessages")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|m| m.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    if let Some(map) = v.get("errors").and_then(Value::as_object) {
        parts.extend(
            map.iter()
                .map(|(k, m)| format!("{k}: {}", m.as_str().unwrap_or_default())),
        );
    }
    let msg = if parts.is_empty() {
        format!("status {status}")
    } else {
        parts.join("; ")
    };
    msg.chars().take(300).collect()
}

fn escape_path(key: &str) -> String {
    key.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect()
}

#[async_trait::async_trait]
impl JiraApi for HttpJira {
    fn env(&self) -> JiraEnv {
        self.env
    }

    async fn myself(&self) -> Result<JiraUser, JiraError> {
        let v = self.get("/myself", &[]).await?;
        Ok(user_of(self.env, &v))
    }

    async fn projects(&self) -> Result<Vec<JiraProject>, JiraError> {
        let to_project = |p: &Value| JiraProject {
            id: s(p, "/id"),
            key: s(p, "/key"),
            name: s(p, "/name"),
        };
        match self.env {
            JiraEnv::Cloud => {
                let mut out = Vec::new();
                let mut start = 0_u64;
                loop {
                    let v = self
                        .get(
                            "/project/search",
                            &[("startAt", start.to_string()), ("maxResults", "50".into())],
                        )
                        .await?;
                    let values = v.get("values").and_then(Value::as_array).cloned();
                    let page = values.unwrap_or_default();
                    out.extend(page.iter().map(to_project));
                    let last = v.get("isLast").and_then(Value::as_bool).unwrap_or(true);
                    if last || page.is_empty() || out.len() >= 1_000 {
                        break;
                    }
                    start += page.len() as u64;
                }
                Ok(out)
            }
            JiraEnv::DataCenter => {
                let v = self.get("/project", &[]).await?;
                Ok(v.as_array()
                    .map(|a| a.iter().map(to_project).collect())
                    .unwrap_or_default())
            }
        }
    }

    async fn statuses(&self, project_key: &str) -> Result<Vec<JiraStatus>, JiraError> {
        let v = self
            .get(
                &format!("/project/{}/statuses", escape_path(project_key)),
                &[],
            )
            .await?;
        let mut out: Vec<JiraStatus> = Vec::new();
        for t in v.as_array().map_or(&[][..], Vec::as_slice) {
            for st in t
                .get("statuses")
                .and_then(Value::as_array)
                .map_or(&[][..], Vec::as_slice)
            {
                let id = s(st, "/id");
                if !id.is_empty() && !out.iter().any(|o| o.id == id) {
                    out.push(JiraStatus {
                        id,
                        name: s(st, "/name"),
                        category: StatusCategory::from_jira(&s(st, "/statusCategory/key")),
                    });
                }
            }
        }
        Ok(out)
    }

    async fn fields(&self) -> Result<Vec<JiraField>, JiraError> {
        let v = self.get("/field", &[]).await?;
        Ok(v.as_array()
            .map(|a| {
                a.iter()
                    .map(|f| JiraField {
                        id: s(f, "/id"),
                        name: s(f, "/name"),
                        custom: f.get("custom").and_then(Value::as_bool).unwrap_or(false),
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    async fn users(&self, project_key: &str) -> Result<Vec<JiraUser>, JiraError> {
        let mut q = vec![
            ("project", project_key.to_string()),
            ("maxResults", "200".to_string()),
        ];
        if self.env == JiraEnv::DataCenter {
            q.push(("username", String::new()));
        }
        let v = self.get("/user/assignable/search", &q).await?;
        Ok(v.as_array()
            .map(|a| a.iter().map(|u| user_of(self.env, u)).collect())
            .unwrap_or_default())
    }

    async fn search(
        &self,
        jql: &str,
        fields: &[String],
        next: Option<&str>,
    ) -> Result<SearchPage, JiraError> {
        let issues_of = |v: &Value| -> Vec<JiraIssue> {
            v.get("issues")
                .and_then(Value::as_array)
                .map(|a| a.iter().map(issue_of).collect())
                .unwrap_or_default()
        };
        match self.env {
            JiraEnv::Cloud => {
                let mut body = json!({ "jql": jql, "fields": fields, "maxResults": 50 });
                if let Some(t) = next {
                    body["nextPageToken"] = json!(t);
                }
                let v = self
                    .post("/search/jql", &body)
                    .await?
                    .unwrap_or(Value::Null);
                let token = v
                    .get("nextPageToken")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                let last = v.get("isLast").and_then(Value::as_bool).unwrap_or(true);
                Ok(SearchPage {
                    issues: issues_of(&v),
                    next: if last { None } else { token },
                })
            }
            JiraEnv::DataCenter => {
                let start: u64 = next.and_then(|n| n.parse().ok()).unwrap_or(0);
                let body =
                    json!({ "jql": jql, "fields": fields, "maxResults": 50, "startAt": start });
                let v = self.post("/search", &body).await?.unwrap_or(Value::Null);
                let issues = issues_of(&v);
                let total = v.get("total").and_then(Value::as_u64).unwrap_or(0);
                let after = start + issues.len() as u64;
                Ok(SearchPage {
                    next: (!issues.is_empty() && after < total).then(|| after.to_string()),
                    issues,
                })
            }
        }
    }

    async fn issue(&self, key: &str, fields: &[String]) -> Result<JiraIssue, JiraError> {
        let v = self
            .get(
                &format!("/issue/{}", escape_path(key)),
                &[("fields", fields.join(","))],
            )
            .await?;
        Ok(issue_of(&v))
    }

    async fn create_issue(&self, fields: Value) -> Result<IssueRef, JiraError> {
        let v = self
            .post("/issue", &json!({ "fields": fields }))
            .await?
            .unwrap_or(Value::Null);
        let r = IssueRef {
            id: s(&v, "/id"),
            key: s(&v, "/key"),
        };
        if r.id.is_empty() || r.key.is_empty() {
            return Err(JiraError::Decode("issue id missing".into()));
        }
        Ok(r)
    }

    async fn update_issue(&self, key: &str, fields: Value) -> Result<(), JiraError> {
        self.put(
            &format!("/issue/{}", escape_path(key)),
            &json!({ "fields": fields }),
        )
        .await?;
        Ok(())
    }

    async fn transitions(&self, key: &str) -> Result<Vec<Transition>, JiraError> {
        let v = self
            .get(&format!("/issue/{}/transitions", escape_path(key)), &[])
            .await?;
        Ok(v.get("transitions")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|t| Transition {
                        id: s(t, "/id"),
                        to_id: s(t, "/to/id"),
                        to_name: s(t, "/to/name"),
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    async fn transition(&self, key: &str, transition_id: &str) -> Result<(), JiraError> {
        self.post(
            &format!("/issue/{}/transitions", escape_path(key)),
            &json!({ "transition": { "id": transition_id } }),
        )
        .await?;
        Ok(())
    }

    async fn comments(&self, key: &str) -> Result<Vec<JiraComment>, JiraError> {
        let mut out = Vec::new();
        let mut start = 0_u64;
        loop {
            let v = self
                .get(
                    &format!("/issue/{}/comment", escape_path(key)),
                    &[("startAt", start.to_string()), ("maxResults", "100".into())],
                )
                .await?;
            let page: Vec<JiraComment> = v
                .get("comments")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .map(|c| JiraComment {
                            id: s(c, "/id"),
                            author: c
                                .get("author")
                                .map(|a| user_of(self.env, a))
                                .unwrap_or_default(),
                            body: body_in(c.get("body").unwrap_or(&Value::Null)),
                        })
                        .collect()
                })
                .unwrap_or_default();
            let n = page.len() as u64;
            out.extend(page);
            let total = v.get("total").and_then(Value::as_u64).unwrap_or(0);
            start += n;
            if n == 0 || start >= total || out.len() >= 2_000 {
                break;
            }
        }
        Ok(out)
    }

    async fn add_comment(&self, key: &str, body: &str) -> Result<JiraComment, JiraError> {
        let v = self
            .post(
                &format!("/issue/{}/comment", escape_path(key)),
                &json!({ "body": body_out(self.env, body) }),
            )
            .await?
            .unwrap_or(Value::Null);
        Ok(JiraComment {
            id: s(&v, "/id"),
            author: v
                .get("author")
                .map(|a| user_of(self.env, a))
                .unwrap_or_default(),
            body: body.to_string(),
        })
    }

    async fn download(&self, url: &str, max_bytes: usize) -> Result<Vec<u8>, JiraError> {
        let parsed =
            reqwest::Url::parse(url).map_err(|_| JiraError::Rejected("url_invalid".into()))?;
        if parsed.host_str() != Some(self.host.as_str()) {
            return Err(JiraError::Rejected("url_forbidden".into()));
        }
        let mut res = self
            .authed(self.http.get(parsed).timeout(self.download_timeout))
            .send()
            .await
            .map_err(|e| net(&e))?;
        match res.status().as_u16() {
            200..=299 => {}
            401 => return Err(JiraError::Unauthorized),
            404 => return Err(JiraError::NotFound),
            s => return Err(JiraError::Network(format!("status {s}"))),
        }
        let mut out = Vec::new();
        while let Some(chunk) = res.chunk().await.map_err(|e| net(&e))? {
            if out.len() + chunk.len() > max_bytes {
                return Err(JiraError::Rejected("attachment_too_large".into()));
            }
            out.extend_from_slice(&chunk);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_jira_timestamps() {
        let a = parse_time("2026-11-14T10:00:00.000+0000").unwrap();
        let b = parse_time("2026-11-14T17:00:00.000+0700").unwrap();
        assert_eq!(a, b);
        assert!(parse_time("bukan waktu").is_none());
    }

    #[test]
    fn error_message_joins_jira_errors() {
        assert_eq!(
            error_message(
                400,
                r#"{"errorMessages":["x"],"errors":{"summary":"wajib"}}"#
            ),
            "x; summary: wajib"
        );
        assert_eq!(error_message(403, "<html>"), "status 403");
    }
}
