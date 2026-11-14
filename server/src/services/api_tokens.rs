//! Token API pribadi (M11): buat (token mentah tampil sekali), cabut, dan autentikasi request
//! `/api/v1` (bearer `ag_pat_…`) beserta rate limit per token.
//!
//! Token bertindak sebagai pembuatnya: [`authenticate`] memuat ulang user, keanggotaan, dan
//! role terbaru setiap request (seperti sesi MCP), jadi user yang dinonaktifkan, keluar dari
//! workspace, atau kehilangan akses project langsung kehilangan akses lewat token juga. Cakupan
//! dan batas project token adalah pembatas tambahan di atas izin role, bukan pengganti.

use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbErr, TransactionTrait};
use uuid::Uuid;

use crate::{
    common::text,
    models::{
        api_rate_limits,
        api_tokens::{self, ApiScope, NewApiToken, TokenExpiry},
        permission::Permission,
        projects, roles, users, workspace_members,
        workspaces::{self},
    },
    services::{
        audit::{self, AuditAction, AuditSubject},
        tickets::{TicketFailure, access_in},
        workspaces::{Acting, lock_as},
    },
};

pub const SCOPES_REQUIRED: &str = "api-token-scopes-required";
pub const EXPIRY_INVALID: &str = "api-token-expiry-invalid";
pub const PROJECT_INVALID: &str = "api-token-project-invalid";
/// Kode error yang dipakai dari Rust (kontrak i18n).
pub const ERROR_CODES: &[&str] = &[SCOPES_REQUIRED, EXPIRY_INVALID, PROJECT_INVALID];

/// Isian form Token baru.
#[derive(Debug, Clone, Default)]
pub struct TokenInput {
    pub name: String,
    pub scopes: Vec<String>,
    /// `30`, `90`, `365`, `never`.
    pub expiry: String,
    /// Kunci project; kosong = semua project.
    pub project: String,
}

/// Buat token untuk pelaku (`ManageIntegrations`). Token bertindak sebagai pelaku.
///
/// # Errors
///
/// `Invalid` (nama, cakupan, masa berlaku, project), `NotFound` (tanpa izin), error database.
pub async fn create(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    input: &TokenInput,
) -> Result<(api_tokens::Model, String), TicketFailure> {
    if !acting.role.has(Permission::ManageIntegrations) {
        return Err(TicketFailure::NotFound);
    }
    let name = text::normalize_name(&input.name);
    if let Err(e) = text::validate_name("api_token", &name) {
        let mut errors = validator::ValidationErrors::new();
        errors.add("name", e);
        return Err(TicketFailure::errors(errors));
    }
    let mut scopes: Vec<ApiScope> = Vec::new();
    for s in &input.scopes {
        match ApiScope::parse(s) {
            Some(scope) if !scopes.contains(&scope) => scopes.push(scope),
            Some(_) => {}
            None => return Err(TicketFailure::field("scopes", SCOPES_REQUIRED)),
        }
    }
    if scopes.is_empty() {
        return Err(TicketFailure::field("scopes", SCOPES_REQUIRED));
    }
    let expiry =
        TokenExpiry::parse(&input.expiry).ok_or(TicketFailure::field("expiry", EXPIRY_INVALID))?;
    let wid = acting.workspace_id();
    let project_id = match input.project.trim() {
        "" | "all" => None,
        key => {
            let project = projects::Model::find_by_key(db, wid, key)
                .await?
                .ok_or(TicketFailure::field("project", PROJECT_INVALID))?;
            access_in(db, acting, project.id, Permission::ViewProject)
                .await
                .map_err(|_| TicketFailure::field("project", PROJECT_INVALID))?;
            Some(project.id)
        }
    };
    let txn = db.begin().await?;
    lock_as(&txn, acting, |r| r.has(Permission::ManageIntegrations))
        .await?
        .ok_or(TicketFailure::NotFound)?;
    let (token, raw) = api_tokens::ActiveModel::create(
        &txn,
        NewApiToken {
            workspace_id: wid,
            user_id: acting.user.id,
            project_id,
            name,
            scopes,
            expires_at: expiry.expires_at(Utc::now()),
        },
    )
    .await?;
    audit::record(
        &txn,
        acting.actor().audit(
            AuditAction::ApiTokenCreated,
            (AuditSubject::ApiToken, token.id),
            serde_json::json!({
                "name": token.name,
                "prefix": token.prefix,
                "scopes": token.scopes,
                "project_id": token.project_id,
                "expires_at": token.expires_at,
            }),
        ),
    )
    .await?;
    txn.commit().await?;
    Ok((token, raw))
}

/// Cabut token workspace (`ManageIntegrations`).
///
/// # Errors
///
/// `NotFound` (bukan milik workspace, tanpa izin), error database.
pub async fn revoke(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    token_id: Uuid,
) -> Result<api_tokens::Model, TicketFailure> {
    let txn = db.begin().await?;
    lock_as(&txn, acting, |r| r.has(Permission::ManageIntegrations))
        .await?
        .ok_or(TicketFailure::NotFound)?;
    let token = api_tokens::Model::find_in(&txn, acting.workspace_id(), token_id)
        .await?
        .filter(|t| t.revoked_at.is_none())
        .ok_or(TicketFailure::NotFound)?;
    let token = api_tokens::ActiveModel::revoke(&txn, token).await?;
    audit::record(
        &txn,
        acting.actor().audit(
            AuditAction::ApiTokenRevoked,
            (AuditSubject::ApiToken, token.id),
            serde_json::json!({ "name": token.name, "prefix": token.prefix }),
        ),
    )
    .await?;
    txn.commit().await?;
    Ok(token)
}

/// Pemanggil API yang sudah terautentikasi: token dan pembuatnya (keadaan terbaru).
#[derive(Debug, Clone)]
pub struct ApiCaller {
    pub token: api_tokens::Model,
    pub user: users::Model,
    pub workspace: workspaces::Model,
    pub role: roles::Model,
    /// Sisa request di menit ini.
    pub remaining: i32,
}

impl ApiCaller {
    #[must_use]
    pub fn acting(&self) -> Acting<'_> {
        Acting {
            workspace: &self.workspace,
            user: &self.user,
            role: &self.role,
            request_id: None,
        }
    }

    #[must_use]
    pub fn has(&self, scope: ApiScope) -> bool {
        self.token.has(scope)
    }

    /// Project `id` berada dalam batas token (token tanpa project = semua).
    #[must_use]
    pub fn allows_project(&self, project_id: Uuid) -> bool {
        self.token.project_id.is_none_or(|p| p == project_id)
    }
}

/// Hasil autentikasi.
#[derive(Debug)]
pub enum AuthOutcome {
    Ok(Box<ApiCaller>),
    /// Token tidak dikenal, dicabut, kedaluwarsa, atau pembuatnya tidak lagi berhak.
    Invalid,
    /// Melewati batas request per menit.
    Limited,
}

/// Autentikasi token mentah pada `now` dan hitung rate limit-nya (`limit` request/menit).
///
/// # Errors
///
/// Error database.
pub async fn authenticate<C: ConnectionTrait>(
    db: &C,
    raw: &str,
    limit: i32,
    now: DateTime<Utc>,
) -> Result<AuthOutcome, DbErr> {
    let Some(token) = api_tokens::Model::find_by_raw(db, raw)
        .await?
        .filter(|t| t.is_usable(now))
    else {
        return Ok(AuthOutcome::Invalid);
    };
    let Some(user) = users::Model::find_by_id(db, token.user_id)
        .await?
        .filter(users::Model::is_active)
    else {
        return Ok(AuthOutcome::Invalid);
    };
    let Some(workspace) = workspaces::Model::find_by_id(db, token.workspace_id).await? else {
        return Ok(AuthOutcome::Invalid);
    };
    let Some((_, role)) =
        workspace_members::Model::find_with_role(db, workspace.workspace_id(), user.id).await?
    else {
        return Ok(AuthOutcome::Invalid);
    };
    if !workspace.is_open_to(&role) || workspace.requires_two_factor_from(&user) {
        return Ok(AuthOutcome::Invalid);
    }
    let used = api_rate_limits::hit(db, token.id, now).await?;
    if used > limit {
        return Ok(AuthOutcome::Limited);
    }
    api_tokens::Model::touch(db, token.id, now).await?;
    Ok(AuthOutcome::Ok(Box::new(ApiCaller {
        token,
        user,
        workspace,
        role,
        remaining: (limit - used).max(0),
    })))
}
