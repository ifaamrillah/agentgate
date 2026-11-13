//! Project → Roadmap (Gantt) dan Kalender. Dirender penuh di server (tanpa JS kecuali gulir ke
//! "hari ini", `static/js/timeline.js`).
//!
//! - Roadmap: baris tiket per sprint (lalu "Tanpa sprint"), bar dari
//!   [`planning::timeline::span`], progres dari posisi stage di workflow, penanda hari ini dan
//!   target rilis, pita sprint di kepala, panah "blocks" (SVG). Zoom `?zoom=week|month|quarter`,
//!   cari `?q=`.
//! - Kalender: grid bulan `?month=YYYY-MM` dengan tenggat, perkiraan selesai AI, dan target
//!   rilis; filter jenis `?kind=due|forecast|release`; pita sprint; kartu "Tanpa tanggal".
//!
//! Butuh `ViewProject`.
use std::collections::HashMap;

use axum::http::StatusCode;
use chrono::{Datelike, Duration, NaiveDate};
use loco_rs::prelude::*;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::Serialize;
use uuid::Uuid;

use super::{board::project_definitions, lookup, project_paths, summary};
use crate::{
    controllers::{extractors::ProjectAccess, shell::app_shell},
    models::{
        permission::Permission,
        releases::{self, ReleaseStatus},
        sprints::{self, SprintState},
        ticket_links, tickets,
    },
    services::planning::{self, estimates, timeline},
    views::{self, nav, planning as pv, projects::PROJECT_VIEWS_ACTION, tickets::TicketRow},
};

/// Tinggi kepala, baris grup, dan baris tiket (px), sama dengan CSS `app-tl*`.
const HEAD_H: i64 = 88;
const GROUP_H: i64 = 32;
const ROW_H: i64 = 36;

/// Zoom: (kunci, kunci Fluent, px per hari, jumlah bulan jendela).
const ZOOMS: &[(&str, &str, i64, i32)] = &[
    ("week", "roadmap-zoom-week", 30, 3),
    ("month", "roadmap-zoom-month", 12, 4),
    ("quarter", "roadmap-zoom-quarter", 6, 6),
];

/// Jenis acara kalender: (kunci, kunci Fluent).
const KINDS: &[(&str, &str)] = &[
    ("due", "calendar-kind-due"),
    ("forecast", "calendar-kind-forecast"),
    ("release", "calendar-kind-release"),
];

/// Kunci Fluent yang dibangun di kode (dites kontrak i18n).
#[must_use]
pub fn label_keys() -> Vec<String> {
    ZOOMS
        .iter()
        .map(|z| z.1)
        .chain(KINDS.iter().map(|k| k.1))
        .chain(["roadmap-no-sprint"])
        .map(str::to_string)
        .collect()
}

/// Query halaman: `q`, `zoom`, `month`, `kind` (boleh berulang).
#[derive(Debug, Default)]
struct TimelineQuery {
    q: String,
    zoom: String,
    month: String,
    kind: Vec<String>,
}

impl TimelineQuery {
    fn parse(pairs: Vec<(String, String)>) -> Self {
        let mut out = Self::default();
        for (k, v) in pairs {
            match k.as_str() {
                "q" => out.q = v,
                "zoom" => out.zoom = v,
                "month" => out.month = v,
                "kind" => out.kind.push(v),
                _ => {}
            }
        }
        out
    }
}

fn matches(t: &tickets::Model, q: &str) -> bool {
    q.is_empty() || {
        let q = q.to_lowercase();
        t.code.to_lowercase().contains(&q) || t.title.to_lowercase().contains(&q)
    }
}

/// Tiket aktif project (bukan sampah), urut backlog.
async fn project_tickets(ctx: &AppContext, access: &ProjectAccess) -> Result<Vec<tickets::Model>> {
    Ok(tickets::Entity::find()
        .filter(tickets::Column::WorkspaceId.eq(access.tenant.workspace_id().get()))
        .filter(tickets::Column::ProjectId.eq(access.project.id))
        .filter(tickets::Column::DeletedAt.is_null())
        .filter(tickets::Column::CancelledAt.is_null())
        .order_by_asc(tickets::Column::BacklogRank)
        .order_by_asc(tickets::Column::Id)
        .all(&ctx.db)
        .await?)
}

/// Sisa jam estimasi (× kalibrasi) per tiket.
async fn remaining(
    ctx: &AppContext,
    access: &ProjectAccess,
    list: &[tickets::Model],
) -> Result<HashMap<Uuid, f64>> {
    let wid = access.tenant.workspace_id();
    let ids: Vec<Uuid> = list.iter().map(|t| t.id).collect();
    let latest = estimates::latest(&ctx.db, wid, &ids).await?;
    let calib = estimates::calibration(&ctx.db, wid, access.project.id).await?;
    Ok(list
        .iter()
        .filter_map(|t| {
            estimates::remaining_hours(
                t,
                latest.get(&t.id).and_then(|e| e.current.as_ref()),
                &calib,
            )
            .map(|h| (t.id, h))
        })
        .collect())
}

fn base_data(access: &ProjectAccess, view: &str, current: &str) -> serde_json::Value {
    let home = project_paths(access).home;
    serde_json::json!({
        "project": summary(access),
        "views": nav::project_views(&home, &access.tenant.user.project_views, view),
        "views_action": PROJECT_VIEWS_ACTION,
        "return_to": current,
        "base": current,
        "backlog": format!("{home}/backlog"),
    })
}

fn merge(mut a: serde_json::Value, b: serde_json::Value) -> serde_json::Value {
    if let (Some(a), serde_json::Value::Object(b)) = (a.as_object_mut(), b) {
        a.extend(b);
    }
    a
}

// ---------------------------------------------------------------------------------------------
// Roadmap

#[derive(Debug, Serialize)]
struct TlRow {
    /// Judul grup; `None` = baris tiket.
    group: Option<String>,
    ticket: Option<TicketRow>,
    left: i64,
    width: i64,
    progress: u32,
    estimated: bool,
    late: bool,
    epic: bool,
    tip: String,
}

#[derive(Debug, Serialize)]
struct Mark {
    label: String,
    left: i64,
    width: i64,
    href: String,
    old: bool,
    tip: String,
}

/// Posisi stage di workflow (0–100).
fn progress(t: &tickets::Model, stages: &[Vec<String>]) -> u32 {
    if t.status == "done" {
        return 100;
    }
    if t.status == "backlog" || t.status == "todo" {
        return 0;
    }
    stages
        .iter()
        .find_map(|keys| {
            keys.iter().position(|k| *k == t.stage_key).map(|i| {
                let n = keys.len().saturating_sub(1).max(1);
                u32::try_from(i * 100 / n).unwrap_or(0).min(95)
            })
        })
        .unwrap_or(10)
}

#[allow(clippy::too_many_lines)]
#[debug_handler]
pub(super) async fn roadmap(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: ProjectAccess,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Result<Response> {
    access.require(Permission::ViewProject)?;
    let query = TimelineQuery::parse(pairs);
    let db = &ctx.db;
    let wid = access.tenant.workspace_id();
    let today = planning::today();
    let (zoom, _, px, months) = ZOOMS
        .iter()
        .copied()
        .find(|z| z.0 == query.zoom)
        .unwrap_or(ZOOMS[0]);
    let start = timeline::add_months(timeline::month_start(today), -1);
    let end = timeline::add_months(start, months) - Duration::days(1);
    let days = (end - start).num_days() + 1;
    let x = |d: NaiveDate| (d - start).num_days().clamp(0, days) * px;

    let all = project_tickets(&ctx, &access).await?;
    let sprint_list = sprints::Model::of_project(db, wid, access.project.id).await?;
    let hours = remaining(&ctx, &access, &all).await?;
    let q = query.q.trim();
    let mut spans: Vec<(tickets::Model, timeline::Span)> = Vec::new();
    for t in &all {
        if !matches(t, q) {
            continue;
        }
        let sprint = t.sprint_id.and_then(|id| sprint_list.iter().find(|s| s.id == id));
        if let Some(s) = timeline::span(t, sprint, hours.get(&t.id).copied(), today)
            && s.end >= start
            && s.start <= end
        {
            spans.push((t.clone(), s));
        }
    }
    let shown: Vec<tickets::Model> = spans.iter().map(|(t, _)| t.clone()).collect();
    let lookup = lookup(db, &access.tenant, &shown, &[]).await?;
    let stages: Vec<Vec<String>> = project_definitions(&ctx, &access, &shown)
        .await?
        .iter()
        .map(|d| d.stages.iter().map(|s| s.key.clone()).collect())
        .collect();

    // Grup: sprint (urut tanggal mulai), lalu tanpa sprint.
    let mut order: Vec<Option<&sprints::Model>> = sprint_list.iter().map(Some).collect();
    order.sort_by_key(|s| s.map(|s| (s.starts_on, s.id)));
    order.push(None);
    let mut rows: Vec<TlRow> = Vec::new();
    let mut y = HEAD_H;
    let mut centers: HashMap<Uuid, (i64, i64, i64)> = HashMap::new();
    for group in order {
        let mine: Vec<&(tickets::Model, timeline::Span)> = spans
            .iter()
            .filter(|(t, _)| t.sprint_id == group.map(|s| s.id))
            .collect();
        if mine.is_empty() {
            continue;
        }
        let title = group.map_or_else(String::new, |s| {
            format!("{} · {}", s.name, pv::range(s.starts_on, s.ends_on))
        });
        rows.push(TlRow {
            group: Some(title),
            ticket: None,
            left: 0,
            width: 0,
            progress: 0,
            estimated: false,
            late: false,
            epic: false,
            tip: String::new(),
        });
        y += GROUP_H;
        for (t, s) in mine {
            let row = lookup.row(t);
            let left = x(s.start);
            let width = (x(s.end + Duration::days(1)) - left).max(px);
            centers.insert(t.id, (y + ROW_H / 2, left, left + width));
            y += ROW_H;
            let p = progress(t, &stages);
            let mut tip = format!(
                "{} · {}\n{} – {}",
                t.code,
                t.title,
                pv::long_day(s.start),
                pv::long_day(s.end)
            );
            if s.estimated {
                tip.push_str("\nBelum mulai · tanggal dari estimasi");
            } else {
                tip.push_str(&format!("\nProgres {p}%"));
            }
            if s.late {
                tip.push_str("\nPerkiraan selesai melewati tenggat");
            }
            rows.push(TlRow {
                epic: row.ticket_type.key == "epic",
                group: None,
                ticket: Some(row),
                left,
                width,
                progress: p,
                estimated: s.estimated,
                late: s.late,
                tip,
            });
        }
    }
    let height = y;

    // Kepala: bulan, hari, pita sprint, penanda.
    let mut month_labels = Vec::new();
    let mut m = start;
    while m <= end {
        month_labels.push(serde_json::json!({
            "label": format!("{} {}", pv::month_name(m), m.year()),
            "left": x(m),
        }));
        m = timeline::add_months(m, 1);
    }
    let mut day_labels = Vec::new();
    if zoom != "quarter" {
        for d in start.iter_days().take_while(|d| *d <= end) {
            let monday = d.weekday() == chrono::Weekday::Mon;
            if zoom == "week" || monday {
                day_labels.push(serde_json::json!({
                    "label": d.day(),
                    "left": x(d),
                    "width": if zoom == "week" { px } else { 7 * px },
                    "today": d == today,
                    "weekend": !planning::is_workday(d),
                }));
            }
        }
    }
    let bands: Vec<Mark> = sprint_list
        .iter()
        .filter(|s| s.ends_on >= start && s.starts_on <= end)
        .map(|s| {
            let left = x(s.starts_on);
            Mark {
                label: s.name.clone(),
                left: left + 1,
                width: (x(s.ends_on + Duration::days(1)) - left - 2).max(px),
                href: String::new(),
                old: s.state() == SprintState::Completed,
                tip: format!("{} · {}", s.name, pv::range(s.starts_on, s.ends_on)),
            }
        })
        .collect();
    let home = project_paths(&access).home;
    let release_marks: Vec<Mark> = releases::Model::of_project(db, wid, access.project.id)
        .await?
        .into_iter()
        .filter(|r| r.status() != ReleaseStatus::Archived)
        .filter_map(|r| {
            let d = r.target_on?;
            (d >= start && d <= end).then(|| Mark {
                tip: format!("{} · {}", r.name, pv::long_day(d)),
                left: x(d + Duration::days(1)),
                width: 0,
                href: format!("{home}/releases/{}", r.id),
                old: r.status() == ReleaseStatus::Released,
                label: r.name,
            })
        })
        .collect();
    let today_x = x(today) + px / 2;

    // Panah "blocks" di antara bar yang tampil.
    let ids: Vec<Uuid> = centers.keys().copied().collect();
    let links = ticket_links::Entity::find()
        .filter(ticket_links::Column::WorkspaceId.eq(wid.get()))
        .filter(ticket_links::Column::Kind.eq("blocks"))
        .filter(ticket_links::Column::FromId.is_in(ids.clone()))
        .filter(ticket_links::Column::ToId.is_in(ids))
        .all(db)
        .await?;
    let code_of = |id: Uuid| all.iter().find(|t| t.id == id).map(|t| t.code.clone()).unwrap_or_default();
    let arrows: Vec<serde_json::Value> = links
        .iter()
        .filter_map(|l| {
            let (y1, _, x1) = *centers.get(&l.from_id)?;
            let (y2, x2, _) = *centers.get(&l.to_id)?;
            let path = if x2 > x1 + 16 {
                let mx = (x1 + 8).max(x2 - 8);
                format!("M{x1},{y1} H{mx} V{y2} H{}", x2 - 2)
            } else {
                format!("M{x1},{y1} h8 V{} H{} V{y2} H{}", (y1 + y2) / 2, x2 - 10, x2 - 2)
            };
            Some(serde_json::json!({
                "d": path,
                "tip": format!("{} blocks {}", code_of(l.from_id), code_of(l.to_id)),
            }))
        })
        .collect();

    let base = format!("{home}/roadmap");
    let zooms: Vec<serde_json::Value> = ZOOMS
        .iter()
        .map(|(k, label, ..)| {
            let mut href = format!("{base}?zoom={k}");
            if !q.is_empty() {
                href.push_str(&format!("&q={}", urlencoding(q)));
            }
            serde_json::json!({ "key": k, "label": label, "href": href, "current": *k == zoom })
        })
        .collect();
    let shell = app_shell(&ctx, &access.tenant.user, Some(&access.tenant), Some(&access))
        .await?
        .at("project_roadmap");
    let data = merge(
        base_data(&access, "roadmap", &base),
        serde_json::json!({
            "q": q,
            "zoom": zoom,
            "zooms": zooms,
            "total": all.len(),
            "shown": shown.len(),
            "rows": rows,
            "track": days * px,
            "height": height,
            "months": month_labels,
            "days": day_labels,
            "bands": bands,
            "releases": release_marks,
            "today_x": today_x,
            "arrows": arrows,
        }),
    );
    views::tickets::render(&v, &shell, "tickets/roadmap.html", StatusCode::OK, data)
}

fn urlencoding(s: &str) -> String {
    serde_urlencoded::to_string([("", s)])
        .unwrap_or_default()
        .trim_start_matches('=')
        .to_string()
}

// ---------------------------------------------------------------------------------------------
// Kalender

#[derive(Debug, Serialize)]
struct Event {
    kind: &'static str,
    code: String,
    title: String,
    href: String,
    icon: String,
    color: String,
    late: bool,
    tip: String,
}

#[derive(Debug, Serialize)]
struct Day {
    date: String,
    number: u32,
    label: String,
    out: bool,
    weekend: bool,
    today: bool,
    band: String,
    events: Vec<Event>,
}

#[allow(clippy::too_many_lines)]
#[debug_handler]
pub(super) async fn calendar(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: ProjectAccess,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Result<Response> {
    access.require(Permission::ViewProject)?;
    let query = TimelineQuery::parse(pairs);
    let db = &ctx.db;
    let wid = access.tenant.workspace_id();
    let today = planning::today();
    let month = timeline::parse_month(&query.month).unwrap_or_else(|| timeline::month_start(today));
    let grid = timeline::month_grid(month);
    let first = grid[0][0];
    let last = grid[5][6];
    let kinds: Vec<&str> = query
        .kind
        .iter()
        .filter_map(|k| KINDS.iter().find(|x| x.0 == k).map(|x| x.0))
        .collect();
    let want = |k: &str| kinds.is_empty() || kinds.contains(&k);
    let q = query.q.trim();

    let all = project_tickets(&ctx, &access).await?;
    let hours = remaining(&ctx, &access, &all).await?;
    let shown: Vec<tickets::Model> = all.iter().filter(|t| matches(t, q)).cloned().collect();
    let lookup = lookup(db, &access.tenant, &shown, &[]).await?;
    let mut by_day: HashMap<NaiveDate, Vec<Event>> = HashMap::new();
    let mut undated = Vec::new();
    for t in &shown {
        let row = lookup.row(t);
        let forecast = timeline::forecast(t, hours.get(&t.id).copied(), today);
        if t.status != "done" && t.due_on.is_none() && forecast.is_none() {
            undated.push(row.clone());
        }
        if let Some(due) = t.due_on.filter(|d| *d >= first && *d <= last && want("due")) {
            let late = t.status != "done" && (due < today || forecast.is_some_and(|f| f > due));
            by_day.entry(due).or_default().push(Event {
                kind: "due",
                tip: format!(
                    "{} · {}\nTenggat {}{}",
                    t.code,
                    t.title,
                    pv::long_day(due),
                    if late { " · berisiko lewat" } else { "" }
                ),
                code: t.code.clone(),
                title: t.title.clone(),
                href: row.href.clone(),
                icon: row.ticket_type.icon.clone(),
                color: row.ticket_type.color.clone(),
                late,
            });
        }
        if let Some(f) = forecast.filter(|d| *d >= first && *d <= last && want("forecast")) {
            let late = t.due_on.is_some_and(|due| f > due);
            by_day.entry(f).or_default().push(Event {
                kind: "forecast",
                tip: format!("Perkiraan selesai AI: {}{}", pv::long_day(f), if late { ", melewati tenggat" } else { "" }),
                code: t.code.clone(),
                title: t.title.clone(),
                href: row.href.clone(),
                icon: "sparkles".into(),
                color: String::new(),
                late,
            });
        }
    }
    let home = project_paths(&access).home;
    if want("release") && q.is_empty() {
        for r in releases::Model::of_project(db, wid, access.project.id).await? {
            if r.status() == ReleaseStatus::Archived {
                continue;
            }
            if let Some(d) = r.target_on.filter(|d| *d >= first && *d <= last) {
                by_day.entry(d).or_default().push(Event {
                    kind: "release",
                    tip: format!("Rilis {} · target {}", r.name, pv::long_day(d)),
                    code: String::new(),
                    href: format!("{home}/releases/{}", r.id),
                    icon: "rocket".into(),
                    color: String::new(),
                    late: false,
                    title: r.name,
                });
            }
        }
    }
    let event_count: usize = by_day.values().map(Vec::len).sum();
    let sprint_list = sprints::Model::of_project(db, wid, access.project.id).await?;
    let weeks: Vec<Vec<Day>> = grid
        .iter()
        .map(|week| {
            week.iter()
                .map(|d| {
                    // Pita sprint di hari pertama sprint dan di Senin selama sprint berjalan.
                    let band = sprint_list
                        .iter()
                        .find(|s| {
                            *d >= s.starts_on
                                && *d <= s.ends_on
                                && (*d == s.starts_on || d.weekday() == chrono::Weekday::Mon)
                        })
                        .map(|s| s.name.clone())
                        .unwrap_or_default();
                    let out = d.month() != month.month();
                    Day {
                        date: d.to_string(),
                        number: d.day(),
                        label: if d.day() == 1 { pv::day_month(*d) } else { d.day().to_string() },
                        out,
                        weekend: !planning::is_workday(*d),
                        today: *d == today,
                        band,
                        events: by_day.remove(d).unwrap_or_default(),
                    }
                })
                .collect()
        })
        .collect();

    let base = format!("{home}/calendar");
    let keep = |month: NaiveDate| {
        let mut pairs = vec![("month".to_string(), format!("{:04}-{:02}", month.year(), month.month()))];
        pairs.extend(kinds.iter().map(|k| ("kind".to_string(), (*k).to_string())));
        if !q.is_empty() {
            pairs.push(("q".into(), q.to_string()));
        }
        format!("{base}?{}", serde_urlencoded::to_string(&pairs).unwrap_or_default())
    };
    let kind_chips: Vec<serde_json::Value> = KINDS
        .iter()
        .map(|(k, label)| {
            let on = kinds.contains(k);
            let mut next: Vec<&str> = kinds.iter().copied().filter(|x| x != k).collect();
            if !on {
                next.push(k);
            }
            let mut pairs = vec![("month".to_string(), format!("{:04}-{:02}", month.year(), month.month()))];
            pairs.extend(next.iter().map(|x| ("kind".to_string(), (*x).to_string())));
            if !q.is_empty() {
                pairs.push(("q".into(), q.to_string()));
            }
            serde_json::json!({
                "key": k, "label": label, "on": on,
                "href": format!("{base}?{}", serde_urlencoded::to_string(&pairs).unwrap_or_default()),
            })
        })
        .collect();
    let undated_count = undated.len();
    undated.truncate(8);
    let shell = app_shell(&ctx, &access.tenant.user, Some(&access.tenant), Some(&access))
        .await?
        .at("project_calendar");
    let data = merge(
        base_data(&access, "calendar", &base),
        serde_json::json!({
            "q": q,
            "month": format!("{:04}-{:02}", month.year(), month.month()),
            "month_label": format!("{} {}", pv::month_long(month), month.year()),
            "prev": keep(timeline::add_months(month, -1)),
            "next": keep(timeline::add_months(month, 1)),
            "today_href": base,
            "kinds": kind_chips,
            "kind_values": kinds,
            "weeks": weeks,
            "events": event_count,
            "undated": undated,
            "undated_count": undated_count,
        }),
    );
    views::tickets::render(&v, &shell, "tickets/calendar.html", StatusCode::OK, data)
}
