//! `GET|POST /register` (rules §7 Pendaftaran) dan `GET /check-email`. Bila
//! `settings.auth.registration_open` mati, form daftar tidak ada (404).
use axum::http::StatusCode;
use loco_rs::prelude::*;
use secrecy::SecretString;
use serde::Deserialize;

use super::CHECK_EMAIL;
use crate::{
    common::settings::Settings,
    controllers::extractors::{ClientIp, MaybeCurrentUser, ValidForm},
    services::auth::registration::{self, Registered, Registration},
    views::{self, auth::RegisterPage},
};

#[derive(Debug, Deserialize)]
pub struct RegisterForm {
    name: String,
    email: String,
    password: SecretString,
}

fn ensure_open(settings: &Settings) -> Result<()> {
    if settings.auth.registration_open {
        Ok(())
    } else {
        Err(Error::NotFound)
    }
}

#[debug_handler]
async fn show(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    MaybeCurrentUser(user): MaybeCurrentUser,
) -> Result<Response> {
    ensure_open(&Settings::from_ctx(&ctx)?)?;
    if user.is_some() {
        return format::redirect("/");
    }
    views::auth::register(&v, &RegisterPage::new("", "", None), StatusCode::OK)
}

/// Respons sama untuk email baru dan email yang sudah terdaftar: `303` ke `/check-email`.
#[debug_handler]
async fn submit(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    ClientIp(ip): ClientIp,
    ValidForm(form): ValidForm<RegisterForm>,
) -> Result<Response> {
    let settings = Settings::from_ctx(&ctx)?;
    ensure_open(&settings)?;
    let registration = Registration {
        name: form.name,
        email: form.email,
        password: form.password,
    };
    match registration::register(&ctx, &settings, &registration, ip).await? {
        Registered::Done => format::redirect(CHECK_EMAIL),
        Registered::Invalid(errors) => views::auth::register(
            &v,
            &RegisterPage::new(&registration.name, &registration.email, Some(&errors)),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        Registered::Locked => {
            let mut page = RegisterPage::new(&registration.name, &registration.email, None);
            page.error = views::auth::TOO_MANY_REQUESTS;
            views::auth::register(&v, &page, StatusCode::TOO_MANY_REQUESTS)
        }
    }
}

#[debug_handler]
async fn check_email(ViewEngine(v): ViewEngine<TeraView>) -> Result<Response> {
    views::auth::check_email(&v)
}

pub fn routes() -> Routes {
    Routes::new()
        .add("/register", get(show).post(submit))
        .add(CHECK_EMAIL, get(check_email))
}
