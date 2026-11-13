//! Login CLI lewat device code flow (RFC 8628, spec "Login runner memakai device code flow").
//!
//! 1. CLI: `POST /oauth/device_authorization` (`client_id=agentgate-cli`) → `device_code` (rahasia,
//!    digest di DB) + `user_code` 8 huruf (`WDJB-MJHT`) berlaku [`DEVICE_TTL_MINUTES`].
//! 2. User membuka `/device`, mengetik kode, memilih workspace, lalu Setujui/Tolak.
//! 3. CLI polling `POST /oauth/token` grant [`GRANT_TYPE`] tiap `interval` detik →
//!    `authorization_pending` / `slow_down` / `access_denied` / `expired_token` / token.
//!
//! Hasilnya grant OAuth biasa (client × user × workspace) dengan token yang sama dengan MCP,
//! jadi tampil di Akun → Client AI dan bisa dicabut.

use chrono::{Duration, Utc};
use rand::Rng;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr,
    EntityTrait, IntoActiveModel, QueryFilter, TransactionTrait,
};

use super::oauth::{self, INVALID_GRANT, OAuthError, TokenResponse, oauth_error};
use crate::{
    common::crypto::{random_token, sha256_hex},
    models::{oauth_clients, oauth_device_codes, users, workspaces},
};

/// Client publik bawaan untuk CLI `agentgate`.
pub const CLI_CLIENT_ID: &str = "agentgate-cli";
pub const CLI_CLIENT_NAME: &str = "Agent Gate CLI";
pub const GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:device_code";
pub const DEVICE_TTL_MINUTES: i64 = 15;
pub const INTERVAL_SECONDS: i32 = 5;
const DEVICE_NAME_MAX: usize = 80;
/// Huruf tanpa vokal dan huruf mirip (RFC 8628 §6.1).
const ALPHABET: &[u8] = b"BCDFGHJKLMNPQRSTVWXZ";

/// Keadaan kode perangkat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceStatus {
    Pending,
    Approved,
    Denied,
    Used,
}

impl DeviceStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Denied => "denied",
            Self::Used => "used",
        }
    }
}

/// Respons `POST /oauth/device_authorization` (RFC 8628 §3.2).
#[derive(Debug, Clone, serde::Serialize)]
pub struct DeviceStart {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: String,
    pub expires_in: i64,
    pub interval: i32,
}

/// Client CLI bawaan (dibuat saat pertama dipakai).
///
/// # Errors
///
/// Error database.
pub async fn cli_client<C: ConnectionTrait>(db: &C) -> Result<oauth_clients::Model, DbErr> {
    if let Some(c) = oauth_clients::Entity::find()
        .filter(oauth_clients::Column::ClientId.eq(CLI_CLIENT_ID))
        .one(db)
        .await?
    {
        return Ok(c);
    }
    let inserted = oauth_clients::ActiveModel {
        client_id: Set(CLI_CLIENT_ID.to_string()),
        name: Set(CLI_CLIENT_NAME.to_string()),
        redirect_uris: Set(Vec::new()),
        ..Default::default()
    }
    .insert(db)
    .await;
    match inserted {
        Ok(c) => Ok(c),
        // Dua permintaan bersamaan: yang kalah membaca baris pemenang.
        Err(_) => oauth_clients::Entity::find()
            .filter(oauth_clients::Column::ClientId.eq(CLI_CLIENT_ID))
            .one(db)
            .await?
            .ok_or_else(|| DbErr::RecordNotFound(CLI_CLIENT_ID.into())),
    }
}

fn new_user_code() -> String {
    let mut rng = rand::rng();
    (0..8)
        .map(|_| ALPHABET[rng.random_range(0..ALPHABET.len())] as char)
        .collect()
}

pub use crate::models::oauth_device_codes::format_user_code;

/// Kode ketikan user → bentuk tersimpan (8 huruf alfabet), mengabaikan spasi/strip dan huruf
/// kecil. `None` bila bentuknya tidak mungkin valid.
#[must_use]
pub fn normalize_user_code(input: &str) -> Option<String> {
    let code: String = input
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_uppercase())
        .collect();
    (code.len() == 8 && code.bytes().all(|b| ALPHABET.contains(&b))).then_some(code)
}

/// Mulai login perangkat.
///
/// # Errors
///
/// `invalid_client` bila bukan client CLI; error database.
pub async fn start(
    db: &DatabaseConnection,
    client_id: &str,
    device_name: &str,
    base_url: &str,
) -> Result<DeviceStart, OAuthError> {
    if client_id != CLI_CLIENT_ID {
        return Err(oauth_error("invalid_client", "unknown client_id"));
    }
    let client = cli_client(db).await?;
    let device_code = random_token();
    let device_name: String = device_name.trim().chars().take(DEVICE_NAME_MAX).collect();
    // Tabrakan user_code pending (kecil sekali) → coba kode lain.
    for _ in 0..5 {
        let user_code = new_user_code();
        let row = oauth_device_codes::ActiveModel {
            client_id: Set(client.id),
            device_code_digest: Set(sha256_hex(&device_code)),
            user_code: Set(user_code.clone()),
            device_name: Set(device_name.clone()),
            status: Set(DeviceStatus::Pending.as_str().into()),
            interval_seconds: Set(INTERVAL_SECONDS),
            expires_at: Set((Utc::now() + Duration::minutes(DEVICE_TTL_MINUTES)).into()),
            ..Default::default()
        }
        .insert(db)
        .await;
        if row.is_ok() {
            let display = format_user_code(&user_code);
            return Ok(DeviceStart {
                device_code,
                verification_uri: format!("{base_url}/device"),
                verification_uri_complete: format!("{base_url}/device?code={display}"),
                user_code: display,
                expires_in: DEVICE_TTL_MINUTES * 60,
                interval: INTERVAL_SECONDS,
            });
        }
    }
    Err(oauth_error("server_error", "could not allocate user code"))
}

/// Kode perangkat yang masih menunggu persetujuan untuk `user_code` (ketikan user).
///
/// # Errors
///
/// Error database.
pub async fn find_pending<C: ConnectionTrait>(
    db: &C,
    user_code: &str,
) -> Result<Option<oauth_device_codes::Model>, DbErr> {
    let Some(code) = normalize_user_code(user_code) else {
        return Ok(None);
    };
    Ok(oauth_device_codes::Entity::find()
        .filter(oauth_device_codes::Column::UserCode.eq(code))
        .filter(oauth_device_codes::Column::Status.eq(DeviceStatus::Pending.as_str()))
        .one(db)
        .await?
        .filter(|r| r.expires_at > Utc::now()))
}

/// User menyetujui kode untuk `workspace_id`. `false` bila kode tidak (lagi) menunggu atau user
/// tidak boleh memakai workspace itu.
///
/// # Errors
///
/// Error database.
pub async fn approve(
    db: &DatabaseConnection,
    user: &users::Model,
    workspace_id: uuid::Uuid,
    user_code: &str,
    request_id: Option<String>,
) -> Result<bool, DbErr> {
    let Some((workspace, _)) = oauth::consent_workspaces(db, user)
        .await?
        .into_iter()
        .find(|(w, _)| w.id == workspace_id)
    else {
        return Ok(false);
    };
    let txn = db.begin().await?;
    let Some(row) = find_pending(&txn, user_code).await? else {
        return Ok(false);
    };
    let client = cli_client(&txn).await?;
    let grant = oauth::ensure_grant(&txn, &client, user.id, &workspace, request_id).await?;
    let updated = oauth_device_codes::Entity::update_many()
        .col_expr(
            oauth_device_codes::Column::Status,
            sea_orm::sea_query::Expr::value(DeviceStatus::Approved.as_str()),
        )
        .col_expr(
            oauth_device_codes::Column::UserId,
            sea_orm::sea_query::Expr::value(user.id),
        )
        .col_expr(
            oauth_device_codes::Column::WorkspaceId,
            sea_orm::sea_query::Expr::value(workspace.id),
        )
        .col_expr(
            oauth_device_codes::Column::GrantId,
            sea_orm::sea_query::Expr::value(grant.id),
        )
        .filter(oauth_device_codes::Column::Id.eq(row.id))
        .filter(oauth_device_codes::Column::Status.eq(DeviceStatus::Pending.as_str()))
        .exec(&txn)
        .await?;
    if updated.rows_affected != 1 {
        return Ok(false);
    }
    txn.commit().await?;
    Ok(true)
}

/// User menolak kode. `false` bila kode tidak (lagi) menunggu.
///
/// # Errors
///
/// Error database.
pub async fn deny(
    db: &DatabaseConnection,
    user: &users::Model,
    user_code: &str,
) -> Result<bool, DbErr> {
    let Some(row) = find_pending(db, user_code).await? else {
        return Ok(false);
    };
    let mut m = row.into_active_model();
    m.status = Set(DeviceStatus::Denied.as_str().into());
    m.user_id = Set(Some(user.id));
    m.update(db).await?;
    Ok(true)
}

/// Polling token oleh CLI (RFC 8628 §3.4–3.5).
///
/// # Errors
///
/// `authorization_pending`, `slow_down`, `access_denied`, `expired_token`, `invalid_grant`,
/// error database.
pub async fn poll(
    db: &DatabaseConnection,
    client_id: &str,
    device_code: &str,
) -> Result<TokenResponse, OAuthError> {
    if client_id != CLI_CLIENT_ID {
        return Err(oauth_error("invalid_client", "unknown client_id"));
    }
    let txn = db.begin().await?;
    let Some(row) = oauth_device_codes::Entity::find()
        .filter(oauth_device_codes::Column::DeviceCodeDigest.eq(sha256_hex(device_code)))
        .one(&txn)
        .await?
    else {
        return Err(INVALID_GRANT);
    };
    let now = Utc::now();
    let too_fast = row.last_polled_at.is_some_and(|t| {
        now.signed_duration_since(t) < Duration::seconds(i64::from(row.interval_seconds))
    });
    let status = row.status.clone();
    let expired = row.expires_at < now;
    let grant_id = row.grant_id;
    let interval = row.interval_seconds;
    let mut m = row.into_active_model();
    m.last_polled_at = Set(Some(now.into()));
    match status.as_str() {
        "pending" if expired => {
            m.update(&txn).await?;
            txn.commit().await?;
            Err(oauth_error("expired_token", "device code expired"))
        }
        "pending" if too_fast => {
            m.interval_seconds = Set((interval + 5).min(60));
            m.update(&txn).await?;
            txn.commit().await?;
            Err(oauth_error("slow_down", "polling too fast"))
        }
        "pending" => {
            m.update(&txn).await?;
            txn.commit().await?;
            Err(oauth_error(
                "authorization_pending",
                "waiting for user approval",
            ))
        }
        "denied" => Err(oauth_error("access_denied", "user denied the request")),
        "approved" => {
            let Some(grant_id) = grant_id else {
                return Err(INVALID_GRANT);
            };
            m.status = Set(DeviceStatus::Used.as_str().into());
            m.update(&txn).await?;
            let tokens = oauth::issue(&txn, grant_id).await?;
            txn.commit().await?;
            Ok(tokens)
        }
        _ => Err(INVALID_GRANT),
    }
}

/// Workspace yang bisa dipilih user saat menyetujui (sama dengan consent MCP).
///
/// # Errors
///
/// Error database.
pub async fn choices<C: ConnectionTrait>(
    db: &C,
    user: &users::Model,
) -> Result<Vec<(workspaces::Model, crate::models::roles::Model)>, DbErr> {
    oauth::consent_workspaces(db, user).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_codes_normalize() {
        assert_eq!(
            normalize_user_code("wdjb-mjht").as_deref(),
            Some("WDJBMJHT")
        );
        assert_eq!(
            normalize_user_code(" WDJB MJHT ").as_deref(),
            Some("WDJBMJHT")
        );
        assert_eq!(normalize_user_code("WDJB-MJH"), None);
        assert_eq!(normalize_user_code("AEIO-UAEI"), None);
        assert_eq!(format_user_code("WDJBMJHT"), "WDJB-MJHT");
        let c = new_user_code();
        assert_eq!(normalize_user_code(&c), Some(c));
    }
}
