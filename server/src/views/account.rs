//! Halaman akun user yang login: profil dan ganti password.
use axum::http::StatusCode;
use loco_rs::{prelude::*, validation::ModelValidationErrors};
use serde::Serialize;

use super::{
    flash::Flash,
    form::{FieldErrors, field_errors},
};

pub const FLASH_PROFILE_SAVED: &str = "flash-profile-saved";
pub const FLASH_PASSWORD_CHANGED: &str = "flash-password-changed";
/// Kunci Fluent yang dikirim sebagai variabel ke template (dijaga tes i18n).
pub const MESSAGE_KEYS: &[&str] = &[FLASH_PROFILE_SAVED, FLASH_PASSWORD_CHANGED];

pub const PROFILE_FIELDS: &[&str] = &["name"];
pub const PASSWORD_FIELDS: &[&str] = &["current_password", "password"];

/// Data `account/show.html`. `name` adalah isian form (nilai tersimpan, atau isian yang gagal
/// validasi); `email` selalu nilai tersimpan.
#[derive(Debug, Serialize)]
pub struct AccountPage {
    pub name: String,
    pub email: String,
    pub errors: FieldErrors,
    pub flash: Option<Flash>,
}

impl AccountPage {
    #[must_use]
    pub fn new(name: &str, email: &str, flash: Option<Flash>) -> Self {
        Self {
            name: name.to_string(),
            email: email.to_string(),
            errors: field_errors(PROFILE_FIELDS, None),
            flash,
        }
    }

    #[must_use]
    pub fn with_errors(mut self, name: &str, errors: &ModelValidationErrors) -> Self {
        self.name = name.to_string();
        self.errors = field_errors(PROFILE_FIELDS, Some(errors));
        self
    }
}

pub fn show(v: &impl ViewRenderer, page: &AccountPage, status: StatusCode) -> Result<Response> {
    format::render()
        .status(status)
        .view(v, "account/show.html", page)
}

pub fn password(
    v: &impl ViewRenderer,
    errors: Option<&ModelValidationErrors>,
    status: StatusCode,
) -> Result<Response> {
    format::render().status(status).view(
        v,
        "account/password.html",
        data!({ "errors": field_errors(PASSWORD_FIELDS, errors) }),
    )
}
