//! Halaman akun user yang login: `GET /account` (profil), `POST /account/profile` (nama),
//! `GET|POST /account/password` (ganti password, rules §7).
use axum::http::StatusCode;
use loco_rs::prelude::*;
use secrecy::SecretString;
use serde::Deserialize;

use crate::{
    common::settings::Settings,
    controllers::{
        extractors::{CurrentUser, ValidForm},
        flash,
    },
    models::users::UserError,
    services::auth::{
        password_change::{self, Changed},
        sign_in,
    },
    views::{self, account::AccountPage, flash::Flash},
};

const ACCOUNT: &str = "/account";

#[derive(Debug, Deserialize)]
pub struct ProfileForm {
    name: String,
}

#[derive(Debug, Deserialize)]
pub struct PasswordForm {
    current_password: SecretString,
    password: SecretString,
}

#[debug_handler(state = AppContext)]
async fn show(
    ViewEngine(v): ViewEngine<TeraView>,
    CurrentUser(user): CurrentUser,
    session: Session,
) -> Result<Response> {
    let (session, flash) = flash::take(session);
    let page = AccountPage::new(&user.name, &user.email, flash);
    Ok((session, views::account::show(&v, &page, StatusCode::OK)?).into_response())
}

#[debug_handler]
async fn update_profile(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    CurrentUser(user): CurrentUser,
    session: Session,
    ValidForm(form): ValidForm<ProfileForm>,
) -> Result<Response> {
    let page = AccountPage::new(&user.name, &user.email, None);
    match user.rename(&ctx.db, &form.name).await {
        Ok(_) => {
            let session = flash::set(
                session,
                &Flash::success(views::account::FLASH_PROFILE_SAVED),
            )?;
            Ok((session, format::redirect(ACCOUNT)?).into_response())
        }
        Err(UserError::Invalid(errors)) => views::account::show(
            &v,
            &page.with_errors(&form.name, &errors),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        Err(e) => Err(e.into()),
    }
}

#[debug_handler(state = AppContext)]
async fn show_password(
    ViewEngine(v): ViewEngine<TeraView>,
    CurrentUser(_user): CurrentUser,
) -> Result<Response> {
    views::account::password(&v, None, StatusCode::OK)
}

/// Berhasil: semua sesi user dicabut (service), lalu sesi baru untuk perangkat ini di respons
/// yang sama, sehingga hanya perangkat ini yang tetap login.
#[debug_handler]
async fn change_password(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    CurrentUser(user): CurrentUser,
    session: Session,
    ValidForm(form): ValidForm<PasswordForm>,
) -> Result<Response> {
    let settings = Settings::from_ctx(&ctx)?;
    match password_change::change(&ctx, &settings, user, &form.current_password, form.password)
        .await?
    {
        Changed::Done(user) => {
            let session = flash::set(
                sign_in::start_session(session, &user)?,
                &Flash::success(views::account::FLASH_PASSWORD_CHANGED),
            )?;
            Ok((session, format::redirect(ACCOUNT)?).into_response())
        }
        Changed::Invalid(errors) => {
            views::account::password(&v, Some(&errors), StatusCode::UNPROCESSABLE_ENTITY)
        }
    }
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix(ACCOUNT)
        .add("/", get(show))
        .add("/profile", post(update_profile))
        .add("/password", get(show_password).post(change_password))
}
