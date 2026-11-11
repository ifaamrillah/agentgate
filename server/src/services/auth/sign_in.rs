//! Login dan aturan sesi milik aplikasi (rules §7).
//!
//! - [`with_password`]: satu alur login password (rate limit, hash dummy, rehash).
//! - [`complete`]: **satu-satunya** tempat sesi login dibuat (password, OAuth, 2FA,
//!   `/_dev/login`). Selalu `renew()` dulu, sehingga id sesi yang ditanam sebelum login
//!   (session fixation) tidak terbawa.
//! - [`current_user`]: aturan yang tidak disediakan Loco, yaitu user masih ada, belum
//!   dinonaktifkan, dan sesi belum melewati umur mutlak `absolute_lifetime_hours`.

use std::net::IpAddr;

use loco_rs::{Result, app::AppContext, auth::session::Session};
use sea_orm::{ConnectionTrait, prelude::Uuid};
use secrecy::SecretString;

use super::{
    password::{self, Verify},
    throttle::{self, ThrottleKey, ThrottleState},
};
use crate::{
    common::settings::{AuthSettings, Settings},
    models::users,
};

/// Waktu login (detik epoch, `i64`) di data sesi, untuk umur mutlak.
pub const SIGNED_IN_AT: &str = "signed_in_at";
/// User yang password-nya benar tetapi belum memasukkan kode 2FA. Sesi ini **bukan** sesi
/// login: tidak ada `user_id`, jadi gate dan [`current_user`] menolaknya.
pub const PENDING_USER_ID: &str = "pending_user_id";
/// Waktu sesi pending dibuat (detik epoch, `i64`).
pub const PENDING_SINCE: &str = "pending_since";

/// Hasil [`complete`]. Keduanya membawa sesi yang harus dikembalikan dari handler.
#[derive(Debug)]
pub enum SignInStep {
    /// User login penuh: `user_id` dan [`SIGNED_IN_AT`] terisi.
    Done(Session),
    /// User punya 2FA: sesi baru hanya berisi [`PENDING_USER_ID`] dan [`PENDING_SINCE`].
    NeedsSecondFactor(Session),
}

/// Hasil [`with_password`].
#[derive(Debug)]
pub enum PasswordSignIn {
    /// Password benar, user aktif dan terverifikasi: lanjutkan ke [`complete`].
    Authenticated(users::Model),
    /// Password benar tetapi email belum diverifikasi: tidak ada sesi.
    Unverified(users::Model),
    /// Email tidak dikenal, password salah, user tanpa password, atau user dinonaktifkan.
    /// Sengaja satu varian: alasannya tidak boleh dibedakan ke luar.
    Rejected,
    /// Email atau IP sedang terkunci rate limit.
    Locked,
}

/// Buat sesi login untuk `user`. Pemanggil mengembalikan sesi dari [`SignInStep`] di respons.
///
/// # Errors
///
/// Bila data sesi tidak bisa di-serialize.
pub fn complete(_ctx: &AppContext, session: Session, user: &users::Model) -> Result<SignInStep> {
    let now = now();
    let session = session.renew();
    if user.totp_enabled_at.is_some() {
        let session = session
            .insert(PENDING_USER_ID, &user.id.to_string())?
            .insert(PENDING_SINCE, &now)?;
        return Ok(SignInStep::NeedsSecondFactor(session));
    }
    let session = session
        .set_user_id(user.id.to_string())
        .insert(SIGNED_IN_AT, &now)?;
    Ok(SignInStep::Done(session))
}

/// Login email + password dari IP `ip`.
///
/// Urutannya menjaga dua hal: setiap percobaan memverifikasi satu hash (email tidak dikenal
/// memakai hash dummy, sehingga durasinya setara), dan setiap kegagalan dicatat untuk email
/// **dan** IP. Login berhasil mereset penghitung email saja.
///
/// # Errors
///
/// Error database, atau 503 bila antrean hash penuh.
pub async fn with_password<C: ConnectionTrait>(
    db: &C,
    settings: &AuthSettings,
    email: &str,
    password: &SecretString,
    ip: IpAddr,
) -> Result<PasswordSignIn> {
    let keys = [ThrottleKey::Email(email.to_string()), ThrottleKey::Ip(ip)];
    if throttle::check(db, &keys).await? == ThrottleState::Locked {
        return Ok(PasswordSignIn::Locked);
    }

    let user = users::Model::find_by_email(db, email).await?;
    let verdict = match user.as_ref().and_then(|u| u.password_hash.as_deref()) {
        Some(stored) => password::verify(settings, password, stored).await?,
        None => {
            password::verify_dummy(settings, password).await?;
            Verify::Mismatch
        }
    };
    let Some(user) = user.filter(|u| u.is_active() && verdict != Verify::Mismatch) else {
        throttle::record_failure(db, settings, &keys).await?;
        return Ok(PasswordSignIn::Rejected);
    };

    throttle::reset(db, &keys[0]).await?;
    let user = if verdict == Verify::OkNeedsRehash {
        let hash = password::hash(settings, password.clone()).await?;
        user.rehash_password(db, hash).await?
    } else {
        user
    };
    if user.email_verified_at.is_none() {
        return Ok(PasswordSignIn::Unverified(user));
    }
    Ok(PasswordSignIn::Authenticated(user))
}

/// User pemilik sesi, atau `None` bila sesi harus ditolak: tanpa `user_id`, user sudah
/// tidak ada atau dinonaktifkan, atau sesi melewati umur mutlak.
///
/// # Errors
///
/// Error database atau settings.
pub async fn current_user(ctx: &AppContext, session: &Session) -> Result<Option<users::Model>> {
    let Some(id) = session.user_id().and_then(|id| Uuid::parse_str(&id).ok()) else {
        return Ok(None);
    };
    let settings = Settings::from_ctx(ctx)?;
    let Some(signed_in_at) = session.get::<i64>(SIGNED_IN_AT) else {
        return Ok(None);
    };
    if expired(signed_in_at, now(), settings.auth.absolute_lifetime_hours) {
        return Ok(None);
    }
    Ok(users::Model::find_by_id(&ctx.db, id)
        .await?
        .filter(users::Model::is_active))
}

/// Sesi berumur `lifetime_hours` atau lebih sudah kedaluwarsa (batas inklusif, sehingga
/// umur 0 jam menolak setiap sesi).
fn expired(signed_in_at: i64, now: i64, lifetime_hours: i64) -> bool {
    now.saturating_sub(signed_in_at) >= lifetime_hours.saturating_mul(3600)
}

/// `return_to` yang aman diikuti: path internal yang diawali `/`, bukan `//`, tanpa `\` dan
/// tanpa karakter kontrol (browser membuang tab/newline, sehingga `/\t/evil` menjadi
/// `//evil`). Nilai lain dibuang, bukan dikoreksi (open redirect).
#[must_use]
pub fn safe_return_to(value: &str) -> Option<String> {
    let ok = value.starts_with('/')
        && !value.starts_with("//")
        && !value.contains('\\')
        && !value.chars().any(char::is_control);
    ok.then(|| value.to_string())
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn return_to_accepts_internal_paths_only() {
        assert_eq!(safe_return_to("/").as_deref(), Some("/"));
        assert_eq!(
            safe_return_to("/projects?x=1#a").as_deref(),
            Some("/projects?x=1#a")
        );
        for bad in [
            "",
            "projects",
            "//evil.test",
            "/\\evil.test",
            "\\\\evil.test",
            "https://evil.test",
            "/\t/evil.test",
            "/\n/evil.test",
        ] {
            assert_eq!(safe_return_to(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn absolute_lifetime_is_inclusive() {
        assert!(!expired(1000, 1000 + 3599, 1));
        assert!(expired(1000, 1000 + 3600, 1));
        assert!(expired(1000, 1000, 0));
    }
}
