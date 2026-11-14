//! Template kode tiket (spec "Kode tiket"): mis. `{PROJECT}-{NUM}` → `KSR-123`, atau
//! `{TEAM}-{YYYY}-{NUM:4}` → `BE-2026-0042`. Default di workspace, bisa ditimpa per project.
//!
//! Token: `{PROJECT}` (key project), `{TEAM}` (kode tim project, bawaan = key project), `{TYPE}`
//! (key tipe, huruf besar), `{YYYY}`/`{YY}` (tahun tiket dibuat), `{NUM}` dan `{NUM:n}` (nomor,
//! diisi nol sampai `n` digit). Karakter lain hanya `A–Z a–z 0–9 - _ .`, supaya kode aman di URL,
//! nama branch, dan pesan commit.
//!
//! Penghitung nomor ([`CounterScope`]) per project, per tipe, atau per tahun. Template harus bisa
//! membedakan nomor dari penghitung berbeda: skop `type` wajib memuat `{TYPE}`, skop `year` wajib
//! memuat `{YYYY}` atau `{YY}`.

use sea_orm::entity::prelude::*;
use validator::ValidationError;

pub const TEMPLATE_DEFAULT: &str = "{PROJECT}-{NUM}";
/// Panjang maksimal template.
pub const TEMPLATE_MAX_LEN: usize = 40;

pub const CODE_TEMPLATE_INVALID: &str = "ticket-code-template-invalid";
pub const CODE_TEMPLATE_NUM_MISSING: &str = "ticket-code-template-num-missing";
pub const CODE_TEMPLATE_TYPE_MISSING: &str = "ticket-code-template-type-missing";
pub const CODE_TEMPLATE_YEAR_MISSING: &str = "ticket-code-template-year-missing";
pub const TEAM_CODE_INVALID: &str = "ticket-team-code-invalid";
/// Mengganti template membuat dua tiket berkode sama.
pub const CODE_COLLISION: &str = "ticket-code-collision";
pub const ERROR_CODES: &[&str] = &[
    CODE_TEMPLATE_INVALID,
    CODE_TEMPLATE_NUM_MISSING,
    CODE_TEMPLATE_TYPE_MISSING,
    CODE_TEMPLATE_YEAR_MISSING,
    TEAM_CODE_INVALID,
    CODE_COLLISION,
];

/// Skop penghitung nomor tiket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum CounterScope {
    #[sea_orm(string_value = "project")]
    Project,
    #[sea_orm(string_value = "type")]
    Type,
    #[sea_orm(string_value = "year")]
    Year,
}

impl CounterScope {
    /// Nilai kolom `scope` di `ticket_counters` untuk tiket bertipe `type_id` di tahun `year`.
    #[must_use]
    pub fn counter_key(self, type_id: Uuid, year: i32) -> String {
        match self {
            Self::Project => String::new(),
            Self::Type => format!("type:{type_id}"),
            Self::Year => format!("year:{year}"),
        }
    }
}

/// Bagian satu template yang sudah di-parse.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Part {
    Literal(String),
    Project,
    Team,
    Type,
    Year4,
    Year2,
    Num(usize),
}

/// Nilai token untuk satu tiket.
#[derive(Debug, Clone)]
pub struct CodeParts<'a> {
    pub project: &'a str,
    pub team: &'a str,
    pub type_key: &'a str,
    pub year: i32,
    pub number: i32,
}

fn parse(template: &str) -> Result<Vec<Part>, ValidationError> {
    let invalid = || ValidationError::new(CODE_TEMPLATE_INVALID);
    if template.is_empty() || template.chars().count() > TEMPLATE_MAX_LEN {
        let mut e = invalid();
        e.add_param("max".into(), &TEMPLATE_MAX_LEN);
        return Err(e);
    }
    let mut parts = Vec::new();
    let mut rest = template;
    let mut literal = String::new();
    while let Some(c) = rest.chars().next() {
        if c == '{' {
            let end = rest.find('}').ok_or_else(invalid)?;
            let token = &rest[1..end];
            let part = match token {
                "PROJECT" => Part::Project,
                "TEAM" => Part::Team,
                "TYPE" => Part::Type,
                "YYYY" => Part::Year4,
                "YY" => Part::Year2,
                "NUM" => Part::Num(0),
                _ => match token
                    .strip_prefix("NUM:")
                    .and_then(|n| n.parse::<usize>().ok())
                {
                    Some(n @ 1..=9) => Part::Num(n),
                    _ => return Err(invalid()),
                },
            };
            if !literal.is_empty() {
                parts.push(Part::Literal(std::mem::take(&mut literal)));
            }
            parts.push(part);
            rest = &rest[end + 1..];
        } else if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
            literal.push(c);
            rest = &rest[1..];
        } else {
            return Err(invalid());
        }
    }
    if !literal.is_empty() {
        parts.push(Part::Literal(literal));
    }
    Ok(parts)
}

/// Template diterima untuk skop `scope`.
///
/// # Errors
///
/// [`ValidationError`] dengan salah satu kode `ticket-code-template-*`.
pub fn validate(template: &str, scope: CounterScope) -> Result<(), ValidationError> {
    let parts = parse(template)?;
    if !parts.iter().any(|p| matches!(p, Part::Num(_))) {
        return Err(ValidationError::new(CODE_TEMPLATE_NUM_MISSING));
    }
    if scope == CounterScope::Type && !parts.contains(&Part::Type) {
        return Err(ValidationError::new(CODE_TEMPLATE_TYPE_MISSING));
    }
    if scope == CounterScope::Year && !parts.iter().any(|p| matches!(p, Part::Year4 | Part::Year2))
    {
        return Err(ValidationError::new(CODE_TEMPLATE_YEAR_MISSING));
    }
    Ok(())
}

/// Kode tim untuk `{TEAM}`: kosong (= key project) atau 1–10 huruf/angka, disimpan huruf besar.
///
/// # Errors
///
/// [`TEAM_CODE_INVALID`].
pub fn normalize_team_code(raw: &str) -> Result<Option<String>, ValidationError> {
    let code = raw.trim().to_uppercase();
    if code.is_empty() {
        return Ok(None);
    }
    if code.len() <= 10
        && code
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
    {
        Ok(Some(code))
    } else {
        Err(ValidationError::new(TEAM_CODE_INVALID))
    }
}

/// Kode tiket dari template yang sudah valid. Template tidak valid (tidak pernah tersimpan)
/// menghasilkan bentuk bawaan.
#[must_use]
pub fn render(template: &str, values: &CodeParts<'_>) -> String {
    let parts = parse(template).unwrap_or_else(|_| parse(TEMPLATE_DEFAULT).unwrap_or_default());
    let mut out = String::new();
    for part in parts {
        match part {
            Part::Literal(s) => out.push_str(&s),
            Part::Project => out.push_str(values.project),
            Part::Team => out.push_str(values.team),
            Part::Type => out.push_str(&values.type_key.to_uppercase()),
            Part::Year4 => out.push_str(&format!("{:04}", values.year)),
            Part::Year2 => out.push_str(&format!("{:02}", values.year.rem_euclid(100))),
            Part::Num(0) => out.push_str(&values.number.to_string()),
            Part::Num(n) => out.push_str(&format!("{:0n$}", values.number)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts(number: i32) -> CodeParts<'static> {
        CodeParts {
            project: "KSR",
            team: "BE",
            type_key: "story",
            year: 2026,
            number,
        }
    }

    #[test]
    fn renders_spec_examples() {
        assert_eq!(render("{PROJECT}-{NUM}", &parts(123)), "KSR-123");
        assert_eq!(render("{TEAM}-{YYYY}-{NUM:4}", &parts(42)), "BE-2026-0042");
        assert_eq!(render("{PROJECT}-{TYPE}-{NUM}", &parts(7)), "KSR-STORY-7");
        assert_eq!(render("{PROJECT}{YY}.{NUM:2}", &parts(5)), "KSR26.05");
    }

    #[test]
    fn rejects_bad_templates() {
        for bad in [
            "",
            "{PROJECT}",
            "{PROJECT}/{NUM}",
            "{FOO}-{NUM}",
            "{NUM:0}",
            "{NUM",
            "a b{NUM}",
        ] {
            assert!(validate(bad, CounterScope::Project).is_err(), "{bad}");
        }
        assert!(validate(&"A".repeat(41), CounterScope::Project).is_err());
        assert!(validate("{PROJECT}-{NUM}", CounterScope::Project).is_ok());
    }

    #[test]
    fn scope_requires_distinguishing_token() {
        assert!(validate("{PROJECT}-{NUM}", CounterScope::Type).is_err());
        assert!(validate("{PROJECT}-{TYPE}-{NUM}", CounterScope::Type).is_ok());
        assert!(validate("{PROJECT}-{NUM}", CounterScope::Year).is_err());
        assert!(validate("{PROJECT}-{YY}-{NUM}", CounterScope::Year).is_ok());
    }

    #[test]
    fn team_code_is_optional_upper_alnum() {
        assert_eq!(normalize_team_code(" be ").unwrap(), Some("BE".into()));
        assert_eq!(normalize_team_code("").unwrap(), None);
        assert!(normalize_team_code("B-E").is_err());
    }
}
