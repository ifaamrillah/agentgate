//! Hubungkan/putus provider git (Pengaturan → Git provider, izin `ManageIntegrations`) dan
//! tautkan repo project ke koneksi + pasang webhook.
//!
//! Token diverifikasi ke provider (`GET /user`) sebelum disimpan; yang disimpan hanya blob
//! terenkripsi. Satu koneksi per host: menghubungkan host yang sama lagi = ganti token.

use agentgate_core::git::repo_path;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait,
    IntoActiveModel, ModelTrait, QueryFilter, TransactionTrait, sea_query::Expr,
};
use uuid::Uuid;

use super::{TOKEN_INVALID, URL_INVALID, app, app_code, invalid, key, provider_code, provider_for};
use crate::{
    common::{crypto, settings::Settings},
    models::{
        git_connections::{self, GitProviderKind},
        permission::Permission,
        repos::{self, RepoProvider},
        workspaces::WorkspaceId,
    },
    services::{
        audit::{self, AuditAction, AuditSubject},
        tickets::TicketFailure,
        webhooks::url_guard,
        workspaces::{Acting, lock_as},
    },
};

/// Panjang maksimal token.
pub const TOKEN_MAX: usize = 400;

/// Isian dialog Hubungkan.
#[derive(Clone)]
pub struct ConnectInput {
    pub kind: GitProviderKind,
    /// URL server GitHub Enterprise (`https://github.perusahaan.local`); diabaikan untuk github.com.
    pub url: String,
    pub token: String,
}

impl std::fmt::Debug for ConnectInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectInput")
            .field("kind", &self.kind)
            .field("url", &self.url)
            .field("token", &"[redacted]")
            .finish()
    }
}

/// (host, api_base, web_base) untuk isian.
///
/// # Errors
///
/// `git-url-invalid`: bukan https (http hanya untuk loopback), atau tanpa host.
pub fn endpoints(
    settings: &Settings,
    kind: GitProviderKind,
    url: &str,
) -> Result<(String, String, String), TicketFailure> {
    match kind {
        GitProviderKind::Github => Ok((
            "github.com".into(),
            settings.git.github_api_base.trim_end_matches('/').into(),
            settings.git.github_web_base.trim_end_matches('/').into(),
        )),
        GitProviderKind::GithubEnterprise => enterprise(url, settings.git.allow_loopback),
    }
}

/// (host, api_base, web_base) GitHub Enterprise Server dari URL isian.
fn enterprise(url: &str, allow_loopback: bool) -> Result<(String, String, String), TicketFailure> {
    let url = url.trim().trim_end_matches('/');
    let (scheme, rest) = url
        .split_once("://")
        .ok_or_else(|| invalid("url", URL_INVALID))?;
    let host_port = rest.split('/').next().unwrap_or_default();
    let host = host_port
        .rsplit_once(':')
        .map_or(host_port, |(h, _)| h)
        .to_ascii_lowercase();
    let loopback = matches!(host.as_str(), "127.0.0.1" | "localhost" | "[::1]");
    let ok_scheme = scheme == "https" || (scheme == "http" && loopback && allow_loopback);
    // SSRF (M12): IP literal internal (privat, link-local/metadata cloud, loopback di
    // production) ditolak; nama host diperiksa setelah resolve di [`connect`].
    let literal = host
        .trim_start_matches('[')
        .trim_end_matches(']')
        .parse::<std::net::IpAddr>()
        .ok();
    if (loopback && !allow_loopback)
        || literal.is_some_and(|ip| url_guard::is_forbidden(ip, allow_loopback))
    {
        return Err(invalid("url", URL_INVALID));
    }
    let ok_host = !host.is_empty()
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '[' | ']' | ':'))
        && !rest.contains('@')
        && !rest.contains('?');
    if !ok_scheme || !ok_host || url.len() > 200 {
        return Err(invalid("url", URL_INVALID));
    }
    let base = format!("{scheme}://{host_port}");
    Ok((host, format!("{base}/api/v3"), base))
}

/// Hubungkan (atau ganti token) provider. Repo workspace di host itu langsung ditautkan.
///
/// # Errors
///
/// `NotFound` (izin), `git-url-invalid`, `git-token-invalid`, `git-unreachable`, error database.
pub async fn connect(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    settings: &Settings,
    input: &ConnectInput,
) -> Result<git_connections::Model, TicketFailure> {
    if !acting.role.has(Permission::ManageIntegrations) {
        return Err(TicketFailure::NotFound);
    }
    let token = input.token.trim();
    if token.is_empty() || token.len() > TOKEN_MAX || token.chars().any(char::is_whitespace) {
        return Err(invalid("token", TOKEN_INVALID));
    }
    let (host, api_base, web_base) = endpoints(settings, input.kind, &input.url)?;
    if input.kind == GitProviderKind::GithubEnterprise {
        // Semua alamat hasil resolve harus publik (cegah SSRF ke jaringan internal server).
        url_guard::resolve(&web_base, settings.git.allow_loopback)
            .await
            .map_err(|_| invalid("url", URL_INVALID))?;
    }
    let probe = provider::GithubProvider::new(
        &api_base,
        token,
        std::time::Duration::from_secs(settings.git.timeout_seconds.max(1)),
    )
    .map_err(|e| invalid("token", provider_code(&e)))?;
    let account = provider::GitProvider::whoami(&probe)
        .await
        .map_err(|e| invalid("token", provider_code(&e)))?;
    let key = key(settings)?;
    let token_encrypted =
        crypto::encrypt(&key, token.as_bytes()).map_err(|_| invalid("token", TOKEN_INVALID))?;
    let wid = acting.workspace_id();
    let txn = db.begin().await?;
    lock_as(&txn, acting, |r| r.has(Permission::ManageIntegrations))
        .await?
        .ok_or(TicketFailure::NotFound)?;
    let existing = git_connections::Model::for_host(&txn, wid, &host).await?;
    let (conn, action) = if let Some(old) = existing {
        let mut m = old.into_active_model();
        m.provider = Set(sea_orm::ActiveEnum::to_value(&input.kind));
        m.api_base = Set(api_base);
        m.web_base = Set(web_base);
        m.account = Set(account.clone());
        m.token_encrypted = Set(token_encrypted);
        m.auth = Set("token".into());
        m.installation_id = Set(None);
        m.app_permissions = Set(serde_json::json!({}));
        m.app_repo_selection = Set(String::new());
        m.app_repos = Set(serde_json::json!([]));
        m.suspended_at = Set(None);
        (m.update(&txn).await?, AuditAction::GitUpdated)
    } else {
        let secret = crypto::random_token();
        let webhook_secret_encrypted = crypto::encrypt(&key, secret.as_bytes())
            .map_err(|_| invalid("token", TOKEN_INVALID))?;
        let m = git_connections::ActiveModel {
            workspace_id: Set(wid.get()),
            provider: Set(sea_orm::ActiveEnum::to_value(&input.kind)),
            host: Set(host.clone()),
            api_base: Set(api_base),
            web_base: Set(web_base),
            account: Set(account.clone()),
            token_encrypted: Set(token_encrypted),
            webhook_secret_encrypted: Set(webhook_secret_encrypted),
            created_by: Set(Some(acting.user.id)),
            ..Default::default()
        };
        (m.insert(&txn).await?, AuditAction::GitConnected)
    };
    audit::record(
        &txn,
        acting.actor().audit(
            action,
            (AuditSubject::GitConnection, conn.id),
            serde_json::json!({
                "provider": conn.provider,
                "host": conn.host,
                "account": account,
            }),
        ),
    )
    .await?;
    txn.commit().await?;
    for repo in repos_on_host(db, wid, &host).await? {
        let _ = link_repo(db, settings, wid, repo, Some(acting)).await;
    }
    Ok(conn)
}

use super::provider;

/// Repo GitHub workspace yang URL-nya di `host`.
async fn repos_on_host(
    db: &DatabaseConnection,
    wid: WorkspaceId,
    host: &str,
) -> Result<Vec<repos::Model>, sea_orm::DbErr> {
    Ok(repos::Entity::find()
        .filter(repos::Column::WorkspaceId.eq(wid.get()))
        .filter(repos::Column::Provider.eq(sea_orm::ActiveEnum::to_value(&RepoProvider::Github)))
        .all(db)
        .await?
        .into_iter()
        .filter(|r| repo_path(&r.url).is_some_and(|(h, _)| h == host))
        .collect())
}

/// Putuskan provider: koneksi dihapus, repo kembali tidak tertaut (webhook di provider tetap
/// ada sampai dihapus manual; event dari sana ditolak 404).
///
/// # Errors
///
/// `NotFound` (koneksi/izin), error database.
pub async fn disconnect(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
) -> Result<(), TicketFailure> {
    let wid = acting.workspace_id();
    let txn = db.begin().await?;
    lock_as(&txn, acting, |r| r.has(Permission::ManageIntegrations))
        .await?
        .ok_or(TicketFailure::NotFound)?;
    let conn = git_connections::Model::find_in(&txn, wid, id)
        .await?
        .ok_or(TicketFailure::NotFound)?;
    repos::Entity::update_many()
        .col_expr(repos::Column::WebhookId, Expr::value(Option::<i64>::None))
        .col_expr(repos::Column::WebhookError, Expr::value(String::new()))
        .filter(repos::Column::ConnectionId.eq(conn.id))
        .exec(&txn)
        .await?;
    audit::record(
        &txn,
        acting.actor().audit(
            AuditAction::GitDisconnected,
            (AuditSubject::GitConnection, conn.id),
            serde_json::json!({ "provider": conn.provider, "host": conn.host }),
        ),
    )
    .await?;
    conn.delete(&txn).await?;
    txn.commit().await?;
    Ok(())
}

/// URL webhook koneksi.
#[must_use]
pub fn webhook_url(settings: &Settings, conn: &git_connections::Model) -> String {
    format!("{}/webhooks/github/{}", settings.public_base_url, conn.id)
}

/// Tautkan repo ke koneksi provider untuk host URL-nya, lalu pasang webhook bila belum.
/// Repo non-GitHub atau host tanpa koneksi: tautan dilepas. Gagal pasang webhook tidak
/// menggagalkan tautan; kodenya disimpan di `webhook_error` (pasang manual).
///
/// # Errors
///
/// Error database.
pub async fn link_repo(
    db: &DatabaseConnection,
    settings: &Settings,
    wid: WorkspaceId,
    repo: repos::Model,
    acting: Option<&Acting<'_>>,
) -> Result<repos::Model, TicketFailure> {
    let path = (repo.provider == sea_orm::ActiveEnum::to_value(&RepoProvider::Github))
        .then(|| repo_path(&repo.url))
        .flatten();
    let conn = match &path {
        Some((host, _)) => git_connections::Model::for_host(db, wid, host).await?,
        None => None,
    };
    let full_name = path.map(|(_, f)| f).unwrap_or_default();
    let same = repo.connection_id == conn.as_ref().map(|c| c.id) && repo.full_name == full_name;
    let mut m = repo.clone().into_active_model();
    if !same {
        m.webhook_id = Set(None);
        m.webhook_error = Set(String::new());
    }
    m.connection_id = Set(conn.as_ref().map(|c| c.id));
    m.full_name = Set(full_name.clone());
    if let Some(conn) = conn.as_ref().filter(|c| c.is_app()) {
        // GitHub App: satu webhook App untuk semua repo, tidak dipasang per repo. Repo yang
        // tidak diberi akses ke instalasi ditandai supaya admin menambahkannya di GitHub.
        m.webhook_id = Set(None);
        m.webhook_error = Set(if conn.grants(&full_name) {
            String::new()
        } else {
            super::APP_REPO_NOT_GRANTED.to_string()
        });
    } else if let Some(conn) = &conn
        && (repo.webhook_id.is_none() || !same)
    {
        let secret = conn
            .webhook_secret(&key(settings)?)
            .map_err(|_| invalid("token", TOKEN_INVALID))?;
        let hook = match provider_for(conn, settings).await {
            Ok(p) => p
                .create_webhook(&full_name, &webhook_url(settings, conn), &secret)
                .await
                .map_err(|e| provider_code(&e)),
            Err(_) => Err(TOKEN_INVALID),
        };
        match hook {
            Ok(id) => {
                m.webhook_id = Set(Some(id));
                m.webhook_error = Set(String::new());
            }
            Err(code) => {
                m.webhook_id = Set(None);
                m.webhook_error = Set(code.to_string());
            }
        }
    }
    let txn = db.begin().await?;
    let updated = m.update(&txn).await?;
    if let (Some(acting), Some(conn)) = (acting, &conn)
        && (!same
            || updated.webhook_id != repo.webhook_id
            || updated.webhook_error != repo.webhook_error)
    {
        audit::record(
            &txn,
            acting.actor().audit(
                AuditAction::RepoLinked,
                (AuditSubject::Repo, updated.id),
                serde_json::json!({
                    "connection": conn.id,
                    "full_name": updated.full_name,
                    "webhook": updated.webhook_id.is_some(),
                    "webhook_error": updated.webhook_error,
                }),
            ),
        )
        .await?;
    }
    txn.commit().await?;
    Ok(updated)
}

/// Data instalasi App yang disimpan di koneksi.
struct AppSnapshot {
    account: String,
    account_type: String,
    permissions: serde_json::Value,
    selection: String,
    repos: Vec<String>,
    suspended: bool,
}

/// Ambil instalasi + repo yang dapat diakses dari GitHub.
async fn snapshot(
    client: &app::AppClient<'_>,
    installation_id: i64,
) -> Result<AppSnapshot, TicketFailure> {
    let inst = client
        .installation(installation_id)
        .await
        .map_err(|e| invalid("app", app_code(&e)))?;
    let repos = if inst.suspended {
        Vec::new()
    } else {
        let token = app::installation_token(client, installation_id)
            .await
            .map_err(|e| invalid("app", app_code(&e)))?;
        client
            .repositories(&token)
            .await
            .map_err(|e| invalid("app", provider_code(&e)))?
    };
    Ok(AppSnapshot {
        account: inst.account,
        account_type: inst.account_type,
        permissions: inst.permissions,
        selection: inst.repository_selection,
        repos,
        suspended: inst.suspended,
    })
}

fn apply_snapshot(m: &mut git_connections::ActiveModel, snap: &AppSnapshot) {
    m.account = Set(snap.account.clone());
    m.app_account_type = Set(snap.account_type.clone());
    m.app_permissions = Set(snap.permissions.clone());
    m.app_repo_selection = Set(snap.selection.clone());
    m.app_repos = Set(serde_json::json!(snap.repos));
    m.suspended_at = Set(snap.suspended.then(|| chrono::Utc::now().into()));
}

/// Hubungkan github.com lewat GitHub App setelah callback instalasi. `code` OAuth membuktikan
/// user yang memasang punya akses ke instalasi (`GET /user/installations`). Koneksi PAT yang
/// sudah ada untuk github.com diganti menjadi App.
///
/// # Errors
///
/// `NotFound` (izin), `git-app-not-configured`, `git-app-install-invalid`, kode provider,
/// error database.
pub async fn connect_app(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    settings: &Settings,
    installation_id: i64,
    code: &str,
) -> Result<git_connections::Model, TicketFailure> {
    if !acting.role.has(Permission::ManageIntegrations) {
        return Err(TicketFailure::NotFound);
    }
    if !settings.git.app.enabled() || !app::key_ok(&settings.git.app) {
        return Err(invalid("app", super::APP_NOT_CONFIGURED));
    }
    let (host, api_base, web_base) = endpoints(settings, GitProviderKind::Github, "")?;
    let client = app::AppClient {
        api_base: &api_base,
        web_base: &web_base,
        app: &settings.git.app,
        timeout: std::time::Duration::from_secs(settings.git.timeout_seconds.max(1)),
    };
    let code = code.trim();
    if code.is_empty() || code.len() > 200 {
        return Err(invalid("app", super::APP_INSTALL_INVALID));
    }
    let user_token = client
        .exchange_code(code)
        .await
        .map_err(|_| invalid("app", super::APP_INSTALL_INVALID))?;
    let mine = client
        .user_installations(&user_token)
        .await
        .map_err(|_| invalid("app", super::APP_INSTALL_INVALID))?;
    if !mine.contains(&installation_id) {
        return Err(invalid("app", super::APP_INSTALL_INVALID));
    }
    let snap = snapshot(&client, installation_id).await?;
    let key = key(settings)?;
    // Koneksi App tidak menyimpan token (token instalasi diminta saat dipakai); kolom wajib diisi
    // blob kosong terenkripsi.
    let empty = crypto::encrypt(&key, b"").map_err(|_| invalid("token", TOKEN_INVALID))?;
    let wid = acting.workspace_id();
    let txn = db.begin().await?;
    lock_as(&txn, acting, |r| r.has(Permission::ManageIntegrations))
        .await?
        .ok_or(TicketFailure::NotFound)?;
    let existing = git_connections::Model::for_host(&txn, wid, &host).await?;
    let (conn, action) = if let Some(old) = existing {
        let mut m = old.into_active_model();
        m.provider = Set(sea_orm::ActiveEnum::to_value(&GitProviderKind::Github));
        m.api_base = Set(api_base.clone());
        m.web_base = Set(web_base.clone());
        m.token_encrypted = Set(empty);
        m.auth = Set("app".into());
        m.installation_id = Set(Some(installation_id));
        apply_snapshot(&mut m, &snap);
        (m.update(&txn).await?, AuditAction::GitUpdated)
    } else {
        let secret = crypto::random_token();
        let webhook_secret_encrypted = crypto::encrypt(&key, secret.as_bytes())
            .map_err(|_| invalid("token", TOKEN_INVALID))?;
        let mut m = git_connections::ActiveModel {
            workspace_id: Set(wid.get()),
            provider: Set(sea_orm::ActiveEnum::to_value(&GitProviderKind::Github)),
            host: Set(host.clone()),
            api_base: Set(api_base.clone()),
            web_base: Set(web_base.clone()),
            token_encrypted: Set(empty),
            webhook_secret_encrypted: Set(webhook_secret_encrypted),
            created_by: Set(Some(acting.user.id)),
            auth: Set("app".into()),
            installation_id: Set(Some(installation_id)),
            ..Default::default()
        };
        apply_snapshot(&mut m, &snap);
        (m.insert(&txn).await?, AuditAction::GitConnected)
    };
    audit::record(
        &txn,
        acting.actor().audit(
            action,
            (AuditSubject::GitConnection, conn.id),
            serde_json::json!({
                "provider": conn.provider,
                "host": conn.host,
                "account": conn.account,
                "auth": "app",
                "installation": installation_id,
                "repos": snap.repos.len(),
            }),
        ),
    )
    .await?;
    txn.commit().await?;
    for repo in repos_on_host(db, wid, &host).await? {
        let _ = link_repo(db, settings, wid, repo, Some(acting)).await;
    }
    Ok(conn)
}

/// Perbarui data instalasi App koneksi (tombol Segarkan, webhook `installation*`), lalu
/// tautkan ulang repo di host-nya (status akses repo).
///
/// # Errors
///
/// Kode provider/App, error database.
pub async fn refresh_app(
    db: &DatabaseConnection,
    settings: &Settings,
    conn: git_connections::Model,
) -> Result<git_connections::Model, TicketFailure> {
    let Some(id) = conn.installation_id.filter(|_| conn.is_app()) else {
        return Ok(conn);
    };
    app::forget(&conn.api_base, id);
    let snap = snapshot(&super::app_client(&conn, settings), id).await?;
    let wid = crate::models::workspaces::Model::find_by_id(db, conn.workspace_id)
        .await?
        .ok_or(TicketFailure::NotFound)?
        .workspace_id();
    let host = conn.host.clone();
    let mut m = conn.into_active_model();
    apply_snapshot(&mut m, &snap);
    let conn = m.update(db).await?;
    for repo in repos_on_host(db, wid, &host).await? {
        let _ = link_repo(db, settings, wid, repo, None).await;
    }
    Ok(conn)
}

/// Segarkan koneksi App dari halaman Git provider (`ManageIntegrations`).
///
/// # Errors
///
/// `NotFound`, kode provider/App, error database.
pub async fn refresh(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    settings: &Settings,
    id: Uuid,
) -> Result<git_connections::Model, TicketFailure> {
    if !acting.role.has(Permission::ManageIntegrations) {
        return Err(TicketFailure::NotFound);
    }
    let conn = git_connections::Model::find_in(db, acting.workspace_id(), id)
        .await?
        .ok_or(TicketFailure::NotFound)?;
    refresh_app(db, settings, conn).await
}

/// Pasang ulang webhook repo `repo_id` (tombol di halaman Git provider).
///
/// # Errors
///
/// `NotFound` (repo/izin), error database.
pub async fn relink(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    settings: &Settings,
    repo_id: Uuid,
) -> Result<repos::Model, TicketFailure> {
    if !acting.role.has(Permission::ManageIntegrations) {
        return Err(TicketFailure::NotFound);
    }
    let wid = acting.workspace_id();
    let repo = repos::Entity::find_by_id(repo_id)
        .filter(repos::Column::WorkspaceId.eq(wid.get()))
        .one(db)
        .await?
        .ok_or(TicketFailure::NotFound)?;
    let mut m = repo.into_active_model();
    m.webhook_id = Set(None);
    let repo = m.update(db).await?;
    link_repo(db, settings, wid, repo, Some(acting)).await
}

/// Repo yang tertaut ke koneksi mana pun di workspace.
///
/// # Errors
///
/// Error database.
pub async fn linked_repos(
    db: &DatabaseConnection,
    wid: WorkspaceId,
) -> Result<Vec<repos::Model>, sea_orm::DbErr> {
    repos::Entity::find()
        .filter(repos::Column::WorkspaceId.eq(wid.get()))
        .filter(repos::Column::Provider.eq(sea_orm::ActiveEnum::to_value(&RepoProvider::Github)))
        .all(db)
        .await
}

#[cfg(test)]
mod ssrf_tests {
    use super::*;

    fn ghes(url: &str, allow_loopback: bool) -> bool {
        enterprise(url, allow_loopback).is_ok()
    }

    #[test]
    fn enterprise_url_rejects_internal_addresses() {
        for url in [
            "https://10.0.0.5",
            "https://192.168.1.1:8443",
            "https://169.254.169.254",
            "https://127.0.0.1",
            "http://127.0.0.1:8080",
            "https://localhost",
            "http://ghe.example.com",
        ] {
            assert!(!ghes(url, false), "{url}");
        }
        assert!(ghes("https://ghe.example.com", false));
        assert!(ghes("http://127.0.0.1:8080", true));
        assert!(!ghes("https://10.0.0.5", true));
    }
}
