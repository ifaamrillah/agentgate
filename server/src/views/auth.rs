//! Halaman autentikasi. Data view berisi field yang dirender saja (tanpa entity user).
use axum::http::StatusCode;
use loco_rs::prelude::*;
use serde::Serialize;

pub const INVALID_CREDENTIALS: &str = "auth-invalid-credentials";
pub const TOO_MANY_ATTEMPTS: &str = "auth-too-many-attempts";
/// Kunci Fluent yang dikirim sebagai variabel ke template (dijaga tes i18n).
pub const MESSAGE_KEYS: &[&str] = &[INVALID_CREDENTIALS, TOO_MANY_ATTEMPTS];

/// Data `auth/login.html`. Password tidak pernah ada di sini: form gagal dirender ulang
/// dengan field password kosong.
#[derive(Debug, Serialize)]
pub struct LoginPage {
    pub email: String,
    /// Sudah lewat `safe_return_to`; nilai tidak aman dibuang.
    pub return_to: Option<String>,
    /// Kunci Fluent pesan error form, atau kosong.
    pub error: &'static str,
    pub oauth_google: bool,
    pub oauth_github: bool,
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

/// Password benar tetapi email belum diverifikasi: tidak ada sesi (rules §7).
pub fn verify_pending(v: &impl ViewRenderer) -> Result<Response> {
    format::render().view(v, "auth/verify_pending.html", data!({}))
}
