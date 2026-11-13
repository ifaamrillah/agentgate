//! Data view planning (M9): backlog & sprint, laporan, roadmap, kalender, rilis.
use std::collections::HashMap;

use chrono::NaiveDate;
use serde::Serialize;
use uuid::Uuid;

use crate::{
    models::{
        sprints::{self, SprintState},
        tickets,
    },
    services::planning::{self, estimates::Latest},
    views::{
        tickets::{Lookup, TicketRow},
        ui::Stat,
    },
};

pub const FLASH_CREATED: &str = "sprint-flash-created";
pub const FLASH_SAVED: &str = "sprint-flash-saved";
pub const FLASH_DELETED: &str = "sprint-flash-deleted";
pub const FLASH_STARTED: &str = "sprint-flash-started";
pub const FLASH_COMPLETED: &str = "sprint-flash-completed";
pub const FLASH_MOVED: &str = "sprint-flash-moved";
pub const FLASH_KEYS: &[&str] = &[
    FLASH_CREATED,
    FLASH_SAVED,
    FLASH_DELETED,
    FLASH_STARTED,
    FLASH_COMPLETED,
    FLASH_MOVED,
];
/// Kunci Fluent statistik backlog (dites kontrak i18n).
pub const STAT_KEYS: &[&str] = &[
    "backlog-stat-active",
    "backlog-stat-active-hint",
    "backlog-stat-no-active",
    "backlog-stat-velocity",
    "backlog-stat-velocity-hint",
    "backlog-stat-velocity-none",
    "backlog-stat-accuracy",
    "backlog-stat-accuracy-hint",
    "backlog-stat-forecast",
    "backlog-stat-forecast-hint",
    "backlog-stat-forecast-late",
];

/// Kunci Fluent yang dibangun di kode (dites kontrak i18n).
#[must_use]
pub fn label_keys() -> Vec<String> {
    FLASH_KEYS
        .iter()
        .chain(STAT_KEYS)
        .chain(&[
            "sprint-complete-done",
            "sprint-complete-left",
            "sprint-points-hint",
            "estimate-flash-saved",
            "estimate-flash-requested",
        ])
        .map(|k| (*k).to_string())
        .collect()
}

/// Nama bulan pendek (Indonesia).
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "Mei", "Jun", "Jul", "Agu", "Sep", "Okt", "Nov", "Des",
];

/// `18 Nov`.
#[must_use]
pub fn day_month(d: NaiveDate) -> String {
    use chrono::Datelike;
    format!("{} {}", d.day(), MONTHS[d.month0() as usize])
}

/// `18 Nov 2026`.
#[must_use]
pub fn long_day(d: NaiveDate) -> String {
    use chrono::Datelike;
    format!("{} {}", day_month(d), d.year())
}

/// Nama bulan pendek (`Nov`).
#[must_use]
pub fn month_name(d: NaiveDate) -> &'static str {
    use chrono::Datelike;
    MONTHS[d.month0() as usize]
}

/// Nama bulan panjang (`November`).
#[must_use]
pub fn month_long(d: NaiveDate) -> &'static str {
    use chrono::Datelike;
    const LONG: [&str; 12] = [
        "Januari", "Februari", "Maret", "April", "Mei", "Juni", "Juli", "Agustus", "September",
        "Oktober", "November", "Desember",
    ];
    LONG[d.month0() as usize]
}

/// `18 Nov – 1 Des`.
#[must_use]
pub fn range(from: NaiveDate, to: NaiveDate) -> String {
    format!("{} – {}", day_month(from), day_month(to))
}

/// Satu baris tiket di backlog/sprint.
#[derive(Debug, Serialize)]
pub struct BacklogRow {
    #[serde(flatten)]
    pub row: TicketRow,
    /// Nama stage sekarang.
    pub stage: String,
    /// Usulan poin AI terakhir (`None` = belum ada usulan).
    pub ai_points: Option<i32>,
    /// AI diusulkan tetapi poin disepakati berbeda.
    pub ai_changed: bool,
    /// Jam (AI, manusia) estimasi yang berlaku, untuk tooltip.
    pub hours: Option<Hours>,
    pub done: bool,
    /// POST pindah sprint/urutan.
    pub move_action: String,
}

/// Jam estimasi (AI, manusia) siap tampil.
#[derive(Debug, Serialize)]
pub struct Hours {
    pub ai: String,
    pub human: String,
}

/// Blok sprint (atau backlog bila `id` kosong).
#[derive(Debug, Serialize)]
pub struct SprintBlock {
    /// `""` = backlog.
    pub id: String,
    pub name: String,
    /// `planned` | `active` | `""` (backlog).
    pub state: &'static str,
    pub range: String,
    pub goal: String,
    pub starts_on: String,
    pub ends_on: String,
    pub capacity_input: String,
    pub rows: Vec<BacklogRow>,
    pub count: usize,
    pub points: i32,
    pub done_points: i32,
    /// Kapasitas (manual atau velocity); `None` = belum diketahui.
    pub capacity: Option<i32>,
    pub over: bool,
    /// Persen isi terhadap kapasitas (0–100).
    pub fill: u32,
    /// Persen poin selesai (sprint aktif).
    pub done_pct: u32,
    pub unestimated: usize,
    /// Hari kerja tersisa (sprint aktif).
    pub workdays_left: i64,
    pub days_left: i64,
    pub base: String,
    /// Angka dialog "Selesaikan sprint" (selesai, belum selesai).
    pub complete_stats: Vec<Stat>,
    /// Jumlah tiket belum selesai.
    pub left_count: usize,
    /// Sprint rencana lain sebagai tujuan tiket belum selesai.
    pub targets: Vec<SprintTarget>,
}

/// Sprint tujuan di dialog "Selesaikan sprint".
#[derive(Debug, Clone, Serialize)]
pub struct SprintTarget {
    pub id: String,
    pub name: String,
    pub range: String,
    pub points: i32,
}

/// Bangun baris backlog.
#[must_use]
pub fn rows(
    lookup: &Lookup,
    tickets: &[tickets::Model],
    stage_names: &HashMap<String, String>,
    estimates: &HashMap<Uuid, Latest>,
    move_base: &str,
) -> Vec<BacklogRow> {
    tickets
        .iter()
        .map(|t| {
            let est = estimates.get(&t.id);
            let ai_points = est.and_then(|e| e.ai.as_ref()).and_then(|a| a.points);
            BacklogRow {
                row: lookup.row(t),
                stage: stage_names
                    .get(&t.stage_key)
                    .cloned()
                    .unwrap_or_else(|| t.stage_key.clone()),
                ai_points,
                ai_changed: ai_points.is_some() && t.story_points.is_some() && ai_points != t.story_points,
                hours: est.and_then(Latest::hours).map(|(ai, human)| Hours {
                    ai: decimal(ai),
                    human: decimal(human),
                }),
                done: t.status == "done",
                move_action: format!("{move_base}/{}", t.id),
            }
        })
        .collect()
}

fn pct(part: i32, whole: i32) -> u32 {
    if whole <= 0 {
        return 0;
    }
    let p = (f64::from(part) / f64::from(whole) * 100.0).round().clamp(0.0, 100.0);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    {
        p as u32
    }
}

/// Blok sprint.
#[must_use]
pub fn sprint_block(
    sprint: &sprints::Model,
    rows: Vec<BacklogRow>,
    avg_velocity: Option<f64>,
    base: &str,
) -> SprintBlock {
    let points: i32 = rows.iter().filter_map(|r| r.row.points).sum();
    let done_points: i32 = rows.iter().filter(|r| r.done).filter_map(|r| r.row.points).sum();
    let capacity = planning::sprints::capacity(sprint, avg_velocity);
    let today = planning::today();
    let state = match sprint.state() {
        SprintState::Planned => "planned",
        SprintState::Active => "active",
        SprintState::Completed => "completed",
    };
    SprintBlock {
        id: sprint.id.to_string(),
        name: sprint.name.clone(),
        state,
        range: range(sprint.starts_on, sprint.ends_on),
        goal: sprint.goal.clone(),
        starts_on: sprint.starts_on.to_string(),
        ends_on: sprint.ends_on.to_string(),
        capacity_input: sprint.capacity.map(|c| c.to_string()).unwrap_or_default(),
        count: rows.len(),
        unestimated: rows.iter().filter(|r| r.row.points.is_none()).count(),
        points,
        done_points,
        over: capacity.is_some_and(|c| points > c),
        fill: capacity.map_or(0, |c| pct(points, c)),
        done_pct: pct(done_points, points),
        capacity,
        workdays_left: if today > sprint.ends_on {
            0
        } else {
            i64::try_from(planning::workdays(today.max(sprint.starts_on), sprint.ends_on).len()).unwrap_or(0)
        },
        days_left: (sprint.ends_on - today).num_days().max(0),
        rows,
        base: format!("{base}/{}", sprint.id),
        complete_stats: Vec::new(),
        left_count: 0,
        targets: Vec::new(),
    }
}

impl SprintBlock {
    /// Isi angka dialog selesai dan sprint tujuan dari blok lain.
    pub fn finish_setup(&mut self, all: &[SprintTarget]) {
        let left = self.rows.iter().filter(|r| !r.done).count();
        self.left_count = left;
        self.complete_stats = vec![
            Stat::new("sprint-complete-done", self.rows.len() - left, "circle-check")
                .tone("success")
                .hint("sprint-points-hint", self.done_points),
            Stat::new("sprint-complete-left", left, "clock")
                .tone("warn")
                .hint("sprint-points-hint", self.points - self.done_points),
        ];
        self.targets = all.iter().filter(|t| t.id != self.id).cloned().collect();
    }

    #[must_use]
    pub fn as_target(&self) -> SprintTarget {
        SprintTarget {
            id: self.id.clone(),
            name: self.name.clone(),
            range: self.range.clone(),
            points: self.points,
        }
    }
}

/// Blok backlog (tanpa sprint).
#[must_use]
pub fn backlog_block(rows: Vec<BacklogRow>) -> SprintBlock {
    SprintBlock {
        id: String::new(),
        name: String::new(),
        state: "",
        range: String::new(),
        goal: String::new(),
        starts_on: String::new(),
        ends_on: String::new(),
        capacity_input: String::new(),
        count: rows.len(),
        unestimated: rows.iter().filter(|r| r.row.points.is_none()).count(),
        points: rows.iter().filter_map(|r| r.row.points).sum(),
        done_points: 0,
        capacity: None,
        over: false,
        fill: 0,
        done_pct: 0,
        workdays_left: 0,
        days_left: 0,
        rows,
        base: String::new(),
        complete_stats: Vec::new(),
        left_count: 0,
        targets: Vec::new(),
    }
}

/// Angka satu desimal gaya Indonesia (`18,3`).
#[must_use]
pub fn decimal(v: f64) -> String {
    let s = format!("{v:.1}");
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    s.replace('.', ",")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_and_numbers_read_in_indonesian() {
        let d = NaiveDate::from_ymd_opt(2026, 11, 18).unwrap();
        assert_eq!(day_month(d), "18 Nov");
        assert_eq!(long_day(d), "18 Nov 2026");
        assert_eq!(
            range(d, NaiveDate::from_ymd_opt(2026, 12, 1).unwrap()),
            "18 Nov – 1 Des"
        );
        assert_eq!(decimal(18.333), "18,3");
        assert_eq!(decimal(18.0), "18");
        assert_eq!(pct(5, 10), 50);
        assert_eq!(pct(15, 10), 100);
        assert_eq!(pct(1, 0), 0);
    }
}
