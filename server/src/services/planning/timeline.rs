//! Tanggal roadmap dan kalender (fungsi murni).
//!
//! Rentang tiket di roadmap:
//! - selesai: mulai = awal sprint (bila setelah tiket dibuat) atau tanggal dibuat, selesai =
//!   tanggal `done_at`;
//! - sedang dikerjakan: mulai sama, selesai = perkiraan dari sisa jam estimasi (× kalibrasi),
//!   lalu tenggat, lalu akhir sprint, lalu hari ini;
//! - belum mulai (bar putus-putus): mulai = hari ini atau awal sprint, selesai = perkiraan dari
//!   estimasi, lalu tenggat, lalu akhir sprint; tanpa ketiganya tiket tidak punya rentang.
//!
//! "Terlambat" = perkiraan selesai melewati tenggat dan tiket belum selesai.
use chrono::{Datelike, Duration, NaiveDate};

use super::{estimates, wib_date};
use crate::models::{sprints, tickets};

/// Rentang tiket di roadmap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: NaiveDate,
    pub end: NaiveDate,
    /// Belum mulai: tanggal dari estimasi/rencana.
    pub estimated: bool,
    pub late: bool,
}

/// Rentang tiket; `None` bila belum mulai dan tidak punya estimasi, tenggat, maupun sprint.
#[must_use]
pub fn span(
    t: &tickets::Model,
    sprint: Option<&sprints::Model>,
    remaining_hours: Option<f64>,
    today: NaiveDate,
) -> Option<Span> {
    let created = wib_date(&t.created_at);
    let started = t.status != "backlog" && t.status != "todo";
    let hours = remaining_hours.filter(|h| *h > 0.0);
    let (start, end, estimated) = if t.status == "done" {
        let start = sprint.map_or(created, |s| s.starts_on.max(created));
        let end = t
            .done_at
            .as_ref()
            .map_or_else(|| wib_date(&t.updated_at), wib_date);
        (start, end, false)
    } else if started {
        let start = sprint
            .map_or(created, |s| s.starts_on.max(created))
            .min(today);
        let end = hours
            .map(|h| estimates::finish_date(today, h))
            .or(t.due_on)
            .or(sprint.map(|s| s.ends_on))
            .unwrap_or(today);
        (start, end, false)
    } else {
        let start = sprint.map_or(today, |s| s.starts_on.max(today));
        let end = hours
            .map(|h| estimates::finish_date(start, h))
            .or(t.due_on)
            .or(sprint.map(|s| s.ends_on))?;
        (start, end, true)
    };
    let end = end.max(start);
    Some(Span {
        start,
        end,
        estimated,
        late: t.status != "done" && t.due_on.is_some_and(|d| end > d),
    })
}

/// Perkiraan selesai tiket yang belum selesai dari sisa jam estimasi.
#[must_use]
pub fn forecast(
    t: &tickets::Model,
    remaining_hours: Option<f64>,
    today: NaiveDate,
) -> Option<NaiveDate> {
    if t.status == "done" {
        return None;
    }
    remaining_hours
        .filter(|h| *h > 0.0)
        .map(|h| estimates::finish_date(today, h))
}

/// Tanggal pertama bulan `d`.
#[must_use]
pub fn month_start(d: NaiveDate) -> NaiveDate {
    d.with_day(1).unwrap_or(d)
}

/// Tanggal pertama bulan berikutnya (`delta` bulan; negatif = mundur).
#[must_use]
pub fn add_months(d: NaiveDate, delta: i32) -> NaiveDate {
    let m0 = d.year() * 12 + d.month0().cast_signed() + delta;
    NaiveDate::from_ymd_opt(m0.div_euclid(12), m0.rem_euclid(12).cast_unsigned() + 1, 1)
        .unwrap_or(d)
}

/// Grid kalender bulan: 6 minggu Senin–Minggu yang memuat tanggal 1.
#[must_use]
pub fn month_grid(month: NaiveDate) -> Vec<[NaiveDate; 7]> {
    let first = month_start(month);
    let monday = first - Duration::days(i64::from(first.weekday().num_days_from_monday()));
    (0..6)
        .map(|w| std::array::from_fn(|i| monday + Duration::days(w * 7 + i as i64)))
        .collect()
}

/// Bulan dari `YYYY-MM` (tidak valid → `None`).
#[must_use]
pub fn parse_month(raw: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(&format!("{}-01", raw.trim()), "%Y-%m-%d").ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn ticket(status: &str, created: &str) -> tickets::Model {
        let at: chrono::DateTime<chrono::FixedOffset> =
            chrono::DateTime::parse_from_rfc3339(&format!("{created}T03:00:00+00:00")).unwrap();
        tickets::Model {
            id: uuid::Uuid::nil(),
            workspace_id: uuid::Uuid::nil(),
            project_id: uuid::Uuid::nil(),
            type_id: uuid::Uuid::nil(),
            number: 1,
            code: "KSR-1".into(),
            title: String::new(),
            description: String::new(),
            status: status.into(),
            priority: "medium".into(),
            assignee_id: None,
            reporter_id: None,
            parent_id: None,
            labels: vec![],
            due_on: None,
            story_points: None,
            custom_values: serde_json::json!({}),
            position: 0,
            done_at: None,
            deleted_at: None,
            deleted_by: None,
            created_by: None,
            created_at: at,
            updated_at: at,
            workflow_id: uuid::Uuid::nil(),
            stage_key: "backlog".into(),
            stage_version: 1,
            stage_state: "active".into(),
            fail_count: 0,
            fail_stage_key: None,
            stage_entered_at: at,
            cancelled_at: None,
            sprint_id: None,
            release_id: None,
            component_id: None,
            backlog_rank: 0,
        }
    }

    #[test]
    fn not_started_without_dates_has_no_span() {
        let t = ticket("todo", "2026-11-02");
        assert_eq!(span(&t, None, None, d("2026-11-12")), None);
    }

    #[test]
    fn not_started_uses_estimate_and_flags_late() {
        let mut t = ticket("todo", "2026-11-02");
        t.due_on = Some(d("2026-11-13"));
        // Kamis 12 Nov + 24 jam = 3 hari kerja → Senin 16 Nov.
        let s = span(&t, None, Some(24.0), d("2026-11-12")).unwrap();
        assert_eq!((s.start, s.end), (d("2026-11-12"), d("2026-11-16")));
        assert!(s.estimated && s.late);
    }

    #[test]
    fn in_progress_starts_at_creation_and_ends_at_due() {
        let mut t = ticket("in_progress", "2026-11-04");
        t.due_on = Some(d("2026-11-20"));
        let s = span(&t, None, None, d("2026-11-12")).unwrap();
        assert_eq!(
            (s.start, s.end, s.estimated, s.late),
            (d("2026-11-04"), d("2026-11-20"), false, false)
        );
    }

    #[test]
    fn calendar_grid_starts_on_monday() {
        let g = month_grid(d("2026-11-15"));
        assert_eq!(g.len(), 6);
        assert_eq!(g[0][0], d("2026-10-26"));
        assert_eq!(g[5][6], d("2026-12-06"));
        assert_eq!(add_months(d("2026-12-01"), 1), d("2027-01-01"));
        assert_eq!(add_months(d("2026-01-01"), -1), d("2025-12-01"));
        assert_eq!(parse_month("2026-02"), Some(d("2026-02-01")));
        assert_eq!(parse_month("x"), None);
    }
}
