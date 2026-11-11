//! Lupa dan reset password (rules §7 Reset password).
//!
//! - `POST /password/forgot` selalu menjawab `303 /check-email`, terdaftar atau tidak.
//! - `GET /password/reset?token=` hanya menampilkan form (tidak mengonsumsi token);
//!   `POST /password/reset` mengonsumsinya, mencabut semua sesi user, lalu ke login.
use axum::{extract::Query, http::StatusCode};
use loco_rs::prelude::*;
use secrecy::SecretString;
use serde::Deserialize;

use super::CHECK_EMAIL;
use crate::{
    common::settings::Settings,
    controllers::{
        extractors::{ClientIp, ValidForm},
        flash,
    },
    services::auth::{
        password_reset::{self, Reset},
        throttle::ThrottleState,
    },
    views::{
        self,
        auth::{ForgotPage, TokenKind},
        flash::Flash,
    },
};

#[derive(Debug, Deserialize)]
pub struct ForgotForm {
    email: String,
}

#[derive(Debug, Deserialize)]
pub struct TokenParams {
    #[serde(default)]
    token: String,
}

#[derive(Debug, Deserialize)]
pub struct ResetForm {
    token: String,
    password: SecretString,
}

#[debug_handler]
async fn forgot(ViewEngine(v): ViewEngine<TeraView>) -> Result<Response> {
    let page = ForgotPage {
        email: String::new(),
        error: "",
    };
    views::auth::forgot(&v, &page, StatusCode::OK)
}

#[debug_handler]
async fn request(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    ClientIp(ip): ClientIp,
    ValidForm(form): ValidForm<ForgotForm>,
) -> Result<Response> {
    let settings = Settings::from_ctx(&ctx)?;
    match password_reset::request(&ctx, &settings, &form.email, ip).await? {
        ThrottleState::Open => format::redirect(CHECK_EMAIL),
        ThrottleState::Locked => {
            let page = ForgotPage {
                email: form.email.trim().to_string(),
                error: views::auth::TOO_MANY_REQUESTS,
            };
            views::auth::forgot(&v, &page, StatusCode::TOO_MANY_REQUESTS)
        }
    }
}

#[debug_handler]
async fn show_reset(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    Query(params): Query<TokenParams>,
) -> Result<Response> {
    if password_reset::is_valid(&ctx.db, &params.token).await? {
        views::auth::reset(&v, &params.token, None, "", StatusCode::OK)
    } else {
        views::auth::token_invalid(&v, TokenKind::Reset, StatusCode::OK)
    }
}

#[debug_handler]
async fn reset(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    ClientIp(ip): ClientIp,
    session: Session,
    ValidForm(form): ValidForm<ResetForm>,
) -> Result<Response> {
    let settings = Settings::from_ctx(&ctx)?;
    match password_reset::reset(&ctx, &settings, &form.token, form.password, ip).await? {
        Reset::Done => {
            // Semua sesi user sudah dicabut (termasuk sesi browser ini bila miliknya): flash
            // ditaruh di sesi baru.
            let session = flash::set(
                session.renew(),
                &Flash::success(views::auth::FLASH_PASSWORD_RESET),
            )?;
            Ok((session, format::redirect("/login")?).into_response())
        }
        Reset::InvalidPassword(errors) => views::auth::reset(
            &v,
            &form.token,
            Some(&errors),
            "",
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        Reset::Locked => views::auth::reset(
            &v,
            &form.token,
            None,
            views::auth::TOO_MANY_REQUESTS,
            StatusCode::TOO_MANY_REQUESTS,
        ),
        Reset::InvalidToken => {
            views::auth::token_invalid(&v, TokenKind::Reset, StatusCode::UNPROCESSABLE_ENTITY)
        }
    }
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/password")
        .add("/forgot", get(forgot).post(request))
        .add("/reset", get(show_reset).post(reset))
}
