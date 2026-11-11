//! Rate limit login per email dan per IP di tabel `login_throttles` (rules §7, tanpa Redis).
//!
//! Satu baris per kunci (`email:<lower>` / `ip:<addr>`). Setiap gagal menambah `failures` di
//! jendela `login_window_minutes`; mencapai batas mengunci kunci itu `login_lockout_minutes`.
//! Kunci email berlaku untuk string email apa pun, terdaftar atau tidak, supaya "terlalu
//! banyak percobaan" tidak membocorkan email mana yang punya akun. Baris kedaluwarsa
//! dibersihkan job harian (M1 Task 9), bukan di sini.

use std::net::IpAddr;

use loco_rs::Result;
use sea_orm::{
    ConnectionTrait, DbBackend, ExprTrait, Statement, Value, entity::prelude::*, sea_query::Expr,
};

use crate::{
    common::settings::AuthSettings,
    models::{login_throttles, users},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThrottleKey {
    /// Email apa adanya dari form; dinormalisasi sama seperti `users.email`.
    Email(String),
    Ip(IpAddr),
}

impl ThrottleKey {
    /// Nilai kolom `login_throttles.key`.
    #[must_use]
    pub fn key(&self) -> String {
        match self {
            Self::Email(email) => format!("email:{}", users::normalize_email(email)),
            Self::Ip(ip) => format!("ip:{ip}"),
        }
    }

    fn max_failures(&self, settings: &AuthSettings) -> i32 {
        match self {
            Self::Email(_) => settings.login_max_failures_per_email,
            Self::Ip(_) => settings.login_max_failures_per_ip,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThrottleState {
    Open,
    /// Salah satu kunci sedang terkunci.
    Locked,
}

/// `Locked` bila salah satu `keys` masih terkunci.
///
/// # Errors
///
/// Error database.
pub async fn check<C: ConnectionTrait>(db: &C, keys: &[ThrottleKey]) -> Result<ThrottleState> {
    let locked = login_throttles::Entity::find()
        .filter(login_throttles::Column::Key.is_in(keys.iter().map(ThrottleKey::key)))
        .filter(Expr::col(login_throttles::Column::LockedUntil).gt(Expr::current_timestamp()))
        .count(db)
        .await?;
    Ok(if locked > 0 {
        ThrottleState::Locked
    } else {
        ThrottleState::Open
    })
}

/// Catat satu kegagalan untuk setiap kunci, dalam satu upsert atomik per kunci (dua request
/// paralel tidak saling menimpa hitungan). Jendela yang sudah lewat, atau kunci yang masa
/// kuncinya sudah habis, mulai dihitung dari 1 lagi.
///
/// # Errors
///
/// Error database.
pub async fn record_failure<C: ConnectionTrait>(
    db: &C,
    settings: &AuthSettings,
    keys: &[ThrottleKey],
) -> Result<()> {
    // Semua ekspresi di `DO UPDATE SET` membaca nilai baris lama, jadi kondisi "mulai ulang"
    // ditulis ulang di setiap kolom.
    const SQL: &str = "
        INSERT INTO login_throttles AS t (key, failures, window_started_at, locked_until)
        VALUES ($1, 1, now(),
                CASE WHEN 1 >= $2 THEN now() + make_interval(mins => $4) END)
        ON CONFLICT (key) DO UPDATE SET
            failures = CASE WHEN t.window_started_at <= now() - make_interval(mins => $3)
                                 OR t.locked_until <= now()
                            THEN 1 ELSE t.failures + 1 END,
            window_started_at = CASE WHEN t.window_started_at <= now() - make_interval(mins => $3)
                                          OR t.locked_until <= now()
                                     THEN now() ELSE t.window_started_at END,
            locked_until = CASE
                WHEN t.window_started_at <= now() - make_interval(mins => $3)
                     OR t.locked_until <= now()
                    THEN CASE WHEN 1 >= $2 THEN now() + make_interval(mins => $4) END
                WHEN t.failures + 1 >= $2 THEN now() + make_interval(mins => $4)
                ELSE t.locked_until END,
            updated_at = now()";
    let window = minutes(settings.login_window_minutes);
    let lockout = minutes(settings.login_lockout_minutes);
    for key in keys {
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            SQL,
            [
                Value::from(key.key()),
                Value::from(key.max_failures(settings)),
                Value::from(window),
                Value::from(lockout),
            ],
        ))
        .await?;
    }
    Ok(())
}

/// Hapus penghitung satu kunci (login berhasil mereset penghitung email-nya).
///
/// # Errors
///
/// Error database.
pub async fn reset<C: ConnectionTrait>(db: &C, key: &ThrottleKey) -> Result<()> {
    login_throttles::Entity::delete_by_id(key.key())
        .exec(db)
        .await?;
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
    }
}
