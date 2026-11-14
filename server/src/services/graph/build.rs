//! Bangun ulang graph satu repo dari `graph_files` (fakta tree-sitter tersimpan, tanpa parsing
//! ulang) plus SCIP bila ada, lalu hitung ulang relasi lintas repo project. Simbol dan edge
//! ditulis ulang dalam satu transaksi: pembaca melihat graph lama atau baru, tidak campuran.
use std::collections::BTreeMap;

use agentgate_graph::{
    FileFacts, SymbolKind,
    build::{
        self as graph_build, Confidence, RepoGraph, SourceFile, Surface, SurfaceEvent, SurfaceHttp,
        SurfaceRoute,
    },
    cross,
    protocol::words,
};
use sea_orm::{
    ActiveEnum, ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, DbErr,
    EntityTrait, IntoActiveModel, QueryFilter, QueryOrder, QuerySelect, TransactionTrait,
};
use uuid::Uuid;

use super::{ensure_row, now};
use crate::models::{
    graph_edges, graph_files, graph_links,
    graph_repos::{self, IndexStatus, LangCount, ModuleRow, RepoStats, SkipCount, SurfaceRow},
    graph_symbols, repos,
    workspaces::WorkspaceId,
};

/// Kompleksitas siklomatik di atas ini dihitung "kompleks" (panel Kualitas kode).
pub const COMPLEX_THRESHOLD: u16 = 15;
const SKIP_EXAMPLES: usize = 5;

type FileRow = (Uuid, String, String, i32, String, serde_json::Value);

/// Hasil build siap tulis.
struct Built {
    graph: RepoGraph,
    files: Vec<SourceFile>,
    file_ids: Vec<Uuid>,
    node_ids: Vec<Uuid>,
    languages: BTreeMap<String, LangCount>,
    skipped: BTreeMap<String, SkipCount>,
    stats: RepoStats,
    skipped_count: i32,
}

fn compute(rows: Vec<FileRow>, scip: Option<&graph_build::ScipData>) -> Built {
    let mut files = Vec::new();
    let mut file_ids = Vec::new();
    let mut languages: BTreeMap<String, LangCount> = BTreeMap::new();
    let mut skipped: BTreeMap<String, SkipCount> = BTreeMap::new();
    let mut stats = RepoStats::default();
    let mut skipped_count = 0;
    for (id, path, lang, lines, skip, facts) in rows {
        if !skip.is_empty() {
            skipped_count += 1;
            let s = skipped.entry(skip).or_default();
            s.count += 1;
            if s.examples.len() < SKIP_EXAMPLES {
                s.examples.push(path);
            }
            continue;
        }
        let Ok(facts) = serde_json::from_value::<FileFacts>(facts) else {
            skipped_count += 1;
            let s = skipped.entry("parse_failed".into()).or_default();
            s.count += 1;
            if s.examples.len() < SKIP_EXAMPLES {
                s.examples.push(path);
            }
            continue;
        };
        if facts.lang.as_str() != "manifest" {
            let l = languages.entry(lang).or_default();
            l.files += 1;
            l.lines += u32::try_from(lines).unwrap_or(0);
        }
        stats.lines += facts.lines;
        stats.todos += facts.todos;
        if facts.errors > 0 {
            stats.parse_errors += 1;
        }
        stats.complex += u32::try_from(
            facts
                .symbols
                .iter()
                .filter(|s| s.kind.is_callable() && s.complexity > COMPLEX_THRESHOLD)
                .count(),
        )
        .unwrap_or(0);
        files.push(SourceFile { path, facts });
        file_ids.push(id);
    }
    let graph = graph_build::build(&files, scip);
    stats.build = graph.stats;
    stats.dead =
        u32::try_from(graph.nodes.iter().filter(|n| n.dead.is_some()).count()).unwrap_or(0);
    let node_ids = graph.nodes.iter().map(|_| Uuid::now_v7()).collect();
    Built {
        graph,
        files,
        file_ids,
        node_ids,
        languages,
        skipped,
        stats,
        skipped_count,
    }
}

fn symbol_rows(b: &Built, wid: WorkspaceId, repo: Uuid) -> Vec<graph_symbols::ActiveModel> {
    b.graph
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let file = &b.files[n.file as usize];
            let fact = n.symbol.and_then(|s| file.facts.symbols.get(s as usize));
            let w = if n.kind == SymbolKind::File {
                words(&[&file.path])
            } else {
                words(&[&n.qualified])
            };
            graph_symbols::ActiveModel {
                id: Set(b.node_ids[i]),
                workspace_id: Set(wid.get()),
                repo_id: Set(repo),
                file_id: Set(b.file_ids[n.file as usize]),
                name: Set(n.name.chars().take(300).collect()),
                qualified: Set(n.qualified.chars().take(600).collect()),
                kind: Set(n.kind.as_str().to_string()),
                start_line: Set(i32::try_from(n.start).unwrap_or(i32::MAX)),
                end_line: Set(i32::try_from(n.end).unwrap_or(i32::MAX)),
                exported: Set(n.exported),
                test: Set(n.test),
                module: Set(n.module.clone()),
                central: Set(n.central),
                dead: Set(n.dead.map_or("", Confidence::as_str).to_string()),
                callers: Set(i32::try_from(n.callers).unwrap_or(i32::MAX)),
                callees: Set(i32::try_from(n.callees).unwrap_or(i32::MAX)),
                complexity: Set(fact.map_or(0, |f| i32::from(f.complexity))),
                signature: Set(fact.map(|f| f.signature.clone()).unwrap_or_default()),
                doc: Set(fact
                    .map(|f| f.doc.chars().take(2000).collect())
                    .unwrap_or_default()),
                words: Set(w),
                embed_hash: Set(String::new()),
            }
        })
        .collect()
}

fn surface_row(b: &Built) -> SurfaceRow {
    let id = |n: u32| b.node_ids[n as usize];
    let s = &b.graph.surface;
    SurfaceRow {
        routes: s
            .routes
            .iter()
            .map(|r| (id(r.node), r.method.clone(), r.path.clone()))
            .collect(),
        http: s
            .http
            .iter()
            .map(|h| {
                (
                    id(h.node),
                    h.method.clone(),
                    h.path.clone(),
                    h.exact,
                    h.host.clone(),
                )
            })
            .collect(),
        events: s
            .events
            .iter()
            .map(|e| (id(e.node), e.topic.clone(), e.publish))
            .collect(),
        packages: s.packages.clone(),
        deps: s.deps.clone(),
    }
}

/// Bangun ulang graph `repo` lalu relasi lintas repo project-nya.
///
/// # Errors
///
/// Error database.
pub async fn rebuild(
    db: &DatabaseConnection,
    wid: WorkspaceId,
    repo: &repos::Model,
) -> Result<graph_repos::Model, DbErr> {
    let row = ensure_row(db, wid, repo).await?;
    let rows: Vec<FileRow> = graph_files::Entity::find()
        .select_only()
        .column(graph_files::Column::Id)
        .column(graph_files::Column::Path)
        .column(graph_files::Column::Lang)
        .column(graph_files::Column::Lines)
        .column(graph_files::Column::SkipReason)
        .column(graph_files::Column::Facts)
        .filter(graph_files::Column::WorkspaceId.eq(wid.get()))
        .filter(graph_files::Column::RepoId.eq(repo.id))
        .order_by_asc(graph_files::Column::Path)
        .into_tuple()
        .all(db)
        .await?;
    let scip = row.scip();
    let built = tokio::task::spawn_blocking(move || compute(rows, scip.as_ref()))
        .await
        .map_err(|e| DbErr::Custom(format!("build task: {e}")))?;

    let symbols = symbol_rows(&built, wid, repo.id);
    let edges: Vec<graph_edges::ActiveModel> = built
        .graph
        .edges
        .iter()
        .map(|e| graph_edges::ActiveModel {
            id: Set(Uuid::now_v7()),
            workspace_id: Set(wid.get()),
            repo_id: Set(repo.id),
            from_id: Set(built.node_ids[e.from as usize]),
            to_id: Set(built.node_ids[e.to as usize]),
            kind: Set(e.kind.as_str().to_string()),
            confidence: Set(e.confidence.as_str().to_string()),
            source: Set(e.source.as_str().to_string()),
            line: Set(i32::try_from(e.line).unwrap_or(0)),
        })
        .collect();
    let modules: Vec<ModuleRow> = built
        .graph
        .modules
        .iter()
        .map(|m| ModuleRow {
            name: m.name.clone(),
            files: m.files,
            symbols: m.symbols,
            central: m.central.map(|c| built.node_ids[c as usize]),
            central_name: m
                .central
                .map(|c| built.graph.nodes[c as usize].qualified.clone())
                .unwrap_or_default(),
        })
        .collect();
    let symbol_count = built
        .graph
        .nodes
        .iter()
        .filter(|n| n.kind != SymbolKind::File)
        .count();
    let certain = built
        .graph
        .edges
        .iter()
        .filter(|e| e.confidence == Confidence::Certain)
        .count();
    let to_i32 = |n: usize| i32::try_from(n).unwrap_or(i32::MAX);

    let txn = db.begin().await?;
    graph_symbols::Entity::delete_many()
        .filter(graph_symbols::Column::WorkspaceId.eq(wid.get()))
        .filter(graph_symbols::Column::RepoId.eq(repo.id))
        .exec(&txn)
        .await?;
    for chunk in symbols.chunks(1000) {
        graph_symbols::Entity::insert_many(chunk.to_vec())
            .exec_without_returning(&txn)
            .await?;
    }
    for chunk in edges.chunks(2000) {
        graph_edges::Entity::insert_many(chunk.to_vec())
            .exec_without_returning(&txn)
            .await?;
    }
    let mut am = row.into_active_model();
    am.status = Set(IndexStatus::Ready.to_value());
    am.files_indexed = Set(to_i32(built.files.len()));
    am.files_skipped = Set(built.skipped_count);
    am.files_parsed = Set(to_i32(
        built.files.iter().filter(|f| f.facts.errors == 0).count(),
    ));
    am.symbols = Set(to_i32(symbol_count));
    am.edges = Set(to_i32(built.graph.edges.len()));
    am.edges_certain = Set(to_i32(certain));
    am.stats = Set(serde_json::to_value(built.stats).unwrap_or_default());
    am.languages = Set(serde_json::to_value(&built.languages).unwrap_or_default());
    am.skipped = Set(serde_json::to_value(&built.skipped).unwrap_or_default());
    am.modules = Set(serde_json::to_value(&modules).unwrap_or_default());
    am.surface = Set(serde_json::to_value(surface_row(&built)).unwrap_or_default());
    am.error = Set(String::new());
    am.indexed_at = Set(Some(now()));
    let row = am.update(&txn).await?;
    relink(&txn, wid, repo.project_id).await?;
    crate::services::knowledge::triggers::on_graph_rebuilt(&txn, wid, repo).await?;
    txn.commit().await?;
    Ok(row)
}

/// Hitung ulang relasi lintas repo project dari permukaan tersimpan tiap repo.
///
/// # Errors
///
/// Error database.
pub async fn relink<C: sea_orm::ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project: Uuid,
) -> Result<usize, DbErr> {
    let rows = graph_repos::Model::of_project(db, wid, project).await?;
    // Id simbol lokal per repo → indeks node untuk `cross::link`.
    let mut ids: Vec<Vec<Uuid>> = Vec::new();
    let mut surfaces: Vec<Surface> = Vec::new();
    for r in &rows {
        let s = r.surface();
        let mut local: Vec<Uuid> = Vec::new();
        let mut ix = |id: Uuid| {
            local.push(id);
            u32::try_from(local.len() - 1).unwrap_or(0)
        };
        let routes = s
            .routes
            .iter()
            .map(|(id, m, p)| SurfaceRoute {
                node: ix(*id),
                method: m.clone(),
                path: p.clone(),
            })
            .collect();
        let http = s
            .http
            .iter()
            .map(|(id, m, p, exact, host)| SurfaceHttp {
                node: ix(*id),
                method: m.clone(),
                path: p.clone(),
                exact: *exact,
                host: host.clone(),
            })
            .collect();
        let events = s
            .events
            .iter()
            .map(|(id, topic, publish)| SurfaceEvent {
                node: ix(*id),
                topic: topic.clone(),
                publish: *publish,
            })
            .collect();
        surfaces.push(Surface {
            routes,
            http,
            events,
            packages: s.packages,
            deps: s.deps,
        });
        ids.push(local);
    }
    let input: Vec<(usize, &Surface)> = surfaces.iter().enumerate().collect();
    let links = cross::link(&input);
    graph_links::Entity::delete_many()
        .filter(graph_links::Column::WorkspaceId.eq(wid.get()))
        .filter(graph_links::Column::ProjectId.eq(project))
        .exec(db)
        .await?;
    let t = now();
    let models: Vec<graph_links::ActiveModel> = links
        .iter()
        .map(|l| graph_links::ActiveModel {
            id: Set(Uuid::now_v7()),
            workspace_id: Set(wid.get()),
            project_id: Set(project),
            kind: Set(l.kind.as_str().to_string()),
            confidence: Set(l.confidence.as_str().to_string()),
            from_repo_id: Set(rows[l.from_repo].repo_id),
            from_symbol_id: Set(l.from_node.map(|n| ids[l.from_repo][n as usize])),
            to_repo_id: Set(l.to_repo.map(|r| rows[r].repo_id)),
            to_symbol_id: Set(l.to_repo.zip(l.to_node).map(|(r, n)| ids[r][n as usize])),
            detail: Set(l.detail.chars().take(500).collect()),
            created_at: Set(t),
        })
        .collect();
    for chunk in models.chunks(1000) {
        graph_links::Entity::insert_many(chunk.to_vec())
            .exec_without_returning(db)
            .await?;
    }
    Ok(models.len())
}
