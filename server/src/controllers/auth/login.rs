//! `GET /login` dan `POST /login`: email + password.
use axum::{extract::Query, http::StatusCode};
use loco_rs::prelude::*;
use secrecy::SecretString;
use serde::Deserialize;

use crate::{
    common::settings::Settings,
    controllers::{
        extractors::{ClientIp, MaybeCurrentUser, ValidForm},
        flash,
    },
    services::auth::sign_in::{self, PasswordSignIn, SignInStep},
    views::{self, auth::LoginPage},
};

/// Halaman setelah login bila tidak ada `return_to`.
const HOME: &str = "/";

#[derive(Debug, Deserialize)]
pub struct ShowParams {
    return_to: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginForm {
    email: String,
    password: SecretString,
    #[serde(default)]
    return_to: Option<String>,
}

#[debug_handler]
async fn show(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    MaybeCurrentUser(user): MaybeCurrentUser,
    session: Session,
    Query(params): Query<ShowParams>,
) -> Result<Response> {
    if user.is_some() {
        return format::redirect(HOME);
    }
    // Flash dari verifikasi email atau reset password ("silakan masuk").
    let (session, flash) = flash::take(session);
    let mut page = login_page(&ctx, "", params.return_to.as_deref())?;
    page.flash = flash;
    Ok((session, views::auth::login(&v, &page, StatusCode::OK)?).into_response())
}

#[debug_handler]
async fn submit(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    ClientIp(ip): ClientIp,
    session: Session,
    ValidForm(form): ValidForm<LoginForm>,
) -> Result<Response> {
    let settings = Settings::from_ctx(&ctx)?;
    let page = login_page(&ctx, &form.email, form.return_to.as_deref())?;
    match sign_in::with_password(&ctx.db, &settings.auth, &form.email, &form.password, ip).await? {
        PasswordSignIn::Authenticated(user) => match sign_in::complete(&ctx, session, &user)? {
            SignInStep::Done(session) => {
                let target = page.return_to.as_deref().unwrap_or(HOME);
                Ok((session, format::redirect(target)?).into_response())
            }
            SignInStep::NeedsSecondFactor(session) => {
                Ok((session, format::redirect(sign_in::SECOND_FACTOR_PATH)?).into_response())
            }
        },
        // Bukan sesi login: sesi baru hanya mengingat user untuk tombol kirim ulang.
        PasswordSignIn::Unverified(user) => {
            let session = sign_in::await_verification(session, &user)?;
            let page = views::auth::verify_pending(&v, "", StatusCode::OK)?;
            Ok((session, page).into_response())
        }
        PasswordSignIn::Rejected => views::auth::login(
            &v,
            &page.with_error(views::auth::INVALID_CREDENTIALS),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        PasswordSignIn::Locked => views::auth::login(
            &v,
            &page.with_error(views::auth::TOO_MANY_ATTEMPTS),
            StatusCode::TOO_MANY_REQUESTS,
        ),
    }
}

/// Data halaman login. `return_to` yang tidak aman dibuang di sini, sehingga form maupun
/// redirect hanya pernah memakai nilai yang lolos `safe_return_to`.
fn login_page(ctx: &AppContext, email: &str, return_to: Option<&str>) -> Result<LoginPage> {
    // Tombol OAuth hanya untuk provider yang dikonfigurasi (`auth.oauth.providers`).
    let providers = ctx
        .config
        .auth
        .as_ref()
        .and_then(|a| a.oauth.as_ref())
        .map(|o| &o.providers);
    let has = |name: &str| providers.is_some_and(|p| p.contains_key(name));
    Ok(LoginPage {
        email: email.trim().to_string(),
        return_to: return_to.and_then(sign_in::safe_return_to),
        error: "",
        oauth_google: has("google"),
        oauth_github: has("github"),
        registration_open: Settings::from_ctx(ctx)?.auth.registration_open,
        flash: None,
    })
}

pub fn routes() -> Routes {
    Routes::new().add("/login", get(show).post(submit))
}
