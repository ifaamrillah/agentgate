//! Tanda tangan evidence (spec "Evidence … dihasilkan dan ditandatangani oleh runner").
//!
//! Setiap runner membuat kunci Ed25519 sendiri saat pertama dinyalakan; kunci privat disimpan
//! di state dir perangkat (0600) dan tidak pernah dikirim. Kunci publik didaftarkan ke server
//! bersama runner. Runner menandatangani [`evidence_payload`]; server memverifikasi dengan kunci
//! publik runner yang memegang job itu, sehingga evidence tidak bisa dipalsukan oleh agent lewat
//! MCP maupun oleh runner lain.

use base64::{Engine, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use uuid::Uuid;

use super::protocol::{EvidenceKind, EvidenceSummary};

const DOMAIN: &str = "agentgate-evidence-v1";

/// Byte kanonik yang ditandatangani untuk satu evidence.
#[must_use]
pub fn evidence_payload(
    job_id: Uuid,
    kind: EvidenceKind,
    name: &str,
    sha256: &str,
    summary: &EvidenceSummary,
) -> Vec<u8> {
    // Struct berurutan tetap → JSON deterministik.
    let summary = serde_json::to_string(summary).unwrap_or_default();
    format!(
        "{DOMAIN}\n{job_id}\n{}\n{name}\n{sha256}\n{summary}",
        kind.as_str()
    )
    .into_bytes()
}

/// Kunci penanda tangan dari 32 byte rahasia.
#[must_use]
pub fn signing_key(secret: &[u8; 32]) -> SigningKey {
    SigningKey::from_bytes(secret)
}

/// Kunci publik (base64) dari rahasia.
#[must_use]
pub fn public_key_b64(secret: &[u8; 32]) -> String {
    STANDARD.encode(signing_key(secret).verifying_key().to_bytes())
}

/// Tanda tangan (base64) atas `payload`.
#[must_use]
pub fn sign(secret: &[u8; 32], payload: &[u8]) -> String {
    STANDARD.encode(signing_key(secret).sign(payload).to_bytes())
}

/// Kunci publik base64 valid (32 byte, titik kurva valid).
#[must_use]
pub fn public_key_valid(public_b64: &str) -> bool {
    parse_public(public_b64).is_some()
}

fn parse_public(public_b64: &str) -> Option<VerifyingKey> {
    let bytes: [u8; 32] = STANDARD.decode(public_b64.trim()).ok()?.try_into().ok()?;
    VerifyingKey::from_bytes(&bytes).ok()
}

/// Verifikasi tanda tangan base64 atas `payload` dengan kunci publik base64.
#[must_use]
pub fn verify(public_b64: &str, payload: &[u8], signature_b64: &str) -> bool {
    let Some(key) = parse_public(public_b64) else {
        return false;
    };
    let Ok(sig) = STANDARD.decode(signature_b64.trim()) else {
        return false;
    };
    let Ok(sig): Result<[u8; 64], _> = sig.try_into() else {
        return false;
    };
    key.verify_strict(payload, &Signature::from_bytes(&sig))
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_and_verify_round_trip() {
        let secret = [7u8; 32];
        let other = [9u8; 32];
        let summary = EvidenceSummary {
            passed: Some(true),
            total: Some(9),
            failed: Some(0),
            note: "cargo test".into(),
        };
        let payload = evidence_payload(
            Uuid::nil(),
            EvidenceKind::TestReport,
            "t.txt",
            "ab",
            &summary,
        );
        let sig = sign(&secret, &payload);
        let public = public_key_b64(&secret);
        assert!(public_key_valid(&public));
        assert!(verify(&public, &payload, &sig));
        assert!(!verify(&public_key_b64(&other), &payload, &sig));
        let tampered = evidence_payload(
            Uuid::nil(),
            EvidenceKind::TestReport,
            "t.txt",
            "ab",
            &EvidenceSummary {
                passed: Some(true),
                failed: Some(1),
                ..summary
            },
        );
        assert!(!verify(&public, &tampered, &sig));
        assert!(!verify("bukan-base64", &payload, &sig));
        assert!(!verify(&public, &payload, "AAAA"));
    }
}
