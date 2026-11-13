//! Pratinjau mockup HTML (M10, spec §Keamanan "Mockup HTML di iframe sandbox domain terpisah").
//!
//! Mockup = artifact tiket bernama `*.html` (dibuat persona UI/UX lewat `ag_artifact`). Isinya
//! tidak pernah dilayani di origin aplikasi: halaman mockup memuat iframe ke
//! `{preview_base_url}/preview/{token}`. Token = id versi artifact + waktu kedaluwarsa (1 jam),
//! ditandatangani HMAC-SHA256 dengan kunci turunan `encryption_key`, jadi domain pratinjau tidak
//! butuh sesi/cookie dan token tidak bisa dirakit. Respons pratinjau membawa CSP `sandbox`
//! sendiri (origin unik, tanpa akses storage) dan `frame-ancestors` hanya origin aplikasi.
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use uuid::Uuid;

use crate::common::crypto::ct_eq;

/// Umur token pratinjau (detik).
pub const TOKEN_TTL_SECONDS: i64 = 3600;

type HmacSha256 = Hmac<Sha256>;

/// Artifact ini mockup HTML?
#[must_use]
pub fn is_mockup(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".html") || lower.ends_with(".htm")
}

/// Kunci tanda tangan pratinjau, diturunkan dari kunci enkripsi aplikasi (tidak dipakai ulang
/// apa adanya untuk tujuan lain).
#[must_use]
pub fn signing_key(encryption_key: &[u8; 32]) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(encryption_key).expect("HMAC menerima kunci apa pun");
    mac.update(b"agentgate/mockup-preview/v1");
    mac.finalize().into_bytes().into()
}

fn mac_of(key: &[u8; 32], payload: &str) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC menerima kunci apa pun");
    mac.update(payload.as_bytes());
    mac.finalize().into_bytes().to_vec()
}

/// Token pratinjau versi `version_id` yang berlaku sampai `expires_at` (detik unix).
#[must_use]
pub fn sign(key: &[u8; 32], version_id: Uuid, expires_at: i64) -> String {
    let payload = format!("{}.{expires_at}", version_id.simple());
    format!(
        "{payload}.{}",
        URL_SAFE_NO_PAD.encode(mac_of(key, &payload))
    )
}

/// Id versi dari token yang sah dan belum kedaluwarsa pada `now` (detik unix).
#[must_use]
pub fn verify(key: &[u8; 32], token: &str, now: i64) -> Option<Uuid> {
    let mut parts = token.splitn(3, '.');
    let (id, exp, sig) = (parts.next()?, parts.next()?, parts.next()?);
    let payload = format!("{id}.{exp}");
    let given = URL_SAFE_NO_PAD.decode(sig).ok()?;
    if !ct_eq(&given, &mac_of(key, &payload)) {
        return None;
    }
    let exp: i64 = exp.parse().ok()?;
    if exp < now {
        return None;
    }
    Uuid::parse_str(id).ok()
}

/// CSP respons pratinjau: sandbox (origin unik, skrip boleh untuk interaksi mockup), tanpa
/// koneksi keluar selain gambar/font, dan hanya boleh dibingkai origin aplikasi.
#[must_use]
pub fn preview_csp(app_origin: &str) -> String {
    format!(
        "sandbox allow-scripts allow-forms allow-popups; default-src 'none'; \
         script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data: https:; \
         font-src data: https:; frame-ancestors {app_origin}; base-uri 'none'; form-action 'none'"
    )
}

/// CSP halaman aplikasi yang membingkai pratinjau: CSP aplikasi + `frame-src` domain pratinjau.
#[must_use]
pub fn app_csp_with_frame(app_csp: &str, preview_origin: &str) -> String {
    let base = app_csp.trim().trim_end_matches(';');
    format!("{base}; frame-src {preview_origin}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_round_trips_and_rejects_tampering_and_expiry() {
        let key = signing_key(&[7u8; 32]);
        let id = Uuid::now_v7();
        let token = sign(&key, id, 1_000);
        assert_eq!(verify(&key, &token, 999), Some(id));
        assert_eq!(verify(&key, &token, 1_001), None, "kedaluwarsa");
        let other = Uuid::now_v7();
        let forged = token.replacen(&id.simple().to_string(), &other.simple().to_string(), 1);
        assert_eq!(verify(&key, &forged, 999), None, "id ditukar");
        let longer = token.replacen(".1000.", ".9999999999.", 1);
        assert_eq!(verify(&key, &longer, 999), None, "kedaluwarsa diubah");
        assert_eq!(
            verify(&signing_key(&[8u8; 32]), &token, 999),
            None,
            "kunci lain"
        );
        assert_eq!(verify(&key, "x", 0), None);
    }

    #[test]
    fn only_html_artifacts_are_mockups() {
        assert!(is_mockup("checkout.html"));
        assert!(is_mockup("Mockup.HTM"));
        assert!(!is_mockup("prd.md"));
    }

    #[test]
    fn csp_sandboxes_and_limits_framing() {
        let csp = preview_csp("http://localhost:5150");
        assert!(csp.starts_with("sandbox allow-scripts"));
        assert!(!csp.contains("allow-same-origin"));
        assert!(csp.contains("frame-ancestors http://localhost:5150"));
        let app = app_csp_with_frame("default-src 'self'; frame-ancestors 'none'", "http://p.x");
        assert!(app.ends_with("; frame-src http://p.x"));
    }
}
