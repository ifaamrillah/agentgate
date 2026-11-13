//! Pemetaan otomatis header ekspor Jira/Trello/Linear dan pembacaan nilai sel (prioritas,
//! tanggal, story point, label, kategori status). Murni, dites unit.
use agentgate_core::workflow::Category;
use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};

use crate::models::{ticket_imports::ImportField, tickets::TicketPriority};

/// Nilai sel yang tidak bisa dibaca (pemanggil mencatat peringatan lalu mengosongkannya).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BadValue;

/// Header umum (huruf kecil, tanpa spasi berlebih) → field.
const HEADERS: &[(&str, ImportField)] = &[
    ("issue key", ImportField::Alias),
    ("key", ImportField::Alias),
    ("id", ImportField::Alias),
    ("card id", ImportField::Alias),
    ("kode", ImportField::Alias),
    ("kode lama", ImportField::Alias),
    ("summary", ImportField::Title),
    ("title", ImportField::Title),
    ("card name", ImportField::Title),
    ("name", ImportField::Title),
    ("judul", ImportField::Title),
    ("description", ImportField::Description),
    ("card description", ImportField::Description),
    ("deskripsi", ImportField::Description),
    ("issue type", ImportField::Type),
    ("type", ImportField::Type),
    ("tipe", ImportField::Type),
    ("status", ImportField::Stage),
    ("list name", ImportField::Stage),
    ("state", ImportField::Stage),
    ("stage", ImportField::Stage),
    ("assignee", ImportField::Assignee),
    ("assignee email", ImportField::Assignee),
    ("members", ImportField::Assignee),
    ("penerima", ImportField::Assignee),
    ("priority", ImportField::Priority),
    ("prioritas", ImportField::Priority),
    ("story points", ImportField::StoryPoints),
    ("story point estimate", ImportField::StoryPoints),
    ("custom field (story points)", ImportField::StoryPoints),
    ("estimate", ImportField::StoryPoints),
    ("story point", ImportField::StoryPoints),
    ("labels", ImportField::Labels),
    ("label", ImportField::Labels),
    ("sprint", ImportField::Sprint),
    ("cycle name", ImportField::Sprint),
    ("due date", ImportField::DueOn),
    ("due", ImportField::DueOn),
    ("due_date", ImportField::DueOn),
    ("created", ImportField::CreatedAt),
    ("created at", ImportField::CreatedAt),
    ("created date", ImportField::CreatedAt),
    ("tanggal dibuat", ImportField::CreatedAt),
];

/// Pemetaan otomatis: header dikenal → field; field tunggal hanya untuk kolom pertama yang
/// cocok; sisanya Lewati.
#[must_use]
pub fn auto_map(headers: &[String]) -> Vec<ImportField> {
    let mut used: Vec<ImportField> = Vec::new();
    headers
        .iter()
        .map(|h| {
            let norm = h
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .to_lowercase();
            let field = HEADERS
                .iter()
                .find(|(k, _)| *k == norm)
                .map_or(ImportField::Skip, |(_, f)| *f);
            if field.single() && used.contains(&field) {
                return ImportField::Skip;
            }
            used.push(field);
            field
        })
        .collect()
}

/// Prioritas dari nilai Jira (Highest..Lowest), Linear (Urgent..No priority), atau P1–P4.
/// Kosong → `Some(Medium)`; tak dikenal → `None`.
#[must_use]
pub fn priority(raw: &str) -> Option<TicketPriority> {
    match raw.trim().to_lowercase().as_str() {
        "" | "medium" | "normal" | "p3" | "sedang" | "no priority" | "none" => {
            Some(TicketPriority::Medium)
        }
        "highest" | "urgent" | "critical" | "blocker" | "p1" | "mendesak" => {
            Some(TicketPriority::Urgent)
        }
        "high" | "major" | "p2" | "tinggi" => Some(TicketPriority::High),
        "low" | "lowest" | "minor" | "trivial" | "p4" | "rendah" => Some(TicketPriority::Low),
        _ => None,
    }
}

/// Kategori stage dari nama status umum; `None` = tidak dikenal.
#[must_use]
pub fn status_category(raw: &str) -> Option<Category> {
    match raw.trim().to_lowercase().as_str() {
        "backlog" | "icebox" | "triage" => Some(Category::Backlog),
        "to do"
        | "todo"
        | "open"
        | "new"
        | "selected for development"
        | "ready"
        | "unstarted"
        | "reopened" => Some(Category::Todo),
        "in progress" | "doing" | "started" | "in review" | "review" | "code review"
        | "in development" | "testing" | "qa" => Some(Category::InProgress),
        "done" | "closed" | "resolved" | "completed" | "complete" | "selesai" | "canceled"
        | "cancelled" => Some(Category::Done),
        _ => None,
    }
}

/// Story point: bilangan bulat atau desimal (dibulatkan) 0–1000. Kosong → `Ok(None)`.
///
/// # Errors
///
/// [`BadValue`] bila bukan angka dalam rentang.
pub fn story_points(raw: &str) -> Result<Option<i32>, BadValue> {
    let raw = raw.trim().replace(',', ".");
    if raw.is_empty() {
        return Ok(None);
    }
    let n: f64 = raw.parse().map_err(|_| BadValue)?;
    if !(0.0..=f64::from(crate::models::tickets::STORY_POINTS_MAX)).contains(&n) {
        return Err(BadValue);
    }
    // Rentang sudah dicek; pembulatan aman.
    #[allow(clippy::cast_possible_truncation)]
    Ok(Some(n.round() as i32))
}

/// Label dipisah koma atau titik koma, di-trim dan unik (tanpa membedakan huruf besar).
#[must_use]
pub fn labels(values: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for v in values {
        for l in v.split([',', ';']).map(str::trim).filter(|l| !l.is_empty()) {
            if !out.iter().any(|o| o.eq_ignore_ascii_case(l)) {
                out.push(l.to_string());
            }
        }
    }
    out
}

/// Waktu dari format umum ekspor (ISO 8601, `YYYY-MM-DD HH:MM`, Jira `02/Oct/26 10:15 AM`,
/// `DD/MM/YYYY`, `DD.MM.YYYY`). Tanpa zona = UTC. Kosong → `Ok(None)`.
///
/// # Errors
///
/// [`BadValue`] bila tidak dikenali.
pub fn datetime(raw: &str) -> Result<Option<DateTime<Utc>>, BadValue> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(None);
    }
    if let Ok(t) = DateTime::parse_from_rfc3339(raw) {
        return Ok(Some(t.with_timezone(&Utc)));
    }
    for f in [
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M",
        "%d/%b/%y %I:%M %p",
        "%d/%b/%Y %I:%M %p",
        "%d/%m/%Y %H:%M",
        "%m/%d/%Y %I:%M %p",
    ] {
        if let Ok(t) = NaiveDateTime::parse_from_str(raw, f) {
            return Ok(Some(Utc.from_utc_datetime(&t)));
        }
    }
    date(raw).map(|d| {
        d.and_then(|d| d.and_hms_opt(0, 0, 0))
            .map(|t| Utc.from_utc_datetime(&t))
    })
}

/// Tanggal (format seperti [`datetime`], bagian jam diabaikan). Kosong → `Ok(None)`.
///
/// # Errors
///
/// [`BadValue`] bila tidak dikenali.
pub fn date(raw: &str) -> Result<Option<NaiveDate>, BadValue> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(None);
    }
    for f in [
        "%Y-%m-%d", "%d/%b/%y", "%d/%b/%Y", "%d/%m/%Y", "%d.%m.%Y", "%Y/%m/%d",
    ] {
        if let Ok(d) = NaiveDate::parse_from_str(raw, f) {
            return Ok(Some(d));
        }
    }
    // Waktu lengkap → ambil tanggalnya.
    let head = raw.split([' ', 'T']).next().unwrap_or(raw);
    if head != raw {
        return date(head);
    }
    Err(BadValue)
}

/// Kode lama yang layak jadi alias: 1–40 karakter tanpa spasi/kontrol.
#[must_use]
pub fn valid_alias(code: &str) -> bool {
    let n = code.chars().count();
    (1..=40).contains(&n) && !code.chars().any(|c| c.is_whitespace() || c.is_control())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn auto_map_jira_trello_linear_headers() {
        use ImportField as F;
        assert_eq!(
            auto_map(&h(&[
                "Issue key",
                "Summary",
                "Issue Type",
                "Status",
                "Assignee",
                "Priority",
                "Story Points",
                "Labels",
                "Labels",
                "Sprint",
                "Due date",
                "Created",
                "Toko"
            ])),
            [
                F::Alias,
                F::Title,
                F::Type,
                F::Stage,
                F::Assignee,
                F::Priority,
                F::StoryPoints,
                F::Labels,
                F::Labels,
                F::Sprint,
                F::DueOn,
                F::CreatedAt,
                F::Skip
            ]
        );
        assert_eq!(
            auto_map(&h(&["Card Name", "List Name", "Card Description"])),
            [F::Title, F::Stage, F::Description]
        );
        assert_eq!(auto_map(&h(&["Title", "Title"])), [F::Title, F::Skip]);
    }

    #[test]
    fn reads_cell_values() {
        assert_eq!(priority("Highest"), Some(TicketPriority::Urgent));
        assert_eq!(priority("P4"), Some(TicketPriority::Low));
        assert_eq!(priority(""), Some(TicketPriority::Medium));
        assert_eq!(priority("whatever"), None);
        assert_eq!(story_points("8.0"), Ok(Some(8)));
        assert_eq!(story_points("2,5"), Ok(Some(3)));
        assert!(story_points("banyak").is_err());
        assert_eq!(labels(&["a, b", "A;c"]), ["a", "b", "c"]);
        assert_eq!(status_category("In Progress"), Some(Category::InProgress));
        assert_eq!(status_category("Analysis"), None);
        assert_eq!(
            date("02/Oct/26 10:15 AM"),
            Ok(NaiveDate::from_ymd_opt(2026, 10, 2))
        );
        assert_eq!(date("2026-10-02"), Ok(NaiveDate::from_ymd_opt(2026, 10, 2)));
        assert!(date("kemarin").is_err());
        assert!(datetime("2026-10-02T08:00:00+07:00").unwrap().is_some());
        assert!(datetime("02/Oct/26 10:15 AM").unwrap().is_some());
        assert!(valid_alias("SR-311"));
        assert!(!valid_alias("SR 311"));
    }
}
