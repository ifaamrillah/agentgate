//! Abstraksi provider git (spec: GitHub, GitLab, Bitbucket + versi perusahaan). M6 memasang
//! [`GithubProvider`] untuk github.com dan GitHub Enterprise Server (REST v3, path sama dengan
//! basis `https://host/api/v3`). Provider lain cukup mengimplementasikan [`GitProvider`].
//!
//! Token tidak pernah masuk log: error hanya membawa status dan pesan provider.

use std::time::Duration;

use agentgate_core::git::PrState;
use serde::Deserialize;
use serde_json::json;

/// Info repo dari provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoInfo {
    pub full_name: String,
    pub default_branch: String,
    pub html_url: String,
}

/// PR baru.
#[derive(Debug, Clone)]
pub struct NewPull {
    pub head: String,
    pub base: String,
    pub title: String,
    pub body: String,
}

/// PR menurut provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullInfo {
    pub number: i64,
    pub url: String,
    pub title: String,
    pub state: PrState,
    pub head_branch: String,
    pub head_sha: String,
    pub base_branch: String,
    pub additions: i32,
    pub deletions: i32,
    pub author: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    /// Token ditolak (401/403 tanpa pesan lain).
    #[error("provider rejected the token")]
    Unauthorized,
    #[error("not found on provider")]
    NotFound,
    /// Ditolak validasi provider (422, 405 merge tidak bisa, 409 konflik).
    #[error("provider rejected the request: {0}")]
    Rejected(String),
    #[error("provider unreachable: {0}")]
    Network(String),
    #[error("unexpected provider response: {0}")]
    Decode(String),
}

/// Operasi provider yang dipakai Agent Gate.
#[async_trait::async_trait]
pub trait GitProvider: Send + Sync {
    /// Akun pemilik token.
    async fn whoami(&self) -> Result<String, ProviderError>;
    async fn repo(&self, full_name: &str) -> Result<RepoInfo, ProviderError>;
    async fn create_pull(&self, full_name: &str, pull: &NewPull)
    -> Result<PullInfo, ProviderError>;
    /// PR terbuka dari `head` (branch di repo yang sama), bila ada.
    async fn find_open_pull(
        &self,
        full_name: &str,
        head: &str,
    ) -> Result<Option<PullInfo>, ProviderError>;
    async fn pull(&self, full_name: &str, number: i64) -> Result<PullInfo, ProviderError>;
    /// Merge (squash).
    async fn merge_pull(&self, full_name: &str, number: i64) -> Result<(), ProviderError>;
    /// Pasang webhook (event PR dan CI) → id hook.
    async fn create_webhook(
        &self,
        full_name: &str,
        url: &str,
        secret: &str,
    ) -> Result<i64, ProviderError>;
    /// Arsip `.tar.gz` isi repo di `git_ref` (index code graph oleh server, M7), maks.
    /// `max_bytes`.
    async fn tarball(
        &self,
        full_name: &str,
        git_ref: &str,
        max_bytes: usize,
    ) -> Result<Vec<u8>, ProviderError>;
}

/// Baca body respons dengan batas ukuran.
async fn read_limited(
    mut res: reqwest::Response,
    max_bytes: usize,
) -> Result<Vec<u8>, ProviderError> {
    let mut out = Vec::new();
    while let Some(chunk) = res
        .chunk()
        .await
        .map_err(|e| ProviderError::Network(e.without_url().to_string()))?
    {
        if out.len() + chunk.len() > max_bytes {
            return Err(ProviderError::Rejected("archive-too-large".into()));
        }
        out.extend_from_slice(&chunk);
    }
    Ok(out)
}

/// GitHub REST v3.
pub struct GithubProvider {
    api_base: String,
    token: String,
    http: reqwest::Client,
}

impl std::fmt::Debug for GithubProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GithubProvider")
            .field("api_base", &self.api_base)
            .field("token", &"[redacted]")
            .finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
struct GhUser {
    login: String,
}

#[derive(Deserialize)]
struct GhRepo {
    full_name: String,
    default_branch: String,
    html_url: String,
}

#[derive(Deserialize)]
struct GhRef {
    #[serde(rename = "ref")]
    name: String,
    sha: String,
}

#[derive(Deserialize)]
struct GhPull {
    number: i64,
    html_url: String,
    title: String,
    state: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    merged: bool,
    #[serde(default)]
    merged_at: Option<String>,
    head: GhRef,
    base: GhRef,
    #[serde(default)]
    additions: i32,
    #[serde(default)]
    deletions: i32,
    #[serde(default)]
    user: Option<GhUser>,
}

impl From<GhPull> for PullInfo {
    fn from(p: GhPull) -> Self {
        let state = if p.merged || p.merged_at.is_some() {
            PrState::Merged
        } else if p.state == "closed" {
            PrState::Closed
        } else if p.draft {
            PrState::Draft
        } else {
            PrState::Open
        };
        Self {
            number: p.number,
            url: p.html_url,
            title: p.title,
            state,
            head_branch: p.head.name,
            head_sha: p.head.sha,
            base_branch: p.base.name,
            additions: p.additions,
            deletions: p.deletions,
            author: p.user.map(|u| u.login).unwrap_or_default(),
        }
    }
}

/// Bentuk PR GitHub di payload webhook (`pull_request`) → [`PullInfo`].
///
/// # Errors
///
/// Bentuk JSON tidak dikenal.
pub fn github_pull_from_json(value: &serde_json::Value) -> Result<PullInfo, ProviderError> {
    serde_json::from_value::<GhPull>(value.clone())
        .map(PullInfo::from)
        .map_err(|e| ProviderError::Decode(e.to_string()))
}

#[derive(Deserialize)]
struct GhError {
    #[serde(default)]
    message: String,
}

impl GithubProvider {
    /// # Errors
    ///
    /// Client HTTP tidak bisa dibuat.
    pub fn new(api_base: &str, token: &str, timeout: Duration) -> Result<Self, ProviderError> {
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(concat!("agentgate/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| ProviderError::Network(e.to_string()))?;
        Ok(Self {
            api_base: api_base.trim_end_matches('/').to_string(),
            token: token.to_string(),
            http,
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.api_base)
    }

    fn req(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        self.http
            .request(method, self.url(path))
            .bearer_auth(&self.token)
            .header("accept", "application/vnd.github+json")
            .header("x-github-api-version", "2022-11-28")
    }

    async fn send<T: serde::de::DeserializeOwned>(
        &self,
        req: reqwest::RequestBuilder,
    ) -> Result<T, ProviderError> {
        let res = req
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
        let message = res
            .json::<GhError>()
            .await
            .map(|e| e.message)
            .unwrap_or_default();
        Err(match status.as_u16() {
            401 => ProviderError::Unauthorized,
            403 if message.is_empty() || message.contains("Bad credentials") => {
                ProviderError::Unauthorized
            }
            404 => ProviderError::NotFound,
            _ => ProviderError::Rejected(if message.is_empty() {
                status.to_string()
            } else {
                message
            }),
        })
    }
}

fn split(full_name: &str) -> Result<(&str, &str), ProviderError> {
    full_name
        .split_once('/')
        .filter(|(o, r)| !o.is_empty() && !r.is_empty() && !r.contains('/'))
        .ok_or(ProviderError::NotFound)
}

#[async_trait::async_trait]
impl GitProvider for GithubProvider {
    async fn whoami(&self) -> Result<String, ProviderError> {
        let user: GhUser = self.send(self.req(reqwest::Method::GET, "/user")).await?;
        Ok(user.login)
    }

    async fn repo(&self, full_name: &str) -> Result<RepoInfo, ProviderError> {
        let (o, r) = split(full_name)?;
        let repo: GhRepo = self
            .send(self.req(reqwest::Method::GET, &format!("/repos/{o}/{r}")))
            .await?;
        Ok(RepoInfo {
            full_name: repo.full_name,
            default_branch: repo.default_branch,
            html_url: repo.html_url,
        })
    }

    async fn create_pull(
        &self,
        full_name: &str,
        pull: &NewPull,
    ) -> Result<PullInfo, ProviderError> {
        let (o, r) = split(full_name)?;
        let body = json!({
            "title": pull.title,
            "head": pull.head,
            "base": pull.base,
            "body": pull.body,
        });
        let pr: GhPull = self
            .send(
                self.req(reqwest::Method::POST, &format!("/repos/{o}/{r}/pulls"))
                    .json(&body),
            )
            .await?;
        Ok(pr.into())
    }

    async fn find_open_pull(
        &self,
        full_name: &str,
        head: &str,
    ) -> Result<Option<PullInfo>, ProviderError> {
        let (o, r) = split(full_name)?;
        let prs: Vec<GhPull> = self
            .send(
                self.req(reqwest::Method::GET, &format!("/repos/{o}/{r}/pulls"))
                    .query(&[("state", "open"), ("head", &format!("{o}:{head}"))]),
            )
            .await?;
        Ok(prs.into_iter().next().map(PullInfo::from))
    }

    async fn pull(&self, full_name: &str, number: i64) -> Result<PullInfo, ProviderError> {
        let (o, r) = split(full_name)?;
        let pr: GhPull = self
            .send(self.req(
                reqwest::Method::GET,
                &format!("/repos/{o}/{r}/pulls/{number}"),
            ))
            .await?;
        Ok(pr.into())
    }

    async fn merge_pull(&self, full_name: &str, number: i64) -> Result<(), ProviderError> {
        let (o, r) = split(full_name)?;
        let _: serde_json::Value = self
            .send(
                self.req(
                    reqwest::Method::PUT,
                    &format!("/repos/{o}/{r}/pulls/{number}/merge"),
                )
                .json(&json!({ "merge_method": "squash" })),
            )
            .await?;
        Ok(())
    }

    async fn tarball(
        &self,
        full_name: &str,
        git_ref: &str,
        max_bytes: usize,
    ) -> Result<Vec<u8>, ProviderError> {
        let (o, r) = split(full_name)?;
        let res = self
            .req(
                reqwest::Method::GET,
                &format!("/repos/{o}/{r}/tarball/{git_ref}"),
            )
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.without_url().to_string()))?;
        // GitHub mengalihkan ke codeload (URL bertanda tangan, tanpa header token).
        let res = if res.status().is_redirection() {
            let location = res
                .headers()
                .get("location")
                .and_then(|v| v.to_str().ok())
                .ok_or_else(|| ProviderError::Decode("redirect tanpa location".into()))?
                .to_string();
            // Redirect hanya ke https di alamat publik, atau ke basis API yang sama (GHES di
            // loopback saat tes). IP literal internal ditolak (SSRF, M12).
            let internal = reqwest::Url::parse(&location).ok().is_none_or(|u| {
                u.host_str().is_none_or(|h| {
                    let h = h.trim_start_matches('[').trim_end_matches(']');
                    h.eq_ignore_ascii_case("localhost")
                        || h.parse::<std::net::IpAddr>().is_ok_and(|ip| {
                            crate::services::webhooks::url_guard::is_forbidden(ip, false)
                        })
                })
            });
            let same_api = location.starts_with(&self.api_base);
            if !same_api && (!location.starts_with("https://") || internal) {
                return Err(ProviderError::Decode("redirect tidak aman".into()));
            }
            self.http
                .get(location)
                .send()
                .await
                .map_err(|e| ProviderError::Network(e.without_url().to_string()))?
        } else {
            res
        };
        match res.status().as_u16() {
            200 => read_limited(res, max_bytes).await,
            401 | 403 => Err(ProviderError::Unauthorized),
            404 => Err(ProviderError::NotFound),
            s => Err(ProviderError::Rejected(format!("HTTP {s}"))),
        }
    }

    async fn create_webhook(
        &self,
        full_name: &str,
        url: &str,
        secret: &str,
    ) -> Result<i64, ProviderError> {
        #[derive(Deserialize)]
        struct Hook {
            id: i64,
        }
        let (o, r) = split(full_name)?;
        let body = json!({
            "name": "web",
            "active": true,
            "events": ["pull_request", "check_run", "check_suite", "status"],
            "config": {
                "url": url,
                "content_type": "json",
                "secret": secret,
                "insecure_ssl": "0",
            },
        });
        let hook: Hook = self
            .send(
                self.req(reqwest::Method::POST, &format!("/repos/{o}/{r}/hooks"))
                    .json(&body),
            )
            .await?;
        Ok(hook.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pull_state_from_github_fields() {
        let base = json!({
            "number": 7, "html_url": "https://github.com/a/b/pull/7", "title": "t",
            "state": "open", "draft": false,
            "head": { "ref": "ag/KSR-1-x", "sha": "abc" }, "base": { "ref": "main", "sha": "d" },
            "user": { "login": "dewi" }
        });
        let p = github_pull_from_json(&base).unwrap();
        assert_eq!(p.state, PrState::Open);
        assert_eq!(
            (p.head_branch.as_str(), p.author.as_str()),
            ("ag/KSR-1-x", "dewi")
        );
        let mut v = base.clone();
        v["draft"] = json!(true);
        assert_eq!(github_pull_from_json(&v).unwrap().state, PrState::Draft);
        let mut v = base.clone();
        v["state"] = json!("closed");
        assert_eq!(github_pull_from_json(&v).unwrap().state, PrState::Closed);
        v["merged_at"] = json!("2026-11-13T00:00:00Z");
        assert_eq!(github_pull_from_json(&v).unwrap().state, PrState::Merged);
    }

    #[test]
    fn debug_hides_the_token() {
        let p = GithubProvider::new(
            "https://api.github.com",
            "ghp_secret",
            Duration::from_secs(1),
        )
        .unwrap();
        assert!(!format!("{p:?}").contains("ghp_secret"));
        assert!(split("a/b").is_ok());
        assert!(split("a/b/c").is_err());
        assert!(split("/b").is_err());
    }
}
