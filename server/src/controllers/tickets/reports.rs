//! Project → Laporan: tab burndown, burnup, velocity, sprint report, cumulative flow, kalibrasi
//! estimasi, dan cycle time (`?tab=`), sprint dipilih lewat `?sprint=` (bawaan: sprint aktif,
//! lalu sprint selesai terakhir). Grafik SVG server-side (`views::charts`); "Cetak / PDF" memakai
//! cetak peramban. Butuh `ViewProject`.
use std::collections::HashMap;

use axum::http::StatusCode;
use loco_rs::prelude::*;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::Serialize;
use uuid::Uuid;

use super::{board::project_definitions, project_paths, summary};
use crate::{
    controllers::{extractors::ProjectAccess, shell::app_shell},
    models::{
        permission::Permission,
        sprints::{self, SprintState},
        tickets,
    },
    services::planning::{self, estimates, reports, sprints as sprint_service},
    views::{
        self,
        charts::{self, Mark, Series},
        nav::{self, NavItem},
        planning as pv,
        projects::PROJECT_VIEWS_ACTION,
        ui::Stat,
    },
};

/// Tab laporan: (kunci, kunci Fluent).
pub const TABS: &[(&str, &str)] = &[
    ("burndown", "report-tab-burndown"),
    ("burnup", "report-tab-burnup"),
    ("velocity", "report-tab-velocity"),
    ("sprint", "report-tab-sprint"),
    ("cfd", "report-tab-cfd"),
    ("calibration", "report-tab-calibration"),
    ("cycle", "report-tab-cycle"),
];

#[derive(Debug, serde::Deserialize, Default)]
pub(super) struct ReportQuery {
    #[serde(default)]
    tab: String,
    #[serde(default)]
    sprint: Option<Uuid>,
}

#[derive(Debug, Serialize)]
struct SprintOption {
    id: String,
    name: String,
    range: String,
    state: &'static str,
    href: String,
    current: bool,
}

#[derive(Debug, Serialize)]
struct Row {
    code: String,
    title: String,
    href: String,
    points: Option<i32>,
    estimate: String,
    actual: String,
    over: bool,
}

#[derive(Debug, Serialize)]
struct StageRow {
    name: String,
    estimate: String,
    actual: String,
    factor: String,
    high: bool,
    samples: usize,
}

#[derive(Debug, Serialize)]
struct CycleRow {
    name: String,
    ai: String,
    human: String,
    total: String,
    /// Lebar batang (% dari terpanjang) dan porsi AI (% dari batang).
    width: u32,
    ai_pct: u32,
}

fn pct(v: f64) -> u32 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    {
        v.clamp(0.0, 100.0).round() as u32
    }
}

#[allow(clippy::too_many_lines)]
#[debug_handler]
pub(super) async fn index(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: ProjectAccess,
    Query(query): Query<ReportQuery>,
) -> Result<Response> {
    access.require(Permission::ViewProject)?;
    let db = &ctx.db;
    let wid = access.tenant.workspace_id();
    let pid = access.project.id;
    let home = project_paths(&access).home;
    let base = format!("{home}/reports");
    let tab = TABS
        .iter()
        .find(|(k, _)| *k == query.tab)
        .map_or("burndown", |(k, _)| *k);
    let today = planning::today();

    let mut all_sprints = sprints::Model::of_project(db, wid, pid).await?;
    all_sprints.sort_by_key(|s| (s.starts_on, s.id));
    let chosen = query
        .sprint
        .and_then(|id| all_sprints.iter().find(|s| s.id == id))
        .or_else(|| all_sprints.iter().find(|s| s.state() == SprintState::Active))
        .or_else(|| {
            all_sprints
                .iter()
                .filter(|s| s.state() == SprintState::Completed)
                .max_by_key(|s| s.completed_at)
        })
        .cloned();
    let sprint_options: Vec<SprintOption> = all_sprints
        .iter()
        .rev()
        .filter(|s| s.state() != SprintState::Planned)
        .map(|s| SprintOption {
            id: s.id.to_string(),
            name: s.name.clone(),
            range: pv::range(s.starts_on, s.ends_on),
            state: if s.state() == SprintState::Active { "active" } else { "completed" },
            href: format!("{base}?tab={tab}&sprint={}", s.id),
            current: chosen.as_ref().is_some_and(|c| c.id == s.id),
        })
        .collect();
    let tabs: Vec<NavItem> = TABS
        .iter()
        .map(|(k, label)| {
            let mut href = format!("{base}?tab={k}");
            if let Some(c) = &chosen {
                href.push_str(&format!("&sprint={}", c.id));
            }
            let mut item = NavItem::link(k, label, "chart-line", &href);
            item.current = *k == tab;
            item
        })
        .collect();

    let project_tickets: Vec<tickets::Model> = tickets::Entity::find()
        .filter(tickets::Column::WorkspaceId.eq(wid.get()))
        .filter(tickets::Column::ProjectId.eq(pid))
        .filter(tickets::Column::DeletedAt.is_null())
        .all(db)
        .await?;
    let stage_names: HashMap<String, String> = project_definitions(&ctx, &access, &project_tickets)
        .await?
        .iter()
        .flat_map(|d| d.stages.iter().map(|s| (s.key.clone(), s.name.clone())))
        .collect();
    let stage_name = |k: &str| stage_names.get(k).cloned().unwrap_or_else(|| k.to_string());
    let done_sprints = sprint_service::velocity(db, wid, pid, 8).await?;
    let avg = sprint_service::average_velocity(&done_sprints);

    let mut chart = String::new();
    let mut legend: Vec<(&str, &str, bool)> = Vec::new();
    let mut stats: Vec<Stat> = Vec::new();
    let mut note = String::new();
    let mut note_arg = String::new();
    let mut done_rows: Vec<Row> = Vec::new();
    let mut left_rows: Vec<Row> = Vec::new();
    let mut moved_to = String::new();
    let mut stage_rows: Vec<StageRow> = Vec::new();
    let mut cycle_rows: Vec<CycleRow> = Vec::new();
    let mut scatter = String::new();
    let mut empty = false;
    let ticket_href = |code: &str| format!("/w/{}/t/{code}", access.tenant.workspace.slug);

    match tab {
        "burndown" => match &chosen {
            Some(s) => {
                let events = reports::scope_events(db, wid, s.id).await?;
                let b = reports::burndown(s, &events, today);
                let labels: Vec<String> = b.days.iter().map(|d| pv::day_month(d.day)).collect();
                let tips: Vec<String> = b
                    .days
                    .iter()
                    .map(|d| {
                        let mut t = format!("{}\nIdeal: {} poin", pv::day_month(d.day), charts::label(d.ideal));
                        if let Some(r) = d.remaining {
                            t.push_str(&format!("\nSisa: {r} poin"));
                        }
                        if let Some(p) = d.projected {
                            t.push_str(&format!("\nProyeksi: {} poin", charts::label(p)));
                        }
                        if d.added > 0 {
                            t.push_str(&format!("\nDitambahkan: +{} poin", d.added));
                        }
                        t
                    })
                    .collect();
                let marks: Vec<Mark> = b
                    .today
                    .map(|i| Mark {
                        index: i,
                        tone: "danger",
                        label: "Hari ini".into(),
                    })
                    .into_iter()
                    .collect();
                chart = charts::line_chart(
                    "Burndown",
                    &labels,
                    &[
                        Series {
                            tone: "muted",
                            values: b.days.iter().map(|d| Some(d.ideal)).collect(),
                            dash: true,
                            area: false,
                        },
                        Series {
                            tone: "accent",
                            values: b.days.iter().map(|d| d.remaining.map(f64::from)).collect(),
                            dash: false,
                            area: true,
                        },
                        Series {
                            tone: "warn",
                            values: b.days.iter().map(|d| d.projected).collect(),
                            dash: true,
                            area: false,
                        },
                    ],
                    &marks,
                    &tips,
                    680.0,
                );
                legend = vec![
                    ("report-legend-ideal", "muted", true),
                    ("report-legend-remaining", "accent", false),
                    ("report-legend-projection", "warn", true),
                ];
                stats = vec![
                    Stat::new("report-stat-committed", b.committed, "target")
                        .hint("report-hint-points", ""),
                    Stat::new("report-stat-remaining", b.remaining, "hourglass").tone("accent"),
                    Stat::new("report-stat-added", format!("+{}", b.added), "plus"),
                ];
                note = "report-note-burndown".into();
            }
            None => empty = true,
        },
        "burnup" => {
            let points = reports::burnup(&all_sprints, &project_tickets, avg);
            if points.is_empty() {
                empty = true;
            } else {
                let labels: Vec<String> = points.iter().map(|p| p.name.clone()).collect();
                let tips: Vec<String> = points
                    .iter()
                    .map(|p| {
                        let mut t = format!("{}\nScope: {} poin", p.name, p.scope);
                        if let Some(d) = p.done {
                            t.push_str(&format!("\nSelesai: {d} poin"));
                        }
                        if let (Some(pr), None) = (p.projected, p.done) {
                            t.push_str(&format!("\nProyeksi: {} poin", charts::label(pr)));
                        }
                        t
                    })
                    .collect();
                chart = charts::line_chart(
                    "Burnup",
                    &labels,
                    &[
                        Series {
                            tone: "muted",
                            values: points.iter().map(|p| Some(f64::from(p.scope))).collect(),
                            dash: false,
                            area: false,
                        },
                        Series {
                            tone: "success",
                            values: points.iter().map(|p| p.done.map(f64::from)).collect(),
                            dash: false,
                            area: true,
                        },
                        Series {
                            tone: "success",
                            values: points.iter().map(|p| p.projected).collect(),
                            dash: true,
                            area: false,
                        },
                    ],
                    &[],
                    &tips,
                    1000.0,
                );
                legend = vec![
                    ("report-legend-scope", "muted", false),
                    ("report-legend-done", "success", false),
                    ("report-legend-projection", "success", true),
                ];
                note = "report-note-burnup".into();
            }
        }
        "velocity" => {
            let mut groups: Vec<(String, f64, Option<f64>)> = done_sprints
                .iter()
                .map(|s| {
                    (
                        s.name.clone(),
                        f64::from(s.committed_points),
                        Some(f64::from(s.completed_points)),
                    )
                })
                .collect();
            if let Some(a) = all_sprints.iter().find(|s| s.state() == SprintState::Active) {
                groups.push((a.name.clone(), f64::from(a.committed_points), None));
            }
            if groups.is_empty() {
                empty = true;
            } else {
                chart = charts::pair_bars("Velocity", &groups, avg, "berjalan", 1000.0);
                legend = vec![
                    ("report-legend-commitment", "muted", false),
                    ("report-legend-done", "accent", false),
                    ("report-legend-average", "warn", true),
                ];
                let committed: i32 = done_sprints.iter().map(|s| s.committed_points).sum();
                let completed: i32 = done_sprints.iter().map(|s| s.completed_points).sum();
                stats = vec![
                    Stat::new(
                        "report-stat-velocity",
                        avg.map_or_else(|| "–".to_string(), pv::decimal),
                        "chart-line",
                    ),
                    Stat::new(
                        "report-stat-completion",
                        if committed > 0 {
                            format!("{}%", (f64::from(completed) / f64::from(committed) * 100.0).round())
                        } else {
                            "–".into()
                        },
                        "circle-check",
                    )
                    .tone("success"),
                    Stat::new("report-stat-sprints", done_sprints.len(), "flag"),
                ];
                note = "report-note-velocity".into();
            }
        }
        "sprint" => match &chosen {
            Some(s) if s.state() == SprintState::Completed => {
                let r = s.report();
                let row = |t: &sprints::ReportTicket| Row {
                    code: t.code.clone(),
                    title: t.title.clone(),
                    href: ticket_href(&t.code),
                    points: t.points,
                    estimate: t
                        .estimate_hours
                        .map(|h| format!("{} jam", charts::label(h)))
                        .unwrap_or_else(|| "–".into()),
                    actual: t
                        .actual_hours
                        .map(|h| format!("{} jam", charts::label(h)))
                        .unwrap_or_else(|| "–".into()),
                    over: matches!((t.estimate_hours, t.actual_hours), (Some(e), Some(a)) if a > e * 1.15),
                };
                done_rows = r.done.iter().map(row).collect();
                left_rows = r.not_done.iter().map(row).collect();
                moved_to.clone_from(&r.moved_to);
                let left_points: i32 = r.not_done.iter().filter_map(|t| t.points).sum();
                stats = vec![
                    Stat::new("report-stat-committed", r.committed, "target")
                        .hint("report-hint-points", ""),
                    Stat::new("report-stat-completed", r.completed, "circle-check")
                        .tone("success")
                        .hint(
                            "report-hint-of-commitment",
                            if r.committed > 0 {
                                format!("{}%", (f64::from(r.completed) / f64::from(r.committed) * 100.0).round())
                            } else {
                                "–".into()
                            },
                        ),
                    Stat::new("report-stat-not-done", r.not_done.len(), "arrow-right")
                        .tone("warn")
                        .hint("report-hint-points-n", left_points),
                    Stat::new("report-stat-added", format!("+{}", r.added), "plus"),
                ];
                note_arg.clone_from(&s.goal);
            }
            Some(_) => {
                note = "report-note-sprint-running".into();
                empty = true;
            }
            None => empty = true,
        },
        "cfd" => {
            let days: Vec<chrono::NaiveDate> = (0..21)
                .rev()
                .filter_map(|i| today.checked_sub_signed(chrono::Duration::days(i)))
                .filter(|d| planning::is_workday(*d))
                .collect();
            let flow = reports::cumulative_flow(db, wid, &project_tickets, &days).await?;
            if flow.is_empty() {
                empty = true;
            } else {
                // Urutan pita: stage selesai di bawah, mengikuti urutan workflow terbalik.
                let order: Vec<String> = {
                    let mut keys: Vec<String> = Vec::new();
                    for d in project_definitions(&ctx, &access, &project_tickets).await? {
                        for s in d.stages {
                            if !keys.contains(&s.key) {
                                keys.push(s.key);
                            }
                        }
                    }
                    keys.into_iter().rev().collect()
                };
                let mut layers = Vec::new();
                let mut keys: Vec<&String> = flow.keys().collect();
                keys.sort_by_key(|k| order.iter().position(|o| o == *k).unwrap_or(usize::MAX));
                for (i, k) in keys.iter().enumerate() {
                    layers.push((
                        stage_name(k),
                        charts::TONES[i % charts::TONES.len()],
                        flow[*k].clone(),
                    ));
                }
                let labels: Vec<String> = days.iter().map(|d| pv::day_month(*d)).collect();
                chart = charts::stacked_area("Cumulative flow", &labels, &layers, 1000.0);
                note = "report-note-cfd".into();
                legend = Vec::new();
                stage_rows = layers
                    .iter()
                    .rev()
                    .map(|(n, tone, _)| StageRow {
                        name: n.clone(),
                        estimate: (*tone).to_string(),
                        actual: String::new(),
                        factor: String::new(),
                        high: false,
                        samples: 0,
                    })
                    .collect();
            }
        }
        "calibration" => {
            let calib = estimates::calibration(db, wid, pid).await?;
            if calib.points.is_empty() {
                empty = true;
            } else {
                scatter = charts::scatter(
                    "Estimasi vs aktual",
                    &calib
                        .points
                        .iter()
                        .map(|p| (p.estimate, p.actual, p.code.clone()))
                        .collect::<Vec<_>>(),
                    "estimasi (jam)",
                    "aktual (jam)",
                );
                stage_rows = calib
                    .stages
                    .iter()
                    .map(|s| StageRow {
                        name: stage_name(&s.stage),
                        estimate: format!("{} + {}", charts::label(s.estimate.0), charts::label(s.estimate.1)),
                        actual: format!("{} + {}", charts::label(s.actual.0), charts::label(s.actual.1)),
                        factor: format!("×{}", charts::label((s.factor * 100.0).round() / 100.0)),
                        high: s.factor > 1.2,
                        samples: s.samples,
                    })
                    .collect();
                stats = vec![
                    Stat::new(
                        "backlog-stat-accuracy",
                        calib
                            .accuracy
                            .map_or_else(|| "–".to_string(), |a| format!("{}%", (a * 100.0).round())),
                        "sparkles",
                    )
                    .tone("success"),
                    Stat::new(
                        "report-stat-factor",
                        calib.overall.map_or_else(|| "–".to_string(), |o| format!("×{}", charts::label((o * 100.0).round() / 100.0))),
                        "adjustments-horizontal",
                    ),
                    Stat::new("report-stat-samples", calib.points.len(), "ticket"),
                ];
                note = "report-note-calibration".into();
            }
        }
        _ => {
            // Cycle time: tiket selesai 90 hari terakhir.
            let since = chrono::Utc::now() - chrono::Duration::days(90);
            let done: Vec<tickets::Model> = project_tickets
                .iter()
                .filter(|t| t.done_at.is_some_and(|d| d.with_timezone(&chrono::Utc) >= since))
                .cloned()
                .collect();
            let actuals = estimates::actuals(db, wid, &done).await?;
            let cycle = reports::cycle_time(&actuals);
            if cycle.is_empty() {
                empty = true;
            } else {
                let longest = cycle.values().map(|(a, h)| a + h).fold(0.0, f64::max).max(0.1);
                let total_ai: f64 = cycle.values().map(|(a, _)| a).sum();
                let total: f64 = cycle.values().map(|(a, h)| a + h).sum();
                cycle_rows = cycle
                    .iter()
                    .map(|(k, (a, h))| CycleRow {
                        name: stage_name(k),
                        ai: charts::label((a * 10.0).round() / 10.0),
                        human: charts::label((h * 10.0).round() / 10.0),
                        total: charts::label(((a + h) * 10.0).round() / 10.0),
                        width: pct((a + h) / longest * 100.0).max(3),
                        ai_pct: if a + h > 0.0 { pct(a / (a + h) * 100.0) } else { 0 },
                    })
                    .collect();
                legend = vec![
                    ("report-legend-ai", "accent", false),
                    ("report-legend-human", "warn", false),
                ];
                if total > 0.0 {
                    note = "report-note-cycle".into();
                    note_arg = format!("{}%", pct((total - total_ai) / total * 100.0));
                }
                stats = vec![Stat::new("report-stat-samples", done.len(), "ticket")];
            }
        }
    }

    let shell = app_shell(&ctx, &access.tenant.user, Some(&access.tenant), Some(&access))
        .await?
        .at("project_reports");
    views::tickets::render(
        &v,
        &shell,
        "tickets/reports.html",
        StatusCode::OK,
        data!({
            "project": summary(&access),
            "views": nav::project_views(&home, &access.tenant.user.project_views, "reports"),
            "views_action": PROJECT_VIEWS_ACTION,
            "return_to": base,
            "tab": tab,
            "tabs": tabs,
            "sprint": chosen.as_ref().map(|s| serde_json::json!({
                "id": s.id.to_string(), "name": s.name, "goal": s.goal,
                "range": pv::range(s.starts_on, s.ends_on),
                "state": if s.state() == SprintState::Active { "active" } else if s.state() == SprintState::Completed { "completed" } else { "planned" },
            })),
            "sprints": sprint_options,
            "chart": chart,
            "legend": legend.iter().map(|(k, c, d)| serde_json::json!({"key": k, "tone": c, "dash": d})).collect::<Vec<_>>(),
            "stats": stats,
            "note": note,
            "note_arg": note_arg,
            "done_rows": done_rows,
            "left_rows": left_rows,
            "moved_to": moved_to,
            "stage_rows": stage_rows,
            "cycle_rows": cycle_rows,
            "scatter": scatter,
            "empty": empty,
            "backlog": format!("{home}/backlog"),
            "updated": views::time::wib_datetime(&chrono::Utc::now()),
        }),
    )
}

/// Kunci Fluent tab dan statistik laporan (dites kontrak i18n).
#[must_use]
pub fn label_keys() -> Vec<String> {
    TABS.iter()
        .map(|(_, k)| (*k).to_string())
        .chain(
            [
                "report-stat-committed",
                "report-stat-remaining",
                "report-stat-added",
                "report-stat-velocity",
                "report-stat-completion",
                "report-stat-sprints",
                "report-stat-completed",
                "report-stat-not-done",
                "report-stat-factor",
                "report-stat-samples",
                "report-hint-points",
                "report-hint-points-n",
                "report-hint-of-commitment",
                "report-legend-ideal",
                "report-legend-remaining",
                "report-legend-projection",
                "report-legend-scope",
                "report-legend-done",
                "report-legend-commitment",
                "report-legend-average",
                "report-legend-ai",
                "report-legend-human",
                "report-note-burndown",
                "report-note-burnup",
                "report-note-velocity",
                "report-note-sprint-running",
                "report-note-cfd",
                "report-note-calibration",
                "report-note-cycle",
            ]
            .iter()
            .map(|k| (*k).to_string()),
        )
        .chain(TABS.iter().map(|(k, _)| format!("report-empty-{k}")))
        .chain(
            ["burnup", "velocity", "cfd"]
                .iter()
                .map(|k| format!("report-{k}-description")),
        )
        .collect()
}
