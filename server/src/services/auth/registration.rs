//! Pendaftaran publik (rules §7 Pendaftaran): respons sama untuk email baru dan email yang
//! sudah terdaftar. Email baru mendapat link verifikasi; email terdaftar mendapat
//! pemberitahuan berisi link reset password. Tanpa ini, form daftar menjadi alat mengecek
//! email mana yang punya akun.
//!
//! [`create_account`] dipisah dari pengiriman email supaya pendaftaran lewat undangan
//! (M1 Task 11, email terkunci dan langsung terverifikasi) memakai pembuatan akun yang sama.

use std::net::IpAddr;

use loco_rs::{Result, app::AppContext, validation::ModelValidationErrors};
use secrecy::SecretString;

use super::{
    password,
    throttle::{self, ThrottleKey, ThrottleState},
    verification,
};
use crate::{
    common::settings::Settings,
    mailers::auth::AuthMailer,
    models::{
        user_tokens::{self, TokenPurpose},
        users::{self, NewUser, UserError},
    },
};

/// Isi form daftar.
#[derive(Debug)]
pub struct Registration {
    pub name: String,
    pub email: String,
    pub password: SecretString,
}

/// Hasil [`register`]. `Done` sengaja satu varian untuk email baru maupun terdaftar.
#[derive(Debug)]
pub enum Registered {
    Done,
    /// Field tidak valid; tidak ada yang dibuat atau dikirim.
    Invalid(ModelValidationErrors),
    /// Terlalu banyak permintaan untuk email atau IP ini.
    Locked,
}

/// Hasil [`create_account`].
#[derive(Debug)]
pub enum Account {
    /// User baru, email belum terverifikasi.
    Created(users::Model),
    /// Email (case-insensitive) sudah dipakai user ini; tidak ada yang diubah.
    Existing(users::Model),
}

/// Daftar dari IP `ip`: validasi semua field sekaligus, rate limit permintaan email, lalu
/// buat akun dan kirim email yang sesuai.
///
/// # Errors
///
/// Error database, hashing (503), atau antrean email.
pub async fn register(
    ctx: &AppContext,
    settings: &Settings,
    form: &Registration,
    ip: IpAddr,
) -> Result<Registered> {
    if let Err(errors) = validate(form) {
        return Ok(Registered::Invalid(errors));
    }
    let keys = [
        ThrottleKey::MailEmail(form.email.clone()),
        ThrottleKey::MailIp(ip),
    ];
    if throttle::reserve(&ctx.db, &settings.auth, &keys).await? == ThrottleState::Locked {
        return Ok(Registered::Locked);
    }
    match create_account(ctx, settings, form).await? {
        Account::Created(user) => verification::send(ctx, settings, &user).await?,
        // User yang dinonaktifkan tidak mendapat email apa pun; responsnya tetap sama.
        Account::Existing(user) if user.is_active() => {
            let raw = user_tokens::issue(
                &ctx.db,
                user.id,
                TokenPurpose::PasswordReset,
                chrono::Duration::minutes(settings.auth.reset_token_minutes),
            )
            .await?;
            AuthMailer::send_already_registered(ctx, &user, &raw).await?;
        }
        Account::Existing(_) => {}
    }
    Ok(Registered::Done)
}

/// Buat user dari form yang sudah lolos validasi. Password selalu di-hash lebih dulu, juga
/// untuk email yang sudah terdaftar, sehingga durasinya tidak membedakan keduanya.
///
/// # Errors
///
/// Validasi (bila dipanggil tanpa [`register`]), error database, atau hashing (503).
pub async fn create_account(
    ctx: &AppContext,
    settings: &Settings,
    form: &Registration,
) -> Result<Account> {
    let password_hash = password::hash(&settings.auth, form.password.clone()).await?;
    let new = NewUser {
        email: form.email.clone(),
        name: form.name.clone(),
        password_hash: Some(password_hash),
    };
    match users::ActiveModel::create(&ctx.db, new).await {
        Ok(user) => Ok(Account::Created(user)),
        Err(UserError::EmailTaken) => users::Model::find_by_email(&ctx.db, &form.email)
            .await?
            .map(Account::Existing)
            .ok_or_else(|| loco_rs::Error::string("email taken but user not found")),
        Err(e) => Err(e.into()),
    }
}

/// Semua aturan field form daftar sekaligus: email dan nama (model), panjang password. Juga
/// dipakai daftar lewat undangan (`services::workspaces::accept_invite::register`).
///
/// # Errors
///
/// Kode error per field (`email`, `name`, `password`).
pub fn validate(form: &Registration) -> std::result::Result<(), ModelValidationErrors> {
    let mut errors = users::check_new(&form.email, &form.name)
        .err()
        .map_or_else(Default::default, |e| e.errors);
    if let Err(e) = password::validate(&form.password) {
        errors.extend(e.errors);
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(ModelValidationErrors { errors })
    }
}
