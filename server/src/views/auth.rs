//! Halaman autentikasi. Data view berisi field yang dirender saja (tanpa entity user).
//! Password tidak pernah ada di data view: form gagal dirender ulang dengan field password
//! kosong.
use axum::http::StatusCode;
use loco_rs::{prelude::*, validation::ModelValidationErrors};
use serde::Serialize;

use super::{
    flash::Flash,
    form::{FieldErrors, field_errors},
};

pub const INVALID_CREDENTIALS: &str = "auth-invalid-credentials";
pub const TOO_MANY_ATTEMPTS: &str = "auth-too-many-attempts";
/// Rate limit permintaan yang mengirim email (daftar, lupa password, kirim ulang).
pub const TOO_MANY_REQUESTS: &str = "auth-too-many-requests";
pub const FLASH_EMAIL_VERIFIED: &str = "flash-email-verified";
pub const FLASH_PASSWORD_RESET: &str = "flash-password-reset";
/// Kode langkah kedua (TOTP atau kode pemulihan) salah.
pub const SECOND_FACTOR_INVALID: &str = "auth-second-factor-invalid";
/// Login pending dibuang setelah terlalu banyak kode salah.
pub const FLASH_SECOND_FACTOR_LOCKED: &str = "flash-second-factor-locked";
/// Login pending melewati batas waktu memasukkan kode.
pub const FLASH_SECOND_FACTOR_EXPIRED: &str = "flash-second-factor-expired";
/// Kunci Fluent yang dikirim sebagai variabel ke template (dijaga tes i18n).
pub const MESSAGE_KEYS: &[&str] = &[
    INVALID_CREDENTIALS,
    TOO_MANY_ATTEMPTS,
    TOO_MANY_REQUESTS,
    FLASH_EMAIL_VERIFIED,
    FLASH_PASSWORD_RESET,
    SECOND_FACTOR_INVALID,
    FLASH_SECOND_FACTOR_LOCKED,
    FLASH_SECOND_FACTOR_EXPIRED,
];

/// Field form daftar, urutan sama dengan template.
pub const REGISTER_FIELDS: &[&str] = &["name", "email", "password"];
/// Field form reset password.
pub const RESET_FIELDS: &[&str] = &["password"];

/// Halaman yang membawa token di URL: `Referrer-Policy: no-referrer`, supaya token tidak
/// ikut terkirim ke situs lain lewat header `Referer` (rules §7 Reset password).
const NO_REFERRER: (&str, &str) = ("referrer-policy", "no-referrer");

/// Data `auth/login.html`.
#[derive(Debug, Serialize)]
pub struct LoginPage {
    pub email: String,
    /// Sudah lewat `safe_return_to`; nilai tidak aman dibuang.
    pub return_to: Option<String>,
    /// Kunci Fluent pesan error form, atau kosong.
    pub error: &'static str,
    pub oauth_google: bool,
    pub oauth_github: bool,
    /// Link "Daftar" hanya bila `settings.auth.registration_open`.
    pub registration_open: bool,
    pub flash: Option<Flash>,
}

impl LoginPage {
    #[must_use]
    pub fn with_error(mut self, key: &'static str) -> Self {
        self.error = key;
        self
    }
}

pub fn login(v: &impl ViewRenderer, page: &LoginPage, status: StatusCode) -> Result<Response> {
    format::render()
        .status(status)
        .view(v, "auth/login.html", page)
}

/// Password benar tetapi email belum diverifikasi: tidak ada sesi login, hanya tombol kirim
/// ulang (rules §7). `error`: kunci Fluent atau kosong.
pub fn verify_pending(v: &impl ViewRenderer, error: &str, status: StatusCode) -> Result<Response> {
    format::render()
        .status(status)
        .view(v, "auth/verify_pending.html", data!({ "error": error }))
}

/// Form kode langkah kedua login (`/login/2fa`). `error`: kunci Fluent atau kosong. Kode
/// yang dikirim tidak dirender ulang.
pub fn two_factor(v: &impl ViewRenderer, error: &str, status: StatusCode) -> Result<Response> {
    format::render()
        .status(status)
        .view(v, "auth/two_factor.html", data!({ "error": error }))
}

/// Data `auth/register.html`.
#[derive(Debug, Serialize)]
pub struct RegisterPage {
    pub name: String,
    pub email: String,
    pub errors: FieldErrors,
    /// Kunci Fluent pesan error form (rate limit), atau kosong.
    pub error: &'static str,
}

impl RegisterPage {
    #[must_use]
    pub fn new(name: &str, email: &str, errors: Option<&ModelValidationErrors>) -> Self {
        Self {
            name: name.trim().to_string(),
            email: email.trim().to_string(),
            errors: field_errors(REGISTER_FIELDS, errors),
            error: "",
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
        .view(v, "auth/register.html", page)
}

/// "Cek email kamu": tujuan setelah daftar, lupa password, dan kirim ulang verifikasi.
/// Sengaja tidak menyebut apakah email terdaftar.
pub fn check_email(v: &impl ViewRenderer) -> Result<Response> {
    format::render().view(v, "auth/check_email.html", data!({}))
}

/// Tombol konfirmasi verifikasi email untuk token yang masih berlaku (`GET` tidak
/// mengonsumsi token).
pub fn verify_email(v: &impl ViewRenderer, token: &str) -> Result<Response> {
    format::render().header(NO_REFERRER.0, NO_REFERRER.1).view(
        v,
        "auth/verify_email.html",
        data!({ "token": token }),
    )
}

/// Link verifikasi atau reset yang tidak berlaku. `kind`: `verify` atau `reset` (menentukan
/// teks dan link permintaan baru).
pub fn token_invalid(
    v: &impl ViewRenderer,
    kind: TokenKind,
    status: StatusCode,
) -> Result<Response> {
    format::render()
        .status(status)
        .header(NO_REFERRER.0, NO_REFERRER.1)
        .view(v, "auth/token_invalid.html", data!({ "kind": kind }))
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TokenKind {
    Verify,
    Reset,
}

/// Data `auth/forgot.html`.
#[derive(Debug, Serialize)]
pub struct ForgotPage {
    pub email: String,
    pub error: &'static str,
}

pub fn forgot(v: &impl ViewRenderer, page: &ForgotPage, status: StatusCode) -> Result<Response> {
    format::render()
        .status(status)
        .view(v, "auth/forgot.html", page)
}

/// Form password baru untuk token reset yang masih berlaku. `error`: kunci Fluent pesan
/// form (rate limit) atau kosong.
pub fn reset(
    v: &impl ViewRenderer,
    token: &str,
    errors: Option<&ModelValidationErrors>,
    error: &str,
    status: StatusCode,
) -> Result<Response> {
    format::render()
        .status(status)
        .header(NO_REFERRER.0, NO_REFERRER.1)
        .view(
            v,
            "auth/reset.html",
            data!({
                "token": token,
                "errors": field_errors(RESET_FIELDS, errors),
                "error": error,
            }),
        )
}
