//! Verifikasi email (rules §7). `GET /verify-email?token=` hanya menampilkan tombol: pemindai
//! link di layanan email dan preload membuka link tanpa diklik user. Token dikonsumsi
//! `POST /verify-email`, lalu user diarahkan ke login (tidak login otomatis).
//! `POST /verify-email/resend` mengirim ulang link untuk user di sesi menunggu verifikasi.
use axum::{extract::Query, http::StatusCode};
use loco_rs::prelude::*;
use serde::Deserialize;

use super::CHECK_EMAIL;
use crate::{
    common::settings::Settings,
    controllers::{
        extractors::{ClientIp, ValidForm},
        flash,
    },
    services::auth::{
        sign_in,
        verification::{self, Resent},
    },
    views::{self, auth::TokenKind, flash::Flash},
};

#[derive(Debug, Deserialize)]
pub struct TokenParams {
    #[serde(default)]
    token: String,
}

#[debug_handler]
async fn show(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    Query(params): Query<TokenParams>,
) -> Result<Response> {
    if verification::is_valid(&ctx.db, &params.token).await? {
        views::auth::verify_email(&v, &params.token)
    } else {
        views::auth::token_invalid(&v, TokenKind::Verify, StatusCode::OK)
    }
}

#[debug_handler]
async fn confirm(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    session: Session,
    ValidForm(form): ValidForm<TokenParams>,
) -> Result<Response> {
    if verification::confirm(&ctx.db, &form.token).await?.is_none() {
        return views::auth::token_invalid(&v, TokenKind::Verify, StatusCode::UNPROCESSABLE_ENTITY);
    }
    // Sesi baru hanya untuk flash; sesi menunggu verifikasi tidak lagi dibutuhkan.
    let session = flash::set(
        session.renew(),
        &Flash::success(views::auth::FLASH_EMAIL_VERIFIED),
    )?;
    Ok((session, format::redirect("/login")?).into_response())
}

#[debug_handler]
async fn resend(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    ClientIp(ip): ClientIp,
    session: Session,
) -> Result<Response> {
    let Some(user_id) = sign_in::awaiting_verification(&session) else {
        return format::redirect("/login");
    };
    let settings = Settings::from_ctx(&ctx)?;
    match verification::resend(&ctx, &settings, user_id, ip).await? {
        Resent::Sent | Resent::Skipped => format::redirect(CHECK_EMAIL),
        Resent::Locked => views::auth::verify_pending(
            &v,
            views::auth::TOO_MANY_REQUESTS,
            StatusCode::TOO_MANY_REQUESTS,
        ),
    }
}

pub fn routes() -> Routes {
    Routes::new()
        .add("/verify-email", get(show).post(confirm))
        .add("/verify-email/resend", post(resend))
}
