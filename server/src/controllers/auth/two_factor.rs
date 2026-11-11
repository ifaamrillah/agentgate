//! 2FA (TOTP + kode pemulihan). Adapter tipis; logikanya di `services::auth::totp`.
//!
//! Langkah kedua login (publik, sesi pending tanpa `user_id`):
//! - `GET|POST /login/2fa`: kode TOTP atau kode pemulihan di satu field. Sesi pending berumur
//!   5 menit; kode salah ke-5 (per user) membuang sesi pending.
//!
//! Pengaturan (butuh sesi login penuh, `CurrentUser`; sesi pending ditolak):
//! - `GET /account/2fa`: QR code dan secret untuk setup, atau status bila sudah aktif.
//!   `?required=<slug>` (dari extractor `Tenant`, workspace yang mewajibkan 2FA) hanya
//!   menampilkan pemberitahuan; nilainya tidak dipakai.
//! - `POST /account/2fa`: konfirmasi setup dengan kode; menampilkan kode pemulihan sekali.
//! - `POST /account/2fa/disable`: matikan 2FA (butuh password saat ini dan kode TOTP atau kode
//!   pemulihan; rate limit per user, terkunci → 429).
//! - `POST /account/2fa/recovery-codes`: buat ulang kode pemulihan.
use axum::http::StatusCode;
use loco_rs::{prelude::*, validation::ModelValidationErrors};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use validator::{ValidationError, ValidationErrors};

use super::login::HOME;
use crate::{
    common::settings::Settings,
    controllers::{
        account::too_many_attempts,
        extractors::{CurrentUser, MaybeCurrentUser, ValidForm},
        flash,
        shell::app_shell,
    },
    models::{recovery_codes, users},
    services::auth::{
        password_change::CURRENT_PASSWORD_WRONG,
        sign_in::{self, PendingSecondFactor},
        totp::{self, Confirmed, Disabled, SecondStep},
    },
    views::{self, account::TwoFactorSetupPage, flash::Flash},
};

const LOGIN: &str = "/login";
const SETTINGS: &str = "/account/2fa";
const ACCOUNT: &str = "/account";
/// Secret setup yang belum dikonfirmasi di data sesi (`SetupSecret::sealed`, terenkripsi).
/// Tidak pernah ditulis ke `users` sebelum kode konfirmasinya benar.
const SETUP_SECRET: &str = "totp_setup_secret";

#[derive(Debug, Deserialize)]
pub struct CodeForm {
    /// Kode TOTP atau kode pemulihan; disamarkan di `Debug` seperti password.
    #[serde(default = "empty_secret")]
    code: SecretString,
}

#[derive(Debug, Deserialize)]
pub struct DisableForm {
    #[serde(default = "empty_secret")]
    current_password: SecretString,
    /// Kode TOTP atau kode pemulihan; disamarkan di `Debug` seperti password.
    #[serde(default = "empty_secret")]
    code: SecretString,
}

/// Query `GET /account/2fa`. Field lain diabaikan.
#[derive(Debug, Deserialize)]
pub struct SettingsQuery {
    required: Option<String>,
}

fn empty_secret() -> SecretString {
    SecretString::from(String::new())
}

#[debug_handler(state = AppContext)]
async fn show(
    ViewEngine(v): ViewEngine<TeraView>,
    MaybeCurrentUser(user): MaybeCurrentUser,
    session: Session,
) -> Result<Response> {
    if user.is_some() {
        return format::redirect(HOME);
    }
    match sign_in::pending_second_factor(&session) {
        PendingSecondFactor::None => format::redirect(LOGIN),
        PendingSecondFactor::Expired => {
            drop_pending(session, views::auth::FLASH_SECOND_FACTOR_EXPIRED)
        }
        PendingSecondFactor::Active(_) => views::auth::two_factor(&v, "", StatusCode::OK),
    }
}

#[debug_handler]
async fn submit(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    session: Session,
    ValidForm(form): ValidForm<CodeForm>,
) -> Result<Response> {
    let user_id = match sign_in::pending_second_factor(&session) {
        PendingSecondFactor::None => return format::redirect(LOGIN),
        PendingSecondFactor::Expired => {
            return drop_pending(session, views::auth::FLASH_SECOND_FACTOR_EXPIRED);
        }
        PendingSecondFactor::Active(id) => id,
    };
    // User dihapus, dinonaktifkan, atau mematikan 2FA sejak sesi pending dibuat: mulai lagi.
    let Some(user) = users::Model::find_by_id(&ctx.db, user_id)
        .await?
        .filter(|u| u.is_active() && u.totp_enabled_at.is_some())
    else {
        return Ok((session.clear(), format::redirect(LOGIN)?).into_response());
    };
    match totp::second_step(&ctx, &user, form.code.expose_secret()).await? {
        SecondStep::Accepted => {
            let (session, return_to) = sign_in::complete_second_factor(session, &user)?;
            let target = return_to.as_deref().unwrap_or(HOME);
            Ok((session, format::redirect(target)?).into_response())
        }
        SecondStep::Rejected => views::auth::two_factor(
            &v,
            views::auth::SECOND_FACTOR_INVALID,
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        SecondStep::Locked => drop_pending(session, views::auth::FLASH_SECOND_FACTOR_LOCKED),
    }
}

/// Buang sesi pending (`clear`, id sesi baru) dan kembali ke login dengan pesan `key`.
fn drop_pending(session: Session, key: &str) -> Result<Response> {
    let session = flash::set(session.clear(), &Flash::error(key))?;
    Ok((session, format::redirect(LOGIN)?).into_response())
}

#[debug_handler]
async fn settings(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    CurrentUser(user): CurrentUser,
    session: Session,
    Query(query): Query<SettingsQuery>,
) -> Result<Response> {
    if user.totp_enabled_at.is_some() {
        let remaining = recovery_codes::count_unused(&ctx.db, user.id).await?;
        return views::account::two_factor_status(
            &v,
            &app_shell(&ctx, &user, None, None).await?.at("two_factor"),
            remaining,
            None,
            StatusCode::OK,
        );
    }
    // Secret pending dipakai lagi bila ada, sehingga reload atau preload tidak menggantinya.
    let settings = Settings::from_ctx(&ctx)?;
    let sealed = session.get::<String>(SETUP_SECRET);
    let setup = totp::begin_setup(&settings, &user, sealed.as_deref())?;
    let session = if sealed.as_deref() == Some(setup.sealed.as_str()) {
        session
    } else {
        session.insert(SETUP_SECRET, &setup.sealed)?
    };
    let page = TwoFactorSetupPage {
        qr_svg: setup.qr_svg,
        secret: setup.secret_base32,
        error: "",
        required: query.required.is_some(),
    };
    let res = views::account::two_factor_setup(
        &v,
        &app_shell(&ctx, &user, None, None).await?.at("two_factor"),
        &page,
        StatusCode::OK,
    )?;
    Ok((session, res).into_response())
}

#[debug_handler]
async fn confirm(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    CurrentUser(user): CurrentUser,
    session: Session,
    ValidForm(form): ValidForm<CodeForm>,
) -> Result<Response> {
    let sealed = session.get::<String>(SETUP_SECRET);
    let (false, Some(sealed)) = (user.totp_enabled_at.is_some(), sealed) else {
        return format::redirect(SETTINGS);
    };
    match totp::confirm(&ctx, &user, &sealed, form.code.expose_secret()).await? {
        Confirmed::Enabled(codes) => {
            let session = session.remove(SETUP_SECRET);
            let res = views::account::recovery_codes(
                &v,
                &app_shell(&ctx, &user, None, None).await?.at("two_factor"),
                &codes,
            )?;
            Ok((session, res).into_response())
        }
        // Klik ganda: request pertama sudah mengaktifkan 2FA dan menampilkan kodenya.
        Confirmed::AlreadyEnabled => {
            Ok((session.remove(SETUP_SECRET), format::redirect(SETTINGS)?).into_response())
        }
        Confirmed::WrongCode => {
            let settings = Settings::from_ctx(&ctx)?;
            let setup = totp::begin_setup(&settings, &user, Some(&sealed))?;
            // Secret di sesi tidak bisa dibuka lagi (mis. kunci enkripsi diganti): QR baru
            // ditampilkan, jadi secret-nya harus disimpan, atau konfirmasi tidak pernah cocok.
            let session = if setup.sealed == sealed {
                session
            } else {
                session.insert(SETUP_SECRET, &setup.sealed)?
            };
            let page = TwoFactorSetupPage {
                qr_svg: setup.qr_svg,
                secret: setup.secret_base32,
                error: views::account::TOTP_CODE_INVALID,
                required: false,
            };
            let res = views::account::two_factor_setup(
                &v,
                &app_shell(&ctx, &user, None, None).await?.at("two_factor"),
                &page,
                StatusCode::UNPROCESSABLE_ENTITY,
            )?;
            Ok((session, res).into_response())
        }
    }
}

#[debug_handler]
async fn disable(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    CurrentUser(user): CurrentUser,
    session: Session,
    ValidForm(form): ValidForm<DisableForm>,
) -> Result<Response> {
    if user.totp_enabled_at.is_none() {
        return format::redirect(ACCOUNT);
    }
    let (errors, status) = match totp::disable(
        &ctx,
        &user,
        &form.current_password,
        form.code.expose_secret(),
    )
    .await?
    {
        Disabled::Done => {
            let session = flash::set(
                session,
                &Flash::success(views::account::FLASH_TWO_FACTOR_DISABLED),
            )?;
            return Ok((session, format::redirect(ACCOUNT)?).into_response());
        }
        Disabled::WrongPassword => (
            field_error("current_password", CURRENT_PASSWORD_WRONG),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        Disabled::WrongCode => (
            field_error("code", views::account::TOTP_DISABLE_CODE_INVALID),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        Disabled::Locked => (too_many_attempts(), StatusCode::TOO_MANY_REQUESTS),
    };
    let remaining = recovery_codes::count_unused(&ctx.db, user.id).await?;
    views::account::two_factor_status(
        &v,
        &app_shell(&ctx, &user, None, None).await?.at("two_factor"),
        remaining,
        Some(&errors),
        status,
    )
}

fn field_error(field: &'static str, code: &'static str) -> ModelValidationErrors {
    let mut errors = ValidationErrors::new();
    errors.add(field, ValidationError::new(code));
    errors.into()
}

#[debug_handler]
async fn regenerate_recovery_codes(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    CurrentUser(user): CurrentUser,
) -> Result<Response> {
    match totp::regenerate_recovery_codes(&ctx, &user).await? {
        Some(codes) => views::account::recovery_codes(
            &v,
            &app_shell(&ctx, &user, None, None).await?.at("two_factor"),
            &codes,
        ),
        None => format::redirect(SETTINGS),
    }
}

pub fn routes() -> Routes {
    Routes::new()
        .add("/login/2fa", get(show).post(submit))
        .add(SETTINGS, get(settings).post(confirm))
        .add("/account/2fa/disable", post(disable))
        .add(
            "/account/2fa/recovery-codes",
            post(regenerate_recovery_codes),
        )
}
