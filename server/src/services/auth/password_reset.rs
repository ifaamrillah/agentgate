//! Lupa dan reset password (rules §7 Reset password).
//!
//! - [`request`] menjawab sama untuk email terdaftar atau tidak: rate limit dipesan lebih dulu
//!   untuk email apa pun, dan email dikirim lewat job sehingga durasinya tidak membedakan.
//! - Token hanya dikonsumsi [`reset`] (`POST`); [`is_valid`] untuk `GET` tidak mengonsumsi.
//! - Reset berhasil mencabut semua sesi user itu.

use std::net::IpAddr;

use loco_rs::{Result, app::AppContext, auth::session_store, validation::ModelValidationErrors};
use sea_orm::ConnectionTrait;
use secrecy::SecretString;

use super::{
    password,
    throttle::{self, ThrottleKey, ThrottleState},
};
use crate::{
    common::settings::Settings,
    mailers::auth::AuthMailer,
    models::{
        user_tokens::{self, TokenPurpose},
        users,
    },
};

/// Hasil [`reset`].
#[derive(Debug)]
pub enum Reset {
    /// Password diganti dan semua sesi user dicabut.
    Done,
    /// Password baru tidak memenuhi aturan; token belum dikonsumsi.
    InvalidPassword(ModelValidationErrors),
    /// Token tidak ada, sudah dipakai, kedaluwarsa, atau diganti permintaan baru.
    InvalidToken,
}

/// Permintaan reset untuk `email` dari IP `ip`. Mengembalikan `Locked` bila email atau IP
/// terlalu sering meminta; selain itu `Open`, terkirim atau tidak.
///
/// # Errors
///
/// Error database atau antrean email.
pub async fn request(
    ctx: &AppContext,
    settings: &Settings,
    email: &str,
    ip: IpAddr,
) -> Result<ThrottleState> {
    let keys = [
        ThrottleKey::MailEmail(email.to_string()),
        ThrottleKey::MailIp(ip),
    ];
    if throttle::reserve(&ctx.db, &settings.auth, &keys).await? == ThrottleState::Locked {
        return Ok(ThrottleState::Locked);
    }
    // User yang dinonaktifkan tidak mendapat link; responsnya tetap sama.
    if let Some(user) = users::Model::find_by_email(&ctx.db, email)
        .await?
        .filter(users::Model::is_active)
    {
        let raw = user_tokens::issue(
            &ctx.db,
            user.id,
            TokenPurpose::PasswordReset,
            chrono::Duration::minutes(settings.auth.reset_token_minutes),
        )
        .await?;
        AuthMailer::send_password_reset(ctx, &user, &raw).await?;
    }
    Ok(ThrottleState::Open)
}

/// Apakah link reset masih berlaku, tanpa mengonsumsinya (untuk `GET`).
///
/// # Errors
///
/// Error database.
pub async fn is_valid<C: ConnectionTrait>(db: &C, raw: &str) -> Result<bool> {
    Ok(user_tokens::peek(db, raw, TokenPurpose::PasswordReset).await?)
}

/// Ganti password lewat token reset. Aturan password dicek **sebelum** token dikonsumsi,
/// sehingga password yang terlalu pendek tidak menghanguskan link. Berhasil: hash baru,
/// `password_changed_at`, `email_verified_at` bila masih kosong, lalu semua sesi dicabut.
///
/// # Errors
///
/// Error database, hashing (503), atau session store.
pub async fn reset(
    ctx: &AppContext,
    settings: &Settings,
    raw: &str,
    new_password: SecretString,
) -> Result<Reset> {
    if let Err(errors) = password::validate(&new_password) {
        return Ok(Reset::InvalidPassword(errors));
    }
    let hash = password::hash(&settings.auth, new_password).await?;
    let Some(user_id) = user_tokens::consume(&ctx.db, raw, TokenPurpose::PasswordReset).await?
    else {
        return Ok(Reset::InvalidToken);
    };
    let Some(user) = users::Model::find_by_id(&ctx.db, user_id).await? else {
        return Ok(Reset::InvalidToken);
    };
    let user = user.reset_password(&ctx.db, hash).await?;
    session_store::sign_out_everywhere(ctx, &user.id.to_string()).await?;
    Ok(Reset::Done)
}
