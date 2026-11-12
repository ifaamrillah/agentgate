//! Palet 18 warna (aturan UX 14) untuk project, avatar orang, dan pemilih warna, serta inisial.
//! Warna bawaan dipilih stabil dari sebuah benih (id/nama), jadi orang atau project yang sama
//! selalu mendapat warna yang sama tanpa menyimpan apa pun.

/// Urutan sama dengan prototype (`AG.COLORS`).
pub const COLORS: [&str; 18] = [
    "#7c3aed", "#5b5bd6", "#2563eb", "#0ea5e9", "#0891b2", "#0d9488", "#0f8a5f", "#65a30d",
    "#ca8a04", "#f59e0b", "#ea580c", "#dc2626", "#e11d48", "#db2777", "#c026d3", "#64748b",
    "#475569", "#1f2937",
];

/// Warna palet yang stabil untuk `seed` (FNV-1a, bukan untuk keamanan).
#[must_use]
pub fn color_for(seed: &str) -> &'static str {
    let hash = seed.bytes().fold(0x811c_9dc5_u32, |h, b| {
        (h ^ u32::from(b)).wrapping_mul(0x0100_0193)
    });
    COLORS[(hash % 18) as usize]
}

/// Warna hex `#rrggbb` yang valid (warna custom dari pemilih warna).
pub use crate::common::text::is_hex_color;

/// Inisial 1–2 huruf: huruf pertama dua kata pertama, atau huruf pertama `fallback`.
#[must_use]
pub fn initials(name: &str, fallback: &str) -> String {
    let from_name: String = name
        .split_whitespace()
        .take(2)
        .filter_map(|w| w.chars().next())
        .flat_map(char::to_uppercase)
        .collect();
    if from_name.is_empty() {
        fallback
            .chars()
            .next()
            .into_iter()
            .flat_map(char::to_uppercase)
            .collect()
    } else {
        from_name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_is_stable_and_from_palette() {
        assert_eq!(color_for("budi@contoh.id"), color_for("budi@contoh.id"));
        assert!(COLORS.contains(&color_for("x")));
    }

    #[test]
    fn hex_color_validation() {
        assert!(is_hex_color("#5b5bd6"));
        assert!(!is_hex_color("5b5bd6"));
        assert!(!is_hex_color("#5b5bdz"));
        assert!(!is_hex_color("#fff"));
    }

    #[test]
    fn initials_from_name_or_fallback() {
        assert_eq!(initials("olivia owner", "o@x"), "OO");
        assert_eq!(initials("", "dewi@x"), "D");
    }
}
