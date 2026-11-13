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

use super::{TOKEN_INVALID, URL_INVALID, invalid, key, provider_code, provider_for};
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
        GitProviderKind::GithubEnterprise => {
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
            let ok_scheme = scheme == "https" || (scheme == "http" && loopback);
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
    }
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
    if let Some(conn) = &conn
        && (repo.webhook_id.is_none() || !same)
    {
        let secret = conn
            .webhook_secret(&key(settings)?)
            .map_err(|_| invalid("token", TOKEN_INVALID))?;
        let hook = match provider_for(conn, settings) {
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
        && (!same || updated.webhook_id != repo.webhook_id)
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
