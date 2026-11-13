//! Code graph di web (M7), tab project `graph` (`ViewProject`):
//!
//! - `GET /w/{slug}/p/{key}/graph` — cari simbol (`?q=&kind=&repo=`); tanpa kueri: modul dan
//!   node pusatnya;
//! - `GET …/graph/symbol/{id}` — detail simbol: kode, pemanggil, yang dipanggil, lintas repo,
//!   dampak;
//! - `GET …/graph/index[/{panel}]` — status index (Repo & index, Lintas repo, Riwayat git,
//!   Kualitas kode, Benchmark);
//! - `POST …/graph/reindex` (`EditProject`) — antrekan index server untuk repo terhubung.
//!
//! Viewer visual = M10 (tab "segera"). Tanpa izin → 404.
use std::collections::HashMap;

use axum::http::StatusCode;
use loco_rs::prelude::*;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    controllers::{extractors::ProjectAccess, flash, shell::app_shell},
    models::{graph_repos, permission::Permission, repos},
    services::graph::{
        self as svc,
        query::{self, Direction, Hop, LinkRow, SearchFilter, SymbolHit},
    },
    views::{
        self,
        datatable::{ColumnSpec, FilterOption, FilterSpec, TableSpec, TableView},
        flash::Flash,
        graph::{self as gv, RepoIndexView, SymbolRow, num, pct},
        nav::{self, NavItem},
        projects::{ProjectPaths, ProjectSummary},
        ui::Stat,
    },
};

const PANELS: &[(&str, &str, &str, &str)] = &[
    (
        "repo",
        "graph-panel-repo",
        "database",
        "graph-panel-repo-desc",
    ),
    (
        "cross",
        "graph-panel-cross",
        "arrows-exchange",
        "graph-panel-cross-desc",
    ),
    ("git", "graph-panel-git", "history", "graph-panel-git-desc"),
    (
        "quality",
        "graph-panel-quality",
        "trash",
        "graph-panel-quality-desc",
    ),
    (
        "bench",
        "graph-panel-bench",
        "gauge",
        "graph-panel-bench-desc",
    ),
];

fn paths(access: &ProjectAccess) -> ProjectPaths {
    ProjectPaths::new(&access.tenant.workspace.slug, &access.project.key)
}

fn symbol_base(p: &ProjectPaths) -> String {
    format!("{}/symbol", p.graph)
}

/// Tab Cari simbol / Status index / Viewer (M10, segera).
fn graph_tabs(p: &ProjectPaths, current: &str) -> Vec<NavItem> {
    let mut search = NavItem::link("search", "graph-tab-search", "search", &p.graph);
    search.current = current == "search";
    let mut index = NavItem::link("index", "graph-tab-index", "database", &p.graph_index);
    index.current = current == "index";
    vec![
        search,
        index,
        NavItem::soon("viewer", "graph-tab-viewer", "topology-star-3"),
    ]
}

/// Data bersama semua halaman graph.
struct Base {
    paths: ProjectPaths,
    repos: Vec<repos::Model>,
    rows: Vec<graph_repos::Model>,
    can_edit: bool,
}

impl Base {
    async fn load(ctx: &AppContext, access: &ProjectAccess) -> Result<Self> {
        let wid = access.tenant.workspace_id();
        Ok(Self {
            paths: paths(access),
            repos: repos::Model::of_project(&ctx.db, wid, access.project.id).await?,
            rows: graph_repos::Model::of_project(&ctx.db, wid, access.project.id).await?,
            can_edit: access.role.has(Permission::EditProject),
        })
    }

    fn row(&self, repo: Uuid) -> Option<&graph_repos::Model> {
        self.rows.iter().find(|r| r.repo_id == repo)
    }

    fn views(&self) -> Vec<RepoIndexView> {
        self.repos
            .iter()
            .map(|r| RepoIndexView::of(r, self.row(r.id)))
            .collect()
    }

    fn indexed(&self) -> bool {
        self.rows.iter().any(|r| r.indexed_at.is_some())
    }

    /// Stat ringkas: simbol, relasi (% pasti), file terindeks, cakupan.
    fn stats(&self) -> Vec<Stat> {
        let sym: i64 = self.rows.iter().map(|r| i64::from(r.symbols)).sum();
        let edges: i64 = self.rows.iter().map(|r| i64::from(r.edges)).sum();
        let certain: i64 = self.rows.iter().map(|r| i64::from(r.edges_certain)).sum();
        let files: i64 = self.rows.iter().map(|r| i64::from(r.files_indexed)).sum();
        let skipped: i64 = self.rows.iter().map(|r| i64::from(r.files_skipped)).sum();
        let coverage = if files + skipped > 0 {
            files as f64 * 100.0 / (files + skipped) as f64
        } else {
            0.0
        };
        let certain_pct = if edges > 0 {
            certain as f64 * 100.0 / edges as f64
        } else {
            0.0
        };
        vec![
            Stat::new("graph-stat-symbols", num(sym), "code").hint("graph-stat-symbols-hint", ""),
            Stat::new("graph-stat-edges", num(edges), "topology-star-3")
                .hint("graph-stat-edges-hint", pct(certain_pct)),
            Stat::new("graph-stat-files", num(files), "file-code")
                .of(num(files + skipped))
                .hint("graph-stat-files-hint", num(skipped)),
            Stat::new(
                "graph-stat-coverage",
                format!("{}%", pct(coverage)),
                "gauge",
            )
            .tone(if coverage >= 95.0 { "success" } else { "" }),
        ]
    }
}

async fn render(
    ctx: &AppContext,
    v: &TeraView,
    access: &ProjectAccess,
    base: &Base,
    (template, tab, return_to): (&str, &str, &str),
    mut data: serde_json::Value,
) -> Result<Response> {
    let shell = app_shell(ctx, &access.tenant.user, Some(&access.tenant), Some(access))
        .await?
        .at("project_graph");
    let p = &base.paths;
    let extra = serde_json::json!({
        "project": ProjectSummary::new(&access.project, access.role.label(), p),
        "views": nav::project_views(&p.home, &access.tenant.user.project_views, "graph"),
        "views_action": views::projects::PROJECT_VIEWS_ACTION,
        "return_to": return_to,
        "paths": p,
        "graph_tabs": graph_tabs(p, tab),
        "can_edit": base.can_edit,
        "repos": base.views(),
        "indexed": base.indexed(),
        "reindex_action": format!("{}/reindex", p.graph),
    });
    if let (Some(d), serde_json::Value::Object(e)) = (data.as_object_mut(), extra) {
        d.extend(e);
    }
    views::tickets::render(v, &shell, template, StatusCode::OK, data)
}

#[derive(Debug, Default, Deserialize)]
struct SearchQuery {
    #[serde(default)]
    q: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    repo: String,
}

#[debug_handler]
async fn search(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: ProjectAccess,
    session: Session,
    Query(sq): Query<SearchQuery>,
) -> Result<Response> {
    access.require(Permission::ViewProject)?;
    let (session, flash) = flash::take(session);
    let base = Base::load(&ctx, &access).await?;
    let wid = access.tenant.workspace_id();
    let sb = symbol_base(&base.paths);
    let kind = agentgate_graph::SymbolKind::parse(sq.kind.trim());
    let repo = base
        .repos
        .iter()
        .find(|r| r.name == sq.repo.trim())
        .map(|r| r.id);
    let q = sq.q.trim().chars().take(200).collect::<String>();
    let results: Vec<SymbolRow> = if q.is_empty() || !base.indexed() {
        Vec::new()
    } else {
        query::search(
            &ctx.db,
            wid,
            access.project.id,
            &q,
            &SearchFilter {
                kind,
                repo,
                include_tests: true,
            },
            50,
        )
        .await?
        .iter()
        .map(|h| symbol_row(h, &sb))
        .collect()
    };
    let names: HashMap<Uuid, &str> = base.repos.iter().map(|r| (r.id, r.name.as_str())).collect();
    let modules: Vec<serde_json::Value> = base
        .rows
        .iter()
        .flat_map(|r| {
            let repo = names
                .get(&r.repo_id)
                .copied()
                .unwrap_or_default()
                .to_string();
            let sb = sb.clone();
            r.modules().into_iter().map(move |m| {
                serde_json::json!({
                    "repo": repo,
                    "name": m.name,
                    "files": m.files,
                    "symbols": num(i64::from(m.symbols)),
                    "central": m.central_name,
                    "href": m.central.map(|c| format!("{sb}/{c}")),
                })
            })
        })
        .take(24)
        .collect();
    let kinds: Vec<serde_json::Value> = [
        "function",
        "method",
        "class",
        "interface",
        "struct",
        "enum",
        "trait",
        "type",
        "route",
    ]
    .iter()
    .map(|k| serde_json::json!({ "value": k, "label_key": format!("graph-kind-{k}") }))
    .collect();
    let res = render(
        &ctx,
        &v,
        &access,
        &base,
        ("graph/search.html", "search", &base.paths.graph),
        serde_json::json!({
            "stats": base.stats(),
            "q": q,
            "kind": sq.kind,
            "repo_filter": sq.repo,
            "kinds": kinds,
            "results": results,
            "modules": modules,
            "symbol_base": sb,
            "flash": flash,
        }),
    )
    .await?;
    Ok((session, res).into_response())
}

#[debug_handler]
async fn symbol(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: ProjectAccess,
    Path((_, _, id)): Path<(String, String, String)>,
) -> Result<Response> {
    access.require(Permission::ViewProject)?;
    let id = Uuid::parse_str(&id).map_err(|_| Error::NotFound)?;
    let base = Base::load(&ctx, &access).await?;
    let wid = access.tenant.workspace_id();
    let pid = access.project.id;
    let hit = query::symbol(&ctx.db, wid, pid, id)
        .await?
        .ok_or(Error::NotFound)?;
    let sb = symbol_base(&base.paths);
    let snippet = query::snippet(&ctx.db, wid, pid, &hit, 0).await?;
    let callers = query::trace(&ctx.db, wid, pid, &[id], Direction::Callers, 2, 60).await?;
    let callees = query::trace(&ctx.db, wid, pid, &[id], Direction::Callees, 2, 60).await?;
    let links = query::links(&ctx.db, wid, pid, Some(id)).await?;
    let impact = query::impact(&ctx.db, wid, pid, vec![hit.clone()], 3).await?;
    let return_to = format!("{sb}/{id}");
    render(
        &ctx,
        &v,
        &access,
        &base,
        ("graph/symbol.html", "search", &return_to),
        serde_json::json!({
            "symbol": symbol_row(&hit, &sb),
            "snippet": snippet.as_ref().map(|s| serde_json::json!({
                "path": s.path,
                "start": s.start,
                "end": s.end,
                "truncated": s.truncated,
                "lines": gv::code_lines(&s.code, s.start),
            })),
            "callers": hops(&callers, &sb),
            "callees": hops(&callees, &sb),
            "links": links.iter().map(|l| link_view(l, &sb)).collect::<Vec<_>>(),
            "impact": {
                "risk": impact.risk,
                "risk_key": format!("graph-risk-{}", impact.risk),
                "variant": match impact.risk {
                    "tinggi" => "destructive",
                    "sedang" => "gate",
                    _ => "success",
                },
                "affected": impact.affected.len(),
                "test_count": impact.tests.len(),
                "tests": impact.tests.iter().take(6).map(|t| symbol_row(t, &sb)).collect::<Vec<_>>(),
                "repos": impact.repos.join(", "),
            },
        }),
    )
    .await
}

/// Relasi lintas repo (filter/sort/halaman di memori).
pub static LINKS_TABLE: TableSpec = TableSpec {
    id: "graph-links",
    columns: &[
        ColumnSpec::new("graph-col-from").sort("from"),
        ColumnSpec::new("graph-col-arrow").hide("sm"),
        ColumnSpec::new("graph-col-to").sort("to"),
        ColumnSpec::new("graph-col-kind").sort("kind").hide("sm"),
        ColumnSpec::new("graph-col-confidence").sort("confidence"),
    ],
    filters: &[
        FilterSpec {
            key: "kind",
            label_key: "graph-col-kind",
            icon: "plug-connected",
        },
        FilterSpec {
            key: "confidence",
            label_key: "graph-col-confidence",
            icon: "circle-check",
        },
    ],
    default_sort: "kind",
    default_desc: false,
    default_per: 25,
    search_key: Some("graph-links-search"),
};

/// Kandidat kode mati.
pub static DEAD_TABLE: TableSpec = TableSpec {
    id: "graph-dead",
    columns: &[
        ColumnSpec::new("graph-col-symbol").sort("name"),
        ColumnSpec::new("graph-col-repo").sort("repo").hide("sm"),
        ColumnSpec::new("graph-col-confidence").sort("confidence"),
        ColumnSpec::new("graph-col-actions").right(),
    ],
    filters: &[
        FilterSpec {
            key: "repo",
            label_key: "graph-col-repo",
            icon: "git-branch",
        },
        FilterSpec {
            key: "confidence",
            label_key: "graph-col-confidence",
            icon: "circle-check",
        },
    ],
    default_sort: "confidence",
    default_desc: false,
    default_per: 25,
    search_key: Some("graph-dead-search"),
};

fn confidence_options() -> Vec<FilterOption> {
    ["certain", "inferred", "unmatched", "external"]
        .iter()
        .map(|c| FilterOption::key(c, &format!("graph-conf-{c}")))
        .collect()
}

#[debug_handler]
async fn index_root(State(_ctx): State<AppContext>, access: ProjectAccess) -> Result<Response> {
    access.require(Permission::ViewProject)?;
    format::redirect(&format!("{}/repo", paths(&access).graph_index))
}

#[debug_handler]
#[allow(clippy::too_many_lines)]
async fn index_panel(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: ProjectAccess,
    session: Session,
    Path((_, _, panel)): Path<(String, String, String)>,
    Query(query_pairs): Query<Vec<(String, String)>>,
) -> Result<Response> {
    access.require(Permission::ViewProject)?;
    if !PANELS.iter().any(|(k, ..)| *k == panel) {
        return Err(Error::NotFound);
    }
    let (session, flash) = flash::take(session);
    let base = Base::load(&ctx, &access).await?;
    let wid = access.tenant.workspace_id();
    let pid = access.project.id;
    let sb = symbol_base(&base.paths);
    let here = format!("{}/{panel}", base.paths.graph_index);
    let names: HashMap<Uuid, String> = base.repos.iter().map(|r| (r.id, r.name.clone())).collect();
    let links = query::links(&ctx.db, wid, pid, None).await?;
    let dead_n: u64 = base.rows.iter().map(|r| u64::from(r.stats().dead)).sum();
    let subnav: Vec<NavItem> = PANELS
        .iter()
        .map(|(k, label, icon, desc)| {
            let mut item =
                NavItem::link(k, label, icon, &format!("{}/{k}", base.paths.graph_index))
                    .describe(desc);
            match *k {
                "repo" => item = item.counted(base.repos.len() as u64, false),
                "cross" => item = item.counted(links.len() as u64, false),
                "quality" => item = item.counted(dead_n, false),
                _ => {}
            }
            item.current = *k == panel;
            item
        })
        .collect();
    let mut data = serde_json::json!({
        "panel": panel,
        "subnav": subnav,
        "stats": base.stats(),
        "flash": flash,
    });
    match panel.as_str() {
        "repo" => {
            let mut skipped: std::collections::BTreeMap<String, (u32, Vec<String>)> =
                std::collections::BTreeMap::new();
            for r in &base.rows {
                let repo = names.get(&r.repo_id).cloned().unwrap_or_default();
                for (reason, c) in r.skipped() {
                    let e = skipped.entry(reason).or_default();
                    e.0 += c.count;
                    for ex in c.examples.into_iter().take(2) {
                        if e.1.len() < 3 {
                            e.1.push(format!("{repo}/{ex}"));
                        }
                    }
                }
            }
            let mut langs: Vec<String> = base
                .rows
                .iter()
                .flat_map(|r| r.languages().into_iter().map(|(l, _)| gv::lang_label(&l)))
                .collect();
            langs.sort();
            langs.dedup();
            data["skipped"] = serde_json::json!(
                skipped
                    .into_iter()
                    .map(|(k, (n, ex))| serde_json::json!({
                        "key": format!("graph-skip-{}", k.replace('_', "-")),
                        "count": n,
                        "examples": ex.join(", "),
                    }))
                    .collect::<Vec<_>>()
            );
            data["languages"] = langs.join(", ").into();
            data["scip_repos"] = base
                .rows
                .iter()
                .filter(|r| !r.scip_indexer.is_empty())
                .count()
                .into();
            data["failed"] = base
                .views()
                .into_iter()
                .filter(|r| r.status == "failed")
                .collect::<Vec<_>>()
                .into_iter()
                .map(|r| serde_json::json!({ "name": r.name, "error": r.error }))
                .collect::<Vec<_>>()
                .into();
            data["stale_scip"] = base
                .views()
                .into_iter()
                .filter(|r| r.scip.as_ref().is_some_and(|s| s.stale))
                .map(|r| r.name)
                .collect::<Vec<_>>()
                .into();
        }
        "cross" => {
            let mut rows: Vec<gv::LinkView> = links.iter().map(|l| link_view(l, &sb)).collect();
            let state = LINKS_TABLE.parse(&query_pairs);
            if let Some(q) = state.query() {
                let q = q.to_lowercase();
                rows.retain(|l| {
                    format!(
                        "{} {} {} {}",
                        l.from_name, l.from_repo, l.to_name, l.to_repo
                    )
                    .to_lowercase()
                    .contains(&q)
                });
            }
            let kinds = state.filter("kind").to_vec();
            if !kinds.is_empty() {
                rows.retain(|l| kinds.contains(&l.kind));
            }
            let confs = state.filter("confidence").to_vec();
            if !confs.is_empty() {
                rows.retain(|l| confs.contains(&l.confidence));
            }
            match state.sort {
                "from" => rows.sort_by(|a, b| a.from_name.cmp(&b.from_name)),
                "to" => rows.sort_by(|a, b| a.to_name.cmp(&b.to_name)),
                "confidence" => rows.sort_by(|a, b| a.confidence.cmp(&b.confidence)),
                _ => rows.sort_by(|a, b| a.kind.cmp(&b.kind)),
            }
            if state.desc {
                rows.reverse();
            }
            let total = rows.len() as u64;
            let page: Vec<gv::LinkView> = rows
                .into_iter()
                .skip(usize::try_from(state.offset()).unwrap_or(0))
                .take(usize::try_from(state.per).unwrap_or(25))
                .collect();
            let count = |c: &str| links.iter().filter(|l| l.confidence == c).count();
            data["link_stats"] = serde_json::json!([
                Stat::new("graph-stat-links", links.len(), "arrows-exchange")
                    .hint("graph-stat-links-hint", base.repos.len()),
                Stat::new("graph-stat-conf-certain", count("certain"), "circle-check")
                    .tone("success"),
                Stat::new("graph-stat-conf-inferred", count("inferred"), "help-circle")
                    .tone("warn"),
                Stat::new(
                    "graph-stat-conf-unmatched",
                    count("unmatched"),
                    "alert-triangle"
                )
                .tone(if count("unmatched") > 0 { "danger" } else { "" }),
            ]);
            data["table"] = serde_json::to_value(
                TableView::new(&LINKS_TABLE, &state, &here, total)
                    .with_filter_options(
                        "kind",
                        ["http", "event", "package"]
                            .iter()
                            .map(|k| FilterOption::key(k, &format!("graph-link-{k}")))
                            .collect(),
                    )
                    .with_filter_options("confidence", confidence_options()),
            )
            .unwrap_or_default();
            data["links"] = serde_json::to_value(page).unwrap_or_default();
        }
        "git" => {
            let mut hot = Vec::new();
            let mut coupling = Vec::new();
            let mut owners = Vec::new();
            let mut window = 90;
            let mut commits = 0;
            for r in &base.rows {
                let g = r.git();
                if g.commits == 0 {
                    continue;
                }
                window = g.window_days;
                commits += g.commits;
                let repo = names.get(&r.repo_id).cloned().unwrap_or_default();
                let max = g.hotspots.first().map_or(1, |h| h.commits.max(1));
                for h in g.hotspots.iter().take(8) {
                    hot.push(serde_json::json!({
                        "repo": repo, "path": h.path, "commits": h.commits,
                        "churn": h.churn, "author": h.top_author,
                        "width": h.commits * 100 / max,
                    }));
                }
                for c in g.coupling.iter().take(8) {
                    coupling.push(serde_json::json!({ "repo": repo, "a": c.a, "b": c.b, "percent": c.percent, "together": c.together }));
                }
                for o in g.owners.iter().take(8) {
                    owners.push(serde_json::json!({ "repo": repo, "module": o.module, "author": o.author, "percent": o.percent, "commits": o.commits }));
                }
            }
            data["git"] = serde_json::json!({
                "window": window, "commits": commits,
                "hotspots": hot, "coupling": coupling, "owners": owners,
            });
        }
        "quality" => {
            let dead = query::dead_code(&ctx.db, wid, pid, None, 500).await?;
            let mut rows: Vec<SymbolRow> = dead.iter().map(|d| symbol_row(d, &sb)).collect();
            let state = DEAD_TABLE.parse(&query_pairs);
            if let Some(q) = state.query() {
                let q = q.to_lowercase();
                rows.retain(|r| {
                    format!("{} {} {}", r.qualified, r.path, r.repo)
                        .to_lowercase()
                        .contains(&q)
                });
            }
            let repos_f = state.filter("repo").to_vec();
            if !repos_f.is_empty() {
                rows.retain(|r| repos_f.contains(&r.repo));
            }
            let confs = state.filter("confidence").to_vec();
            if !confs.is_empty() {
                rows.retain(|r| confs.contains(&r.dead));
            }
            match state.sort {
                "name" => rows.sort_by(|a, b| a.qualified.cmp(&b.qualified)),
                "repo" => rows.sort_by(|a, b| a.repo.cmp(&b.repo)),
                _ => {}
            }
            if state.desc {
                rows.reverse();
            }
            let total = rows.len() as u64;
            let page: Vec<SymbolRow> = rows
                .into_iter()
                .skip(usize::try_from(state.offset()).unwrap_or(0))
                .take(usize::try_from(state.per).unwrap_or(25))
                .collect();
            let todos: u32 = base.rows.iter().map(|r| r.stats().todos).sum();
            let complex = query::complex(&ctx.db, wid, pid, None, 10).await?;
            let mut repo_names: Vec<String> = base.repos.iter().map(|r| r.name.clone()).collect();
            repo_names.sort();
            data["quality_stats"] = serde_json::json!([
                Stat::new("graph-stat-dead", dead.len(), "trash").tone("warn"),
                Stat::new("graph-stat-todos", todos, "list-check"),
                Stat::new("graph-stat-complex", complex.len(), "alert-triangle")
                    .tone(if complex.is_empty() { "" } else { "danger" })
                    .hint("graph-stat-complex-hint", svc::build::COMPLEX_THRESHOLD),
            ]);
            data["table"] = serde_json::to_value(
                TableView::new(&DEAD_TABLE, &state, &here, total)
                    .with_filter_options(
                        "repo",
                        repo_names.iter().map(|r| FilterOption::raw(r, r)).collect(),
                    )
                    .with_filter_options(
                        "confidence",
                        ["certain", "inferred"]
                            .iter()
                            .map(|c| FilterOption::key(c, &format!("graph-conf-{c}")))
                            .collect(),
                    ),
            )
            .unwrap_or_default();
            data["dead"] = serde_json::to_value(page).unwrap_or_default();
            data["complex"] = serde_json::to_value(
                complex
                    .iter()
                    .map(|c| symbol_row(c, &sb))
                    .collect::<Vec<_>>(),
            )
            .unwrap_or_default();
        }
        _ => {
            // Benchmark korpus call graph (hand-written, 10 bahasa), dihitung saat dibuka.
            let report = tokio::task::spawn_blocking(agentgate_graph::bench::run)
                .await
                .map_err(|e| Error::string(&e.to_string()))?;
            data["bench"] = serde_json::json!({
                "recall": pct(report.recall),
                "precision": pct(report.precision),
                "files": report.files,
                "millis": pct(report.micros as f64 / 1000.0),
                "pass": report.recall >= 95.0 && report.precision >= 90.0,
                "langs": report.langs.iter().map(|l| serde_json::json!({
                    "lang": gv::lang_label(&l.lang),
                    "expected": l.expected,
                    "recall": pct(l.recall()),
                    "recall_width": l.recall().round() as u32,
                    "precision": pct(l.precision()),
                    "missed": l.missed.join(", "),
                })).collect::<Vec<_>>(),
            });
        }
    }
    let res = render(
        &ctx,
        &v,
        &access,
        &base,
        ("graph/index.html", "index", &here),
        data,
    )
    .await?;
    Ok((session, res).into_response())
}

#[derive(Debug, Deserialize)]
struct ReindexForm {
    #[serde(default)]
    scope: String,
    #[serde(default)]
    mode: String,
}

#[debug_handler]
async fn reindex(
    State(ctx): State<AppContext>,
    access: ProjectAccess,
    session: Session,
    Form(form): Form<ReindexForm>,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    let base = Base::load(&ctx, &access).await?;
    let wid = access.tenant.workspace_id();
    let targets: Vec<&repos::Model> = base
        .repos
        .iter()
        .filter(|r| form.scope.is_empty() || form.scope == "all" || r.id.to_string() == form.scope)
        .collect();
    if targets.is_empty() {
        return Err(Error::NotFound);
    }
    let full = form.mode == "full";
    let mut queued = 0;
    for repo in targets {
        if repo.connection_id.is_none() {
            svc::ingest::fail(&ctx.db, wid, repo, svc::fetch::NOT_CONNECTED_MESSAGE).await?;
            continue;
        }
        svc::ingest::queue(&ctx.db, wid, repo, Some(access.tenant.user.id)).await?;
        svc::enqueue(&ctx, repo, Some(access.tenant.user.id), full).await?;
        queued += 1;
    }
    let key = if queued > 0 {
        "graph-flash-queued"
    } else {
        "graph-flash-not-connected"
    };
    let session = flash::set(session, &Flash::success(key))?;
    let to = format!("{}/repo", base.paths.graph_index);
    Ok((session, format::redirect(&to)?).into_response())
}

pub fn routes() -> Routes {
    Routes::new()
        .add("/w/{slug}/p/{key}/graph", get(search))
        .add("/w/{slug}/p/{key}/graph/symbol/{id}", get(symbol))
        .add("/w/{slug}/p/{key}/graph/index", get(index_root))
        .add("/w/{slug}/p/{key}/graph/index/{panel}", get(index_panel))
        .add("/w/{slug}/p/{key}/graph/reindex", post(reindex))
}

/// Hasil query service → baris tampilan (views tidak boleh memakai services).
fn symbol_row(h: &SymbolHit, symbol_base: &str) -> SymbolRow {
    let kind = h.kind();
    SymbolRow {
        id: h.id,
        href: format!("{symbol_base}/{}", h.id),
        name: h.name.clone(),
        qualified: h.qualified.clone(),
        kind: h.kind.clone(),
        kind_key: format!("graph-kind-{}", h.kind),
        icon: gv::kind_icon(kind),
        repo: h.repo.clone(),
        path: h.path.clone(),
        line: h.start_line,
        end: h.end_line,
        signature: h.signature.clone(),
        doc: h
            .doc
            .lines()
            .next()
            .unwrap_or_default()
            .chars()
            .take(160)
            .collect(),
        dead: h.dead.clone(),
        test: h.test,
        exported: h.exported,
        callers: h.callers,
        callees: h.callees,
        complexity: h.complexity,
        module: h.module.clone(),
        lang: gv::lang_label(&h.lang),
    }
}

fn hops(list: &[Hop], symbol_base: &str) -> Vec<gv::HopRow> {
    list.iter()
        .map(|h| gv::HopRow {
            symbol: symbol_row(&h.symbol, symbol_base),
            depth: h.depth,
            confidence: h.confidence,
            via_key: format!("graph-via-{}", h.via),
            via: h.via.clone(),
        })
        .collect()
}

fn link_view(l: &LinkRow, symbol_base: &str) -> gv::LinkView {
    let href = |id: Option<Uuid>| id.map(|i| format!("{symbol_base}/{i}"));
    let to_name = match (&l.to_name, l.confidence.as_str()) {
        (Some(n), _) if l.kind != "http" => n.clone(),
        _ => l.detail.clone(),
    };
    gv::LinkView {
        kind: l.kind.clone(),
        kind_key: format!("graph-link-{}", l.kind),
        confidence_key: format!("graph-conf-{}", l.confidence),
        confidence: l.confidence.clone(),
        from_repo: l.from_repo.clone(),
        from_name: l.from_name.clone().unwrap_or_else(|| l.from_repo.clone()),
        from_href: href(l.from_symbol),
        to_repo: l.to_repo.clone().unwrap_or_default(),
        to_name,
        to_href: href(l.to_symbol),
    }
}
