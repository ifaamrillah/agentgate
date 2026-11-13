//! Katalog komponen `/_dev/ui`. Data contoh statis (tanpa database), termasuk shell dengan
//! workspace dan project supaya sidebar lengkap ikut terlihat. Tabel contoh memakai
//! `views::datatable` sungguhan atas data di memori, jadi sort/filter/halaman bisa dicoba.
use loco_rs::prelude::*;

use super::{
    datatable::{ColumnSpec, FilterOption, FilterSpec, TableSpec, TableView},
    layout::{AppShell, ProjectLink, UserView, WorkspaceLink, WorkspaceView, app_page},
    nav, palette,
    ui::{ComboOption, Stat},
};

fn sample_shell() -> AppShell {
    AppShell::new(UserView::new("Ana Nugraha", "ana@agentgate.test"))
        .with_workspaces(vec![
            WorkspaceLink::new("Acme Studio", "acme"),
            WorkspaceLink::new("Beta Labs", "beta"),
        ])
        .with_current_workspace(WorkspaceView::new("Acme Studio", "acme"))
        .with_projects(vec![
            ProjectLink::new("acme", "Website", "WEB"),
            ProjectLink::new("acme", "Aplikasi Mobile", "APP"),
        ])
        .with_current_project(ProjectLink::new("acme", "Website", "WEB"))
        .can_manage_workspace(true)
        .with_new_project_href(Some("/w/acme/projects/new".into()))
}

const PEOPLE: &[(&str, &str, &str)] = &[
    ("Ana Nugraha", "ana@agentgate.test", "owner"),
    ("Budi Santoso", "budi@agentgate.test", "member"),
    ("Citra Lestari", "citra@agentgate.test", "admin"),
    ("Dewi Kusuma", "dewi@agentgate.test", "member"),
    ("Eko Saputra", "eko@agentgate.test", "viewer"),
    ("Fajar Nugroho", "fajar@agentgate.test", "member"),
    ("Gita Permata", "gita@agentgate.test", "member"),
    ("Hendra Gunawan", "hendra@agentgate.test", "admin"),
    ("Indah Sari", "indah@agentgate.test", "viewer"),
    ("Joko Pratama", "joko@agentgate.test", "member"),
    ("Kartika Dewi", "kartika@agentgate.test", "member"),
    ("Lina Marlina", "lina@agentgate.test", "member"),
];

static COLUMNS: &[ColumnSpec] = &[
    ColumnSpec::new("dev-col-name").sort("name"),
    ColumnSpec::new("dev-col-role").sort("role").hide("md"),
    ColumnSpec::new("").right(),
];
static FILTERS: &[FilterSpec] = &[FilterSpec {
    key: "role",
    label_key: "dev-col-role",
    icon: "key",
}];
/// Tabel contoh katalog (id DOM `catalog-table`).
pub const SAMPLE_TABLE: TableSpec = TableSpec {
    id: "catalog-table",
    columns: COLUMNS,
    filters: FILTERS,
    default_sort: "name",
    default_desc: false,
    default_per: 10,
    search_key: Some("dev-search"),
};

#[derive(serde::Serialize)]
struct Row {
    name: &'static str,
    email: &'static str,
    role: &'static str,
    initials: String,
    color: &'static str,
}

pub fn ui(v: &impl ViewRenderer, query: &[(String, String)]) -> Result<Response> {
    let state = SAMPLE_TABLE.parse(query);
    let mut rows: Vec<Row> = PEOPLE
        .iter()
        .filter(|(name, email, _)| {
            state.query().is_none_or(|q| {
                let q = q.to_lowercase();
                name.to_lowercase().contains(&q) || email.contains(&q)
            })
        })
        .filter(|(.., role)| {
            let roles = state.filter("role");
            roles.is_empty() || roles.iter().any(|r| r == role)
        })
        .map(|&(name, email, role)| Row {
            name,
            email,
            role,
            initials: palette::initials(name, email),
            color: palette::color_for(email),
        })
        .collect();
    rows.sort_by(|a, b| match state.sort {
        "role" => a.role.cmp(b.role).then(a.name.cmp(b.name)),
        _ => a.name.cmp(b.name),
    });
    if state.desc {
        rows.reverse();
    }
    let total = rows.len() as u64;
    let table = TableView::new(&SAMPLE_TABLE, &state, "/_dev/ui", total).with_filter_options(
        "role",
        ["owner", "admin", "member", "viewer"]
            .iter()
            .map(|r| FilterOption::raw(r, r))
            .collect(),
    );
    let offset = usize::try_from((table.page - 1) * table.per).unwrap_or(0);
    let per = usize::try_from(table.per).unwrap_or(10);
    let page_rows: Vec<&Row> = rows.iter().skip(offset).take(per).collect();
    let people: Vec<ComboOption> = PEOPLE
        .iter()
        .map(|(name, email, role)| ComboOption::person(email, name, email).group(role))
        .collect();
    format::render().view(
        v,
        "dev/ui.html",
        app_page(
            &sample_shell(),
            data!({
                "tabs": nav::mark(nav::account_nav(), "password"),
                "subnav": nav::mark(nav::account_nav(), "password"),
                "views": nav::project_views("/_dev/ui", &[], "overview"),
                "stats": [
                    Stat::new("dev-stat-open", 24, "ticket").hint("dev-stat-hint", 3).link("/_dev/ui#stats"),
                    Stat::new("dev-stat-waiting", 4, "eye").tone("warn"),
                    Stat::new("dev-stat-agents", 3, "sparkles").tone("accent"),
                    Stat::new("dev-stat-done", 12, "circle-check").of(31).tone("success"),
                ],
                "people": people,
                "table": table,
                "rows": page_rows,
                "runner": runner_samples(),
                "git": git_samples(),
                "graph": graph_samples(),
                "planning": planning_samples(),
                "kb": {
                    "markdown": "## Alur refund\n\nSupervisor menyetujui refund **di atas Rp500.000**.\n\n- [x] Cek struk\n- [ ] Catat alasan\n\n| Batas | Tindakan |\n| --- | --- |\n| ≤ Rp500.000 | Kasir |\n| > Rp500.000 | Supervisor |\n\n```mermaid\nflowchart LR\n  Kasir --> Supervisor --> Selesai\n```\n\n<script>alert(1)</script>",
                    "sbs": [
                        { "left_kind": "same", "left": "## Alur refund", "right_kind": "same", "right": "## Alur refund" },
                        { "left_kind": "del", "left": "Kasir menyetujui semua refund.", "right_kind": "add", "right": "Supervisor menyetujui refund di atas Rp500.000." },
                        { "left_kind": "gap", "left": "", "right_kind": "add", "right": "- Catat alasan refund" },
                    ],
                    "donut": super::knowledge::Donut::of(6, 2, 2),
                },
            }),
        ),
    )
}

/// Contoh planning (M9) untuk katalog: burndown (`views::charts::line_chart`).
fn planning_samples() -> serde_json::Value {
    use crate::views::charts::{Mark, Series, line_chart};
    let labels: Vec<String> = (1..=10).map(|d| format!("{d} Des")).collect();
    let ideal: Vec<Option<f64>> = (0..10).map(|i| Some(30.0 - 30.0 / 9.0 * f64::from(i))).collect();
    let actual = [30.0, 30.0, 27.0, 25.0, 25.0, 19.0];
    let remaining: Vec<Option<f64>> = (0..10).map(|i| actual.get(i).copied()).collect();
    let tips: Vec<String> = labels.iter().map(|l| format!("{l}\nSisa poin")).collect();
    serde_json::json!({
        "chart": line_chart(
            "Burndown",
            &labels,
            &[
                Series { tone: "muted", values: ideal, dash: true, area: false },
                Series { tone: "accent", values: remaining, dash: false, area: true },
            ],
            &[Mark { index: 5, tone: "warn", label: "Hari ini".into() }],
            &tips,
            640.0,
        ),
    })
}

/// Contoh komponen code graph (M7) untuk katalog.
fn graph_samples() -> serde_json::Value {
    use crate::views::graph::{CodeLine, LangPart, code_lines, lang_color};
    let part = |label: &str, lang: &str, percent: u32| LangPart {
        label: label.into(),
        percent,
        color: lang_color(lang),
    };
    let lines: Vec<CodeLine> = code_lines(
        "export function refundOrder(id: string) {\n\tconst order = findOrder(id);\n\treturn restock(order);\n}",
        42,
    );
    serde_json::json!({
        "levels": ["certain", "inferred", "unmatched", "external"],
        "langs": [part("TypeScript", "typescript", 78), part("Go", "go", 18), part("Python", "python", 4)],
        "lines": lines,
    })
}

/// Contoh status PR dan CI (M6) untuk katalog.
fn git_samples() -> serde_json::Value {
    use crate::views::git::StateView;
    use agentgate_core::git::{CiState, PrState};
    serde_json::json!({
        "prs": PrState::ALL.iter().map(|s| StateView::pr(*s)).collect::<Vec<_>>(),
        "ci": CiState::ALL.iter().map(|s| StateView::ci(*s)).collect::<Vec<_>>(),
    })
}

/// Contoh komponen runner & job (M5) untuk katalog.
fn runner_samples() -> serde_json::Value {
    serde_json::json!({
        "steps": {
            "items": [
                { "name": "Baca konteks dan rencana", "state": "ok", "seconds": "1m 28s" },
                { "name": "Implementasi StockGuard", "state": "now", "seconds": null },
                { "name": "Ajukan review", "state": "wait", "seconds": null },
            ],
            "done": 1,
            "percent": 33,
        },
        "logs": [
            { "stream": "info", "at": "09.41:02", "text": "Job KSR-26 · stage Development · runner MacBook Olivia" },
            { "stream": "tool", "at": "09.41:09", "text": "→ ag_context KSR-26" },
            { "stream": "ok", "at": "09.58:34", "text": "test checkout::stok_cukup_lolos ... ok" },
            { "stream": "fail", "at": "09.58:35", "text": "test checkout::stok_nol_bundle ... FAILED" },
        ],
        "evidence": [
            { "kind": "test_report", "icon": "test-pipe", "name": "junit.xml", "passed": true, "total": 12, "failed": 0, "note": "", "size": "2 KB", "at": "13/11/2026 09:44", "href": "#", "image": false },
            { "kind": "screenshot", "icon": "photo", "name": "checkout.png", "passed": null, "total": null, "failed": null, "note": "", "size": "48 KB", "at": "13/11/2026 09:51", "href": "#", "image": true },
        ],
    })
}
