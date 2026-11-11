//! Halaman akun user yang login: profil dan ganti password.
use axum::http::StatusCode;
use loco_rs::{prelude::*, validation::ModelValidationErrors};
use serde::Serialize;

use super::{
    flash::Flash,
    form::{FieldErrors, field_errors},
    layout::{AppShell, app_page},
};

pub const FLASH_PROFILE_SAVED: &str = "flash-profile-saved";
pub const FLASH_PASSWORD_CHANGED: &str = "flash-password-changed";
pub const FLASH_TWO_FACTOR_DISABLED: &str = "flash-two-factor-disabled";
/// Kode konfirmasi setup 2FA salah.
pub const TOTP_CODE_INVALID: &str = "totp-code-invalid";
/// Kode TOTP/kode pemulihan untuk mematikan 2FA kosong, salah, atau sudah dipakai.
pub const TOTP_DISABLE_CODE_INVALID: &str = "totp-disable-code-invalid";
/// Kunci Fluent yang dikirim sebagai variabel ke template (dijaga tes i18n).
pub const MESSAGE_KEYS: &[&str] = &[
    FLASH_PROFILE_SAVED,
    FLASH_PASSWORD_CHANGED,
    FLASH_TWO_FACTOR_DISABLED,
    TOTP_CODE_INVALID,
    TOTP_DISABLE_CODE_INVALID,
];

/// Halaman yang berisi rahasia (secret TOTP, kode pemulihan): tidak disimpan cache browser
/// maupun proxy, sehingga tombol Back atau riwayat tidak menampilkannya lagi.
const NO_STORE: (&str, &str) = ("cache-control", "no-store");

pub const PROFILE_FIELDS: &[&str] = &["name"];
pub const PASSWORD_FIELDS: &[&str] = &["current_password", "password"];
pub const DISABLE_TWO_FACTOR_FIELDS: &[&str] = &["current_password", "code"];

/// Data `account/show.html`. `name` adalah isian form (nilai tersimpan, atau isian yang gagal
/// validasi); `email` selalu nilai tersimpan.
#[derive(Debug, Serialize)]
pub struct AccountPage {
    pub name: String,
    pub email: String,
    pub errors: FieldErrors,
    pub flash: Option<Flash>,
    /// Status 2FA di bagian Keamanan.
    pub two_factor_enabled: bool,
}

impl AccountPage {
    #[must_use]
    pub fn new(name: &str, email: &str, two_factor_enabled: bool, flash: Option<Flash>) -> Self {
        Self {
            name: name.to_string(),
            email: email.to_string(),
            errors: field_errors(PROFILE_FIELDS, None),
            flash,
            two_factor_enabled,
        }
    }

    #[must_use]
    pub fn with_errors(mut self, name: &str, errors: &ModelValidationErrors) -> Self {
        self.name = name.to_string();
        self.errors = field_errors(PROFILE_FIELDS, Some(errors));
        self
    }
}

pub fn show(
    v: &impl ViewRenderer,
    shell: &AppShell,
    page: &AccountPage,
    status: StatusCode,
) -> Result<Response> {
    format::render()
        .status(status)
        .view(v, "account/show.html", app_page(shell, page))
}

pub fn password(
    v: &impl ViewRenderer,
    shell: &AppShell,
    errors: Option<&ModelValidationErrors>,
    status: StatusCode,
) -> Result<Response> {
    format::render().status(status).view(
        v,
        "account/password.html",
        app_page(
            shell,
            data!({ "errors": field_errors(PASSWORD_FIELDS, errors) }),
        ),
    )
}

/// Data `account/two_factor_setup.html`.
#[derive(Debug, Serialize)]
pub struct TwoFactorSetupPage {
    /// Markup `<svg>` QR code buatan server (`totp::begin_setup`), dirender tanpa escape.
    pub qr_svg: String,
    /// Secret base32 untuk dimasukkan manual.
    pub secret: String,
    /// Kunci Fluent error kode, atau kosong.
    pub error: &'static str,
    /// Dibuka dari workspace yang mewajibkan 2FA: tampilkan pemberitahuannya.
    pub required: bool,
}

/// Setup 2FA: QR code, secret, dan form kode konfirmasi.
pub fn two_factor_setup(
    v: &impl ViewRenderer,
    shell: &AppShell,
    page: &TwoFactorSetupPage,
    status: StatusCode,
) -> Result<Response> {
    format::render()
        .status(status)
        .header(NO_STORE.0, NO_STORE.1)
        .view(v, "account/two_factor_setup.html", app_page(shell, page))
}

/// 2FA aktif: sisa kode pemulihan, buat ulang kode, dan matikan 2FA (butuh password dan kode
/// TOTP atau kode pemulihan).
pub fn two_factor_status(
    v: &impl ViewRenderer,
    shell: &AppShell,
    remaining_codes: u64,
    errors: Option<&ModelValidationErrors>,
    status: StatusCode,
) -> Result<Response> {
    format::render().status(status).view(
        v,
        "account/two_factor.html",
        app_page(
            shell,
            data!({
                "remaining_codes": remaining_codes,
                "errors": field_errors(DISABLE_TWO_FACTOR_FIELDS, errors),
            }),
        ),
    )
}

/// Kode pemulihan baru, ditampilkan sekali (setelah setup atau buat ulang).
pub fn recovery_codes(
    v: &impl ViewRenderer,
    shell: &AppShell,
    codes: &[String],
) -> Result<Response> {
    format::render().header(NO_STORE.0, NO_STORE.1).view(
        v,
        "account/recovery_codes.html",
        app_page(shell, data!({ "codes": codes })),
    )
}
