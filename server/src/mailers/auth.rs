//! Email autentikasi: verifikasi email, pemberitahuan "email sudah terdaftar", dan reset
//! password. Link dibentuk dari `settings.public_base_url`, tidak dari header `Host` request
//! (host header injection).
//!
//! **Pengecualian rules §8 (secret di job args):** token mentah harus sampai ke inbox user,
//! jadi link berisi token ikut di isi email yang disimpan sebagai args job `MailerWorker`.
//! Umur args itu sependek tokennya (verifikasi 24 jam, reset 30 menit), dan token tidak
//! pernah di-log: fungsi di sini tidak mencatat link maupun isi email.

use loco_rs::{Result, app::AppContext};
use serde_json::json;

use super::deliver;
use crate::{common::settings::Settings, models::users};

pub const VERIFY: &str = "auth/verify";
pub const ALREADY_REGISTERED: &str = "auth/already_registered";
pub const PASSWORD_RESET: &str = "auth/password_reset";
/// Semua email modul ini (dijaga tes: setiap folder template terdaftar dan dirender).
pub const TEMPLATES: &[&str] = &[VERIFY, ALREADY_REGISTERED, PASSWORD_RESET];

pub struct AuthMailer;

impl AuthMailer {
    /// Link verifikasi email (`GET /verify-email?token=`), berlaku `verification_token_hours`.
    ///
    /// # Errors
    ///
    /// Settings, render, atau antrean job gagal.
    pub async fn send_verification(ctx: &AppContext, user: &users::Model, raw: &str) -> Result<()> {
        let settings = Settings::from_ctx(ctx)?;
        let locals = json!({
            "name": user.name,
            "link": format!("{}/verify-email?token={raw}", settings.public_base_url),
            "hours": settings.auth.verification_token_hours,
        });
        deliver(ctx, &settings, &user.email, VERIFY, &locals).await
    }

    /// Pemberitahuan untuk pendaftaran dengan email yang sudah punya akun, berisi link reset
    /// password (`GET /password/reset?token=`). Respons `POST /register` sama dengan email
    /// baru; email inilah satu-satunya bedanya, dan hanya pemilik inbox yang melihatnya.
    ///
    /// # Errors
    ///
    /// Settings, render, atau antrean job gagal.
    pub async fn send_already_registered(
        ctx: &AppContext,
        user: &users::Model,
        raw: &str,
    ) -> Result<()> {
        let settings = Settings::from_ctx(ctx)?;
        let locals = json!({
            "name": user.name,
            "link": format!("{}/password/reset?token={raw}", settings.public_base_url),
            "login_link": format!("{}/login", settings.public_base_url),
            "minutes": settings.auth.reset_token_minutes,
        });
        deliver(ctx, &settings, &user.email, ALREADY_REGISTERED, &locals).await
    }

    /// Link reset password (`GET /password/reset?token=`), berlaku `reset_token_minutes`.
    ///
    /// # Errors
    ///
    /// Settings, render, atau antrean job gagal.
    pub async fn send_password_reset(
        ctx: &AppContext,
        user: &users::Model,
        raw: &str,
    ) -> Result<()> {
        let settings = Settings::from_ctx(ctx)?;
        let locals = json!({
            "name": user.name,
            "link": format!("{}/password/reset?token={raw}", settings.public_base_url),
            "minutes": settings.auth.reset_token_minutes,
        });
        deliver(ctx, &settings, &user.email, PASSWORD_RESET, &locals).await
    }
}
