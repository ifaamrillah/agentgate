//! Autentikasi dua faktor: TOTP (RFC 6238) dan kode pemulihan (rules "Ditunda" →
//! Autentikasi dua faktor).
//!
//! - Secret 160 bit dari `OsRng`, disimpan terenkripsi (`crypto::encrypt`, kunci
//!   `settings.auth.encryption_key`). Selama setup secret hanya ada di data sesi, juga
//!   terenkripsi ([`SetupSecret::sealed`]), dan baru masuk database setelah dikonfirmasi
//!   dengan kode yang benar.
//! - Kode: SHA-1, 6 digit, langkah 30 detik, toleransi ±1 langkah. Langkah yang diterima
//!   dicatat di `users.totp_last_step`; langkah yang sama atau lebih lama ditolak (replay).
//! - Kode pemulihan: 10 kode `xxxxx-xxxxx` dari [`RECOVERY_ALPHABET`], ditampilkan sekali;
//!   database hanya menyimpan SHA-256 bentuk ternormalisasinya (huruf kecil, tanpa `-`).
//! - Langkah kedua login ([`second_step`]) punya rate limit sendiri per user
//!   (`ThrottleKey::SecondFactor`).
//! - Mematikan 2FA ([`disable`]) butuh password dan kode TOTP atau kode pemulihan, dengan rate
//!   limit per user (`ThrottleKey::Account`, bersama ganti password): sesi curian tidak cukup.

use base64::{Engine, engine::general_purpose::STANDARD};
use loco_rs::{Error, Result, app::AppContext};
use qrcodegen::{QrCode, QrCodeEcc};
use sea_orm::{ConnectionTrait, TransactionTrait};
use secrecy::SecretString;
use totp_rs::{Algorithm, Secret, TOTP};

use super::{
    password::{self, Verify},
    throttle::{self, ThrottleKey, ThrottleState},
};
use crate::{
    common::{
        crypto,
        settings::{AuthSettings, Settings},
    },
    models::{recovery_codes, users},
};

/// Panjang secret TOTP (byte). 160 bit, rekomendasi RFC 4226.
const SECRET_BYTES: usize = 20;
const DIGITS: usize = 6;
const STEP_SECONDS: u64 = 30;
/// Langkah sebelum dan sesudah langkah saat ini yang masih diterima (jam ponsel meleset).
const SKEW_STEPS: u8 = 1;

/// Jumlah kode pemulihan per user.
pub const RECOVERY_CODE_COUNT: usize = 10;
/// Huruf kode pemulihan: huruf kecil dan angka tanpa yang mudah tertukar (`i l o 0 1`).
pub const RECOVERY_ALPHABET: &[u8] = b"abcdefghjkmnpqrstuvwxyz23456789";
/// Karakter per kode pemulihan, ditampilkan sebagai dua kelompok lima.
const RECOVERY_CODE_LEN: usize = 10;
/// Tepi kosong QR code (modul), minimal 4 menurut spesifikasi QR.
const QR_BORDER: i32 = 4;
/// Lebar QR code di halaman (px).
const QR_SIZE_PX: u32 = 192;

/// Secret yang sedang di-setup: untuk halaman setup dan data sesi.
#[derive(Debug, Clone)]
pub struct SetupSecret {
    /// Secret base32 tanpa padding, untuk dimasukkan manual ke aplikasi authenticator.
    pub secret_base32: String,
    /// URI `otpauth://totp/...` yang dikodekan di QR.
    pub otpauth_uri: String,
    /// Markup `<svg>` QR code, dibuat server dari `otpauth_uri` (bukan isian user).
    pub qr_svg: String,
    /// Secret terenkripsi (base64) untuk data sesi sampai dikonfirmasi.
    pub sealed: String,
}

/// Hasil [`confirm`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Confirmed {
    /// 2FA aktif; berisi kode pemulihan mentah untuk ditampilkan sekali.
    Enabled(Vec<String>),
    /// Kode salah, atau `sealed` tidak bisa dibuka.
    WrongCode,
    /// 2FA sudah aktif (mis. klik ganda pada tombol konfirmasi): tidak ada yang diubah.
    AlreadyEnabled,
}

/// Hasil [`disable`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disabled {
    /// 2FA dimatikan (atau sudah tidak aktif).
    Done,
    /// Password saat ini salah (atau user tanpa password).
    WrongPassword,
    /// Password benar, tetapi kode TOTP/kode pemulihan kosong, salah, atau sudah dipakai.
    WrongCode,
    /// Terlalu banyak percobaan gagal: tidak diperiksa, coba lagi setelah kunci habis.
    Locked,
}

/// Hasil [`second_step`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecondStep {
    /// Kode TOTP atau kode pemulihan benar: lanjutkan ke sesi login.
    Accepted,
    /// Kode salah; masih ada percobaan tersisa.
    Rejected,
    /// Batas kode salah tercapai (atau sudah terkunci): buang sesi pending.
    Locked,
}

/// Mulai (atau lanjutkan) setup 2FA. Bila `sealed` (dari data sesi) masih bisa dibuka,
/// secret yang sama dipakai lagi dan [`SetupSecret::sealed`] sama persis dengan `sealed`,
/// sehingga membuka ulang halaman setup (reload, preload) tidak mengganti secret yang mungkin
/// sudah dipindai dan tidak menulis sesi. Selain itu (kosong, rusak, kunci diganti) secret
/// baru dibuat dengan `sealed` baru yang harus disimpan pemanggil.
///
/// # Errors
///
/// Kunci enkripsi tidak valid, atau QR code tidak bisa dibuat.
pub fn begin_setup(
    settings: &Settings,
    user: &users::Model,
    sealed: Option<&str>,
) -> Result<SetupSecret> {
    let key = settings.auth.encryption_key_bytes()?;
    let reused = sealed.and_then(|s| unseal(&key, s).map(|secret| (secret, s.to_string())));
    let (secret, sealed) = match reused {
        Some(pair) => pair,
        None => {
            let secret = crypto::random_bytes::<SECRET_BYTES>().to_vec();
            let base32 = totp(secret.clone(), None, "")?.get_secret_base32();
            (secret, STANDARD.encode(encrypt(&key, &base32)?))
        }
    };
    let totp = totp(secret, Some(&settings.app_name), &user.email)?;
    let otpauth_uri = totp.get_url();
    let secret_base32 = totp.get_secret_base32();
    Ok(SetupSecret {
        qr_svg: qr_svg(&otpauth_uri)?,
        otpauth_uri,
        secret_base32,
        sealed,
    })
}

/// Konfirmasi setup dengan kode dari authenticator. Kode benar: secret disimpan terenkripsi,
/// 2FA aktif, dan 10 kode pemulihan baru dibuat dalam satu transaksi yang mengunci baris user
/// lebih dulu, sehingga konfirmasi bersamaan berurutan: yang kedua melihat 2FA sudah aktif
/// ([`Confirmed::AlreadyEnabled`]) dan tidak menambah kode.
///
/// # Errors
///
/// Error database atau kunci enkripsi tidak valid.
pub async fn confirm(
    ctx: &AppContext,
    user: &users::Model,
    sealed: &str,
    code: &str,
) -> Result<Confirmed> {
    let key = auth_settings(ctx)?.encryption_key_bytes()?;
    let Some(secret) = unseal(&key, sealed) else {
        return Ok(Confirmed::WrongCode);
    };
    let totp = totp(secret, None, "")?;
    let Some(step) = matching_step(&totp, code, unix_now()) else {
        return Ok(Confirmed::WrongCode);
    };
    let encrypted = encrypt(&key, &totp.get_secret_base32())?;
    let txn = ctx.db.begin().await?;
    let locked = users::Model::lock_for_update(&txn, user.id).await?;
    // Dicek lagi setelah kunci: `user` dimuat sebelum transaksi.
    let Some(locked) = locked.filter(|u| u.totp_enabled_at.is_none()) else {
        return Ok(Confirmed::AlreadyEnabled);
    };
    if !locked.enable_totp(&txn, encrypted, step).await? {
        return Ok(Confirmed::AlreadyEnabled);
    }
    let (codes, digests) = new_recovery_codes();
    recovery_codes::replace_for_user(&txn, user.id, &digests).await?;
    txn.commit().await?;
    Ok(Confirmed::Enabled(codes))
}

/// Kode TOTP benar untuk `user` dan langkahnya lebih baru dari langkah terakhir yang
/// diterima. Langkah itu langsung dicatat (claim bersyarat), sehingga kode yang sama tidak
/// bisa dipakai lagi.
///
/// # Errors
///
/// Error database, kunci enkripsi tidak valid, atau secret tersimpan tidak bisa dibuka.
pub async fn verify(ctx: &AppContext, user: &users::Model, code: &str) -> Result<bool> {
    verify_in(&ctx.db, &auth_settings(ctx)?, user, code).await
}

/// [`verify`] di koneksi atau transaksi `db`.
async fn verify_in<C: ConnectionTrait>(
    db: &C,
    settings: &AuthSettings,
    user: &users::Model,
    code: &str,
) -> Result<bool> {
    let (Some(_), Some(stored)) = (user.totp_enabled_at, &user.totp_secret_encrypted) else {
        return Ok(false);
    };
    let key = settings.encryption_key_bytes()?;
    let base32 = crypto::decrypt(&key, stored)
        .map_err(|e| Error::string(&format!("totp secret of user {}: {e}", user.id)))?;
    let secret = decode_base32(&base32)
        .ok_or_else(|| Error::string(&format!("totp secret of user {} is not base32", user.id)))?;
    let totp = totp(secret, None, "")?;
    match matching_step(&totp, code, unix_now()) {
        Some(step) => Ok(user.claim_totp_step(db, step).await?),
        None => Ok(false),
    }
}

/// Pakai satu kode pemulihan `user`. Huruf besar, `-`, dan spasi diabaikan. Setiap kode hanya
/// berlaku sekali.
///
/// # Errors
///
/// Error database.
pub async fn use_recovery_code(ctx: &AppContext, user: &users::Model, code: &str) -> Result<bool> {
    use_recovery_code_in(&ctx.db, user, code).await
}

/// [`use_recovery_code`] di koneksi atau transaksi `db`.
async fn use_recovery_code_in<C: ConnectionTrait>(
    db: &C,
    user: &users::Model,
    code: &str,
) -> Result<bool> {
    let Some(normalized) = normalize_recovery_code(code) else {
        return Ok(false);
    };
    Ok(recovery_codes::claim(db, user.id, &crypto::sha256_hex(&normalized)).await?)
}

/// Ganti semua kode pemulihan `user` dengan 10 kode baru; kode lama tidak berlaku lagi.
/// Mengembalikan kode mentah untuk ditampilkan sekali, atau `None` bila 2FA tidak (lagi)
/// aktif. Baris user dikunci lebih dulu, sehingga penggantian bersamaan (atau bersamaan
/// dengan mematikan 2FA) berurutan dan tidak meninggalkan lebih dari satu set kode.
///
/// # Errors
///
/// Error database.
pub async fn regenerate_recovery_codes(
    ctx: &AppContext,
    user: &users::Model,
) -> Result<Option<Vec<String>>> {
    let txn = ctx.db.begin().await?;
    let locked = users::Model::lock_for_update(&txn, user.id).await?;
    if !locked.is_some_and(|u| u.totp_enabled_at.is_some()) {
        return Ok(None);
    }
    let (codes, digests) = new_recovery_codes();
    recovery_codes::replace_for_user(&txn, user.id, &digests).await?;
    txn.commit().await?;
    Ok(Some(codes))
}

/// Matikan 2FA bila `current_password` benar **dan** `code` adalah kode TOTP yang berlaku atau
/// kode pemulihan yang belum dipakai: sesi yang dicuri saja tidak cukup untuk melepas 2FA.
/// Kode diperiksa (dan diklaim: langkah TOTP dicatat, kode pemulihan ditandai terpakai) di
/// transaksi yang sama yang menghapus secret, `totp_enabled_at`, `totp_last_step`, dan semua
/// kode pemulihan, setelah baris user dikunci.
///
/// Setiap percobaan dipesan lebih dulu di rate limit per user (`ThrottleKey::Account`) sebelum
/// password diperiksa; password atau kode yang salah tetap terhitung gagal.
///
/// # Errors
///
/// Error database, hashing (503), atau secret tersimpan tidak bisa dibuka.
pub async fn disable(
    ctx: &AppContext,
    user: &users::Model,
    current_password: &SecretString,
    code: &str,
) -> Result<Disabled> {
    let settings = auth_settings(ctx)?;
    let keys = [ThrottleKey::Account(user.id)];
    if throttle::reserve(&ctx.db, &settings, &keys).await? == ThrottleState::Locked {
        return Ok(Disabled::Locked);
    }
    let Some(stored) = user.password_hash.as_deref() else {
        return Ok(Disabled::WrongPassword);
    };
    if password::verify(&settings, current_password, stored).await? == Verify::Mismatch {
        return Ok(Disabled::WrongPassword);
    }
    let txn = ctx.db.begin().await?;
    // Dibaca ulang setelah kunci: `user` dimuat sebelum transaksi.
    let Some(locked) = users::Model::lock_for_update(&txn, user.id)
        .await?
        .filter(|u| u.totp_enabled_at.is_some())
    else {
        return Ok(Disabled::Done);
    };
    let code_ok = if is_totp_code(code) {
        verify_in(&txn, &settings, &locked, code).await?
    } else {
        use_recovery_code_in(&txn, &locked, code).await?
    };
    if !code_ok {
        txn.rollback().await?;
        return Ok(Disabled::WrongCode);
    }
    locked.disable_totp(&txn).await?;
    recovery_codes::delete_for_user(&txn, user.id).await?;
    txn.commit().await?;
    throttle::succeeded(&ctx.db, &settings, &keys).await?;
    Ok(Disabled::Done)
}

/// Langkah kedua login: `code` berupa kode TOTP 6 digit atau kode pemulihan (satu field).
///
/// Setiap percobaan dipesan di rate limit per user sebelum diperiksa (seperti login
/// password). Percobaan yang mencapai batas [`throttle::SECOND_FACTOR_MAX_FAILURES`], dan
/// setiap percobaan selama kunci aktif, menghasilkan [`SecondStep::Locked`].
///
/// # Errors
///
/// Error database atau secret tersimpan tidak bisa dibuka.
pub async fn second_step(ctx: &AppContext, user: &users::Model, code: &str) -> Result<SecondStep> {
    let settings = auth_settings(ctx)?;
    let keys = [ThrottleKey::SecondFactor(user.id)];
    if throttle::reserve(&ctx.db, &settings, &keys).await? == ThrottleState::Locked {
        return Ok(SecondStep::Locked);
    }
    let ok = if is_totp_code(code) {
        verify(ctx, user, code).await?
    } else {
        use_recovery_code(ctx, user, code).await?
    };
    if ok {
        throttle::succeeded(&ctx.db, &settings, &keys).await?;
        return Ok(SecondStep::Accepted);
    }
    if throttle::is_locked(&ctx.db, &keys[0]).await? {
        return Ok(SecondStep::Locked);
    }
    Ok(SecondStep::Rejected)
}

/// Kode TOTP untuk secret base32 `secret_base32` pada waktu `unix_time` (detik epoch). Untuk
/// tes dan alat pengembangan; parameter algoritmenya sama dengan verifikasi.
///
/// # Errors
///
/// Bila `secret_base32` bukan base32 yang valid.
pub fn code_at(secret_base32: &str, unix_time: u64) -> Result<String> {
    let secret = decode_base32(secret_base32.as_bytes())
        .ok_or_else(|| Error::string("totp secret is not base32"))?;
    Ok(totp(secret, None, "")?.generate(unix_time))
}

fn auth_settings(ctx: &AppContext) -> Result<AuthSettings> {
    Ok(Settings::from_ctx(ctx)?.auth)
}

fn totp(secret: Vec<u8>, issuer: Option<&str>, account: &str) -> Result<TOTP> {
    // `:` memisahkan issuer dan akun di URI otpauth; totp-rs menolaknya.
    let clean = |s: &str| s.replace(':', "");
    TOTP::new(
        Algorithm::SHA1,
        DIGITS,
        SKEW_STEPS,
        STEP_SECONDS,
        secret,
        issuer.map(clean),
        clean(account),
    )
    .map_err(|e| Error::string(&format!("totp: {e}")))
}

/// Langkah (`unix_time / 30`) dalam toleransi ±1 yang kodenya sama dengan `code`, dicari dari
/// langkah terbaru. Perbandingan constant-time.
fn matching_step(totp: &TOTP, code: &str, unix_time: u64) -> Option<i64> {
    let code = compact(code);
    if !is_totp_code(&code) {
        return None;
    }
    let current = unix_time / STEP_SECONDS;
    let skew = u64::from(SKEW_STEPS);
    (current.saturating_sub(skew)..=current + skew)
        .rev()
        .find(|step| {
            crypto::ct_eq(
                totp.generate(step * STEP_SECONDS).as_bytes(),
                code.as_bytes(),
            )
        })
        .and_then(|step| i64::try_from(step).ok())
}

/// Kode TOTP: tepat 6 digit setelah spasi dibuang. Selain itu dianggap kode pemulihan.
fn is_totp_code(code: &str) -> bool {
    let code = compact(code);
    code.len() == DIGITS && code.bytes().all(|b| b.is_ascii_digit())
}

/// Tanpa spasi (user sering mengetik `123 456`).
fn compact(code: &str) -> String {
    code.chars().filter(|c| !c.is_whitespace()).collect()
}

/// Bentuk kode pemulihan yang di-digest: huruf kecil, tanpa `-` dan spasi. `None` bila
/// panjang atau hurufnya tidak mungkin kode pemulihan.
fn normalize_recovery_code(code: &str) -> Option<String> {
    let normalized: String = code
        .chars()
        .filter(|c| *c != '-' && !c.is_whitespace())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    let valid = normalized.len() == RECOVERY_CODE_LEN
        && normalized.bytes().all(|b| RECOVERY_ALPHABET.contains(&b));
    valid.then_some(normalized)
}

/// Kode pemulihan baru (`xxxxx-xxxxx`) dan digest-nya.
fn new_recovery_codes() -> (Vec<String>, Vec<String>) {
    (0..RECOVERY_CODE_COUNT)
        .map(|_| {
            let raw = random_recovery_chars();
            let digest = crypto::sha256_hex(&raw);
            (format!("{}-{}", &raw[..5], &raw[5..]), digest)
        })
        .unzip()
}

/// 10 karakter acak dari [`RECOVERY_ALPHABET`]. Rejection sampling: byte di atas kelipatan
/// terbesar panjang alfabet dibuang, supaya setiap huruf sama peluangnya.
fn random_recovery_chars() -> String {
    let n = RECOVERY_ALPHABET.len();
    let limit = 256 - (256 % n);
    let mut out = String::with_capacity(RECOVERY_CODE_LEN);
    while out.len() < RECOVERY_CODE_LEN {
        for b in crypto::random_bytes::<16>() {
            let b = usize::from(b);
            if b < limit && out.len() < RECOVERY_CODE_LEN {
                out.push(char::from(RECOVERY_ALPHABET[b % n]));
            }
        }
    }
    out
}

fn encrypt(key: &[u8; 32], secret_base32: &str) -> Result<Vec<u8>> {
    crypto::encrypt(key, secret_base32.as_bytes()).map_err(|e| Error::string(&e.to_string()))
}

/// Secret mentah dari `sealed` (base64 hasil enkripsi secret base32), atau `None` bila rusak.
fn unseal(key: &[u8; 32], sealed: &str) -> Option<Vec<u8>> {
    let data = STANDARD.decode(sealed).ok()?;
    let base32 = crypto::decrypt(key, &data).ok()?;
    decode_base32(&base32)
}

fn decode_base32(base32: &[u8]) -> Option<Vec<u8>> {
    let text = std::str::from_utf8(base32).ok()?;
    Secret::Encoded(text.to_string()).to_bytes().ok()
}

fn unix_now() -> u64 {
    u64::try_from(chrono::Utc::now().timestamp()).unwrap_or(0)
}

/// QR code `data` sebagai markup SVG: satu `<path>` hitam di atas latar putih (QR butuh
/// kontras gelap-terang, juga di tema gelap). Tanpa script atau URL eksternal, sehingga aman
/// dirender inline di bawah CSP.
fn qr_svg(data: &str) -> Result<String> {
    let qr = QrCode::encode_text(data, QrCodeEcc::Medium)
        .map_err(|e| Error::string(&format!("qr code: {e}")))?;
    let size = qr.size();
    let dim = size + QR_BORDER * 2;
    let mut path = String::new();
    for y in 0..size {
        for x in 0..size {
            if qr.get_module(x, y) {
                path.push_str(&format!("M{},{}h1v1h-1z", x + QR_BORDER, y + QR_BORDER));
            }
        }
    }
    Ok(format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {dim} {dim}" width="{QR_SIZE_PX}" height="{QR_SIZE_PX}" shape-rendering="crispEdges" aria-hidden="true"><rect width="{dim}" height="{dim}" fill="#ffffff"/><path d="{path}" fill="#000000"/></svg>"##
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Vektor uji RFC 6238 (SHA-1, secret ASCII "12345678901234567890"), 6 digit terakhir.
    #[test]
    fn codes_match_rfc_6238_test_vectors() {
        let totp = totp(b"12345678901234567890".to_vec(), None, "").unwrap();
        assert_eq!(totp.generate(59), "287082");
        assert_eq!(totp.generate(1_111_111_109), "081804");
        assert_eq!(totp.generate(2_000_000_000), "279037");
    }

    #[test]
    fn matching_step_accepts_one_step_of_skew_only() {
        let totp = totp(b"12345678901234567890".to_vec(), None, "").unwrap();
        let t = 1_111_111_109;
        let code = totp.generate(t);
        let step = i64::try_from(t / 30).unwrap();
        assert_eq!(matching_step(&totp, &code, t), Some(step));
        assert_eq!(matching_step(&totp, &code, t + 30), Some(step));
        assert_eq!(matching_step(&totp, &code, t - 30), Some(step));
        assert_eq!(matching_step(&totp, &code, t + 60), None);
        let spaced = format!("{} {}", &code[..3], &code[3..]);
        assert_eq!(matching_step(&totp, &spaced, t), Some(step));
        assert_eq!(matching_step(&totp, "12345", t), None);
    }

    #[test]
    fn recovery_codes_use_the_alphabet_and_format() {
        let (codes, digests) = new_recovery_codes();
        assert_eq!(codes.len(), RECOVERY_CODE_COUNT);
        for (code, digest) in codes.iter().zip(&digests) {
            assert_eq!(code.len(), 11);
            assert_eq!(&code[5..6], "-");
            let normalized = normalize_recovery_code(code).unwrap();
            assert_eq!(&crypto::sha256_hex(&normalized), digest);
        }
        assert_eq!(
            normalize_recovery_code("ABCDE-FGHJK").as_deref(),
            Some("abcdefghjk")
        );
        // `0`, `1`, `i`, `l`, `o` bukan bagian alfabet; panjang harus 10.
        assert_eq!(normalize_recovery_code("abcde-fghj0"), None);
        assert_eq!(normalize_recovery_code("abcde-fghj"), None);
    }

    #[test]
    fn qr_svg_is_plain_markup() {
        let svg = qr_svg("otpauth://totp/Agent%20Gate:a%40b.test?secret=JBSWY3DP").unwrap();
        assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(svg.ends_with("</svg>"));
        assert!(!svg.contains("<script"));
        assert!(!svg.contains("href"));
    }
}
