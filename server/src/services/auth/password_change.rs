//! Ganti password oleh user yang sudah login (rules §7 Ganti password): wajib password saat
//! ini; berhasil mencabut **semua** sesi user itu. Pemanggil lalu membuat sesi baru untuk
//! perangkat saat ini (`sign_in::start_session`) di respons yang sama, sehingga hanya
//! perangkat yang mengganti password yang tetap login.
//!
//! Setiap percobaan dipesan lebih dulu di rate limit per user (`ThrottleKey::Account`, dipakai
//! bersama "matikan 2FA") sebelum password diperiksa: sesi curian tidak bisa menebak password
//! tanpa batas.

use loco_rs::{Result, app::AppContext, auth::session_store, validation::ModelValidationErrors};
use secrecy::SecretString;
use validator::{ValidationError, ValidationErrors};

use super::{
    password::{self, Verify},
    throttle::{self, ThrottleKey, ThrottleState},
};
use crate::{common::settings::Settings, models::users};

/// Kode error field `current_password`: password saat ini kosong atau salah.
pub const CURRENT_PASSWORD_WRONG: &str = "password-current-wrong";
/// Semua kode error field modul ini (dijaga tes i18n).
pub const ERROR_CODES: &[&str] = &[CURRENT_PASSWORD_WRONG];

/// Hasil [`change`].
#[derive(Debug)]
pub enum Changed {
    /// Password diganti dan semua sesi user dicabut; berisi user terbaru.
    Done(Box<users::Model>),
    /// Password saat ini salah dan/atau password baru tidak memenuhi aturan.
    Invalid(ModelValidationErrors),
    /// Terlalu banyak password salah: tidak diperiksa, coba lagi setelah kunci habis.
    Locked,
}

/// Ganti password `user` menjadi `new_password` bila `current` benar.
///
/// # Errors
///
/// Error database, hashing (503), atau session store.
pub async fn change(
    ctx: &AppContext,
    settings: &Settings,
    user: users::Model,
    current: &SecretString,
    new_password: SecretString,
) -> Result<Changed> {
    let keys = [ThrottleKey::Account(user.id)];
    if throttle::reserve(&ctx.db, &settings.auth, &keys).await? == ThrottleState::Locked {
        return Ok(Changed::Locked);
    }
    let mut errors = password::validate(&new_password)
        .err()
        .map_or_else(Default::default, |e| e.errors);
    // User tanpa password (hanya OAuth) tidak punya password saat ini yang bisa cocok.
    let current_ok = match user.password_hash.as_deref() {
        Some(stored) => {
            password::verify(&settings.auth, current, stored).await? != Verify::Mismatch
        }
        None => false,
    };
    if current_ok {
        throttle::succeeded(&ctx.db, &settings.auth, &keys).await?;
    } else {
        let mut wrong = ValidationErrors::new();
        wrong.add(
            "current_password",
            ValidationError::new(CURRENT_PASSWORD_WRONG),
        );
        errors.extend(ModelValidationErrors::from(wrong).errors);
    }
    if !errors.is_empty() {
        return Ok(Changed::Invalid(ModelValidationErrors { errors }));
    }
    let hash = password::hash(&settings.auth, new_password).await?;
    let user = user.change_password(&ctx.db, hash).await?;
    session_store::sign_out_everywhere(ctx, &user.id.to_string()).await?;
    Ok(Changed::Done(Box::new(user)))
}
