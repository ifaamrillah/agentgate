//! Aturan teks yang sama untuk banyak entitas (rules §5): panjang nama dan karakter tak
//! terlihat. Entitas baru memanggil fungsi di sini dengan namanya sendiri sebagai parameter,
//! tidak menyalin aturannya. Error berisi kode dan argumen (`entity`, `max`), bukan kalimat;
//! teksnya ada di Fluent.

use validator::ValidationError;

pub const NAME_REQUIRED: &str = "name-required";
pub const NAME_TOO_LONG: &str = "name-too-long";
pub const NAME_INVISIBLE_CHARS: &str = "name-invisible-chars";
/// Semua kode error modul ini; masing-masing punya pesan di Fluent (dijaga tes i18n).
pub const ERROR_CODES: &[&str] = &[NAME_REQUIRED, NAME_TOO_LONG, NAME_INVISIBLE_CHARS];

/// Panjang maksimal nama entitas (user, workspace, tim, project), dalam karakter.
pub const NAME_MAX_LEN: usize = 100;

/// Normalisasi nama sebelum validasi dan penyimpanan: spasi di awal dan akhir dibuang.
#[must_use]
pub fn normalize_name(value: &str) -> String {
    value.trim().to_string()
}

/// Nama 1–[`NAME_MAX_LEN`] karakter tanpa karakter tak terlihat. `value` diharapkan sudah
/// lewat [`normalize_name`]. Kode: [`NAME_REQUIRED`], [`NAME_TOO_LONG`], [`NAME_INVISIBLE_CHARS`];
/// semuanya membawa argumen `entity` (mis. `"user"`) supaya Fluent bisa memilih kata bendanya.
///
/// # Errors
///
/// [`ValidationError`] dengan salah satu kode di atas.
pub fn validate_name(entity: &'static str, value: &str) -> Result<(), ValidationError> {
    let error = |code: &'static str| {
        let mut e = ValidationError::new(code);
        e.add_param("entity".into(), &entity);
        e
    };
    // Karakter tak terlihat dicek dulu: nama yang hanya berisi zero-width space bukan "kosong"
    // menurut `is_empty`, tetapi tetap tidak terbaca.
    if value.chars().any(is_invisible) {
        return Err(error(NAME_INVISIBLE_CHARS));
    }
    if value.is_empty() {
        return Err(error(NAME_REQUIRED));
    }
    if value.chars().count() > NAME_MAX_LEN {
        let mut e = error(NAME_TOO_LONG);
        e.add_param("max".into(), &NAME_MAX_LEN);
        return Err(e);
    }
    Ok(())
}

/// Karakter kontrol dan karakter format yang tidak tampil (zero-width, penanda arah bidi,
/// soft hyphen, BOM). Spasi biasa di tengah nama boleh.
fn is_invisible(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{00AD}'
                | '\u{034F}'
                | '\u{061C}'
                | '\u{115F}'
                | '\u{1160}'
                | '\u{17B4}'
                | '\u{17B5}'
                | '\u{180E}'
                | '\u{200B}'..='\u{200F}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2060}'..='\u{206F}'
                | '\u{3164}'
                | '\u{FE00}'..='\u{FE0F}'
                | '\u{FEFF}'
                | '\u{FFA0}'
                | '\u{FFF0}'..='\u{FFF8}'
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(value: &str) -> Option<String> {
        validate_name("user", value)
            .err()
            .map(|e| e.code.to_string())
    }

    #[test]
    fn accepts_normal_names_up_to_the_limit() {
        assert_eq!(code("Ana"), None);
        assert_eq!(code("Siti Nurhaliza"), None);
        assert_eq!(code(&"é".repeat(NAME_MAX_LEN)), None);
    }

    #[test]
    fn rejects_empty_too_long_and_invisible() {
        assert_eq!(code("").as_deref(), Some("name-required"));
        assert_eq!(
            code(&"a".repeat(NAME_MAX_LEN + 1)).as_deref(),
            Some("name-too-long")
        );
        for bad in [
            "\u{200B}",
            "Ana\u{200D}",
            "A\u{202E}na",
            "Ana\n",
            "\u{FEFF}Ana",
        ] {
            assert_eq!(
                code(bad).as_deref(),
                Some("name-invisible-chars"),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn errors_carry_entity_and_max() {
        let e = validate_name("workspace", &"a".repeat(NAME_MAX_LEN + 1)).unwrap_err();
        assert_eq!(e.params["entity"], "workspace");
        assert_eq!(e.params["max"], NAME_MAX_LEN);
    }

    #[test]
    fn normalize_trims_outer_whitespace() {
        assert_eq!(normalize_name("  Ana  "), "Ana");
    }
}
