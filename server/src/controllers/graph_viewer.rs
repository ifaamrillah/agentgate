//! Project → Code graph → **Viewer** (M10, `ViewProject`): `GET /w/{slug}/p/{key}/graph/viewer`.
//!
//! Semua status di query string (GET murni, bisa dibagikan): lingkup level (`repo`, `module`,
//! `file`, `sym`), node terpilih (`sel`), sorot tiket (`tk`), analisis dampak (`imp=1`), filter
//! layer/bahasa yang disembunyikan (`hide`, `lang`), dan cari (`q` → buka simbol teratas di level
//! Fungsi). Tanpa JS semuanya tautan; `graph-viewer.js` menambah geser/zoom roda, klik ganda zoom
//! masuk, dan `/` untuk fokus cari.
use std::collections::{BTreeSet, HashSet};

use agentgate_graph::viewer::{Layer, NODES_MAX};
use loco_rs::prelude::*;
use serde::Deserialize;
use uuid::Uuid;

use super::graph::{Base, render};
use crate::{
    controllers::extractors::ProjectAccess,
    models::permission::Permission,
    services::graph::{
        query::{self, SearchFilter},
        viewer::{self as svc, Scope},
    },
    views::graph_viewer::{self as gvv, FilterChip, Marks},
};

#[derive(Debug, Default, Deserialize)]
pub(super) struct ViewerQuery {
    #[serde(default)]
    repo: String,
    #[serde(default)]
    module: String,
    #[serde(default)]
    file: String,
    #[serde(default)]
    sym: String,
    #[serde(default)]
    sel: String,
    #[serde(default)]
    tk: String,
    #[serde(default)]
    imp: String,
    #[serde(default)]
    hide: String,
    #[serde(default)]
    lang: String,
    #[serde(default)]
    q: String,
}

fn some(s: &str) -> Option<String> {
    let s = s.trim();
    (!s.is_empty()).then(|| s.chars().take(400).collect())
}

/// Status tampilan yang ikut di semua tautan.
#[derive(Debug, Clone, Default)]
struct ViewState {
    tk: Option<String>,
    imp: bool,
    hide: BTreeSet<String>,
    lang: BTreeSet<String>,
}

impl ViewState {
    fn pairs(&self) -> Vec<(&'static str, String)> {
        let mut p = Vec::new();
        if let Some(t) = &self.tk {
            p.push(("tk", t.clone()));
        }
        if self.imp {
            p.push(("imp", "1".into()));
        }
        if !self.hide.is_empty() {
            p.push((
                "hide",
                self.hide.iter().cloned().collect::<Vec<_>>().join(","),
            ));
        }
        if !self.lang.is_empty() {
            p.push((
                "lang",
                self.lang.iter().cloned().collect::<Vec<_>>().join(","),
            ));
        }
        p
    }
}

struct Links {
    base: String,
    scope: String,
}

impl Links {
    fn href(&self, scope: &str, sel: Option<&str>, state: &ViewState) -> String {
        let mut parts: Vec<String> = Vec::new();
        if !scope.is_empty() {
            parts.push(scope.to_string());
        }
        let mut pairs = state.pairs();
        if let Some(s) = sel {
            pairs.push(("sel", s.to_string()));
        }
        let extra = serde_urlencoded::to_string(pairs).unwrap_or_default();
        if !extra.is_empty() {
            parts.push(extra);
        }
        if parts.is_empty() {
            self.base.clone()
        } else {
            format!("{}?{}", self.base, parts.join("&"))
        }
    }

    fn here(&self, sel: Option<&str>, state: &ViewState) -> String {
        self.href(&self.scope, sel, state)
    }
}

fn toggle(set: &BTreeSet<String>, key: &str) -> BTreeSet<String> {
    let mut s = set.clone();
    if !s.remove(key) {
        s.insert(key.to_string());
    }
    s
}

#[debug_handler]
#[allow(clippy::too_many_lines)]
pub(super) async fn page(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: ProjectAccess,
    Query(vq): Query<ViewerQuery>,
) -> Result<Response> {
    access.require(Permission::ViewProject)?;
    let base = Base::load(&ctx, &access).await?;
    let wid = access.tenant.workspace_id();
    let pid = access.project.id;
    let viewer_href = format!("{}/viewer", base.paths.graph);
    let state = ViewState {
        tk: some(&vq.tk),
        imp: vq.imp == "1",
        hide: vq
            .hide
            .split(',')
            .filter(|h| Layer::parse(h).is_some())
            .map(str::to_string)
            .collect(),
        lang: vq
            .lang
            .split(',')
            .filter(|l| !l.trim().is_empty())
            .take(12)
            .map(|l| l.trim().to_string())
            .collect(),
    };

    if !base.indexed() {
        return render(
            &ctx,
            &v,
            &access,
            &base,
            ("graph/viewer.html", "viewer", &viewer_href),
            serde_json::json!({ "level": "project" }),
        )
        .await;
    }

    // Cari: buka simbol teratas di level Fungsi.
    let q = vq.q.trim().chars().take(200).collect::<String>();
    let mut search_empty = false;
    if !q.is_empty() {
        let hits = query::search(&ctx.db, wid, pid, &q, &SearchFilter::default(), 1).await?;
        if let Some(h) = hits.first() {
            let scope = Scope {
                repo: Some(h.repo.clone()),
                module: Some(agentgate_graph::build::module_of(&h.path)),
                file: Some(h.path.clone()),
                symbol: Some(h.id),
            };
            let links = Links {
                base: viewer_href.clone(),
                scope: scope.query(),
            };
            return format::redirect(&links.here(None, &state));
        }
        search_empty = true;
    }

    let scope = Scope {
        repo: some(&vq.repo),
        module: some(&vq.module),
        file: some(&vq.file),
        symbol: some(&vq.sym).and_then(|s| Uuid::parse_str(&s).ok()),
    };
    let loaded = svc::load(&ctx.db, wid, pid, &access.project.name, &scope)
        .await?
        .ok_or(Error::NotFound)?;
    let level = loaded.level;
    let links = Links {
        base: viewer_href.clone(),
        scope: scope.query(),
    };
    let mut graph = loaded.graph;

    // Filter chip dari isi level sebelum disaring.
    let layers_present: BTreeSet<&'static str> =
        graph.nodes.iter().map(|n| n.layer.as_str()).collect();
    let langs_present: BTreeSet<String> = graph
        .nodes
        .iter()
        .filter(|n| !n.lang.is_empty() && n.layer != Layer::Doc)
        .map(|n| n.lang.clone())
        .collect();
    let sel_param = some(&vq.sel);
    let mut layer_chips: Vec<FilterChip> = Vec::new();
    for l in Layer::ALL {
        if !layers_present.contains(l.as_str()) {
            continue;
        }
        let next = ViewState {
            hide: toggle(&state.hide, l.as_str()),
            ..state.clone()
        };
        layer_chips.push(FilterChip {
            key: l.as_str().to_string(),
            label_key: Some(gvv::layer_key(l)),
            label: String::new(),
            color: gvv::layer_color(l),
            on: !state.hide.contains(l.as_str()),
            href: links.here(sel_param.as_deref(), &next),
        });
    }
    let lang_chips: Vec<FilterChip> = langs_present
        .iter()
        .map(|lang| {
            let next = ViewState {
                lang: toggle(&state.lang, lang),
                ..state.clone()
            };
            FilterChip {
                key: lang.clone(),
                label_key: None,
                label: lang.clone(),
                color: gvv::lang_color(lang),
                on: !state.lang.contains(lang),
                href: links.here(sel_param.as_deref(), &next),
            }
        })
        .collect();

    let hidden_layers: Vec<Layer> = state.hide.iter().filter_map(|h| Layer::parse(h)).collect();
    let hidden_langs: Vec<String> = state.lang.iter().cloned().collect();
    graph.filter(&hidden_layers, &hidden_langs);
    let keep: Vec<&str> = [sel_param.as_deref(), loaded.focus.as_deref()]
        .into_iter()
        .flatten()
        .collect();
    graph.prune(NODES_MAX, &keep);
    graph.layout();

    let selected: Option<String> = sel_param
        .filter(|s| graph.node(s).is_some())
        .or_else(|| loaded.focus.clone().filter(|f| graph.node(f).is_some()));

    // Sorot tiket.
    let tickets = svc::ticket_files(&ctx.db, wid, pid).await?;
    let ticket = state
        .tk
        .as_ref()
        .and_then(|code| tickets.iter().find(|t| &t.code == code));
    let touched = ticket.map_or_else(HashSet::new, |t| svc::touched(&graph, &t.files, level));
    let impact = if state.imp {
        selected.as_ref().map(|s| graph.impact(s))
    } else {
        None
    };

    let level_label = format!("gv-level-{}", level.as_str());
    let marks = Marks {
        selected: selected.as_deref(),
        touched: touched.clone(),
        impact: impact.clone(),
    };
    let svg = gvv::svg(
        &graph,
        &marks,
        &format!("Graph level {}", level.as_str()),
        &|id| links.here(Some(id), &state),
        &|q| links.href(q, None, &state),
    );

    // Panel node terpilih.
    let docs = svc::docs(&ctx.db, wid, pid).await?;
    let knowledge_base = base.paths.knowledge.clone();
    let symbol_base = format!("{}/symbol", base.paths.graph);
    let panel = selected.as_ref().and_then(|id| graph.node(id)).map(|n| {
        let rel = |list: Vec<(&agentgate_graph::viewer::Node, bool)>| -> Vec<serde_json::Value> {
            list.into_iter()
                .map(|(o, certain)| {
                    serde_json::json!({
                        "label": o.label,
                        "href": links.here(Some(&o.id), &state),
                        "certain": certain,
                    })
                })
                .collect()
        };
        let name = n.label.trim_end_matches("()").to_lowercase();
        let stem = name.split('.').next().unwrap_or(&name).to_string();
        let node_docs: Vec<serde_json::Value> = if n.layer == Layer::Doc {
            Vec::new()
        } else {
            docs.iter()
                .filter(|d| stem.len() >= 3 && d.body.to_lowercase().contains(&stem))
                .take(5)
                .map(|d| serde_json::json!({ "title": d.title, "href": format!("{knowledge_base}/{}", d.slug) }))
                .collect()
        };
        let node_tickets: Vec<serde_json::Value> = tickets
            .iter()
            .filter(|t| svc::touched(&graph, &t.files, level).contains(&n.id))
            .take(8)
            .map(|t| {
                serde_json::json!({
                    "code": t.code,
                    "title": t.title,
                    "href": crate::views::tickets::ticket_href(&access.tenant.workspace.slug, &t.code),
                })
            })
            .collect();
        let impact_list: Vec<String> = impact
            .as_ref()
            .map(|set| {
                graph
                    .nodes
                    .iter()
                    .filter(|o| set.contains(&o.id))
                    .map(|o| o.label.clone())
                    .collect()
            })
            .unwrap_or_default();
        serde_json::json!({
            "id": n.id,
            "label": n.label,
            "sub": n.sub,
            "kind_key": if n.kind_key.starts_with("graph-kind-") || n.kind_key.starts_with("gv-kind-") { n.kind_key.clone() } else { String::new() },
            "layer_key": gvv::layer_key(n.layer),
            "layer_color": gvv::layer_color(n.layer),
            "lang": n.lang,
            "path": n.path,
            "dead": n.dead,
            "drill": n.drill.as_deref().map(|q| links.href(q, None, &state)),
            "code": n.symbol.as_ref().map(|s| format!("{symbol_base}/{s}")),
            "doc": (n.layer == Layer::Doc).then(|| format!("{knowledge_base}/{}", n.path)),
            "impact_href": links.here(Some(&n.id), &ViewState { imp: !state.imp, ..state.clone() }),
            "impact": impact_list,
            "callers": rel(graph.incoming(&n.id)),
            "callees": rel(graph.outgoing(&n.id)),
            "tickets": node_tickets,
            "docs": node_docs,
        })
    });

    let crumbs: Vec<serde_json::Value> = loaded
        .crumbs
        .iter()
        .map(|c| {
            serde_json::json!({
                "label": c.label,
                "level_key": format!("gv-level-{}", c.level.as_str()),
                "href": links.href(&c.query, None, &state),
                "current": c.level == level,
            })
        })
        .collect();
    let up = loaded
        .crumbs
        .iter()
        .rev()
        .find(|c| c.level.index() < level.index())
        .map(|c| links.href(&c.query, None, &state));
    let ticket_menu: Vec<serde_json::Value> = tickets
        .iter()
        .map(|t| {
            serde_json::json!({
                "code": t.code,
                "title": t.title,
                "href": links.here(selected.as_deref(), &ViewState { tk: Some(t.code.clone()), ..state.clone() }),
                "current": state.tk.as_deref() == Some(t.code.as_str()),
            })
        })
        .collect();
    let clear_ticket = links.here(
        selected.as_deref(),
        &ViewState {
            tk: None,
            ..state.clone()
        },
    );
    let clear_impact = links.here(
        selected.as_deref(),
        &ViewState {
            imp: false,
            ..state.clone()
        },
    );
    let impact_toggle = links.here(
        selected.as_deref(),
        &ViewState {
            imp: !state.imp,
            ..state.clone()
        },
    );

    render(
        &ctx,
        &v,
        &access,
        &base,
        ("graph/viewer.html", "viewer", &links.here(selected.as_deref(), &state)),
        serde_json::json!({
            "level": level.as_str(),
            "level_label": level_label,
            "svg": svg,
            "node_count": graph.nodes.len(),
            "edge_count": graph.edges.len(),
            "more": graph.more,
            "crumbs": crumbs,
            "zoom_in": panel.as_ref().and_then(|p| p["drill"].as_str().map(str::to_string)),
            "zoom_out": up,
            "panel": panel,
            "layer_chips": layer_chips,
            "lang_chips": lang_chips,
            "ticket": ticket.map(|t| serde_json::json!({ "code": t.code, "title": t.title, "count": touched.len() })),
            "ticket_menu": ticket_menu,
            "clear_ticket": clear_ticket,
            "impact_on": impact.is_some(),
            "impact_toggle": impact_toggle,
            "clear_impact": clear_impact,
            "can_impact": selected.is_some(),
            "q": q,
            "search_empty": search_empty,
            "viewer_href": viewer_href,
            "scope": {
                "repo": scope.repo,
                "module": scope.module,
                "file": scope.file,
                "sym": scope.symbol.map(|s| s.to_string()),
            },
            "state": {
                "tk": state.tk,
                "imp": state.imp,
                "hide": state.hide.iter().cloned().collect::<Vec<_>>().join(","),
                "lang": state.lang.iter().cloned().collect::<Vec<_>>().join(","),
            },
        }),
    )
    .await
}
