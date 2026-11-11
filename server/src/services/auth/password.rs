//! Hash dan verifikasi password argon2id (PHC string) dengan parameter dari `settings.auth`
//! (rules §7). Dipakai login, register, reset, dan ganti password.
//!
//! Setiap hash berjalan di `spawn_blocking` (argon2 berat di CPU dan memori) dan harus lebih
//! dulu mendapat izin dari satu `Semaphore` seproses berukuran `max_concurrent_hashes`. Request
//! yang menunggu izin lebih dari `hash_wait_seconds` dijawab 503, bukan antre tanpa batas:
//! rate limit per email/IP tidak menahan serangan dari banyak IP, batas inilah yang menjaga
//! memori server. Izin dipegang task blocking sampai selesai, jadi request yang dibatalkan
//! tidak melepas izin sementara hash-nya masih berjalan.

use std::{
    sync::{
        Arc, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use argon2::{
    Algorithm, Argon2, Params, PasswordHash, PasswordHasher, PasswordVerifier, Version,
    password_hash::SaltString,
};
use axum::http::StatusCode;
use loco_rs::{Error, Result, controller::ErrorDetail, validation::ModelValidationErrors};
use secrecy::{ExposeSecret, SecretString};
use tokio::sync::Semaphore;
use validator::{ValidationError, ValidationErrors};

use crate::common::{crypto, settings::AuthSettings};

/// Panjang password minimal (karakter), NIST SP 800-63B. Tanpa aturan komposisi.
pub const PASSWORD_MIN_LEN: usize = 12;
/// Panjang password maksimal (karakter): input raksasa tidak boleh dipakai membebani hashing.
pub const PASSWORD_MAX_LEN: usize = 128;

pub const PASSWORD_TOO_SHORT: &str = "password-too-short";
pub const PASSWORD_TOO_LONG: &str = "password-too-long";
/// Semua kode error field modul ini; masing-masing punya pesan di Fluent (dijaga tes i18n).
pub const ERROR_CODES: &[&str] = &[PASSWORD_TOO_SHORT, PASSWORD_TOO_LONG];

/// Kode error 503 saat izin hash tidak didapat dalam `hash_wait_seconds`. Bukan error field:
/// halaman error memakai teks status 503 (`error-503`).
pub const BUSY_CODE: &str = "password-hashing-busy";

/// Password tetap untuk hash dummy; tidak pernah menjadi password user mana pun.
const DUMMY_PASSWORD: &[u8] = b"agentgate-dummy-password-never-valid";

/// Hasil verifikasi. `OkNeedsRehash`: password benar tetapi hash memakai parameter lain dari
/// config saat ini; pemanggil menyimpan hash baru.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verify {
    Ok,
    OkNeedsRehash,
    Mismatch,
}

static LIMITER: OnceLock<Arc<Semaphore>> = OnceLock::new();
/// Jumlah [`hash`] yang dimulai sejak proses berjalan. Dipakai tes untuk membuktikan
/// jalur tanpa autentikasi (mis. token reset palsu) tidak menjalankan argon2.
static HASHES_STARTED: AtomicU64 = AtomicU64::new(0);
static DUMMY_HASH: OnceLock<String> = OnceLock::new();

/// Aturan panjang password (`field = "password"`, kode `password-too-short` /
/// `password-too-long` dengan argumen `min` / `max`). Dipanggil `hash`, dan boleh dipanggil
/// lebih dulu oleh form yang memvalidasi semua field sekaligus.
///
/// # Errors
///
/// [`ModelValidationErrors`] berisi satu error pada field `password`.
pub fn validate(password: &SecretString) -> std::result::Result<(), ModelValidationErrors> {
    let len = password.expose_secret().chars().count();
    let error = if len < PASSWORD_MIN_LEN {
        let mut e = ValidationError::new(PASSWORD_TOO_SHORT);
        e.add_param("min".into(), &PASSWORD_MIN_LEN);
        e
    } else if len > PASSWORD_MAX_LEN {
        let mut e = ValidationError::new(PASSWORD_TOO_LONG);
        e.add_param("max".into(), &PASSWORD_MAX_LEN);
        e
    } else {
        return Ok(());
    };
    let mut errors = ValidationErrors::new();
    errors.add("password", error);
    Err(errors.into())
}

/// Hash password baru (PHC string argon2id, salt acak 16 byte).
///
/// # Errors
///
/// `Error::Validation` bila panjangnya di luar batas; 503 bila antrean hash penuh terlalu lama.
pub async fn hash(settings: &AuthSettings, password: SecretString) -> Result<String> {
    validate(&password)?;
    HASHES_STARTED.fetch_add(1, Ordering::Relaxed);
    let argon = hasher(settings)?;
    limited(settings, move || {
        hash_blocking(&argon, password.expose_secret().as_bytes())
    })
    .await?
}

/// Verifikasi password terhadap PHC string tersimpan. Hash yang tidak bisa di-parse dianggap
/// `Mismatch` (dicatat di log), bukan error: login tetap gagal dengan pesan umum.
///
/// # Errors
///
/// 503 bila antrean hash penuh terlalu lama; error internal bila task hash gagal.
pub async fn verify(
    settings: &AuthSettings,
    password: &SecretString,
    stored: &str,
) -> Result<Verify> {
    if too_long(password) {
        return Ok(Verify::Mismatch);
    }
    let params = params(settings)?;
    let password = password.clone();
    let stored = stored.to_owned();
    limited(settings, move || {
        verify_blocking(&params, password.expose_secret().as_bytes(), &stored)
    })
    .await
}

/// Verifikasi terhadap hash dummy, untuk email yang tidak dikenal: durasinya setara dengan
/// verifikasi user sungguhan, sehingga waktu respons tidak membocorkan email terdaftar. Hash
/// dummy dibuat sekali (saat pertama dipakai) dengan parameter config.
///
/// # Errors
///
/// Sama dengan [`verify`].
pub async fn verify_dummy(settings: &AuthSettings, password: &SecretString) -> Result<()> {
    if too_long(password) {
        return Ok(());
    }
    let argon = hasher(settings)?;
    let params = params(settings)?;
    let password = password.clone();
    limited(settings, move || {
        let dummy = match DUMMY_HASH.get() {
            Some(h) => h,
            None => {
                let h = hash_blocking(&argon, DUMMY_PASSWORD)?;
                DUMMY_HASH.get_or_init(|| h)
            }
        };
        verify_blocking(&params, password.expose_secret().as_bytes(), dummy);
        Ok(())
    })
    .await?
}

/// Berapa kali [`hash`] dimulai di proses ini (lihat `HASHES_STARTED`).
#[must_use]
pub fn hashes_started() -> u64 {
    HASHES_STARTED.load(Ordering::Relaxed)
}

fn too_long(password: &SecretString) -> bool {
    password.expose_secret().chars().count() > PASSWORD_MAX_LEN
}

fn params(settings: &AuthSettings) -> Result<Params> {
    Params::new(
        settings.argon2_memory_kib,
        settings.argon2_iterations,
        settings.argon2_parallelism,
        None,
    )
    .map_err(|e| Error::string(&format!("invalid argon2 params: {e}")))
}

fn hasher(settings: &AuthSettings) -> Result<Argon2<'static>> {
    Ok(Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        params(settings)?,
    ))
}

fn hash_blocking(argon: &Argon2<'_>, password: &[u8]) -> Result<String> {
    let salt = SaltString::encode_b64(&crypto::random_bytes::<16>())
        .map_err(|e| Error::string(&format!("argon2 salt: {e}")))?;
    argon
        .hash_password(password, &salt)
        .map(|h| h.to_string())
        .map_err(|e| Error::string(&format!("argon2 hash: {e}")))
}

fn verify_blocking(configured: &Params, password: &[u8], stored: &str) -> Verify {
    let Ok(parsed) = PasswordHash::new(stored) else {
        tracing::warn!("stored password hash is not a valid PHC string");
        return Verify::Mismatch;
    };
    // Argon2 membaca algoritme, versi, dan parameter dari hash tersimpan, bukan dari config.
    if Argon2::default()
        .verify_password(password, &parsed)
        .is_err()
    {
        return Verify::Mismatch;
    }
    if needs_rehash(&parsed, configured) {
        Verify::OkNeedsRehash
    } else {
        Verify::Ok
    }
}

fn needs_rehash(parsed: &PasswordHash<'_>, configured: &Params) -> bool {
    let same_params = Params::try_from(parsed).is_ok_and(|p| {
        p.m_cost() == configured.m_cost()
            && p.t_cost() == configured.t_cost()
            && p.p_cost() == configured.p_cost()
    });
    parsed.algorithm != Algorithm::Argon2id.ident()
        || parsed.version != Some(Version::V0x13.into())
        || !same_params
}

/// Semaphore seproses, dibuat sekali dari setting pertama yang dipakai.
fn limiter(settings: &AuthSettings) -> Arc<Semaphore> {
    LIMITER
        .get_or_init(|| {
            Arc::new(Semaphore::new(
                settings.max_concurrent_hashes.min(Semaphore::MAX_PERMITS),
            ))
        })
        .clone()
}

async fn limited<T, F>(settings: &AuthSettings, work: F) -> Result<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    run_limited(
        limiter(settings),
        Duration::from_secs(settings.hash_wait_seconds),
        work,
    )
    .await
}

/// Tunggu izin paling lama `wait`, lalu jalankan `work` di thread blocking sambil memegang izin.
async fn run_limited<T, F>(semaphore: Arc<Semaphore>, wait: Duration, work: F) -> Result<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let permit = tokio::time::timeout(wait, semaphore.acquire_owned())
        .await
        .map_err(|_| busy())?
        .map_err(|_| Error::string("password hash semaphore closed"))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        work()
    })
    .await
    .map_err(|e| Error::string(&format!("password hash task failed: {e}")))
}

fn busy() -> Error {
    tracing::warn!("password hashing queue full; answering 503");
    Error::CustomError(
        StatusCode::SERVICE_UNAVAILABLE,
        ErrorDetail::with_reason(BUSY_CODE),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn waiting_longer_than_the_limit_for_a_permit_is_503() {
        let semaphore = Arc::new(Semaphore::new(1));
        let _held = semaphore.clone().acquire_owned().await.unwrap();

        let err = run_limited(semaphore.clone(), Duration::from_millis(50), || ())
            .await
            .unwrap_err();
        match err {
            Error::CustomError(status, detail) => {
                assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
                assert_eq!(detail.error.as_deref(), Some(BUSY_CODE));
            }
            other => panic!("expected 503, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn permit_is_released_after_work_finishes() {
        let semaphore = Arc::new(Semaphore::new(1));
        let wait = Duration::from_millis(500);
        assert_eq!(run_limited(semaphore.clone(), wait, || 1).await.unwrap(), 1);
        assert_eq!(run_limited(semaphore.clone(), wait, || 2).await.unwrap(), 2);
        assert_eq!(semaphore.available_permits(), 1);
    }

    #[test]
    fn length_limits_count_characters() {
        let pw = |s: String| SecretString::from(s);
        assert!(validate(&pw("a".repeat(PASSWORD_MIN_LEN))).is_ok());
        assert!(validate(&pw("a".repeat(PASSWORD_MAX_LEN))).is_ok());
        let short = validate(&pw("é".repeat(PASSWORD_MIN_LEN - 1))).unwrap_err();
        assert_eq!(short.errors["password"][0].code, "password-too-short");
        assert_eq!(short.errors["password"][0].params["min"], PASSWORD_MIN_LEN);
        let long = validate(&pw("a".repeat(PASSWORD_MAX_LEN + 1))).unwrap_err();
        assert_eq!(long.errors["password"][0].code, "password-too-long");
    }
}
