//! Login dan aturan sesi milik aplikasi (rules §7).
//!
//! - [`with_password`]: satu alur login password (rate limit, hash dummy, rehash).
//! - [`complete`]: tempat sesi login dibuat (password, OAuth, `/_dev/login`; langkah kedua 2FA
//!   lewat [`complete_second_factor`]), lewat
//!   [`start_session`], satu-satunya yang menulis `user_id` (juga dipakai ganti password).
//!   Selalu `renew()` dulu, sehingga id sesi yang ditanam sebelum login (session fixation)
//!   tidak terbawa.
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
/// `return_to` (sudah lewat [`safe_return_to`]) yang dibawa sesi pending ke langkah kedua.
pub const PENDING_RETURN_TO: &str = "pending_return_to";
/// Umur sesi pending 2FA: lewat dari ini, user harus memasukkan password lagi.
pub const PENDING_TTL_SECONDS: i64 = 5 * 60;
/// User yang password-nya benar tetapi email-nya belum diverifikasi, untuk tombol "kirim
/// ulang email verifikasi" (`POST /verify-email/resend`). Seperti [`PENDING_USER_ID`], sesi ini
/// tanpa `user_id`, jadi bukan sesi login. Dibuat [`await_verification`].
pub const PENDING_VERIFICATION_USER_ID: &str = "pending_verification_user_id";

/// Halaman kode 2FA untuk sesi pending (`SignInStep::NeedsSecondFactor`), tujuan redirect login.
pub const SECOND_FACTOR_PATH: &str = "/login/2fa";

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
/// `return_to` hanya disimpan untuk langkah kedua ([`PENDING_RETURN_TO`]); pada
/// [`SignInStep::Done`] pemanggil sendiri yang mengarahkan ke sana.
///
/// # Errors
///
/// Bila data sesi tidak bisa di-serialize.
pub fn complete(
    _ctx: &AppContext,
    session: Session,
    user: &users::Model,
    return_to: Option<&str>,
) -> Result<SignInStep> {
    if user.totp_enabled_at.is_some() {
        let mut session = session
            .renew()
            .insert(PENDING_USER_ID, &user.id.to_string())?
            .insert(PENDING_SINCE, &now())?;
        if let Some(target) = return_to.and_then(safe_return_to) {
            session = session.insert(PENDING_RETURN_TO, &target)?;
        }
        return Ok(SignInStep::NeedsSecondFactor(session));
    }
    Ok(SignInStep::Done(start_session(session, user)?))
}

/// State sesi untuk langkah kedua login.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingSecondFactor {
    /// Bukan sesi pending 2FA.
    None,
    /// Sesi pending melewati [`PENDING_TTL_SECONDS`]: harus dibuang.
    Expired,
    /// Sesi pending yang masih berlaku untuk user ini.
    Active(Uuid),
}

/// Baca sesi pending 2FA yang dibuat [`complete`].
#[must_use]
pub fn pending_second_factor(session: &Session) -> PendingSecondFactor {
    let Some(id) = session
        .get::<String>(PENDING_USER_ID)
        .and_then(|id| Uuid::parse_str(&id).ok())
    else {
        return PendingSecondFactor::None;
    };
    match session.get::<i64>(PENDING_SINCE) {
        Some(since) if !pending_expired(since, now()) => PendingSecondFactor::Active(id),
        _ => PendingSecondFactor::Expired,
    }
}

/// Sesi pending yang dibuat pada `since` sudah kedaluwarsa pada `now`: berumur
/// [`PENDING_TTL_SECONDS`] atau lebih, atau `since` terlalu jauh di masa depan (jam server
/// mundur), supaya sesi tidak berlaku tanpa batas.
#[must_use]
pub fn pending_expired(since: i64, now: i64) -> bool {
    now.saturating_sub(since).saturating_abs() >= PENDING_TTL_SECONDS
}

/// Langkah kedua berhasil: sesi login penuh lewat [`start_session`] (sesi pending dibuang
/// oleh `renew`). Mengembalikan sesi dan `return_to` yang dibawa sesi pending.
///
/// # Errors
///
/// Bila data sesi tidak bisa di-serialize.
pub fn complete_second_factor(
    session: Session,
    user: &users::Model,
) -> Result<(Session, Option<String>)> {
    let return_to = session
        .get::<String>(PENDING_RETURN_TO)
        .and_then(|target| safe_return_to(&target));
    Ok((start_session(session, user)?, return_to))
}

/// Sesi login penuh yang baru untuk `user` (`renew` + `user_id` + [`SIGNED_IN_AT`]), tanpa
/// pemeriksaan 2FA. Dipakai [`complete`] dan ganti password, yang membuat ulang sesi
/// perangkat yang sedang dipakai setelah semua sesi user dicabut.
///
/// # Errors
///
/// Bila data sesi tidak bisa di-serialize.
pub fn start_session(session: Session, user: &users::Model) -> Result<Session> {
    session
        .renew()
        .set_user_id(user.id.to_string())
        .insert(SIGNED_IN_AT, &now())
}

/// Sesi baru (tanpa `user_id`) yang mengingat `user` yang belum terverifikasi, untuk tombol
/// kirim ulang email verifikasi ([`PENDING_VERIFICATION_USER_ID`]).
///
/// # Errors
///
/// Bila data sesi tidak bisa di-serialize.
pub fn await_verification(session: Session, user: &users::Model) -> Result<Session> {
    session
        .renew()
        .insert(PENDING_VERIFICATION_USER_ID, &user.id.to_string())
}

/// User di [`PENDING_VERIFICATION_USER_ID`], bila ada dan id-nya valid.
#[must_use]
pub fn awaiting_verification(session: &Session) -> Option<Uuid> {
    session
        .get::<String>(PENDING_VERIFICATION_USER_ID)
        .and_then(|id| Uuid::parse_str(&id).ok())
}

/// Login email + password dari IP `ip`.
///
/// Urutannya menjaga dua hal: setiap percobaan dipesan di rate limit email **dan** IP sebelum
/// verifikasi (jalur yang sama untuk email terdaftar dan tidak), dan setiap percobaan yang
/// lolos memverifikasi satu hash (email tidak dikenal memakai hash dummy, sehingga durasinya
/// setara). Password benar mengembalikan pesanan itu (`throttle::succeeded`).
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
    // Pesan percobaan dulu, atomik: burst paralel tidak bisa memverifikasi lebih dari batas.
    if throttle::reserve(db, settings, &keys).await? == ThrottleState::Locked {
        crate::services::metrics::login_failed();
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
    // Gagal: percobaan yang sudah dipesan tetap terhitung.
    let Some(user) = user.filter(|u| u.is_active() && verdict != Verify::Mismatch) else {
        crate::services::metrics::login_failed();
        return Ok(PasswordSignIn::Rejected);
    };

    throttle::succeeded(db, settings, &keys).await?;
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
