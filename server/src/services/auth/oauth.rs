//! Login Google dan GitHub (rules "Ditunda" → Login SSO/OIDC), dibangun dari building block
//! publik `loco_rs::auth::oauth` (`Provider`, `FlowState`, `exchange_code`, `fetch_identity`).
//! Route bawaan `oauth::routes` Loco sengaja tidak dipakai: callback-nya membuat sesi sendiri,
//! sehingga melewati 2FA dan `signed_in_at`. Di sini callback berakhir di
//! [`resolve_user`] lalu `sign_in::complete`, entry point yang sama dengan login password.
//!
//! - Transaksi login (`state`, PKCE verifier, `return_to`) ada di cookie bertanda tangan
//!   berumur [`STATE_TTL_SECONDS`] yang dihapus di callback apa pun hasilnya (controller).
//! - Kedua preset Loco memakai endpoint userinfo dengan access token, bukan verifikasi
//!   `id_token`: identitas datang langsung dari provider lewat TLS, jadi tidak ada JWT yang
//!   perlu diverifikasi. GitHub: preset membaca `/user/emails` dan hanya memberi
//!   `email_verified = Some(true)` untuk alamat primer yang terverifikasi.
//! - Email dari provider hanya menautkan ke akun lama bila `email_verified == Some(true)`
//!   (account takeover).

pub use loco_rs::auth::oauth::state::{STATE_TTL_SECONDS, cookie_name};
use loco_rs::{
    Error, Result,
    app::AppContext,
    auth::oauth::{
        CallbackParams, FailureKind, Identity,
        provider::{self, Provider},
        state::FlowState,
    },
    config::OAuthConfig,
};
use sea_orm::{ActiveEnum, TransactionTrait, prelude::Uuid};

use crate::{
    common::{settings::Settings, text},
    models::{
        identities::{self, IdentityProvider},
        users::{self, NewUser, UserError},
    },
};

/// User membatalkan di halaman provider (`error=access_denied`).
pub const CANCELLED: &str = "oauth-cancelled";
/// Cookie state hilang, kedaluwarsa, atau tidak cocok (tab lama, cookie diblokir, callback
/// palsu).
pub const EXPIRED: &str = "oauth-expired";
/// Kegagalan lain di provider (error lain, token, profil).
pub const FAILED: &str = "oauth-failed";
/// Provider tidak menjamin email-nya terverifikasi.
pub const EMAIL_UNVERIFIED: &str = "oauth-email-unverified";
/// Email belum punya akun dan `settings.auth.registration_open` mati.
pub const REGISTRATION_CLOSED: &str = "oauth-registration-closed";
/// User dinonaktifkan.
pub const USER_DISABLED: &str = "oauth-user-disabled";
/// Semua kode `?oauth_error=` halaman login; masing-masing punya pesan Fluent (dijaga tes i18n).
pub const ERROR_CODES: &[&str] = &[
    CANCELLED,
    EXPIRED,
    FAILED,
    EMAIL_UNVERIFIED,
    REGISTRATION_CLOSED,
    USER_DISABLED,
];

/// Kode error dari query string, hanya bila termasuk [`ERROR_CODES`] (nilai lain dibuang).
#[must_use]
pub fn error_code(value: &str) -> Option<&'static str> {
    ERROR_CODES.iter().copied().find(|code| *code == value)
}

/// Kode untuk halaman login dari kegagalan alur Loco.
#[must_use]
pub const fn failure_code(kind: FailureKind) -> &'static str {
    match kind {
        FailureKind::Cancelled => CANCELLED,
        FailureKind::InvalidState => EXPIRED,
        FailureKind::ProviderError
        | FailureKind::MissingCode
        | FailureKind::TokenExchange
        | FailureKind::Profile
        | FailureKind::Denied
        | FailureKind::Configuration => FAILED,
    }
}

/// Hasil [`resolve_user`].
#[derive(Debug)]
pub enum OAuthOutcome {
    /// User aktif untuk identitas ini; lanjutkan ke `sign_in::complete`.
    SignedIn(Box<users::Model>),
    /// Ditolak dengan salah satu [`ERROR_CODES`].
    Rejected(&'static str),
}

/// Hasil [`finish`].
#[derive(Debug)]
pub enum Callback {
    /// Provider memberikan identitas; `return_to` sudah lewat `safe_return_to`.
    Identity {
        identity: Box<Identity>,
        return_to: Option<String>,
    },
    /// Alur gagal. `detail` untuk log debug saja: bisa berisi teks dari provider, tanpa token.
    Failed {
        kind: FailureKind,
        detail: Option<String>,
    },
}

fn oauth_config(ctx: &AppContext) -> Option<&OAuthConfig> {
    ctx.config.auth.as_ref().and_then(|a| a.oauth.as_ref())
}

/// Nama provider yang dikonfigurasi di `auth.oauth.providers` dan didukung tabel `identities`.
#[must_use]
pub fn configured(ctx: &AppContext) -> Vec<String> {
    oauth_config(ctx)
        .map(|c| {
            c.providers
                .keys()
                .filter(|name| identity_provider(name).is_some())
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

fn identity_provider(name: &str) -> Option<IdentityProvider> {
    IdentityProvider::try_from_value(&name.to_string()).ok()
}

/// Provider `name`, bila didukung (`google`, `github`) dan dikonfigurasi. `None` → 404.
///
/// # Errors
///
/// Konfigurasi provider tidak valid (mis. provider generic tanpa endpoint).
pub fn provider(ctx: &AppContext, name: &str) -> Result<Option<Provider>> {
    if identity_provider(name).is_none() {
        return Ok(None);
    }
    let Some(config) = oauth_config(ctx).and_then(|c| c.providers.get(name)) else {
        return Ok(None);
    };
    Provider::resolve(name, config)
        .map(Some)
        .map_err(Error::Message)
}

/// `redirect_uri` yang dikirim ke provider: `redirect_uri` provider bila di-set, selain itu
/// `{auth.oauth.base_url}/auth/{name}/callback` (fallback `settings.public_base_url`).
fn redirect_uri(ctx: &AppContext, provider: &Provider) -> Result<String> {
    if let Some(uri) = &provider.redirect_uri {
        return Ok(uri.clone());
    }
    let base = match oauth_config(ctx).and_then(|c| c.base_url.clone()) {
        Some(base) => base,
        None => Settings::from_ctx(ctx)?.public_base_url,
    };
    Ok(format!(
        "{}/auth/{}/callback",
        base.trim_end_matches('/'),
        provider.name
    ))
}

/// Awal alur: transaksi baru (state, PKCE verifier, `return_to` yang sudah aman) dan URL
/// authorize provider. Pemanggil menyimpan transaksi di cookie [`cookie_name`].
///
/// # Errors
///
/// `authorize_url` provider bukan URL, atau settings tidak valid.
pub fn start(
    ctx: &AppContext,
    provider: &Provider,
    return_to: Option<&str>,
) -> Result<(FlowState, String)> {
    let return_to = return_to.and_then(super::sign_in::safe_return_to);
    let flow = FlowState::new(provider.pkce, return_to.as_deref());
    let url = provider
        .authorize_url(
            &redirect_uri(ctx, provider)?,
            &flow.state,
            flow.challenge().as_deref(),
        )
        .map_err(Error::Message)?;
    Ok((flow, url))
}

/// Callback: cocokkan state dengan transaksi dari cookie (`None` bila cookie hilang atau
/// kedaluwarsa), tukar code dengan token, lalu ambil identitas dari userinfo.
///
/// # Errors
///
/// Settings tidak valid. Kegagalan di provider bukan error: [`Callback::Failed`].
pub async fn finish(
    ctx: &AppContext,
    provider: &Provider,
    params: CallbackParams,
    flow: Option<FlowState>,
) -> Result<Callback> {
    let failed = |kind, detail: Option<String>| Ok(Callback::Failed { kind, detail });
    if let Some(error) = params.error {
        let kind = if error == "access_denied" {
            FailureKind::Cancelled
        } else {
            FailureKind::ProviderError
        };
        return failed(kind, Some(error));
    }
    let Some(flow) = flow else {
        return failed(
            FailureKind::InvalidState,
            Some("state cookie missing".into()),
        );
    };
    if !params.state.as_deref().is_some_and(|s| flow.matches(s)) {
        return failed(FailureKind::InvalidState, Some("state mismatch".into()));
    }
    let Some(code) = params.code.filter(|c| !c.is_empty()) else {
        return failed(FailureKind::MissingCode, None);
    };
    let redirect_uri = redirect_uri(ctx, provider)?;
    let tokens =
        match provider::exchange_code(provider, &code, &redirect_uri, flow.verifier.as_deref())
            .await
        {
            Ok(tokens) => tokens,
            Err(e) => return failed(FailureKind::TokenExchange, Some(e)),
        };
    match provider::fetch_identity(provider, tokens).await {
        Ok(identity) => Ok(Callback::Identity {
            identity: Box::new(identity),
            return_to: flow
                .return_to
                .as_deref()
                .and_then(super::sign_in::safe_return_to),
        }),
        Err(e) => failed(FailureKind::Profile, Some(e)),
    }
}

/// User untuk identitas dari provider:
/// - identitas `(provider, uid)` yang sudah ditautkan → user-nya;
/// - email `email_verified == Some(true)` milik user lokal → tautkan (email user ditandai
///   terverifikasi, lihat `users::Model::verify_email_from_provider`);
/// - email terverifikasi tanpa akun dan `registration_open` → user baru terverifikasi tanpa
///   password, plus identitasnya;
/// - selain itu [`OAuthOutcome::Rejected`].
///
/// Callback bersamaan aman: identitas ditautkan lewat insert bersyarat, dan user baru yang
/// kalah balapan unique `lower(email)` mengulang lewat jalur tautan.
///
/// # Errors
///
/// Error database atau settings.
pub async fn resolve_user(ctx: &AppContext, identity: &Identity) -> Result<OAuthOutcome> {
    let provider = identity_provider(&identity.provider)
        .ok_or_else(|| Error::string("oauth: unsupported provider"))?;
    if let Some(linked) =
        identities::Model::find_by_provider_uid(&ctx.db, provider, &identity.uid).await?
    {
        return outcome_for(ctx, linked.user_id).await;
    }
    let Some(email) = identity
        .email
        .as_deref()
        .filter(|_| identity.email_verified == Some(true))
    else {
        return Ok(OAuthOutcome::Rejected(EMAIL_UNVERIFIED));
    };
    let registration_open = Settings::from_ctx(ctx)?.auth.registration_open;
    // Percobaan kedua hanya bila callback lain membuat user dengan email ini bersamaan.
    for _ in 0..2 {
        if let Some(user) = users::Model::find_by_email(&ctx.db, email).await? {
            if !user.is_active() {
                return Ok(OAuthOutcome::Rejected(USER_DISABLED));
            }
            return link_existing(ctx, user.id, provider, identity, email).await;
        }
        if !registration_open {
            return Ok(OAuthOutcome::Rejected(REGISTRATION_CLOSED));
        }
        match create_user(ctx, provider, identity, email).await? {
            Created::Linked(user_id) => return outcome_for(ctx, user_id).await,
            Created::Invalid => return Ok(OAuthOutcome::Rejected(FAILED)),
            Created::EmailTaken => {}
        }
    }
    Err(Error::string("oauth: email taken but user not found"))
}

async fn link_existing(
    ctx: &AppContext,
    user_id: Uuid,
    provider: IdentityProvider,
    identity: &Identity,
    email: &str,
) -> Result<OAuthOutcome> {
    let txn = ctx.db.begin().await?;
    // Kunci baris user lebih dulu: insert identitas mengambil KEY SHARE lewat FK, dan
    // menaikkannya ke FOR UPDATE setelahnya membuat dua callback bersamaan deadlock.
    let user = users::Model::lock_for_update(&txn, user_id).await?;
    let linked =
        identities::Model::link(&txn, user_id, provider, &identity.uid, Some(email)).await?;
    if linked.user_id == user_id
        && let Some(user) = user
    {
        user.verify_email_from_provider(&txn).await?;
    }
    txn.commit().await?;
    outcome_for(ctx, linked.user_id).await
}

enum Created {
    Linked(Uuid),
    /// Callback lain membuat user dengan email ini lebih dulu.
    EmailTaken,
    /// Email dari provider tidak lolos validasi user.
    Invalid,
}

async fn create_user(
    ctx: &AppContext,
    provider: IdentityProvider,
    identity: &Identity,
    email: &str,
) -> Result<Created> {
    let txn = ctx.db.begin().await?;
    let new = NewUser {
        email: email.to_string(),
        name: display_name(identity, email),
        password_hash: None,
    };
    let user = match users::ActiveModel::create(&txn, new).await {
        Ok(user) => user,
        // Transaksi di-rollback saat di-drop.
        Err(UserError::EmailTaken) => return Ok(Created::EmailTaken),
        Err(UserError::Invalid(_)) => return Ok(Created::Invalid),
        Err(UserError::Db(e)) => return Err(e.into()),
    };
    let user = user.mark_email_verified(&txn).await?;
    let linked =
        identities::Model::link(&txn, user.id, provider, &identity.uid, Some(email)).await?;
    if linked.user_id != user.id {
        // Identitas ini sudah ditautkan callback lain: user baru tidak dibutuhkan.
        txn.rollback().await?;
        return Ok(Created::Linked(linked.user_id));
    }
    txn.commit().await?;
    Ok(Created::Linked(user.id))
}

async fn outcome_for(ctx: &AppContext, user_id: Uuid) -> Result<OAuthOutcome> {
    let user = users::Model::find_by_id(&ctx.db, user_id)
        .await?
        .ok_or_else(|| Error::string("oauth: identity without user"))?;
    if !user.is_active() {
        return Ok(OAuthOutcome::Rejected(USER_DISABLED));
    }
    Ok(OAuthOutcome::SignedIn(Box::new(user)))
}

/// Nama tampilan user baru: nama dari provider, lalu login (GitHub), lalu bagian lokal email;
/// yang pertama lolos aturan nama (dipotong ke `NAME_MAX_LEN`).
fn display_name(identity: &Identity, email: &str) -> String {
    let local = email.split('@').next().unwrap_or(email);
    [
        identity.name.as_deref(),
        identity.login.as_deref(),
        Some(local),
    ]
    .into_iter()
    .flatten()
    .map(|n| {
        text::normalize_name(n)
            .chars()
            .take(text::NAME_MAX_LEN)
            .collect::<String>()
    })
    .find(|n| text::validate_name("user", n).is_ok())
    .unwrap_or_else(|| local.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_known_error_codes_pass() {
        for code in ERROR_CODES {
            assert_eq!(error_code(code), Some(*code));
        }
        assert_eq!(error_code("auth-invalid-credentials"), None);
        assert_eq!(error_code(""), None);
    }

    #[test]
    fn invalid_state_asks_to_start_again() {
        assert_eq!(failure_code(FailureKind::InvalidState), EXPIRED);
        assert_eq!(failure_code(FailureKind::Cancelled), CANCELLED);
        assert_eq!(failure_code(FailureKind::TokenExchange), FAILED);
    }
}
