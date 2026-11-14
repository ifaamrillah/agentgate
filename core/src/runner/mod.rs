//! Runner job (spec "Runner & eksekusi job"): kode yang dipakai server dan CLI bersama.
//!
//! - [`protocol`]: bentuk JSON API runner (`/api/runner/…`) dan pesan WebSocket.
//! - [`signing`]: payload kanonik evidence + tanda tangan Ed25519 runner.
//! - [`danger`]: klasifikasi perintah shell ke aksi berbahaya persona.
//! - [`rules`]: file aturan Agent Gate di repo, daftar file aturan lama, checksum drift.
//! - [`branch_name`]: nama branch tiket (`ag/KSR-26-validasi-stok`).

pub mod danger;
pub mod protocol;
pub mod rules;
pub mod signing;

/// Runner dianggap online bila heartbeat terakhirnya lebih baru dari ini (detik). Dipakai server
/// (daftar runner, Status sistem, AI Office) dan CLI; keputusan pemilik R1: 90 detik.
pub const ONLINE_SECONDS: i64 = 90;
/// Interval heartbeat runner (detik): tiga heartbeat per jendela online, jadi satu-dua heartbeat
/// yang hilang tidak membuat runner tampil offline.
pub const HEARTBEAT_SECONDS: u64 = 30;

/// Batas kata judul di nama branch.
const BRANCH_WORDS: usize = 4;
/// Batas panjang slug judul di nama branch.
const BRANCH_SLUG_MAX: usize = 40;

/// Nama branch tiket: `ag/<KODE>-<slug judul>` (spec "Konvensi git"). Slug ASCII huruf kecil,
/// maksimal 4 kata dan 40 karakter; judul tanpa huruf/angka → `ag/<KODE>`.
#[must_use]
pub fn branch_name(code: &str, title: &str) -> String {
    let words: Vec<String> = title
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .take(BRANCH_WORDS)
        .map(str::to_ascii_lowercase)
        .collect();
    let mut slug = words.join("-");
    if slug.len() > BRANCH_SLUG_MAX {
        slug.truncate(BRANCH_SLUG_MAX);
        slug = slug.trim_end_matches('-').to_string();
    }
    let code: String = code
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    if slug.is_empty() {
        format!("ag/{code}")
    } else {
        format!("ag/{code}-{slug}")
    }
}

/// SHA-256 heksadesimal huruf kecil.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn online_window_is_ninety_seconds_with_thirty_second_heartbeat() {
        assert_eq!(ONLINE_SECONDS, 90);
        assert_eq!(HEARTBEAT_SECONDS, 30);
        assert_eq!(
            i64::try_from(HEARTBEAT_SECONDS * 3).unwrap(),
            ONLINE_SECONDS
        );
    }

    #[test]
    fn branch_names_follow_convention() {
        assert_eq!(
            branch_name("KSR-26", "Validasi stok sebelum checkout"),
            "ag/KSR-26-validasi-stok-sebelum-checkout"
        );
        assert_eq!(
            branch_name("KSR-1", "Struk: cetak ulang!"),
            "ag/KSR-1-struk-cetak-ulang"
        );
        assert_eq!(branch_name("KSR-2", "—"), "ag/KSR-2");
        assert_eq!(branch_name("KSR 3;rm", "x"), "ag/KSR3rm-x");
        let long = branch_name("A-1", "abcdefghijklmnopqrstu abcdefghijklmnopqrstu abc");
        assert!(long.len() <= "ag/A-1-".len() + BRANCH_SLUG_MAX, "{long}");
        assert!(!long.ends_with('-'));
    }

    #[test]
    fn sha256_hex_matches_known_value() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
