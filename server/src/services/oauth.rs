//! Authorization server OAuth 2.1 untuk MCP (spec "MCP, slash command & aturan project"):
//! dynamic client registration (RFC 7591), kode otorisasi dengan PKCE S256 wajib, token opak
//! (digest SHA-256 di database), refresh token berotasi (dipakai ulang → grant dicabut),
//! pencabutan (RFC 7009).
//!
//! **Grant** = satu client × user × workspace ("Client terhubung"). Token tidak pernah memberi
//! lebih dari user: setiap panggilan MCP membaca ulang keanggotaan dan role user
//! ([`authenticate`]).

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{Duration, Utc};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr,
    EntityTrait, IntoActiveModel, QueryFilter, QueryOrder, TransactionTrait,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    common::crypto::{random_token, sha256_hex},
    models::{
        oauth_clients, oauth_codes, oauth_grants, oauth_tokens, users, workspace_members,
        workspaces,
    },
    services::audit::{self, AuditAction, AuditEntry, AuditSubject},
};

pub const SCOPE: &str = "mcp";
pub const ACCESS_TTL_SECONDS: i64 = 3600;
pub const REFRESH_TTL_DAYS: i64 = 30;
pub const CODE_TTL_MINUTES: i64 = 10;
const MAX_REDIRECT_URIS: usize = 5;
const MAX_URI_LEN: usize = 512;
const MAX_NAME_LEN: usize = 100;

/// Error OAuth (RFC 6749 §5.2 / RFC 7591 §3.2.2): kode + deskripsi untuk respons JSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthError {
    pub code: &'static str,
    pub description: &'static str,
}

pub(crate) const fn oauth_error(code: &'static str, description: &'static str) -> OAuthError {
    OAuthError { code, description }
}

impl From<DbErr> for OAuthError {
    fn from(e: DbErr) -> Self {
        tracing::error!(error = %e, "oauth db error");
        oauth_error("server_error", "internal error")
    }
}

// ---------------------------------------------------------------------------------------------
// Registrasi client

/// Metadata client dari `POST /oauth/register`.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct ClientMetadata {
    #[serde(default)]
    pub client_name: Option<String>,
    #[serde(default)]
    pub redirect_uris: Vec<String>,
    #[serde(default)]
    pub client_uri: Option<String>,
    #[serde(default)]
    pub token_endpoint_auth_method: Option<String>,
    #[serde(default)]
    pub grant_types: Option<Vec<String>>,
    #[serde(default)]
    pub response_types: Option<Vec<String>>,
}

/// Redirect URI yang boleh didaftarkan: `https://…`, `http://` loopback (127.0.0.1, `[::1]`,
/// localhost; port bebas), atau skema privat aplikasi (RFC 8252 §7.1: mengandung titik, mis.
/// `com.example.app:/cb`, atau `cursor`/`vscode`). Tanpa fragment.
#[must_use]
pub fn redirect_uri_allowed(uri: &str) -> bool {
    if uri.len() > MAX_URI_LEN || uri.contains('#') || uri.chars().any(char::is_whitespace) {
        return false;
    }
    let Some((scheme, rest)) = uri.split_once(':') else {
        return false;
    };
    let scheme = scheme.to_ascii_lowercase();
    match scheme.as_str() {
        "https" => rest.starts_with("//") && rest.len() > 2,
        "http" => {
            let Some(after) = rest.strip_prefix("//") else {
                return false;
            };
            let host = after.split(['/', '?']).next().unwrap_or_default();
            let host = host.rsplit_once(':').map_or(host, |(h, port)| {
                if port.chars().all(|c| c.is_ascii_digit()) {
                    h
                } else {
                    host
                }
            });
            matches!(host, "127.0.0.1" | "localhost" | "[::1]")
        }
        "javascript" | "data" | "file" | "vbscript" | "blob" | "about" => false,
        s => {
            s.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'))
                && s.starts_with(|c: char| c.is_ascii_alphabetic())
                && (s.contains('.') || matches!(s, "cursor" | "vscode" | "vscode-insiders"))
        }
    }
}

/// Daftarkan client publik (`token_endpoint_auth_method = none`).
///
/// # Errors
///
/// `invalid_redirect_uri` / `invalid_client_metadata`, error database.
pub async fn register(
    db: &DatabaseConnection,
    meta: &ClientMetadata,
) -> Result<oauth_clients::Model, OAuthError> {
    if meta.redirect_uris.is_empty()
        || meta.redirect_uris.len() > MAX_REDIRECT_URIS
        || !meta.redirect_uris.iter().all(|u| redirect_uri_allowed(u))
    {
        return Err(oauth_error(
            "invalid_redirect_uri",
            "redirect_uris must be https, loopback http, or a private-use scheme",
        ));
    }
    if meta
        .token_endpoint_auth_method
        .as_deref()
        .is_some_and(|m| m != "none")
    {
        return Err(oauth_error(
            "invalid_client_metadata",
            "only public clients (token_endpoint_auth_method=none) are supported",
        ));
    }
    if meta.grant_types.as_ref().is_some_and(|g| {
        g.iter()
            .any(|t| t != "authorization_code" && t != "refresh_token")
    }) || meta
        .response_types
        .as_ref()
        .is_some_and(|r| r.iter().any(|t| t != "code"))
    {
        return Err(oauth_error(
            "invalid_client_metadata",
            "unsupported grant_types or response_types",
        ));
    }
    let name: String = meta
        .client_name
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .unwrap_or("Client MCP")
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_NAME_LEN)
        .collect();
    let client_uri = meta
        .client_uri
        .as_deref()
        .map(str::trim)
        .filter(|u| u.starts_with("https://") && u.len() <= MAX_URI_LEN)
        .map(str::to_string);
    let client = oauth_clients::ActiveModel {
        client_id: Set(format!("ag_{}", random_token())),
        name: Set(name),
        redirect_uris: Set(meta.redirect_uris.clone()),
        client_uri: Set(client_uri),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(client)
}

/// Client terdaftar dengan `client_id`.
///
/// # Errors
///
/// Error database.
pub async fn find_client<C: ConnectionTrait>(
    db: &C,
    client_id: &str,
) -> Result<Option<oauth_clients::Model>, DbErr> {
    oauth_clients::Entity::find()
        .filter(oauth_clients::Column::ClientId.eq(client_id))
        .one(db)
        .await
}

// ---------------------------------------------------------------------------------------------
// Otorisasi

/// Parameter `GET/POST /oauth/authorize`.
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct AuthorizeParams {
    #[serde(default)]
    pub response_type: String,
    #[serde(default)]
    pub client_id: String,
    #[serde(default)]
    pub redirect_uri: String,
    #[serde(default)]
    pub code_challenge: String,
    #[serde(default)]
    pub code_challenge_method: String,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub resource: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
}

/// Hasil pemeriksaan permintaan otorisasi.
#[derive(Debug)]
pub enum AuthorizeCheck {
    /// Client atau redirect URI tidak dikenal: tampilkan halaman error, jangan redirect.
    BadClient,
    /// Redirect ke client dengan `error` (kode RFC 6749 §4.1.2.1).
    Redirect(String),
    Ok(oauth_clients::Model),
}

/// `?code=…`/`?error=…` ditambahkan ke redirect URI client (beserta `state` dan `iss`).
#[must_use]
pub fn redirect_with(redirect_uri: &str, pairs: &[(&str, &str)]) -> String {
    let query: String = pairs
        .iter()
        .map(|(k, v)| format!("{k}={}", urlencode(v)))
        .collect::<Vec<_>>()
        .join("&");
    let sep = if redirect_uri.contains('?') { '&' } else { '?' };
    format!("{redirect_uri}{sep}{query}")
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn pkce_value_ok(v: &str) -> bool {
    (43..=128).contains(&v.len())
        && v.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | '~'))
}

/// Periksa permintaan otorisasi (`resource_url` = URL MCP).
///
/// # Errors
///
/// Error database.
pub async fn check_authorize<C: ConnectionTrait>(
    db: &C,
    p: &AuthorizeParams,
    resource_url: &str,
    issuer: &str,
) -> Result<AuthorizeCheck, DbErr> {
    let Some(client) = find_client(db, &p.client_id).await? else {
        return Ok(AuthorizeCheck::BadClient);
    };
    if !client.redirect_uris.iter().any(|u| u == &p.redirect_uri) {
        return Ok(AuthorizeCheck::BadClient);
    }
    let fail = |code: &str, description: &str| {
        let mut pairs = vec![("error", code), ("error_description", description)];
        if let Some(s) = p.state.as_deref() {
            pairs.push(("state", s));
        }
        pairs.push(("iss", issuer));
        Ok(AuthorizeCheck::Redirect(redirect_with(
            &p.redirect_uri,
            &pairs,
        )))
    };
    if p.response_type != "code" {
        return fail("unsupported_response_type", "response_type must be code");
    }
    if p.code_challenge_method != "S256" || !pkce_value_ok(&p.code_challenge) {
        return fail("invalid_request", "PKCE with S256 is required");
    }
    if p.resource
        .as_deref()
        .is_some_and(|r| r.trim_end_matches('/') != resource_url)
    {
        return fail("invalid_target", "unknown resource");
    }
    if p.scope
        .as_deref()
        .is_some_and(|s| s.split(' ').any(|x| !x.is_empty() && x != SCOPE))
    {
        return fail("invalid_scope", "only the mcp scope is supported");
    }
    Ok(AuthorizeCheck::Ok(client))
}

/// Workspace yang boleh dipilih user di halaman consent (anggota, 2FA terpenuhi).
///
/// # Errors
///
/// Error database.
pub async fn consent_workspaces<C: ConnectionTrait>(
    db: &C,
    user: &users::Model,
) -> Result<Vec<(workspaces::Model, crate::models::roles::Model)>, DbErr> {
    Ok(workspace_members::Model::workspaces_of(db, user.id)
        .await?
        .into_iter()
        .filter(|(w, _)| !w.requires_two_factor_from(user))
        .collect())
}

/// User menyetujui: grant aktif (dibuat bila belum ada) + kode otorisasi sekali pakai.
/// Mengembalikan kode mentah. `None` bila user bukan (lagi) anggota workspace itu.
///
/// # Errors
///
/// Error database.
pub async fn approve(
    db: &DatabaseConnection,
    client: &oauth_clients::Model,
    user: &users::Model,
    workspace_id: Uuid,
    p: &AuthorizeParams,
    resource_url: &str,
    request_id: Option<String>,
) -> Result<Option<String>, DbErr> {
    let Some((workspace, _)) = consent_workspaces(db, user)
        .await?
        .into_iter()
        .find(|(w, _)| w.id == workspace_id)
    else {
        return Ok(None);
    };
    let txn = db.begin().await?;
    let grant = ensure_grant(&txn, client, user.id, &workspace, request_id).await?;
    let code = random_token();
    oauth_codes::ActiveModel {
        grant_id: Set(grant.id),
        code_digest: Set(sha256_hex(&code)),
        redirect_uri: Set(p.redirect_uri.clone()),
        code_challenge: Set(p.code_challenge.clone()),
        resource: Set(resource_url.to_string()),
        expires_at: Set((Utc::now() + Duration::minutes(CODE_TTL_MINUTES)).into()),
        ..Default::default()
    }
    .insert(&txn)
    .await?;
    txn.commit().await?;
    Ok(Some(code))
}

/// Grant aktif client × user × workspace; dibuat (dengan audit `mcp.connected`) bila belum ada.
///
/// # Errors
///
/// Error database.
pub(crate) async fn ensure_grant<C: ConnectionTrait>(
    txn: &C,
    client: &oauth_clients::Model,
    user_id: Uuid,
    workspace: &workspaces::Model,
    request_id: Option<String>,
) -> Result<oauth_grants::Model, DbErr> {
    let existing = oauth_grants::Entity::find()
        .filter(oauth_grants::Column::WorkspaceId.eq(workspace.id))
        .filter(oauth_grants::Column::UserId.eq(user_id))
        .filter(oauth_grants::Column::ClientId.eq(client.id))
        .filter(oauth_grants::Column::RevokedAt.is_null())
        .one(txn)
        .await?;
    if let Some(g) = existing {
        return Ok(g);
    }
    let g = oauth_grants::ActiveModel {
        workspace_id: Set(workspace.id),
        user_id: Set(user_id),
        client_id: Set(client.id),
        scope: Set(SCOPE.to_string()),
        ..Default::default()
    }
    .insert(txn)
    .await?;
    audit::record(
        txn,
        AuditEntry {
            workspace_id: workspace.workspace_id(),
            actor_id: Some(user_id),
            action: AuditAction::McpConnected,
            subject: (AuditSubject::McpClient, g.id),
            changes: serde_json::json!({ "client": client.name }),
            request_id,
        },
    )
    .await?;
    Ok(g)
}

// ---------------------------------------------------------------------------------------------
// Token

/// Respons token (RFC 6749 §5.1).
#[derive(Debug, Clone, serde::Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: &'static str,
    pub expires_in: i64,
    pub refresh_token: String,
    pub scope: &'static str,
}

pub(crate) const INVALID_GRANT: OAuthError =
    oauth_error("invalid_grant", "invalid, expired, or used grant");

pub(crate) async fn issue<C: ConnectionTrait>(
    db: &C,
    grant_id: Uuid,
) -> Result<TokenResponse, DbErr> {
    let access = random_token();
    let refresh = random_token();
    let now = Utc::now();
    for (kind, token, expires) in [
        (
            "access",
            &access,
            now + Duration::seconds(ACCESS_TTL_SECONDS),
        ),
        ("refresh", &refresh, now + Duration::days(REFRESH_TTL_DAYS)),
    ] {
        oauth_tokens::ActiveModel {
            grant_id: Set(grant_id),
            kind: Set(kind.to_string()),
            token_digest: Set(sha256_hex(token)),
            expires_at: Set(expires.into()),
            ..Default::default()
        }
        .insert(db)
        .await?;
    }
    Ok(TokenResponse {
        access_token: access,
        token_type: "Bearer",
        expires_in: ACCESS_TTL_SECONDS,
        refresh_token: refresh,
        scope: SCOPE,
    })
}

async fn active_grant<C: ConnectionTrait>(
    db: &C,
    id: Uuid,
) -> Result<Option<oauth_grants::Model>, DbErr> {
    Ok(oauth_grants::Entity::find_by_id(id)
        .one(db)
        .await?
        .filter(|g| g.revoked_at.is_none()))
}

pub(crate) async fn revoke_grant<C: ConnectionTrait>(db: &C, grant_id: Uuid) -> Result<(), DbErr> {
    oauth_grants::Entity::update_many()
        .col_expr(
            oauth_grants::Column::RevokedAt,
            sea_orm::sea_query::Expr::value(chrono::DateTime::<chrono::FixedOffset>::from(
                Utc::now(),
            )),
        )
        .filter(oauth_grants::Column::Id.eq(grant_id))
        .filter(oauth_grants::Column::RevokedAt.is_null())
        .exec(db)
        .await?;
    Ok(())
}

fn s256(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// `grant_type=authorization_code`: tukar kode + `code_verifier` dengan token. Kode yang
/// dipakai ulang mencabut grant-nya (RFC 6749 §4.1.2).
///
/// # Errors
///
/// `invalid_grant`, `invalid_request`, error database.
pub async fn exchange_code(
    db: &DatabaseConnection,
    client_id: &str,
    code: &str,
    redirect_uri: &str,
    verifier: &str,
) -> Result<TokenResponse, OAuthError> {
    if !pkce_value_ok(verifier) {
        return Err(oauth_error("invalid_request", "code_verifier is required"));
    }
    let txn = db.begin().await?;
    let Some(row) = oauth_codes::Entity::find()
        .filter(oauth_codes::Column::CodeDigest.eq(sha256_hex(code)))
        .one(&txn)
        .await?
    else {
        return Err(INVALID_GRANT);
    };
    if row.used_at.is_some() {
        revoke_grant(&txn, row.grant_id).await?;
        txn.commit().await?;
        return Err(INVALID_GRANT);
    }
    let Some(grant) = active_grant(&txn, row.grant_id).await? else {
        return Err(INVALID_GRANT);
    };
    let client = oauth_clients::Entity::find_by_id(grant.client_id)
        .one(&txn)
        .await?;
    let expired = row.expires_at < Utc::now();
    let pkce_ok =
        crate::common::crypto::ct_eq(s256(verifier).as_bytes(), row.code_challenge.as_bytes());
    if expired
        || client.is_none_or(|c| c.client_id != client_id)
        || row.redirect_uri != redirect_uri
        || !pkce_ok
    {
        return Err(INVALID_GRANT);
    }
    let mut used = row.into_active_model();
    used.used_at = Set(Some(Utc::now().into()));
    used.update(&txn).await?;
    let tokens = issue(&txn, grant.id).await?;
    txn.commit().await?;
    Ok(tokens)
}

/// `grant_type=refresh_token`: rotasi. Refresh token yang dipakai ulang mencabut grant.
///
/// # Errors
///
/// `invalid_grant`, error database.
pub async fn refresh(
    db: &DatabaseConnection,
    client_id: &str,
    refresh_token: &str,
) -> Result<TokenResponse, OAuthError> {
    let txn = db.begin().await?;
    let Some(row) = oauth_tokens::Entity::find()
        .filter(oauth_tokens::Column::TokenDigest.eq(sha256_hex(refresh_token)))
        .filter(oauth_tokens::Column::Kind.eq("refresh"))
        .one(&txn)
        .await?
    else {
        return Err(INVALID_GRANT);
    };
    if row.used_at.is_some() {
        revoke_grant(&txn, row.grant_id).await?;
        txn.commit().await?;
        return Err(INVALID_GRANT);
    }
    let Some(grant) = active_grant(&txn, row.grant_id).await? else {
        return Err(INVALID_GRANT);
    };
    let client = oauth_clients::Entity::find_by_id(grant.client_id)
        .one(&txn)
        .await?;
    if row.expires_at < Utc::now() || client.is_none_or(|c| c.client_id != client_id) {
        return Err(INVALID_GRANT);
    }
    let mut used = row.into_active_model();
    used.used_at = Set(Some(Utc::now().into()));
    used.update(&txn).await?;
    let tokens = issue(&txn, grant.id).await?;
    txn.commit().await?;
    Ok(tokens)
}

/// RFC 7009: refresh token → grant dicabut; access token → kedaluwarsa sekarang. Token tidak
/// dikenal tetap sukses (tidak membocorkan keberadaan token).
///
/// # Errors
///
/// Error database.
pub async fn revoke_token(db: &DatabaseConnection, token: &str) -> Result<(), DbErr> {
    let Some(row) = oauth_tokens::Entity::find()
        .filter(oauth_tokens::Column::TokenDigest.eq(sha256_hex(token)))
        .one(db)
        .await?
    else {
        return Ok(());
    };
    if row.kind == "refresh" {
        revoke_grant(db, row.grant_id).await
    } else {
        let mut r = row.into_active_model();
        r.expires_at = Set(Utc::now().into());
        r.update(db).await.map(|_| ())
    }
}

// ---------------------------------------------------------------------------------------------
// Akses MCP

/// Pemegang access token yang valid: grant, user, dan workspace-nya.
#[derive(Debug, Clone)]
pub struct Principal {
    pub grant_id: Uuid,
    pub user_id: Uuid,
    pub workspace_id: Uuid,
    pub client_name: String,
}

/// Access token bearer → principal, bila token aktif dan grant belum dicabut.
///
/// # Errors
///
/// Error database.
pub async fn authenticate<C: ConnectionTrait>(
    db: &C,
    bearer: &str,
) -> Result<Option<Principal>, DbErr> {
    let Some(token) = oauth_tokens::Entity::find()
        .filter(oauth_tokens::Column::TokenDigest.eq(sha256_hex(bearer)))
        .filter(oauth_tokens::Column::Kind.eq("access"))
        .one(db)
        .await?
    else {
        return Ok(None);
    };
    if token.expires_at < Utc::now() {
        return Ok(None);
    }
    let Some(grant) = active_grant(db, token.grant_id).await? else {
        return Ok(None);
    };
    let client = oauth_clients::Entity::find_by_id(grant.client_id)
        .one(db)
        .await?;
    // Terakhir dipakai, paling sering sekali per menit.
    if grant
        .last_used_at
        .is_none_or(|t| Utc::now().signed_duration_since(t) > Duration::minutes(1))
    {
        let mut g = grant.clone().into_active_model();
        g.last_used_at = Set(Some(Utc::now().into()));
        g.update(db).await?;
    }
    Ok(Some(Principal {
        grant_id: grant.id,
        user_id: grant.user_id,
        workspace_id: grant.workspace_id,
        client_name: client.map(|c| c.name).unwrap_or_default(),
    }))
}

// ---------------------------------------------------------------------------------------------
// Client terhubung (Akun → Client AI)

/// Grant aktif milik user beserta client dan workspace-nya, terbaru dulu.
///
/// # Errors
///
/// Error database.
pub async fn grants_of<C: ConnectionTrait>(
    db: &C,
    user_id: Uuid,
) -> Result<Vec<(oauth_grants::Model, oauth_clients::Model, workspaces::Model)>, DbErr> {
    let grants = oauth_grants::Entity::find()
        .filter(oauth_grants::Column::UserId.eq(user_id))
        .filter(oauth_grants::Column::RevokedAt.is_null())
        .order_by_desc(oauth_grants::Column::CreatedAt)
        .all(db)
        .await?;
    let mut out = Vec::new();
    for g in grants {
        let client = oauth_clients::Entity::find_by_id(g.client_id)
            .one(db)
            .await?;
        let workspace = workspaces::Model::find_by_id(db, g.workspace_id).await?;
        if let (Some(c), Some(w)) = (client, workspace) {
            out.push((g, c, w));
        }
    }
    Ok(out)
}

/// User mencabut grant miliknya sendiri. `false` bila grant tidak ada / bukan miliknya.
///
/// # Errors
///
/// Error database.
pub async fn revoke_own(
    db: &DatabaseConnection,
    user_id: Uuid,
    grant_id: Uuid,
    request_id: Option<String>,
) -> Result<bool, DbErr> {
    let txn = db.begin().await?;
    let Some(grant) = oauth_grants::Entity::find_by_id(grant_id)
        .filter(oauth_grants::Column::UserId.eq(user_id))
        .filter(oauth_grants::Column::RevokedAt.is_null())
        .one(&txn)
        .await?
    else {
        return Ok(false);
    };
    let client = oauth_clients::Entity::find_by_id(grant.client_id)
        .one(&txn)
        .await?;
    revoke_grant(&txn, grant.id).await?;
    let Some(workspace) = workspaces::Model::find_by_id(&txn, grant.workspace_id).await? else {
        txn.commit().await?;
        return Ok(true);
    };
    audit::record(
        &txn,
        AuditEntry {
            workspace_id: workspace.workspace_id(),
            actor_id: Some(user_id),
            action: AuditAction::McpRevoked,
            subject: (AuditSubject::McpClient, grant.id),
            changes: serde_json::json!({ "client": client.map(|c| c.name) }),
            request_id,
        },
    )
    .await?;
    txn.commit().await?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redirect_uris_must_be_https_loopback_or_private_scheme() {
        for ok in [
            "https://client.example/cb",
            "http://127.0.0.1:53682/callback",
            "http://localhost:3000/cb",
            "http://[::1]:8080/cb",
            "cursor://anysphere.cursor-retrieval/oauth/callback",
            "com.example.app:/oauth",
        ] {
            assert!(redirect_uri_allowed(ok), "{ok}");
        }
        for bad in [
            "http://evil.example/cb",
            "http://localhost.evil.example/cb",
            "https://client.example/cb#frag",
            "javascript:alert(1)",
            "data:text/html,x",
            "myapp:/cb",
            "https:",
            "",
        ] {
            assert!(!redirect_uri_allowed(bad), "{bad}");
        }
    }

    #[test]
    fn pkce_s256_matches_rfc_7636_example() {
        assert_eq!(
            s256("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }
}
