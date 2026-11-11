//! Rate limit login, dan permintaan yang mengirim email, per email dan per IP di tabel
//! `login_throttles` (rules §7, tanpa Redis).
//!
//! Satu baris per kunci (`email:<lower>` / `ip:<addr>` untuk login, `mail-email:<lower>` /
//! `mail-ip:<addr>` untuk permintaan email). Setiap percobaan dipesan lebih dulu
//! ([`reserve`]) di jendela `login_window_minutes`; mencapai batas mengunci kunci itu
//! `login_lockout_minutes`. Percobaan yang berhasil dikembalikan ([`succeeded`]).
//! Kunci email berlaku untuk string email apa pun, terdaftar atau tidak, supaya "terlalu
//! banyak percobaan" tidak membocorkan email mana yang punya akun. Baris kedaluwarsa
//! dibersihkan job harian (M1 Task 9), bukan di sini.

use std::net::IpAddr;

use loco_rs::Result;
use sea_orm::{ConnectionTrait, DbBackend, Statement, Value, entity::prelude::*};

use crate::{
    common::settings::AuthSettings,
    models::{login_throttles, users},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThrottleKey {
    /// Percobaan login per email, apa adanya dari form; dinormalisasi sama seperti
    /// `users.email`.
    Email(String),
    /// Percobaan login per IP.
    Ip(IpAddr),
    /// Permintaan yang mengirim email (daftar, lupa password, kirim ulang verifikasi) per
    /// alamat tujuan. Penghitungnya terpisah dari login: permintaan reset berulang tidak
    /// boleh mengunci login pemilik akun, dan login berhasil tidak mereset batas email.
    /// Setiap permintaan dihitung (tidak ada [`succeeded`]); batas dan jendelanya sama dengan
    /// login (`login_max_failures_per_email`, `login_window_minutes`).
    MailEmail(String),
    /// Permintaan yang mengirim email per IP, batas `login_max_failures_per_ip`.
    MailIp(IpAddr),
}

impl ThrottleKey {
    /// Nilai kolom `login_throttles.key`.
    #[must_use]
    pub fn key(&self) -> String {
        match self {
            Self::Email(email) => format!("email:{}", users::normalize_email(email)),
            Self::Ip(ip) => format!("ip:{ip}"),
            Self::MailEmail(email) => format!("mail-email:{}", users::normalize_email(email)),
            Self::MailIp(ip) => format!("mail-ip:{ip}"),
        }
    }

    fn max_failures(&self, settings: &AuthSettings) -> i32 {
        match self {
            Self::Email(_) | Self::MailEmail(_) => settings.login_max_failures_per_email,
            Self::Ip(_) | Self::MailIp(_) => settings.login_max_failures_per_ip,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThrottleState {
    Open,
    /// Salah satu kunci sudah melewati batas: percobaan ini tidak boleh diverifikasi.
    Locked,
}

/// Pesan satu percobaan login untuk setiap kunci **sebelum** password diverifikasi, dalam
/// satu upsert atomik per kunci yang mengembalikan hitungan barunya (`RETURNING`). Percobaan
/// dihitung sebagai gagal sejak awal; yang berhasil dikembalikan lewat [`succeeded`].
///
/// Karena hitungan dinaikkan dan dibaca dalam satu statement (baris dikunci `ON CONFLICT`),
/// N request paralel mendapat hitungan 1..N, dan hanya yang hitungannya <= batas yang boleh
/// lanjut ke verifikasi. Memeriksa lalu mencatat belakangan (dua langkah) bisa dilewati
/// dengan burst: semua request lolos pemeriksaan sebelum ada yang tercatat.
///
/// Aturan per kunci:
/// - percobaan yang membuat hitungan mencapai batas masih boleh, dan mulai mengunci kunci
///   itu `login_lockout_minutes`; percobaan berikutnya selama terkunci ditolak (hitungan tetap
///   naik, masa kunci tidak diperpanjang);
/// - jendela `login_window_minutes` yang sudah lewat (tanpa kunci aktif), atau kunci yang
///   sudah habis, mulai dihitung dari 1 lagi.
///
/// Semua kunci selalu dipesan, walau kunci pertama sudah terkunci: urutan dan jumlah query
/// sama untuk email terdaftar dan tidak.
///
/// # Errors
///
/// Error database.
pub async fn reserve<C: ConnectionTrait>(
    db: &C,
    settings: &AuthSettings,
    keys: &[ThrottleKey],
) -> Result<ThrottleState> {
    // Semua ekspresi di `DO UPDATE SET` membaca nilai baris lama, jadi kondisi "mulai ulang"
    // ditulis ulang di setiap kolom.
    const SQL: &str = "
        INSERT INTO login_throttles AS t (key, failures, window_started_at, locked_until)
        VALUES ($1, 1, now(),
                CASE WHEN 1 >= $2 THEN now() + make_interval(mins => $4) END)
        ON CONFLICT (key) DO UPDATE SET
            failures = CASE WHEN (t.locked_until IS NULL
                                  AND t.window_started_at <= now() - make_interval(mins => $3))
                                 OR t.locked_until <= now()
                            THEN 1 ELSE t.failures + 1 END,
            window_started_at = CASE WHEN (t.locked_until IS NULL
                                           AND t.window_started_at <= now() - make_interval(mins => $3))
                                          OR t.locked_until <= now()
                                     THEN now() ELSE t.window_started_at END,
            locked_until = CASE
                WHEN (t.locked_until IS NULL
                      AND t.window_started_at <= now() - make_interval(mins => $3))
                     OR t.locked_until <= now()
                    THEN CASE WHEN 1 >= $2 THEN now() + make_interval(mins => $4) END
                WHEN t.locked_until IS NULL AND t.failures + 1 >= $2
                    THEN now() + make_interval(mins => $4)
                ELSE t.locked_until END,
            updated_at = now()
        RETURNING failures";
    let window = minutes(settings.login_window_minutes);
    let lockout = minutes(settings.login_lockout_minutes);
    let mut state = ThrottleState::Open;
    for key in keys {
        let max = key.max_failures(settings);
        let row = db
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                SQL,
                [
                    Value::from(key.key()),
                    Value::from(max),
                    Value::from(window),
                    Value::from(lockout),
                ],
            ))
            .await?
            .ok_or_else(|| loco_rs::Error::string("login_throttles upsert returned no row"))?;
        let failures: i32 = row.try_get_by_index(0)?;
        if failures > max {
            state = ThrottleState::Locked;
        }
    }
    Ok(state)
}

/// Percobaan yang dipesan [`reserve`] ternyata berhasil (password benar).
///
/// - Kunci email dihapus: login berhasil mereset penghitung email itu (rules §7).
/// - Kunci IP dikurangi satu, dan kuncinya dibuka bila hitungannya kembali di bawah batas:
///   login berhasil **tidak** dihitung ke batas IP, supaya banyak user di balik satu NAT
///   kantor tidak saling mengunci. Batas IP tetap menghitung semua percobaan gagal.
///
/// # Errors
///
/// Error database.
pub async fn succeeded<C: ConnectionTrait>(
    db: &C,
    settings: &AuthSettings,
    keys: &[ThrottleKey],
) -> Result<()> {
    const RELEASE_SQL: &str = "
        UPDATE login_throttles SET
            failures = GREATEST(failures - 1, 0),
            locked_until = CASE WHEN failures - 1 < $2 THEN NULL ELSE locked_until END,
            updated_at = now()
        WHERE key = $1";
    for key in keys {
        match key {
            ThrottleKey::Email(_) | ThrottleKey::MailEmail(_) => {
                login_throttles::Entity::delete_by_id(key.key())
                    .exec(db)
                    .await?;
            }
            ThrottleKey::Ip(_) | ThrottleKey::MailIp(_) => {
                db.execute_raw(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    RELEASE_SQL,
                    [
                        Value::from(key.key()),
                        Value::from(key.max_failures(settings)),
                    ],
                ))
                .await?;
            }
        }
    }
    Ok(())
}

/// `make_interval(mins => …)` menerima `integer`; setting di luar rentang itu dipotong.
fn minutes(value: i64) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_key_is_normalized() {
        assert_eq!(
            ThrottleKey::Email("  Ana@Example.COM ".into()).key(),
            "email:ana@example.com"
        );
        assert_eq!(
            ThrottleKey::Ip("127.0.0.1".parse().unwrap()).key(),
            "ip:127.0.0.1"
        );
        assert_eq!(
            ThrottleKey::MailEmail(" Ana@Example.COM".into()).key(),
            "mail-email:ana@example.com"
        );
        assert_eq!(
            ThrottleKey::MailIp("::1".parse().unwrap()).key(),
            "mail-ip:::1"
        );
    }
}
