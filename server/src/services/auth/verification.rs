//! Verifikasi email (rules §7): link `user_tokens` (`email_verification`), konsumsi hanya
//! lewat `POST` (pemindai link email membuka `GET` tanpa diklik user), dan kirim ulang yang
//! ikut rate limit permintaan email.

use std::net::IpAddr;

use loco_rs::{Result, app::AppContext};
use sea_orm::{ConnectionTrait, prelude::Uuid};

use super::throttle::{self, ThrottleKey, ThrottleState};
use crate::{
    common::settings::Settings,
    mailers::auth::AuthMailer,
    models::{
        user_tokens::{self, TokenPurpose},
        users,
    },
};

/// Hasil [`resend`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resent {
    /// Link baru dikirim (link lama tidak berlaku lagi).
    Sent,
    /// Tidak ada yang perlu dikirim: user tidak ada, dinonaktifkan, atau sudah terverifikasi.
    Skipped,
    /// Terlalu banyak permintaan untuk email atau IP ini.
    Locked,
}

/// Buat token verifikasi baru untuk `user` (membatalkan yang lama) dan kirim link-nya.
///
/// # Errors
///
/// Error database atau antrean email.
pub async fn send(ctx: &AppContext, settings: &Settings, user: &users::Model) -> Result<()> {
    let raw = user_tokens::issue(
        &ctx.db,
        user.id,
        TokenPurpose::EmailVerification,
        chrono::Duration::hours(settings.auth.verification_token_hours),
    )
    .await?;
    AuthMailer::send_verification(ctx, user, &raw).await
}

/// Kirim ulang link verifikasi untuk user `user_id` (dari sesi menunggu verifikasi) yang
/// diminta dari IP `ip`.
///
/// # Errors
///
/// Error database atau antrean email.
pub async fn resend(
    ctx: &AppContext,
    settings: &Settings,
    user_id: Uuid,
    ip: IpAddr,
) -> Result<Resent> {
    let Some(user) = users::Model::find_by_id(&ctx.db, user_id)
        .await?
        .filter(|u| u.is_active() && u.email_verified_at.is_none())
    else {
        return Ok(Resent::Skipped);
    };
    let keys = [
        ThrottleKey::MailEmail(user.email.clone()),
        ThrottleKey::MailIp(ip),
    ];
    if throttle::reserve(&ctx.db, &settings.auth, &keys).await? == ThrottleState::Locked {
        return Ok(Resent::Locked);
    }
    send(ctx, settings, &user).await?;
    Ok(Resent::Sent)
}

/// Apakah link verifikasi masih berlaku, tanpa mengonsumsinya (untuk `GET`).
///
/// # Errors
///
/// Error database.
pub async fn is_valid<C: ConnectionTrait>(db: &C, raw: &str) -> Result<bool> {
    Ok(user_tokens::peek(db, raw, TokenPurpose::EmailVerification).await?)
}

/// Konsumsi token dan tandai email pemiliknya terverifikasi. `None` untuk token yang tidak
/// berlaku karena alasan apa pun (alasannya sengaja tidak dibedakan).
///
/// # Errors
///
/// Error database.
pub async fn confirm<C: ConnectionTrait>(db: &C, raw: &str) -> Result<Option<users::Model>> {
    let Some(user_id) = user_tokens::consume(db, raw, TokenPurpose::EmailVerification).await?
    else {
        return Ok(None);
    };
    let Some(user) = users::Model::find_by_id(db, user_id).await? else {
        return Ok(None);
    };
    Ok(Some(user.mark_email_verified(db).await?))
}
