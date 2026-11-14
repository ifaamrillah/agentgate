//! Aturan teks yang sama untuk banyak entitas (rules §5): panjang nama, karakter tak
//! terlihat, dan slug URL. Entitas baru memanggil fungsi di sini dengan namanya sendiri sebagai parameter,
//! tidak menyalin aturannya. Error berisi kode dan argumen (`entity`, `max`), bukan kalimat;
//! teksnya ada di Fluent.

use validator::ValidationError;

pub const NAME_REQUIRED: &str = "name-required";
pub const NAME_TOO_LONG: &str = "name-too-long";
pub const NAME_INVISIBLE_CHARS: &str = "name-invisible-chars";
pub const SLUG_TOO_SHORT: &str = "slug-too-short";
pub const SLUG_TOO_LONG: &str = "slug-too-long";
pub const SLUG_INVALID: &str = "slug-invalid";
/// Semua kode error modul ini; masing-masing punya pesan di Fluent (dijaga tes i18n).
pub const ERROR_CODES: &[&str] = &[
    NAME_REQUIRED,
    NAME_TOO_LONG,
    NAME_INVISIBLE_CHARS,
    SLUG_TOO_SHORT,
    SLUG_TOO_LONG,
    SLUG_INVALID,
];

/// Panjang maksimal nama entitas (user, workspace, tim, project), dalam karakter.
pub const NAME_MAX_LEN: usize = 100;
/// Panjang slug URL (workspace), dalam karakter ASCII.
pub const SLUG_MIN_LEN: usize = 3;
pub const SLUG_MAX_LEN: usize = 40;

/// Slug dari teks bebas (mis. nama workspace): huruf kecil ASCII `[a-z0-9-]`. Huruf Latin
/// beraksen ditransliterasi sederhana (`é` → `e`, `ß` → `ss`), huruf lain dibuang, dan
/// karakter lain (spasi, tanda baca) menjadi pemisah `-`. Pemisah berurutan digabung, `-` di
/// awal dan akhir dibuang, lalu dipotong ke [`SLUG_MAX_LEN`]. Hasilnya bisa lebih pendek dari
/// [`SLUG_MIN_LEN`] (mis. nama tanpa huruf Latin); [`validate_slug`] yang menolaknya.
#[must_use]
pub fn slugify(value: &str) -> String {
    let mut raw = String::with_capacity(value.len());
    for c in value.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            raw.push(c);
        } else if let Some(ascii) = transliterate(c) {
            raw.push_str(ascii);
        } else if !c.is_alphanumeric() {
            raw.push('-');
        }
    }
    let mut slug = String::with_capacity(raw.len());
    for part in raw.split('-').filter(|p| !p.is_empty()) {
        if !slug.is_empty() {
            slug.push('-');
        }
        slug.push_str(part);
    }
    slug.truncate(SLUG_MAX_LEN);
    slug.trim_end_matches('-').to_string()
}

/// Slug yang sudah dinormalisasi pemanggil (huruf kecil): [`SLUG_MIN_LEN`]–[`SLUG_MAX_LEN`]
/// karakter, hanya `[a-z0-9-]`, tanpa `-` di awal/akhir atau berurutan (bentuk keluaran
/// [`slugify`]). Kode: [`SLUG_TOO_SHORT`] (`min`), [`SLUG_TOO_LONG`] (`max`), [`SLUG_INVALID`].
///
/// # Errors
///
/// [`ValidationError`] dengan salah satu kode di atas.
pub fn validate_slug(value: &str) -> Result<(), ValidationError> {
    let well_formed = value.split('-').all(|part| {
        !part.is_empty()
            && part
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
    });
    if !value.is_empty() && !well_formed {
        return Err(ValidationError::new(SLUG_INVALID));
    }
    let len = value.len();
    if len < SLUG_MIN_LEN {
        let mut e = ValidationError::new(SLUG_TOO_SHORT);
        e.add_param("min".into(), &SLUG_MIN_LEN);
        return Err(e);
    }
    if len > SLUG_MAX_LEN {
        let mut e = ValidationError::new(SLUG_TOO_LONG);
        e.add_param("max".into(), &SLUG_MAX_LEN);
        return Err(e);
    }
    Ok(())
}

/// Transliterasi sederhana huruf Latin beraksen yang umum (sudah huruf kecil).
fn transliterate(c: char) -> Option<&'static str> {
    Some(match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => "a",
        'æ' => "ae",
        'ç' | 'ć' | 'č' => "c",
        'ď' | 'đ' | 'ð' => "d",
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ė' | 'ę' | 'ě' => "e",
        'ğ' => "g",
        'ì' | 'í' | 'î' | 'ï' | 'ī' | 'į' | 'ı' => "i",
        'ł' => "l",
        'ñ' | 'ń' | 'ň' => "n",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ő' => "o",
        'œ' => "oe",
        'ř' => "r",
        'ś' | 'š' | 'ş' => "s",
        'ß' => "ss",
        'ť' | 'ţ' => "t",
        'þ' => "th",
        'ù' | 'ú' | 'û' | 'ü' | 'ū' | 'ů' | 'ű' | 'ų' => "u",
        'ý' | 'ÿ' => "y",
        'ź' | 'ż' | 'ž' => "z",
        _ => return None,
    })
}

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

/// Warna hex `#rrggbb` (huruf besar/kecil; disimpan huruf kecil oleh model).
#[must_use]
pub fn is_hex_color(value: &str) -> bool {
    value.len() == 7 && value.starts_with('#') && value[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// Pola `LIKE` "mengandung" untuk pencarian teks: huruf kecil, di-trim, dengan `%`, `_`, dan `\`
/// di-escape (dicocokkan dengan `lower(kolom) LIKE pola`).
#[must_use]
pub fn contains_pattern(q: &str) -> String {
    let raw = q.trim().to_lowercase();
    let escaped = raw
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
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

    fn slug_code(value: &str) -> Option<String> {
        validate_slug(value).err().map(|e| e.code.to_string())
    }

    #[test]
    fn slugify_lowercases_and_joins_words_with_dashes() {
        assert_eq!(slugify("Acme Studio"), "acme-studio");
        assert_eq!(slugify("PT Contoh"), "pt-contoh");
        assert_eq!(slugify("  --Tim  QA!!  2026--  "), "tim-qa-2026");
        assert_eq!(slugify("a_b.c/d"), "a-b-c-d");
        assert_eq!(slugify("Already-a-slug"), "already-a-slug");
    }

    #[test]
    fn slugify_transliterates_or_drops_non_ascii() {
        assert_eq!(slugify("Café Señor"), "cafe-senor");
        assert_eq!(slugify("Straße Ærø"), "strasse-aero");
        assert_eq!(slugify("ŁÓDŹ"), "lodz");
        // Huruf non-Latin dibuang tanpa menjadi pemisah; emoji menjadi pemisah.
        assert_eq!(slugify("東京 Tokyo"), "tokyo");
        assert_eq!(slugify("Kopi ☕ Senja"), "kopi-senja");
        assert_eq!(slugify("東京"), "");
    }

    #[test]
    fn slugify_truncates_without_trailing_dash() {
        let long = format!("{} {}", "a".repeat(SLUG_MAX_LEN - 1), "bbbb");
        let slug = slugify(&long);
        assert_eq!(slug, "a".repeat(SLUG_MAX_LEN - 1));
        assert!(slugify(&"x".repeat(100)).len() == SLUG_MAX_LEN);
    }

    #[test]
    fn validate_slug_edges() {
        assert_eq!(slug_code("abc"), None);
        assert_eq!(slug_code("a-1"), None);
        assert_eq!(slug_code(&"a".repeat(SLUG_MAX_LEN)), None);
        assert_eq!(slug_code("ab").as_deref(), Some(SLUG_TOO_SHORT));
        assert_eq!(slug_code("").as_deref(), Some(SLUG_TOO_SHORT));
        assert_eq!(
            slug_code(&"a".repeat(SLUG_MAX_LEN + 1)).as_deref(),
            Some(SLUG_TOO_LONG)
        );
        for bad in ["-abc", "abc-", "a--b", "Abc", "a b c", "café", "a_b"] {
            assert_eq!(slug_code(bad).as_deref(), Some(SLUG_INVALID), "{bad}");
        }
        // Setiap keluaran slugify yang cukup panjang lolos validasi.
        for name in ["Acme Studio", "Café Señor", "  --Tim  QA!!  2026--  "] {
            assert_eq!(slug_code(&slugify(name)), None, "{name}");
        }
    }

    #[test]
    fn normalize_trims_outer_whitespace() {
        assert_eq!(normalize_name("  Ana  "), "Ana");
    }
}
