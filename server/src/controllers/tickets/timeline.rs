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
    common::i18n,
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
        .chain([
            "roadmap-no-sprint",
            "roadmap-no-epic",
            "roadmap-group-sprint",
            "roadmap-group-epic",
            "roadmap-tip-estimated",
            "roadmap-tip-progress",
            "roadmap-tip-late",
            "roadmap-tip-epic",
            "roadmap-tip-blocks",
        ])
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
    /// Roadmap: `sprint` (bawaan) atau `epic` (R3).
    group: String,
    /// Roadmap per epic: kode epic yang dibuka (`?open=KSR-1,KSR-5`; `all` = semua).
    open: Vec<String>,
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
                "group" => out.group = v,
                "open" => out.open.extend(
                    v.split(',')
                        .map(|c| c.trim().to_string())
                        .filter(|c| !c.is_empty())
                        .take(200),
                ),
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
    /// Batang tampil (rentang ada dan masuk jendela).
    bar: bool,
    progress: u32,
    estimated: bool,
    late: bool,
    epic: bool,
    tip: String,
    /// 0 = baris utama, 1 = anak epic (menjorok).
    level: u8,
    /// Tautan buka/tutup anak epic (kosong = tanpa tombol).
    toggle: String,
    open: bool,
    /// Jumlah anak epic.
    count: usize,
}

/// Atribut tambahan baris roadmap per epic.
#[derive(Debug, Default)]
struct RowExtra {
    level: u8,
    toggle: String,
    open: bool,
    count: usize,
}

impl TlRow {
    fn group(title: String) -> Self {
        Self {
            group: Some(title),
            ticket: None,
            left: 0,
            width: 0,
            bar: false,
            progress: 0,
            estimated: false,
            late: false,
            epic: false,
            tip: String::new(),
            level: 0,
            toggle: String::new(),
            open: false,
            count: 0,
        }
    }

    fn ticket(row: TicketRow, epic: bool, progress: u32, tip: String, extra: RowExtra) -> Self {
        Self {
            group: None,
            ticket: Some(row),
            left: 0,
            width: 0,
            bar: false,
            progress,
            estimated: false,
            late: false,
            epic,
            tip,
            level: extra.level,
            toggle: extra.toggle,
            open: extra.open,
            count: extra.count,
        }
    }
}

/// Tautan roadmap dengan zoom, cari, pengelompokan, dan epic yang dibuka.
fn roadmap_href(base: &str, zoom: &str, q: &str, group: &str, open: &[String]) -> String {
    let mut pairs: Vec<(&str, String)> = vec![("zoom", zoom.to_string())];
    if !q.is_empty() {
        pairs.push(("q", q.to_string()));
    }
    if group == "epic" {
        pairs.push(("group", "epic".into()));
        if !open.is_empty() {
            pairs.push(("open", open.join(",")));
        }
    }
    format!(
        "{base}?{}",
        serde_urlencoded::to_string(&pairs).unwrap_or_default()
    )
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
fn progress_of(t: &tickets::Model, stages: &[Vec<String>]) -> u32 {
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

    let base_path = format!("{}/roadmap", project_paths(&access).home);
    let all = project_tickets(&ctx, &access).await?;
    let sprint_list = sprints::Model::of_project(db, wid, access.project.id).await?;
    let hours = remaining(&ctx, &access, &all).await?;
    let q = query.q.trim();
    let by_epic = query.group == "epic";
    let open_all = query.open.iter().any(|c| c == "all");
    // Rentang semua tiket (tanpa filter cari/jendela): batang epic memakai rentang anak.
    let span_of: HashMap<Uuid, timeline::Span> = all
        .iter()
        .filter_map(|t| {
            let sprint = t
                .sprint_id
                .and_then(|id| sprint_list.iter().find(|s| s.id == id));
            timeline::span(t, sprint, hours.get(&t.id).copied(), today).map(|s| (t.id, s))
        })
        .collect();
    let in_window = |s: &timeline::Span| s.end >= start && s.start <= end;
    let lookup = lookup(db, &access.tenant, &all, &[]).await?;
    let stages: Vec<Vec<String>> = project_definitions(&ctx, &access, &all)
        .await?
        .iter()
        .map(|d| d.stages.iter().map(|s| s.key.clone()).collect())
        .collect();
    let is_epic = |t: &tickets::Model| lookup.ticket_type(t.type_id).key == "epic";
    let span_tip = |t: &tickets::Model, s: &timeline::Span, p: u32| {
        let mut tip = format!(
            "{} · {}\n{} – {}",
            t.code,
            t.title,
            pv::long_day(s.start),
            pv::long_day(s.end)
        );
        let line = if s.estimated {
            i18n::text("roadmap-tip-estimated", &[])
        } else {
            i18n::text("roadmap-tip-progress", &[("progress", &p.to_string())])
        };
        tip.push('\n');
        tip.push_str(&line);
        if s.late {
            tip.push('\n');
            tip.push_str(&i18n::text("roadmap-tip-late", &[]));
        }
        tip
    };

    let mut rows: Vec<TlRow> = Vec::new();
    let mut y = HEAD_H;
    let mut centers: HashMap<Uuid, (i64, i64, i64)> = HashMap::new();
    // Anak epic yang tertutup → baris epic-nya (panah blocks tetap terlihat).
    let mut alias: HashMap<Uuid, Uuid> = HashMap::new();
    let mut shown = 0usize;
    let group_row = |rows: &mut Vec<TlRow>, y: &mut i64, title: String| {
        rows.push(TlRow::group(title));
        *y += GROUP_H;
    };
    let mut ticket_row = |rows: &mut Vec<TlRow>,
                          y: &mut i64,
                          t: &tickets::Model,
                          span: Option<timeline::Span>,
                          progress: u32,
                          tip: String,
                          extra: RowExtra| {
        let mut row = TlRow::ticket(lookup.row(t), is_epic(t), progress, tip, extra);
        if let Some(s) = span.filter(|s| in_window(s)) {
            row.left = x(s.start);
            row.width = (x(s.end + Duration::days(1)) - row.left).max(px);
            row.estimated = s.estimated;
            row.late = s.late;
            row.bar = true;
            centers.insert(t.id, (*y + ROW_H / 2, row.left, row.left + row.width));
        }
        *y += ROW_H;
        rows.push(row);
    };

    if by_epic {
        let tree: HashMap<Uuid, timeline::TreeNode> = all
            .iter()
            .map(|t| {
                (
                    t.id,
                    timeline::TreeNode {
                        parent: t.parent_id,
                        epic: is_epic(t),
                    },
                )
            })
            .collect();
        let mut kids: HashMap<Uuid, Vec<&tickets::Model>> = HashMap::new();
        let mut loose: Vec<&tickets::Model> = Vec::new();
        for t in all.iter().filter(|t| !is_epic(t)) {
            match timeline::epic_of(t.id, &tree) {
                Some(e) => kids.entry(e).or_default().push(t),
                None => loose.push(t),
            }
        }
        for epic in all.iter().filter(|t| is_epic(t)) {
            let children = kids.get(&epic.id).cloned().unwrap_or_default();
            if !matches(epic, q) && !children.iter().any(|c| matches(c, q)) {
                continue;
            }
            let child_spans: Vec<timeline::Span> = children
                .iter()
                .filter_map(|c| span_of.get(&c.id).copied())
                .collect();
            let span = timeline::merge_spans(&child_spans).or_else(|| span_of.get(&epic.id).copied());
            let done = children.iter().filter(|c| c.status == "done").count();
            let progress = if epic.status == "done" {
                100
            } else {
                timeline::epic_progress(done, children.len())
            };
            let open = open_all || query.open.iter().any(|c| c.eq_ignore_ascii_case(&epic.code));
            let mut tip = span.map_or_else(
                || format!("{} · {}", epic.code, epic.title),
                |s| format!(
                    "{} · {}\n{} – {}",
                    epic.code,
                    epic.title,
                    pv::long_day(s.start),
                    pv::long_day(s.end)
                ),
            );
            tip.push('\n');
            tip.push_str(&i18n::text(
                "roadmap-tip-epic",
                &[
                    ("count", &children.len().to_string()),
                    ("done", &done.to_string()),
                    ("progress", &progress.to_string()),
                ],
            ));
            let toggled: Vec<String> = if open {
                query
                    .open
                    .iter()
                    .filter(|c| !c.eq_ignore_ascii_case(&epic.code) && *c != "all")
                    .cloned()
                    .collect()
            } else {
                let mut v = query.open.clone();
                v.push(epic.code.clone());
                v
            };
            shown += 1;
            ticket_row(
                &mut rows,
                &mut y,
                epic,
                span,
                progress,
                tip,
                RowExtra {
                    level: 0,
                    toggle: if children.is_empty() {
                        String::new()
                    } else {
                        roadmap_href(&base_path, zoom, q, "epic", &toggled)
                    },
                    open,
                    count: children.len(),
                },
            );
            for c in &children {
                if !open {
                    alias.insert(c.id, epic.id);
                    continue;
                }
                if !matches(c, q) {
                    continue;
                }
                let Some(s) = span_of.get(&c.id).copied().filter(|s| in_window(s)) else {
                    alias.insert(c.id, epic.id);
                    continue;
                };
                let p = progress_of(c, &stages);
                shown += 1;
                ticket_row(
                    &mut rows,
                    &mut y,
                    c,
                    Some(s),
                    p,
                    span_tip(c, &s, p),
                    RowExtra {
                        level: 1,
                        ..RowExtra::default()
                    },
                );
            }
        }
        let rest: Vec<(&tickets::Model, timeline::Span)> = loose
            .into_iter()
            .filter(|t| matches(t, q))
            .filter_map(|t| span_of.get(&t.id).copied().filter(|s| in_window(s)).map(|s| (t, s)))
            .collect();
        if !rest.is_empty() {
            group_row(&mut rows, &mut y, String::new());
            for (t, s) in rest {
                let p = progress_of(t, &stages);
                shown += 1;
                ticket_row(&mut rows, &mut y, t, Some(s), p, span_tip(t, &s, p), RowExtra::default());
            }
        }
    } else {
        // Grup: sprint (urut tanggal mulai), lalu tanpa sprint.
        let mut order: Vec<Option<&sprints::Model>> = sprint_list.iter().map(Some).collect();
        order.sort_by_key(|s| s.map(|s| (s.starts_on, s.id)));
        order.push(None);
        for group in order {
            let mine: Vec<(&tickets::Model, timeline::Span)> = all
                .iter()
                .filter(|t| t.sprint_id == group.map(|s| s.id) && matches(t, q))
                .filter_map(|t| span_of.get(&t.id).copied().filter(|s| in_window(s)).map(|s| (t, s)))
                .collect();
            if mine.is_empty() {
                continue;
            }
            let title = group.map_or_else(String::new, |s| {
                format!("{} · {}", s.name, pv::range(s.starts_on, s.ends_on))
            });
            group_row(&mut rows, &mut y, title);
            for (t, s) in mine {
                let p = progress_of(t, &stages);
                shown += 1;
                ticket_row(&mut rows, &mut y, t, Some(s), p, span_tip(t, &s, p), RowExtra::default());
            }
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

    // Panah "blocks" di antara bar yang tampil; anak epic yang tertutup diwakili baris epic-nya.
    let ids: Vec<Uuid> = all.iter().map(|t| t.id).collect();
    let links = ticket_links::Entity::find()
        .filter(ticket_links::Column::WorkspaceId.eq(wid.get()))
        .filter(ticket_links::Column::Kind.eq("blocks"))
        .filter(ticket_links::Column::FromId.is_in(ids.clone()))
        .filter(ticket_links::Column::ToId.is_in(ids))
        .all(db)
        .await?;
    let code_of = |id: Uuid| {
        all.iter()
            .find(|t| t.id == id)
            .map(|t| t.code.clone())
            .unwrap_or_default()
    };
    let mut drawn: Vec<(Uuid, Uuid)> = Vec::new();
    let arrows: Vec<serde_json::Value> = links
        .iter()
        .filter_map(|l| {
            let from = alias.get(&l.from_id).copied().unwrap_or(l.from_id);
            let to = alias.get(&l.to_id).copied().unwrap_or(l.to_id);
            if from == to || drawn.contains(&(from, to)) {
                return None;
            }
            let (y1, _, x1) = *centers.get(&from)?;
            let (y2, x2, _) = *centers.get(&to)?;
            drawn.push((from, to));
            let path = if x2 > x1 + 16 {
                let mx = (x1 + 8).max(x2 - 8);
                format!("M{x1},{y1} H{mx} V{y2} H{}", x2 - 2)
            } else {
                format!(
                    "M{x1},{y1} h8 V{} H{} V{y2} H{}",
                    (y1 + y2) / 2,
                    x2 - 10,
                    x2 - 2
                )
            };
            Some(serde_json::json!({
                "d": path,
                "tip": i18n::text(
                    "roadmap-tip-blocks",
                    &[("from", &code_of(l.from_id)), ("to", &code_of(l.to_id))],
                ),
            }))
        })
        .collect();

    let base = base_path.clone();
    let group = if by_epic { "epic" } else { "sprint" };
    let zooms: Vec<serde_json::Value> = ZOOMS
        .iter()
        .map(|(k, label, ..)| {
            let href = roadmap_href(&base, k, q, group, &query.open);
            serde_json::json!({ "key": k, "label": label, "href": href, "current": *k == zoom })
        })
        .collect();
    let groups: Vec<serde_json::Value> = [("sprint", "roadmap-group-sprint"), ("epic", "roadmap-group-epic")]
        .iter()
        .map(|(k, label)| {
            serde_json::json!({
                "key": k, "label": label, "current": *k == group,
                "href": roadmap_href(&base, zoom, q, k, &[]),
            })
        })
        .collect();
    let expand_all = roadmap_href(&base, zoom, q, "epic", &["all".to_string()]);
    let collapse_all = roadmap_href(&base, zoom, q, "epic", &[]);
    let shell = app_shell(
        &ctx,
        &access.tenant.user,
        Some(&access.tenant),
        Some(&access),
    )
    .await?
    .at("project_roadmap");
    let data = merge(
        base_data(&access, "roadmap", &base),
        serde_json::json!({
            "q": q,
            "zoom": zoom,
            "zooms": zooms,
            "group": group,
            "groups": groups,
            "open": query.open.join(","),
            "expand_all": expand_all,
            "collapse_all": collapse_all,
            "total": all.len(),
            "shown": shown,
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
        if let Some(due) = t
            .due_on
            .filter(|d| *d >= first && *d <= last && want("due"))
        {
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
                tip: format!(
                    "Perkiraan selesai AI: {}{}",
                    pv::long_day(f),
                    if late { ", melewati tenggat" } else { "" }
                ),
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
                        label: if d.day() == 1 {
                            pv::day_month(*d)
                        } else {
                            d.day().to_string()
                        },
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
        let mut pairs = vec![(
            "month".to_string(),
            format!("{:04}-{:02}", month.year(), month.month()),
        )];
        pairs.extend(kinds.iter().map(|k| ("kind".to_string(), (*k).to_string())));
        if !q.is_empty() {
            pairs.push(("q".into(), q.to_string()));
        }
        format!(
            "{base}?{}",
            serde_urlencoded::to_string(&pairs).unwrap_or_default()
        )
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
    let shell = app_shell(
        &ctx,
        &access.tenant.user,
        Some(&access.tenant),
        Some(&access),
    )
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
