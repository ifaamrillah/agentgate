//! Halaman undangan publik (`/invitations/{token}`): detail undangan dengan tombol terima,
//! daftar, atau masuk; form daftar lewat undangan; dan satu halaman "undangan tidak berlaku"
//! untuk semua alasan. Semua membawa `Referrer-Policy: no-referrer` karena token ada di URL.
use axum::http::StatusCode;
use loco_rs::{prelude::*, validation::ModelValidationErrors};
use serde::Serialize;

use super::{
    NO_REFERRER,
    form::{FieldErrors, field_errors},
};

/// Field form daftar lewat undangan (email terkunci, tidak dikirim).
pub const REGISTER_FIELDS: &[&str] = &["name", "password"];

/// Undangan yang ditampilkan, plus tautan aksinya.
#[derive(Debug, Serialize)]
pub struct InvitationView {
    pub workspace_name: String,
    /// `""` bila pengundang sudah dihapus.
    pub inviter_name: String,
    pub role_name: String,
    pub email: String,
    pub accept_action: String,
    pub register_href: String,
    /// `/login?return_to=` halaman undangan ini.
    pub login_href: String,
}

/// Data `invitations/show.html`. `signed_in`: tombol terima (bila `email_matches`) atau pesan
/// email beda; belum login: tombol Daftar dan Masuk. `error`: kunci Fluent atau kosong.
#[derive(Debug, Serialize)]
pub struct ShowPage {
    pub invitation: InvitationView,
    pub signed_in: bool,
    pub email_matches: bool,
    pub error: &'static str,
}

pub fn show(v: &impl ViewRenderer, page: &ShowPage, status: StatusCode) -> Result<Response> {
    format::render()
        .status(status)
        .header(NO_REFERRER.0, NO_REFERRER.1)
        .view(v, "invitations/show.html", page)
}

/// Data `invitations/register.html`. `account_exists`: email undangan sudah punya akun, jadi
/// halaman hanya menautkan Masuk.
#[derive(Debug, Serialize)]
pub struct RegisterPage {
    pub invitation: InvitationView,
    pub action: String,
    pub name: String,
    pub errors: FieldErrors,
    pub account_exists: bool,
}

impl RegisterPage {
    #[must_use]
    pub fn new(
        invitation: InvitationView,
        account_exists: bool,
        name: &str,
        errors: Option<&ModelValidationErrors>,
    ) -> Self {
        Self {
            action: invitation.register_href.clone(),
            invitation,
            name: name.trim().to_string(),
            errors: field_errors(REGISTER_FIELDS, errors),
            account_exists,
        }
    }
}

pub fn register(
    v: &impl ViewRenderer,
    page: &RegisterPage,
    status: StatusCode,
) -> Result<Response> {
    format::render()
        .status(status)
        .header(NO_REFERRER.0, NO_REFERRER.1)
        .view(v, "invitations/register.html", page)
}

/// Undangan tidak ada, kedaluwarsa, dibatalkan, atau sudah dipakai: satu halaman 404 yang sama
/// (alasannya sengaja tidak dibedakan).
pub fn invalid(v: &impl ViewRenderer) -> Result<Response> {
    format::render()
        .status(StatusCode::NOT_FOUND)
        .header(NO_REFERRER.0, NO_REFERRER.1)
        .view(v, "invitations/invalid.html", data!({}))
}
