//! Tool code graph (M7): `ag_search`, `ag_trace`, `ag_snippet`, `ag_impact`, `ag_graph`.
//! Membaca graph project lewat `services::graph::query` dengan izin `ViewProject`. Respons
//! ringkas: simbol diringkas ke id + nama + lokasi; kode hanya lewat `ag_snippet`.
use agentgate_graph::SymbolKind;
use sea_orm::DatabaseConnection;
use serde_json::{Value, json};
use uuid::Uuid;

use super::{Args, Session, ToolError, find_project, required};
use crate::{
    models::{graph_repos, permission::Permission, projects, repos},
    services::graph::query::{self, Direction, SearchFilter, SymbolHit},
};

/// Project tool: `project` atau satu-satunya project yang bisa Anda buka.
async fn project_of(
    db: &DatabaseConnection,
    s: &Session,
    a: &Args,
) -> Result<projects::Model, ToolError> {
    if !a.project.trim().is_empty() {
        return find_project(db, s, a.project.trim()).await;
    }
    let acting = s.acting();
    let mut list = projects::Model::accessible_for(db, &acting.accessor()).await?;
    if list.len() == 1 {
        let p = list.remove(0);
        crate::services::tickets::access_in(db, &acting, p.id, Permission::ViewProject)
            .await
            .map_err(|_| ToolError::new("`project` wajib diisi."))?;
        return Ok(p);
    }
    Err(ToolError::new(
        "`project` wajib diisi (kode project, mis. KSR).",
    ))
}

async fn repo_id(
    db: &DatabaseConnection,
    s: &Session,
    project: &projects::Model,
    name: &str,
) -> Result<Option<Uuid>, ToolError> {
    if name.trim().is_empty() {
        return Ok(None);
    }
    repos::Model::of_project(db, s.workspace.workspace_id(), project.id)
        .await?
        .into_iter()
        .find(|r| r.name.eq_ignore_ascii_case(name.trim()))
        .map(|r| Some(r.id))
        .ok_or_else(|| ToolError::new(format!("Repo {name} tidak ada di project {}.", project.key)))
}

/// Pastikan project sudah punya index; kalau belum, pesan yang menjelaskan cara meng-index.
async fn ensure_indexed(
    db: &DatabaseConnection,
    s: &Session,
    project: &projects::Model,
) -> Result<(), ToolError> {
    let rows = graph_repos::Model::of_project(db, s.workspace.workspace_id(), project.id).await?;
    if rows.iter().any(|r| r.indexed_at.is_some()) {
        Ok(())
    } else {
        Err(ToolError::new(format!(
            "Code graph project {} belum di-index. Jalankan `agentgate graph index` di repo, atau \
             minta admin menekan Index ulang di halaman Code graph.",
            project.key
        )))
    }
}

/// Satu simbol dari `symbol` (id atau nama). Nama ambigu → daftar kandidat sebagai error.
async fn one_symbol(
    db: &DatabaseConnection,
    s: &Session,
    project: &projects::Model,
    symbol: &str,
) -> Result<SymbolHit, ToolError> {
    let wid = s.workspace.workspace_id();
    let mut hits = query::resolve(db, wid, project.id, symbol).await?;
    if hits.is_empty() {
        hits = query::search(db, wid, project.id, symbol, &SearchFilter::default(), 5).await?;
        if hits.len() != 1 {
            let names: Vec<String> = hits.iter().map(describe).collect();
            return Err(ToolError::new(if names.is_empty() {
                format!("Simbol {symbol} tidak ditemukan. Cari dulu dengan ag_search.")
            } else {
                format!(
                    "Simbol {symbol} tidak persis ditemukan. Mungkin: {}. Pakai id dari ag_search.",
                    names.join("; ")
                )
            }));
        }
    }
    let exact: Vec<&SymbolHit> = hits.iter().filter(|h| h.qualified == symbol).collect();
    if hits.len() > 1 && exact.len() != 1 {
        let names: Vec<String> = hits.iter().take(8).map(describe).collect();
        return Err(ToolError::new(format!(
            "Nama {symbol} ambigu ({} simbol): {}. Pakai id.",
            hits.len(),
            names.join("; ")
        )));
    }
    Ok(exact
        .first()
        .map_or_else(|| hits[0].clone(), |h| (*h).clone()))
}

fn describe(h: &SymbolHit) -> String {
    format!(
        "{} {} ({}:{}:{}) id={}",
        h.kind, h.qualified, h.repo, h.path, h.start_line, h.id
    )
}

pub(super) async fn search(
    db: &DatabaseConnection,
    s: &Session,
    a: &Args,
) -> Result<Value, ToolError> {
    let q = required(&a.query, "query")?;
    let project = project_of(db, s, a).await?;
    ensure_indexed(db, s, &project).await?;
    let kind = if a.kind.trim().is_empty() {
        None
    } else {
        Some(
            SymbolKind::parse(a.kind.trim())
                .ok_or_else(|| ToolError::new(format!("`kind` tidak dikenal: {}", a.kind)))?,
        )
    };
    let filter = SearchFilter {
        kind,
        repo: repo_id(db, s, &project, &a.repo).await?,
        include_tests: a.include_tests,
    };
    let hits = query::search(
        db,
        s.workspace.workspace_id(),
        project.id,
        q,
        &filter,
        a.limit.unwrap_or(15).clamp(1, 50),
    )
    .await?;
    Ok(json!({
        "project": project.key,
        "results": hits.iter().map(SymbolHit::brief).collect::<Vec<_>>(),
        "hint": if hits.is_empty() {
            "Tidak ada yang cocok. Coba kata lain atau pecahan nama (\"reserve stock\")."
        } else {
            "Pakai `id` di ag_trace, ag_snippet, atau ag_impact."
        },
    }))
}

pub(super) async fn trace(
    db: &DatabaseConnection,
    s: &Session,
    a: &Args,
) -> Result<Value, ToolError> {
    let symbol = required(&a.symbol, "symbol")?;
    let project = project_of(db, s, a).await?;
    ensure_indexed(db, s, &project).await?;
    let dir = match a.direction.trim() {
        "" | "callers" => Direction::Callers,
        "callees" => Direction::Callees,
        other => {
            return Err(ToolError::new(format!(
                "`direction` tidak dikenal: {other}"
            )));
        }
    };
    let start = one_symbol(db, s, &project, symbol).await?;
    let hops = query::trace(
        db,
        s.workspace.workspace_id(),
        project.id,
        &[start.id],
        dir,
        a.depth.unwrap_or(2),
        100,
    )
    .await?;
    Ok(json!({
        "symbol": start.brief(),
        "direction": dir,
        "results": hops.iter().map(|h| {
            let mut v = h.symbol.brief();
            v["depth"] = h.depth.into();
            v["confidence"] = h.confidence.into();
            v["via"] = h.via.clone().into();
            v
        }).collect::<Vec<_>>(),
        "truncated": hops.len() >= 100,
    }))
}

pub(super) async fn snippet(
    db: &DatabaseConnection,
    s: &Session,
    a: &Args,
) -> Result<Value, ToolError> {
    let symbol = required(&a.symbol, "symbol")?;
    let project = project_of(db, s, a).await?;
    ensure_indexed(db, s, &project).await?;
    let hit = one_symbol(db, s, &project, symbol).await?;
    let snip = query::snippet(
        db,
        s.workspace.workspace_id(),
        project.id,
        &hit,
        a.context.unwrap_or(0).min(20),
    )
    .await?
    .ok_or_else(|| ToolError::new("Isi file tidak tersedia (file dilewati saat index)."))?;
    Ok(json!({
        "symbol": hit.brief(),
        "repo": snip.repo,
        "file": snip.path,
        "lang": snip.lang,
        "lines": format!("{}-{}", snip.start, snip.end),
        "code": snip.code,
        "truncated": snip.truncated,
    }))
}

pub(super) async fn impact(
    db: &DatabaseConnection,
    s: &Session,
    a: &Args,
) -> Result<Value, ToolError> {
    let project = project_of(db, s, a).await?;
    ensure_indexed(db, s, &project).await?;
    let wid = s.workspace.workspace_id();
    let changed = if !a.diff.trim().is_empty() {
        query::changed_by_diff(db, wid, project.id, &a.diff).await?
    } else if !a.symbol.trim().is_empty() {
        vec![one_symbol(db, s, &project, a.symbol.trim()).await?]
    } else {
        return Err(ToolError::new("Isi `symbol` atau `diff`."));
    };
    let report = query::impact(db, wid, project.id, changed, a.depth.unwrap_or(3)).await?;
    Ok(json!({
        "risk": report.risk,
        "changed": report.changed.iter().map(SymbolHit::brief).collect::<Vec<_>>(),
        "affected": report.affected.iter().take(60).map(|h| {
            let mut v = h.symbol.brief();
            v["depth"] = h.depth.into();
            v["confidence"] = h.confidence.into();
            v
        }).collect::<Vec<_>>(),
        "affected_total": report.affected.len(),
        "tests": report.tests.iter().map(SymbolHit::brief).collect::<Vec<_>>(),
        "other_repos": report.repos,
    }))
}

pub(super) async fn overview(
    db: &DatabaseConnection,
    s: &Session,
    a: &Args,
) -> Result<Value, ToolError> {
    let project = project_of(db, s, a).await?;
    let wid = s.workspace.workspace_id();
    match a.action.trim() {
        "" | "overview" => {
            let names: std::collections::HashMap<Uuid, String> =
                repos::Model::of_project(db, wid, project.id)
                    .await?
                    .into_iter()
                    .map(|r| (r.id, r.name))
                    .collect();
            let rows = graph_repos::Model::of_project(db, wid, project.id).await?;
            let repos: Vec<Value> = rows
                .iter()
                .map(|r| {
                    let git = r.git();
                    json!({
                        "repo": names.get(&r.repo_id),
                        "status": r.status,
                        "commit": r.commit_sha,
                        "indexed_at": r.indexed_at,
                        "files": r.files_indexed,
                        "skipped": r.files_skipped,
                        "symbols": r.symbols,
                        "edges": r.edges,
                        "edges_certain": r.edges_certain,
                        "scip": r.scip_indexer,
                        "languages": r.languages().into_iter().map(|(l, c)| json!({ "lang": l, "files": c.files, "lines": c.lines })).collect::<Vec<_>>(),
                        "modules": r.modules().into_iter().take(15).map(|m| json!({ "name": m.name, "files": m.files, "symbols": m.symbols, "central": m.central_name })).collect::<Vec<_>>(),
                        "hotspots": git.hotspots.iter().take(8).map(|h| json!({ "file": h.path, "commits": h.commits })).collect::<Vec<_>>(),
                    })
                })
                .collect();
            Ok(json!({ "project": project.key, "repos": repos }))
        }
        "dead" => {
            ensure_indexed(db, s, &project).await?;
            let repo = repo_id(db, s, &project, &a.repo).await?;
            let dead = query::dead_code(db, wid, project.id, repo, 60).await?;
            Ok(json!({
                "results": dead.iter().map(|d| {
                    let mut v = d.brief();
                    v["confidence"] = d.dead.clone().into();
                    v
                }).collect::<Vec<_>>(),
                "hint": "certain = tidak dipanggil dan tidak diekspor; inferred = diekspor/publik tapi tidak dipanggil di repo project ini. Cek dulu sebelum menghapus.",
            }))
        }
        "links" => {
            ensure_indexed(db, s, &project).await?;
            let links = query::links(db, wid, project.id, None).await?;
            Ok(json!({ "results": links }))
        }
        "query" => {
            ensure_indexed(db, s, &project).await?;
            let q = required(&a.query, "query")?;
            let res = query::run_cypher(db, wid, project.id, q)
                .await
                .map_err(|e| match e {
                    query::CypherError::Query(m) => ToolError::new(m),
                    query::CypherError::Db(e) => e.into(),
                })?;
            Ok(json!(res))
        }
        other => Err(ToolError::new(format!(
            "`action` harus overview, dead, links, atau query (bukan {other})."
        ))),
    }
}

/// Bagian `code_graph` di `ag_context` tiket: repo ter-index + petunjuk tool.
pub(super) async fn context_section(
    db: &DatabaseConnection,
    s: &Session,
    project: &projects::Model,
) -> Result<Value, ToolError> {
    let wid = s.workspace.workspace_id();
    let rows = graph_repos::Model::of_project(db, wid, project.id).await?;
    let repos: Vec<Value> = repos::Model::of_project(db, wid, project.id)
        .await?
        .into_iter()
        .filter_map(|r| {
            let g = rows.iter().find(|g| g.repo_id == r.id)?;
            g.indexed_at.map(|t| {
                json!({ "repo": r.name, "commit": g.commit_sha, "indexed_at": t, "symbols": g.symbols })
            })
        })
        .collect();
    if repos.is_empty() {
        return Ok(Value::Null);
    }
    Ok(json!({
        "repos": repos,
        "hint": "Jelajahi kode lewat ag_search → ag_trace/ag_snippet; cek ag_impact sebelum selesai.",
    }))
}
