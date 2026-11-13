//! Data laporan: burndown (replay log scope per hari kerja), burnup dan velocity (laporan beku
//! sprint selesai), cumulative flow (kunjungan stage dari riwayat), cycle time per stage (median
//! jam AI/manusia tiket selesai). Semua dari pergerakan tiket dan durasi job, tanpa time tracking.
use std::collections::{BTreeMap, HashMap};

use chrono::{NaiveDate, Utc};
use sea_orm::{ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use super::{
    estimates,
    scope::{self, ScopeEvent},
};
use crate::models::{
    sprint_scope_events,
    sprints::{self, SprintState},
    ticket_events, tickets,
    workspaces::WorkspaceId,
};

/// Satu titik burndown.
#[derive(Debug, Clone, PartialEq)]
pub struct BurnDay {
    pub day: NaiveDate,
    pub ideal: f64,
    /// Sisa poin (None = hari yang belum terjadi).
    pub remaining: Option<i32>,
    pub scope: Option<i32>,
    /// Proyeksi sisa poin (dari hari ini ke akhir sprint).
    pub projected: Option<f64>,
    /// Poin ditambahkan hari itu (setelah sprint mulai).
    pub added: i32,
}

/// Burndown sprint.
#[derive(Debug, Clone, PartialEq)]
pub struct Burndown {
    pub days: Vec<BurnDay>,
    /// Indeks hari ini (sprint aktif dalam rentang).
    pub today: Option<usize>,
    pub committed: i32,
    pub remaining: i32,
    pub added: i32,
}

/// Hitung burndown dari log scope. `now` = batas replay (selesai sprint atau sekarang).
#[must_use]
pub fn burndown(sprint: &sprints::Model, events: &[ScopeEvent], today: NaiveDate) -> Burndown {
    let days = super::workdays(sprint.starts_on, sprint.ends_on);
    let last = days.len().saturating_sub(1).max(1);
    let start_at = sprint
        .started_at
        .map_or_else(|| scope::end_of_day(sprint.starts_on), |s| s.with_timezone(&Utc));
    let committed = if sprint.state() == SprintState::Planned {
        scope::snapshot(&scope::state_at(events, Utc::now())).scope
    } else {
        sprint.committed_points
    };
    let until = match sprint.state() {
        SprintState::Completed => sprint
            .completed_at
            .map_or(Utc::now(), |c| c.with_timezone(&Utc)),
        _ => Utc::now(),
    };
    let end_day = (until + chrono::Duration::hours(7)).date_naive();
    let mut out = Vec::new();
    let mut prev_inside: HashMap<Uuid, bool> = scope::state_at(events, start_at)
        .into_iter()
        .map(|(k, v)| (k, v.inside))
        .collect();
    let mut today_idx = None;
    for (i, day) in days.iter().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let ideal = f64::from(committed) * (1.0 - i as f64 / last as f64);
        let happened = *day <= end_day && sprint.state() != SprintState::Planned;
        let (remaining, scope_pts, added) = if happened {
            // Sprint yang dimulai setelah tanggal mulainya: hari sebelum mulai memakai keadaan
            // saat mulai (komitmen), bukan scope kosong yang lalu terhitung "ditambah".
            let at = scope::end_of_day(*day).max(start_at).min(until);
            let states = scope::state_at(events, at);
            let snap = scope::snapshot(&states);
            let mut added = 0;
            for (id, s) in &states {
                if s.inside && !prev_inside.get(id).copied().unwrap_or(false) {
                    added += s.points;
                }
            }
            prev_inside = states.iter().map(|(k, v)| (*k, v.inside)).collect();
            (Some(snap.remaining()), Some(snap.scope), added)
        } else {
            (None, None, 0)
        };
        if *day == today && sprint.state() == SprintState::Active {
            today_idx = Some(i);
        }
        out.push(BurnDay {
            day: *day,
            ideal,
            remaining,
            scope: scope_pts,
            projected: None,
            added,
        });
    }
    // Proyeksi linear dari laju selesai sejauh ini.
    if sprint.state() == SprintState::Active {
        if let Some(cur) = out.iter().rposition(|d| d.remaining.is_some()) {
            let now_rem = f64::from(out[cur].remaining.unwrap_or(0));
            let burned = f64::from(out[0].scope.unwrap_or(committed)) - now_rem;
            #[allow(clippy::cast_precision_loss)]
            let rate = if cur == 0 { 0.0 } else { burned.max(0.0) / cur as f64 };
            for (j, d) in out.iter_mut().enumerate().skip(cur) {
                #[allow(clippy::cast_precision_loss)]
                let p = (now_rem - rate * (j - cur) as f64).max(0.0);
                d.projected = Some((p * 10.0).round() / 10.0);
            }
        }
    }
    let remaining = out
        .iter()
        .rev()
        .find_map(|d| d.remaining)
        .unwrap_or(committed);
    Burndown {
        added: out.iter().map(|d| d.added).sum(),
        days: out,
        today: today_idx,
        committed,
        remaining,
    }
}

/// Muat log scope sprint.
///
/// # Errors
///
/// Error database.
pub async fn scope_events<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    sprint_id: Uuid,
) -> Result<Vec<ScopeEvent>, DbErr> {
    Ok(sprint_scope_events::Model::of_sprint(db, wid, sprint_id)
        .await?
        .iter()
        .map(ScopeEvent::of)
        .collect())
}

/// Satu kolom burnup (per sprint).
#[derive(Debug, Clone, PartialEq)]
pub struct BurnupPoint {
    pub name: String,
    pub scope: i32,
    pub done: Option<i32>,
    pub projected: Option<f64>,
}

/// Burnup project: scope (poin tiket yang sudah ada di akhir sprint) dan poin selesai kumulatif
/// per sprint, ditambah proyeksi velocity sampai scope tercapai (maks. 6 sprint).
#[must_use]
pub fn burnup(
    sprints: &[sprints::Model],
    tickets: &[tickets::Model],
    avg_velocity: Option<f64>,
) -> Vec<BurnupPoint> {
    let mut out = Vec::new();
    let mut done = 0;
    let shown: Vec<&sprints::Model> = sprints
        .iter()
        .filter(|s| s.state() != SprintState::Planned)
        .collect();
    for s in &shown {
        let end = scope::end_of_day(s.ends_on);
        let scope_pts: i32 = tickets
            .iter()
            .filter(|t| t.created_at.with_timezone(&Utc) <= end || s.state() == SprintState::Active)
            .filter_map(|t| t.story_points)
            .sum();
        let finished = if s.state() == SprintState::Completed {
            s.completed_points
        } else {
            tickets
                .iter()
                .filter(|t| t.sprint_id == Some(s.id) && t.status == "done")
                .filter_map(|t| t.story_points)
                .sum()
        };
        done += finished;
        out.push(BurnupPoint {
            name: s.name.clone(),
            scope: scope_pts,
            done: Some(done),
            projected: None,
        });
    }
    let total: i32 = tickets.iter().filter_map(|t| t.story_points).sum();
    if let (Some(v), Some(last)) = (avg_velocity.filter(|v| *v > 0.0), out.last_mut()) {
        last.projected = last.done.map(f64::from);
        let mut acc = f64::from(last.done.unwrap_or(0));
        let mut n = shown.len();
        while acc < f64::from(total) && n < shown.len() + 6 {
            n += 1;
            acc = (acc + v).min(f64::from(total));
            out.push(BurnupPoint {
                name: format!("+{}", n - shown.len()),
                scope: total,
                done: None,
                projected: Some((acc * 10.0).round() / 10.0),
            });
        }
    }
    out
}

/// Cumulative flow: jumlah tiket per stage di akhir tiap hari `days`.
///
/// # Errors
///
/// Error database.
pub async fn cumulative_flow<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    tickets: &[tickets::Model],
    days: &[NaiveDate],
) -> Result<BTreeMap<String, Vec<f64>>, DbErr> {
    let ids: Vec<Uuid> = tickets.iter().map(|t| t.id).collect();
    let mut out: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    if ids.is_empty() {
        return Ok(out);
    }
    let events = ticket_events::Entity::find()
        .filter(ticket_events::Column::WorkspaceId.eq(wid.get()))
        .filter(ticket_events::Column::TicketId.is_in(ids))
        .order_by_asc(ticket_events::Column::CreatedAt)
        .order_by_asc(ticket_events::Column::Id)
        .all(db)
        .await?;
    let now = Utc::now();
    for t in tickets {
        let ev: Vec<ticket_events::Model> =
            events.iter().filter(|e| e.ticket_id == t.id).cloned().collect();
        let mut visits = estimates::visits(t, &ev, now);
        // Tiket selesai tetap terhitung di stage terakhirnya setelah selesai.
        if let (Some(done), Some(last)) = (t.done_at, visits.last_mut()) {
            last.2 = last.2.max(done.with_timezone(&Utc)).max(now);
        }
        for (i, day) in days.iter().enumerate() {
            let at = scope::end_of_day(*day).min(now);
            if let Some((key, ..)) = visits.iter().find(|(_, from, to)| *from <= at && at <= *to) {
                let col = out
                    .entry(key.clone())
                    .or_insert_with(|| vec![0.0; days.len()]);
                col[i] += 1.0;
            }
        }
    }
    Ok(out)
}

/// Median jam (AI, manusia) per stage dari tiket selesai.
#[must_use]
pub fn cycle_time(actuals: &HashMap<Uuid, estimates::StageActual>) -> BTreeMap<String, (f64, f64)> {
    let mut per: BTreeMap<String, (Vec<f64>, Vec<f64>)> = BTreeMap::new();
    for a in actuals.values() {
        for (stage, (ai, human)) in a {
            let e = per.entry(stage.clone()).or_default();
            e.0.push(*ai);
            e.1.push(*human);
        }
    }
    per.into_iter()
        .map(|(k, (a, h))| (k, (estimates::median(a), estimates::median(h))))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::sprint_scope_events::ScopeKind;

    fn sprint(state: &str) -> sprints::Model {
        let d = |m, day| NaiveDate::from_ymd_opt(2026, m, day).unwrap();
        sprints::Model {
            id: Uuid::nil(),
            workspace_id: Uuid::nil(),
            project_id: Uuid::nil(),
            name: "Sprint 1".into(),
            goal: String::new(),
            starts_on: d(11, 2),
            ends_on: d(11, 13),
            state: state.into(),
            capacity: None,
            started_at: Some(
                chrono::DateTime::parse_from_rfc3339("2026-11-02T02:00:00Z").unwrap(),
            ),
            completed_at: Some(
                chrono::DateTime::parse_from_rfc3339("2026-11-13T09:00:00Z").unwrap(),
            ),
            committed_points: 8,
            completed_points: 5,
            report: serde_json::json!({}),
            created_by: None,
            created_at: chrono::DateTime::parse_from_rfc3339("2026-11-01T00:00:00Z").unwrap(),
            updated_at: chrono::DateTime::parse_from_rfc3339("2026-11-01T00:00:00Z").unwrap(),
        }
    }

    fn ev(ticket: u128, kind: ScopeKind, points: i32, at: &str) -> ScopeEvent {
        ScopeEvent {
            ticket: Uuid::from_u128(ticket),
            kind,
            points,
            at: chrono::DateTime::parse_from_rfc3339(at).unwrap().with_timezone(&Utc),
        }
    }

    #[test]
    fn burndown_replays_scope_per_workday() {
        let events = vec![
            ev(1, ScopeKind::Added, 5, "2026-11-01T03:00:00Z"),
            ev(2, ScopeKind::Added, 3, "2026-11-01T03:00:00Z"),
            ev(3, ScopeKind::Added, 2, "2026-11-04T03:00:00Z"),
            ev(1, ScopeKind::Done, 5, "2026-11-05T03:00:00Z"),
        ];
        let b = burndown(&sprint("completed"), &events, NaiveDate::from_ymd_opt(2026, 11, 20).unwrap());
        assert_eq!(b.days.len(), 10);
        assert_eq!(b.days[0].remaining, Some(8));
        assert_eq!(b.days[2].remaining, Some(10));
        assert_eq!(b.days[2].added, 2);
        assert_eq!(b.days[3].remaining, Some(5));
        assert_eq!(b.days[9].remaining, Some(5));
        assert!((b.days[0].ideal - 8.0).abs() < 1e-9);
        assert!(b.days[9].ideal.abs() < 1e-9);
        assert_eq!(b.added, 2);
        assert_eq!(b.today, None);
    }

    #[test]
    fn late_start_counts_planned_tickets_as_commitment() {
        let mut s = sprint("completed");
        // Tanggal mulai 2 Nov, tetapi baru dimulai 4 Nov; tiket direncanakan 4 Nov pagi.
        s.started_at = Some(chrono::DateTime::parse_from_rfc3339("2026-11-04T05:00:00Z").unwrap());
        let events = vec![
            ev(1, ScopeKind::Added, 5, "2026-11-04T03:00:00Z"),
            ev(2, ScopeKind::Added, 3, "2026-11-04T04:00:00Z"),
        ];
        let b = burndown(&s, &events, NaiveDate::from_ymd_opt(2026, 11, 20).unwrap());
        assert_eq!(b.days[0].remaining, Some(8));
        assert_eq!(b.added, 0);
    }
}
