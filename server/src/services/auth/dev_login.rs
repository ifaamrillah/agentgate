//! Token `/_dev/login` (rules §7): `base64url(user_id || exp)` + `.` + `base64url(HMAC-SHA256)`.
//! Kunci HMAC adalah `auth.session.secret`. Token berlaku 10 menit dan setara password selama
//! itu. Modul ini hanya dikompilasi di build debug.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use sea_orm::prelude::Uuid;
use sha2::Sha256;

/// Umur token, dalam detik.
pub const TTL_SECONDS: i64 = 600;

type HmacSha256 = Hmac<Sha256>;

fn mac(secret: &str, payload: &[u8]) -> Option<HmacSha256> {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).ok()?;
    mac.update(payload);
    Some(mac)
}

/// Token untuk `user_id` yang berlaku sampai `now + TTL_SECONDS` (detik epoch). Kosong bila
/// `secret` tidak bisa dipakai sebagai kunci (HMAC menerima semua panjang, jadi praktis tidak
/// terjadi); token kosong tidak pernah lolos [`verify`].
#[must_use]
pub fn sign(secret: &str, user_id: Uuid, now: i64) -> String {
    let mut payload = Vec::with_capacity(24);
    payload.extend_from_slice(user_id.as_bytes());
    payload.extend_from_slice(&now.saturating_add(TTL_SECONDS).to_be_bytes());
    let Some(mac) = mac(secret, &payload) else {
        return String::new();
    };
    format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(&payload),
        URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
    )
}

/// User id dari token yang tanda tangannya benar dan belum kedaluwarsa pada `now`.
/// Perbandingan tanda tangan constant-time (`verify_slice`).
#[must_use]
pub fn verify(secret: &str, token: &str, now: i64) -> Option<Uuid> {
    let (payload, signature) = token.split_once('.')?;
    let payload = URL_SAFE_NO_PAD.decode(payload).ok()?;
    let signature = URL_SAFE_NO_PAD.decode(signature).ok()?;
    mac(secret, &payload)?.verify_slice(&signature).ok()?;
    let (id, exp) = payload.split_first_chunk::<16>()?;
    let exp = i64::from_be_bytes(exp.try_into().ok()?);
    (now < exp).then(|| Uuid::from_bytes(*id))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "unit-test-secret";

    #[test]
    fn token_round_trips_within_ten_minutes() {
        let id = Uuid::now_v7();
        let token = sign(SECRET, id, 1_000);
        assert_eq!(verify(SECRET, &token, 1_000), Some(id));
        assert_eq!(verify(SECRET, &token, 1_000 + TTL_SECONDS - 1), Some(id));
    }

    #[test]
    fn dev_login_token_expires_after_10_minutes() {
        let token = sign(SECRET, Uuid::now_v7(), 1_000);
        assert_eq!(verify(SECRET, &token, 1_000 + 600), None);
        assert_eq!(verify(SECRET, &token, 1_000 + 3_600), None);
    }

    #[test]
    fn tampered_dev_login_token_is_rejected() {
        let id = Uuid::now_v7();
        let token = sign(SECRET, id, 1_000);
        let (payload, sig) = token.split_once('.').unwrap();
        // Tanda tangan salah kunci.
        assert_eq!(verify("other-secret", &token, 1_000), None);
        // Payload diganti user lain dengan tanda tangan lama.
        let mut forged = URL_SAFE_NO_PAD.decode(payload).unwrap();
        forged[0] ^= 1;
        let forged = format!("{}.{sig}", URL_SAFE_NO_PAD.encode(forged));
        assert_eq!(verify(SECRET, &forged, 1_000), None);
        // Waktu kedaluwarsa diperpanjang.
        let mut longer = URL_SAFE_NO_PAD.decode(payload).unwrap();
        longer[23] ^= 0x40;
        let longer = format!("{}.{sig}", URL_SAFE_NO_PAD.encode(longer));
        assert_eq!(verify(SECRET, &longer, 1_000), None);
        // Bentuk rusak.
        for bad in [
            "",
            ".",
            "abc",
            "a.b",
            &format!("{payload}."),
            &format!(".{sig}"),
        ] {
            assert_eq!(verify(SECRET, bad, 1_000), None, "{bad:?}");
        }
    }
}
