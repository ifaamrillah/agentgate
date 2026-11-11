//! Primitif kriptografi aplikasi: token acak, digest, perbandingan constant-time, dan
//! enkripsi simetris untuk rahasia yang disimpan (secret TOTP). Tidak ada algoritme yang
//! ditulis sendiri (rules §7): semuanya membungkus crate audited.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chacha20poly1305::{Key, KeyInit, XChaCha20Poly1305, XNonce, aead::Aead};
use rand::{RngCore, TryRngCore, rngs::OsRng};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

/// Panjang nonce XChaCha20-Poly1305. 24 byte cukup besar untuk nonce acak tanpa risiko
/// tabrakan, jadi tidak perlu penghitung nonce yang disimpan.
const NONCE_LEN: usize = 24;

/// Byte acak per token email. 32 byte = 256 bit, tidak bisa ditebak.
const TOKEN_BYTES: usize = 32;

#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    /// Data terlalu pendek, kunci salah, atau ciphertext diubah. Sengaja satu varian:
    /// alasan kegagalan tidak dibedakan.
    #[error("ciphertext could not be decrypted")]
    Decrypt,
    /// Hanya terjadi untuk plaintext di atas batas AEAD (~256 GiB).
    #[error("plaintext could not be encrypted")]
    Encrypt,
}

/// Isi `buf` dari RNG sistem operasi. `UnwrapErr` panik bila OS tidak bisa memberi byte acak;
/// tanpa entropi, melanjutkan dengan token lemah lebih buruk daripada request yang gagal.
fn fill_random(buf: &mut [u8]) {
    OsRng.unwrap_err().fill_bytes(buf);
}

/// Token acak 32 byte dari OS, base64url tanpa padding (43 karakter, aman di URL).
#[must_use]
pub fn random_token() -> String {
    let mut bytes = [0u8; TOKEN_BYTES];
    fill_random(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Digest SHA-256 dalam hex huruf kecil. Database hanya menyimpan digest token, bukan tokennya.
#[must_use]
pub fn sha256_hex(input: &str) -> String {
    Sha256::digest(input.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Perbandingan constant-time. Panjang yang berbeda langsung `false` (panjang bukan rahasia).
#[must_use]
pub fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    a.ct_eq(b).into()
}

/// Enkripsi XChaCha20-Poly1305 dengan nonce acak. Hasil: nonce (24 byte) diikuti ciphertext
/// dan tag, satu blob yang cukup disimpan di satu kolom.
pub fn encrypt(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let mut nonce = [0u8; NONCE_LEN];
    fill_random(&mut nonce);
    let ciphertext = cipher(key)
        .encrypt(&XNonce::from(nonce), plaintext)
        .map_err(|_| CryptoError::Encrypt)?;
    let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Kebalikan [`encrypt`]. Gagal bila data diubah, terpotong, atau kuncinya lain.
pub fn decrypt(key: &[u8; 32], data: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let (nonce, ciphertext) = data
        .split_first_chunk::<NONCE_LEN>()
        .ok_or(CryptoError::Decrypt)?;
    cipher(key)
        .decrypt(&XNonce::from(*nonce), ciphertext)
        .map_err(|_| CryptoError::Decrypt)
}

fn cipher(key: &[u8; 32]) -> XChaCha20Poly1305 {
    XChaCha20Poly1305::new(&Key::from(*key))
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: [u8; 32] = [7; 32];

    #[test]
    fn decrypt_returns_the_encrypted_plaintext() {
        let data = encrypt(&KEY, b"JBSWY3DPEHPK3PXP").unwrap();
        assert_eq!(decrypt(&KEY, &data).unwrap(), b"JBSWY3DPEHPK3PXP");
    }

    #[test]
    fn encrypting_twice_uses_a_fresh_nonce() {
        assert_ne!(encrypt(&KEY, b"x").unwrap(), encrypt(&KEY, b"x").unwrap());
    }

    #[test]
    fn a_tampered_ciphertext_is_rejected() {
        let mut data = encrypt(&KEY, b"secret").unwrap();
        let last = data.len() - 1;
        data[last] ^= 1;
        assert!(decrypt(&KEY, &data).is_err());
    }

    #[test]
    fn a_wrong_key_or_truncated_data_is_rejected() {
        let data = encrypt(&KEY, b"secret").unwrap();
        assert!(decrypt(&[8; 32], &data).is_err());
        assert!(decrypt(&KEY, &data[..10]).is_err());
    }

    #[test]
    fn random_tokens_are_distinct_43_char_base64url() {
        let (a, b) = (random_token(), random_token());
        assert_ne!(a, b);
        assert_eq!(a.len(), 43);
        assert!(
            a.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        );
    }

    #[test]
    fn sha256_hex_matches_the_known_digest() {
        assert_eq!(
            sha256_hex("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn ct_eq_compares_content_and_length() {
        assert!(ct_eq(b"abc", b"abc"));
        assert!(!ct_eq(b"abc", b"abd"));
        assert!(!ct_eq(b"abc", b"abcd"));
    }
}
