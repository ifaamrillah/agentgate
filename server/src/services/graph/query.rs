//! Query code graph satu project (dipakai tool MCP dan UI). Pemanggil sudah memeriksa izin
//! `ViewProject`; semua query dibatasi workspace + project.
//!
//! Pencarian bersifat leksikal sadar-identifier (`reserveStock` cocok dengan "reserve stock"):
//! full-text `simple` atas pecahan nama + LIKE awalan nama, diurutkan cocok persis dulu.
//! Embedding semantik belum (keputusan sementara M7, lihat PROGRESS).
use std::collections::{HashMap, HashSet};

use agentgate_graph::{
    SymbolKind,
    analysis::parse_diff,
    query::{self as cypher, QEdge, QGraph, QNode, QueryResult},
};
use sea_orm::{ConnectionTrait, DbBackend, DbErr, FromQueryResult, Statement, Value};
use serde::Serialize;
use uuid::Uuid;

use crate::models::workspaces::WorkspaceId;

/// Batas baris snippet.
pub const SNIPPET_MAX: u32 = 200;
/// Batas kedalaman trace.
pub const TRACE_DEPTH_MAX: u8 = 5;
/// Batas node graph yang dimuat untuk query Cypher.
pub const CYPHER_NODES_MAX: usize = 300_000;

/// Satu simbol hasil query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, FromQueryResult)]
pub struct SymbolHit {
    pub id: Uuid,
    pub repo_id: Uuid,
    pub repo: String,
    pub path: String,
    pub lang: String,
    pub name: String,
    pub qualified: String,
    pub kind: String,
    pub start_line: i32,
    pub end_line: i32,
    pub exported: bool,
    pub test: bool,
    pub module: String,
    pub central: bool,
    pub dead: String,
    pub callers: i32,
    pub callees: i32,
    pub complexity: i32,
    pub signature: String,
    pub doc: String,
}

impl SymbolHit {
    #[must_use]
    pub fn kind(&self) -> SymbolKind {
        SymbolKind::parse(&self.kind).unwrap_or(SymbolKind::Function)
    }

    /// Ringkasan untuk tool MCP.
    #[must_use]
    pub fn brief(&self) -> serde_json::Value {
        let mut v = serde_json::json!({
            "id": self.id,
            "name": self.qualified,
            "kind": self.kind,
            "repo": self.repo,
            "file": self.path,
            "lines": format!("{}-{}", self.start_line, self.end_line),
        });
        if !self.signature.is_empty() {
            v["signature"] = self.signature.clone().into();
        }
        if !self.dead.is_empty() {
            v["dead"] = self.dead.clone().into();
        }
        v
    }
}

const SELECT: &str = "SELECT s.id, s.repo_id, r.name AS repo, f.path, f.lang, s.name, s.qualified,
    s.kind, s.start_line, s.end_line, s.exported, s.test, s.module, s.central, s.dead,
    s.callers, s.callees, s.complexity, s.signature, s.doc
  FROM graph_symbols s
  JOIN graph_files f ON f.id = s.file_id
  JOIN repos r ON r.id = s.repo_id
  WHERE s.workspace_id = $1 AND r.project_id = $2";

fn stmt(sql: String, values: Vec<Value>) -> Statement {
    Statement::from_sql_and_values(DbBackend::Postgres, sql, values)
}

/// Kata pencarian aman untuk `to_tsquery` (huruf/angka saja).
fn terms(q: &str) -> Vec<String> {
    agentgate_graph::protocol::words(&[q])
        .split(' ')
        .filter(|w| !w.is_empty() && w.chars().all(char::is_alphanumeric))
        .take(12)
        .map(str::to_string)
        .collect()
}

/// Filter pencarian.
#[derive(Debug, Clone, Default)]
pub struct SearchFilter {
    pub kind: Option<SymbolKind>,
    pub repo: Option<Uuid>,
    pub include_tests: bool,
}

/// Cari simbol menurut nama/pecahan nama/doc.
///
/// # Errors
///
/// Error database.
pub async fn search<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project: Uuid,
    q: &str,
    filter: &SearchFilter,
    limit: u64,
) -> Result<Vec<SymbolHit>, DbErr> {
    let t = terms(q);
    let whole = q.trim().to_lowercase();
    if t.is_empty() && whole.is_empty() {
        return Ok(Vec::new());
    }
    // Setiap kata cocok awalan (`|`: recall dulu, urutan menyaring).
    let any = t
        .iter()
        .map(|w| format!("{w}:*"))
        .collect::<Vec<_>>()
        .join(" | ");
    let pattern = crate::common::text::contains_pattern(&whole);
    let like = format!("{}%", &pattern[1..pattern.len() - 1]);
    let sql = format!(
        "{SELECT}
           AND s.kind <> 'file'
           AND ($3::text IS NULL OR s.kind = $3)
           AND ($4::uuid IS NULL OR s.repo_id = $4)
           AND ($5 OR NOT s.test)
           AND ((($6::text <> '') AND to_tsvector('simple', s.words || ' ' || s.doc)
                   @@ to_tsquery('simple', $6))
                OR lower(s.name) LIKE $7)
         ORDER BY (lower(s.name) = $8) DESC,
                  (lower(s.name) LIKE $7) DESC,
                  ts_rank(to_tsvector('simple', s.words || ' ' || s.doc),
                          to_tsquery('simple', CASE WHEN $6 = '' THEN 'x' ELSE $6 END)) DESC,
                  s.callers DESC, length(s.name), s.id
         LIMIT $9"
    );
    SymbolHit::find_by_statement(stmt(
        sql,
        vec![
            wid.get().into(),
            project.into(),
            filter.kind.map(|k| k.as_str().to_string()).into(),
            filter.repo.into(),
            filter.include_tests.into(),
            any.into(),
            like.into(),
            whole.into(),
            i64::try_from(limit.min(200)).unwrap_or(50).into(),
        ],
    ))
    .all(db)
    .await
}

/// Mode pencarian simbol (R2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchMode {
    /// Leksikal + semantik digabung (Reciprocal Rank Fusion). Default.
    #[default]
    Hybrid,
    /// Nama/pecahan nama/doc saja.
    Lexical,
    /// Kemiripan makna (embedding) saja.
    Semantic,
}

impl SearchMode {
    /// Dari parameter (`hybrid`/`lexical`/`semantic`; kosong = hybrid).
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "" | "hybrid" => Some(Self::Hybrid),
            "lexical" => Some(Self::Lexical),
            "semantic" => Some(Self::Semantic),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hybrid => "hybrid",
            Self::Lexical => "lexical",
            Self::Semantic => "semantic",
        }
    }
}

/// Hasil pencarian beserta mode yang benar-benar dipakai (semantik bisa jatuh ke leksikal
/// bila embedding mati, model belum siap, atau project belum di-embed).
#[derive(Debug, Clone)]
pub struct SearchOutcome {
    pub hits: Vec<SymbolHit>,
    pub mode: SearchMode,
    /// Alasan jatuh ke leksikal (kosong = sesuai permintaan).
    pub fallback: &'static str,
}

/// Konstanta RRF (Cormack dkk. 2009).
pub const RRF_K: f64 = 60.0;
/// Kandidat per daftar sebelum digabung.
const FUSE_POOL: u64 = 50;

/// Gabungkan beberapa daftar peringkat dengan Reciprocal Rank Fusion.
#[must_use]
pub fn fuse(lists: &[&[SymbolHit]], limit: usize) -> Vec<SymbolHit> {
    let mut score: std::collections::HashMap<Uuid, (f64, usize, SymbolHit)> =
        std::collections::HashMap::new();
    let mut order = 0usize;
    for list in lists {
        for (rank, hit) in list.iter().enumerate() {
            let add = 1.0 / (RRF_K + rank as f64 + 1.0);
            score
                .entry(hit.id)
                .and_modify(|e| e.0 += add)
                .or_insert_with(|| {
                    order += 1;
                    (add, order, hit.clone())
                });
        }
    }
    let mut all: Vec<_> = score.into_values().collect();
    all.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    all.into_iter().take(limit).map(|e| e.2).collect()
}

/// Cari simbol menurut kemiripan makna dengan vektor query.
///
/// # Errors
///
/// Error database.
pub async fn search_semantic<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project: Uuid,
    model: &str,
    vector: &[f32],
    filter: &SearchFilter,
    limit: u64,
) -> Result<Vec<SymbolHit>, DbErr> {
    let sql = format!(
        "{SELECT_EMBED}
           AND s.kind <> 'file'
           AND ($3::text IS NULL OR s.kind = $3)
           AND ($4::uuid IS NULL OR s.repo_id = $4)
           AND ($5 OR NOT s.test)
         ORDER BY e.embedding <=> $7::vector, s.id
         LIMIT $8"
    );
    SymbolHit::find_by_statement(stmt(
        sql,
        vec![
            wid.get().into(),
            project.into(),
            filter.kind.map(|k| k.as_str().to_string()).into(),
            filter.repo.into(),
            filter.include_tests.into(),
            model.to_string().into(),
            super::embed::literal(vector).into(),
            i64::try_from(limit.min(200)).unwrap_or(50).into(),
        ],
    ))
    .all(db)
    .await
}

const SELECT_EMBED: &str = "SELECT s.id, s.repo_id, r.name AS repo, f.path, f.lang, s.name,
    s.qualified, s.kind, s.start_line, s.end_line, s.exported, s.test, s.module, s.central,
    s.dead, s.callers, s.callees, s.complexity, s.signature, s.doc
  FROM graph_symbols s
  JOIN graph_files f ON f.id = s.file_id
  JOIN repos r ON r.id = s.repo_id
  JOIN graph_embeddings e ON e.repo_id = s.repo_id AND e.model = $6 AND e.hash = s.embed_hash
  WHERE s.workspace_id = $1 AND r.project_id = $2";

/// Pencarian dengan mode (UI Cari simbol, MCP `ag_search`).
///
/// # Errors
///
/// Error database.
#[allow(clippy::too_many_arguments)]
pub async fn search_mode<C: ConnectionTrait>(
    db: &C,
    settings: &crate::common::settings::EmbeddingSettings,
    wid: WorkspaceId,
    project: Uuid,
    q: &str,
    filter: &SearchFilter,
    limit: u64,
    mode: SearchMode,
) -> Result<SearchOutcome, DbErr> {
    let lexical = |n| search(db, wid, project, q, filter, n);
    if mode == SearchMode::Lexical {
        return Ok(SearchOutcome {
            hits: lexical(limit).await?,
            mode,
            fallback: "",
        });
    }
    let fallback = |reason: &'static str| async move {
        Ok(SearchOutcome {
            hits: search(db, wid, project, q, filter, limit).await?,
            mode: SearchMode::Lexical,
            fallback: reason,
        })
    };
    if !settings.enabled {
        return fallback("graph-semantic-off").await;
    }
    let Some(embedder) = super::embed::ready(settings) else {
        return fallback("graph-semantic-loading").await;
    };
    let q_owned = q.trim().to_string();
    let e = embedder.clone();
    let vector = match tokio::task::spawn_blocking(move || e.query(&q_owned)).await {
        Ok(Ok(v)) => v,
        _ => return fallback("graph-semantic-failed").await,
    };
    let semantic = search_semantic(
        db,
        wid,
        project,
        &embedder.model,
        &vector,
        filter,
        if mode == SearchMode::Semantic {
            limit
        } else {
            FUSE_POOL
        },
    )
    .await?;
    if semantic.is_empty() {
        return fallback("graph-semantic-empty").await;
    }
    if mode == SearchMode::Semantic {
        return Ok(SearchOutcome {
            hits: semantic,
            mode,
            fallback: "",
        });
    }
    let lex = lexical(FUSE_POOL).await?;
    // Nama persis tetap di urutan pertama (yang dicari biasanya simbol itu sendiri).
    let whole = q.trim().to_lowercase();
    let exact: Vec<SymbolHit> = lex
        .iter()
        .filter(|h| h.name.to_lowercase() == whole || h.qualified.to_lowercase() == whole)
        .cloned()
        .collect();
    let mut hits = exact.clone();
    for h in fuse(&[&lex, &semantic], usize::try_from(limit).unwrap_or(50)) {
        if !exact.iter().any(|x| x.id == h.id) {
            hits.push(h);
        }
    }
    hits.truncate(usize::try_from(limit).unwrap_or(50));
    Ok(SearchOutcome {
        hits,
        mode,
        fallback: "",
    })
}

/// Simbol menurut id.
///
/// # Errors
///
/// Error database.
pub async fn symbol<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project: Uuid,
    id: Uuid,
) -> Result<Option<SymbolHit>, DbErr> {
    SymbolHit::find_by_statement(stmt(
        format!("{SELECT} AND s.id = $3"),
        vec![wid.get().into(), project.into(), id.into()],
    ))
    .one(db)
    .await
}

async fn symbols_by_id<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project: Uuid,
    ids: Vec<Uuid>,
) -> Result<HashMap<Uuid, SymbolHit>, DbErr> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    Ok(SymbolHit::find_by_statement(stmt(
        format!("{SELECT} AND s.id = ANY($3)"),
        vec![wid.get().into(), project.into(), ids.into()],
    ))
    .all(db)
    .await?
    .into_iter()
    .map(|h| (h.id, h))
    .collect())
}

/// Simbol di nama persis atau `qualified` persis (untuk tool yang menerima nama).
///
/// # Errors
///
/// Error database.
pub async fn resolve<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project: Uuid,
    name: &str,
) -> Result<Vec<SymbolHit>, DbErr> {
    if let Ok(id) = Uuid::parse_str(name.trim()) {
        return Ok(symbol(db, wid, project, id).await?.into_iter().collect());
    }
    let n = name.trim();
    SymbolHit::find_by_statement(stmt(
        format!(
            "{SELECT} AND s.kind <> 'file' AND (s.qualified = $3 OR s.name = $3)
             ORDER BY (s.qualified = $3) DESC, s.test, s.callers DESC, s.id LIMIT 20"
        ),
        vec![wid.get().into(), project.into(), n.into()],
    ))
    .all(db)
    .await
}

/// Arah trace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Callers,
    Callees,
}

/// Satu langkah trace.
#[derive(Debug, Clone, Serialize)]
pub struct Hop {
    pub symbol: SymbolHit,
    pub depth: u8,
    /// Keyakinan terlemah sepanjang jalur (`certain`/`inferred`).
    pub confidence: &'static str,
    /// `calls`, `handles`, `http`, `event` (relasi terakhir menuju simbol ini).
    pub via: String,
}

#[derive(Debug, FromQueryResult)]
struct Step {
    from_id: Uuid,
    to_id: Uuid,
    confidence: String,
    via: String,
}

async fn neighbours<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project: Uuid,
    frontier: Vec<Uuid>,
    dir: Direction,
) -> Result<Vec<Step>, DbErr> {
    let (edge_col, link_col) = match dir {
        Direction::Callers => ("e.to_id", "l.to_symbol_id"),
        Direction::Callees => ("e.from_id", "l.from_symbol_id"),
    };
    let sql = format!(
        "SELECT e.from_id, e.to_id, e.confidence, e.kind AS via FROM graph_edges e
          WHERE e.workspace_id = $1 AND e.kind IN ('calls', 'handles') AND {edge_col} = ANY($3)
         UNION ALL
         SELECT l.from_symbol_id, l.to_symbol_id, l.confidence, l.kind FROM graph_links l
          WHERE l.workspace_id = $1 AND l.project_id = $2 AND l.kind IN ('http', 'event')
            AND l.confidence IN ('certain', 'inferred')
            AND l.from_symbol_id IS NOT NULL AND l.to_symbol_id IS NOT NULL
            AND {link_col} = ANY($3)"
    );
    Step::find_by_statement(stmt(
        sql,
        vec![wid.get().into(), project.into(), frontier.into()],
    ))
    .all(db)
    .await
}

/// Trace pemanggil/dipanggil `start` sampai `depth` (lintas repo lewat relasi HTTP/event).
///
/// # Errors
///
/// Error database.
pub async fn trace<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project: Uuid,
    start: &[Uuid],
    dir: Direction,
    depth: u8,
    limit: usize,
) -> Result<Vec<Hop>, DbErr> {
    let depth = depth.clamp(1, TRACE_DEPTH_MAX);
    let mut seen: HashSet<Uuid> = start.iter().copied().collect();
    let mut conf: HashMap<Uuid, bool> = start.iter().map(|s| (*s, true)).collect();
    let mut frontier: Vec<Uuid> = start.to_vec();
    let mut order: Vec<(Uuid, u8, bool, String)> = Vec::new();
    for d in 1..=depth {
        if frontier.is_empty() || order.len() >= limit {
            break;
        }
        let steps = neighbours(db, wid, project, frontier.clone(), dir).await?;
        let mut next = Vec::new();
        for s in steps {
            let (from, to) = match dir {
                Direction::Callers => (s.to_id, s.from_id),
                Direction::Callees => (s.from_id, s.to_id),
            };
            if from == to || !seen.insert(to) || order.len() >= limit {
                continue;
            }
            let certain = conf.get(&from).copied().unwrap_or(false) && s.confidence == "certain";
            conf.insert(to, certain);
            order.push((to, d, certain, s.via));
            next.push(to);
        }
        frontier = next;
    }
    let ids: Vec<Uuid> = order.iter().map(|o| o.0).collect();
    let mut found = symbols_by_id(db, wid, project, ids).await?;
    Ok(order
        .into_iter()
        .filter_map(|(id, depth, certain, via)| {
            found.remove(&id).map(|symbol| Hop {
                symbol,
                depth,
                confidence: if certain { "certain" } else { "inferred" },
                via,
            })
        })
        .collect())
}

/// Potongan kode.
#[derive(Debug, Clone, Serialize)]
pub struct Snippet {
    pub repo: String,
    pub path: String,
    pub lang: String,
    pub start: u32,
    pub end: u32,
    pub code: String,
    pub truncated: bool,
}

#[derive(Debug, FromQueryResult)]
struct FileText {
    repo: String,
    path: String,
    lang: String,
    content: String,
}

/// Kode simbol (maks. [`SNIPPET_MAX`] baris, plus `context` baris di sekitarnya).
///
/// # Errors
///
/// Error database.
pub async fn snippet<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project: Uuid,
    sym: &SymbolHit,
    context: u32,
) -> Result<Option<Snippet>, DbErr> {
    let Some(f) = FileText::find_by_statement(stmt(
        "SELECT r.name AS repo, f.path, f.lang, f.content FROM graph_files f
          JOIN repos r ON r.id = f.repo_id
          JOIN graph_symbols s ON s.file_id = f.id
         WHERE f.workspace_id = $1 AND r.project_id = $2 AND s.id = $3"
            .into(),
        vec![wid.get().into(), project.into(), sym.id.into()],
    ))
    .one(db)
    .await?
    else {
        return Ok(None);
    };
    let start = u32::try_from(sym.start_line)
        .unwrap_or(1)
        .saturating_sub(context)
        .max(1);
    let want_end = u32::try_from(sym.end_line)
        .unwrap_or(1)
        .saturating_add(context);
    let end = want_end.min(start + SNIPPET_MAX - 1);
    let code: Vec<&str> = f
        .content
        .lines()
        .skip((start - 1) as usize)
        .take((end - start + 1) as usize)
        .collect();
    let end = start + u32::try_from(code.len()).unwrap_or(0).saturating_sub(1);
    Ok(Some(Snippet {
        repo: f.repo,
        path: f.path,
        lang: f.lang,
        start,
        end,
        code: code.join("\n"),
        truncated: want_end > end && (want_end - start + 1) > SNIPPET_MAX,
    }))
}

/// Ringkasan dampak perubahan.
#[derive(Debug, Clone, Serialize)]
pub struct ImpactReport {
    pub changed: Vec<SymbolHit>,
    pub affected: Vec<Hop>,
    pub tests: Vec<SymbolHit>,
    /// Repo lain yang ikut terdampak.
    pub repos: Vec<String>,
    /// `rendah`, `sedang`, `tinggi`.
    pub risk: &'static str,
}

/// Dampak perubahan simbol `changed` (pemanggil transitif ≤ `depth`).
///
/// # Errors
///
/// Error database.
pub async fn impact<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project: Uuid,
    changed: Vec<SymbolHit>,
    depth: u8,
) -> Result<ImpactReport, DbErr> {
    let ids: Vec<Uuid> = changed.iter().map(|c| c.id).collect();
    let hops = trace(db, wid, project, &ids, Direction::Callers, depth, 500).await?;
    let (tests, affected): (Vec<Hop>, Vec<Hop>) = hops.into_iter().partition(|h| h.symbol.test);
    let own: HashSet<&str> = changed.iter().map(|c| c.repo.as_str()).collect();
    let mut repos: Vec<String> = affected
        .iter()
        .map(|h| h.symbol.repo.clone())
        .filter(|r| !own.contains(r.as_str()))
        .collect();
    repos.sort();
    repos.dedup();
    let callers = affected
        .iter()
        .filter(|h| h.symbol.kind().is_callable())
        .count();
    let risk = if !repos.is_empty() || callers > 20 {
        "tinggi"
    } else if callers > 5 {
        "sedang"
    } else {
        "rendah"
    };
    Ok(ImpactReport {
        changed,
        affected,
        tests: tests.into_iter().map(|h| h.symbol).collect(),
        repos,
        risk,
    })
}

/// Simbol callable yang bersinggungan dengan unified diff.
///
/// # Errors
///
/// Error database.
pub async fn changed_by_diff<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project: Uuid,
    diff: &str,
) -> Result<Vec<SymbolHit>, DbErr> {
    let mut out: Vec<SymbolHit> = Vec::new();
    for c in parse_diff(diff).into_iter().take(200) {
        for (a, b) in c.ranges.into_iter().take(100) {
            let hits = SymbolHit::find_by_statement(stmt(
                format!(
                    "{SELECT} AND f.path = $3 AND s.kind IN ('function', 'method')
                       AND s.start_line <= $5 AND $4 <= s.end_line
                     ORDER BY s.start_line DESC LIMIT 20"
                ),
                vec![
                    wid.get().into(),
                    project.into(),
                    c.path.clone().into(),
                    i32::try_from(a).unwrap_or(0).into(),
                    i32::try_from(b).unwrap_or(i32::MAX).into(),
                ],
            ))
            .all(db)
            .await?;
            for h in hits {
                if !out.iter().any(|o| o.id == h.id) {
                    out.push(h);
                }
            }
        }
    }
    Ok(out)
}

/// Kandidat kode mati (`certain` dulu).
///
/// # Errors
///
/// Error database.
pub async fn dead_code<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project: Uuid,
    repo: Option<Uuid>,
    limit: u64,
) -> Result<Vec<SymbolHit>, DbErr> {
    SymbolHit::find_by_statement(stmt(
        format!(
            "{SELECT} AND s.dead <> '' AND ($3::uuid IS NULL OR s.repo_id = $3)
             ORDER BY (s.dead = 'certain') DESC, f.path, s.start_line LIMIT $4"
        ),
        vec![
            wid.get().into(),
            project.into(),
            repo.into(),
            i64::try_from(limit.min(500)).unwrap_or(100).into(),
        ],
    ))
    .all(db)
    .await
}

/// Fungsi paling kompleks.
///
/// # Errors
///
/// Error database.
pub async fn complex<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project: Uuid,
    repo: Option<Uuid>,
    limit: u64,
) -> Result<Vec<SymbolHit>, DbErr> {
    SymbolHit::find_by_statement(stmt(
        format!(
            "{SELECT} AND s.kind IN ('function', 'method') AND s.complexity > {}
               AND ($3::uuid IS NULL OR s.repo_id = $3)
             ORDER BY s.complexity DESC, s.id LIMIT $4",
            super::build::COMPLEX_THRESHOLD
        ),
        vec![
            wid.get().into(),
            project.into(),
            repo.into(),
            i64::try_from(limit.min(200)).unwrap_or(50).into(),
        ],
    ))
    .all(db)
    .await
}

/// Satu relasi lintas repo untuk tampilan.
#[derive(Debug, Clone, Serialize, FromQueryResult)]
pub struct LinkRow {
    pub kind: String,
    pub confidence: String,
    pub from_repo: String,
    pub from_symbol: Option<Uuid>,
    pub from_name: Option<String>,
    pub from_path: Option<String>,
    pub to_repo: Option<String>,
    pub to_symbol: Option<Uuid>,
    pub to_name: Option<String>,
    pub detail: String,
}

/// Relasi lintas repo project (tanpa relasi di dalam repo yang sama).
///
/// # Errors
///
/// Error database.
pub async fn links<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project: Uuid,
    symbol: Option<Uuid>,
) -> Result<Vec<LinkRow>, DbErr> {
    LinkRow::find_by_statement(stmt(
        "SELECT l.kind, l.confidence, ra.name AS from_repo, l.from_symbol_id AS from_symbol,
                sa.qualified AS from_name, fa.path AS from_path, rb.name AS to_repo,
                l.to_symbol_id AS to_symbol, sb.qualified AS to_name, l.detail
           FROM graph_links l
           JOIN repos ra ON ra.id = l.from_repo_id
           LEFT JOIN repos rb ON rb.id = l.to_repo_id
           LEFT JOIN graph_symbols sa ON sa.id = l.from_symbol_id
           LEFT JOIN graph_files fa ON fa.id = sa.file_id
           LEFT JOIN graph_symbols sb ON sb.id = l.to_symbol_id
          WHERE l.workspace_id = $1 AND l.project_id = $2
            AND (l.to_repo_id IS NULL OR l.to_repo_id <> l.from_repo_id)
            AND ($3::uuid IS NULL OR l.from_symbol_id = $3 OR l.to_symbol_id = $3)
          ORDER BY l.kind, (l.confidence = 'certain') DESC, ra.name, l.detail
          LIMIT 500"
            .into(),
        vec![wid.get().into(), project.into(), symbol.into()],
    ))
    .all(db)
    .await
}

#[derive(Debug, FromQueryResult)]
struct QRow {
    id: Uuid,
    name: String,
    qualified: String,
    kind: String,
    path: String,
    start_line: i32,
    repo: String,
    module: String,
    exported: bool,
    dead: String,
    test: bool,
}

#[derive(Debug, FromQueryResult)]
struct QEdgeRow {
    from_id: Uuid,
    to_id: Uuid,
    kind: String,
}

/// Galat query Cypher (sintaks atau graph terlalu besar).
#[derive(Debug, thiserror::Error)]
pub enum CypherError {
    #[error("{0}")]
    Query(String),
    #[error(transparent)]
    Db(#[from] DbErr),
}

/// Jalankan query Cypher (subset) atas graph semua repo project.
///
/// # Errors
///
/// [`CypherError::Query`] untuk sintaks/graph terlalu besar, error database.
pub async fn run_cypher<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project: Uuid,
    q: &str,
) -> Result<QueryResult, CypherError> {
    let query = cypher::parse(q).map_err(|e| CypherError::Query(e.0))?;
    let rows = QRow::find_by_statement(stmt(
        format!(
            "SELECT s.id, s.name, s.qualified, s.kind, f.path, s.start_line, r.name AS repo,
                    s.module, s.exported, s.dead, s.test
               FROM graph_symbols s JOIN graph_files f ON f.id = s.file_id
               JOIN repos r ON r.id = s.repo_id
              WHERE s.workspace_id = $1 AND r.project_id = $2 LIMIT {}",
            CYPHER_NODES_MAX + 1
        ),
        vec![wid.get().into(), project.into()],
    ))
    .all(db)
    .await?;
    if rows.len() > CYPHER_NODES_MAX {
        return Err(CypherError::Query(format!(
            "Graph project lebih dari {CYPHER_NODES_MAX} node; persempit lewat ag_search/ag_trace."
        )));
    }
    let index: HashMap<Uuid, usize> = rows.iter().enumerate().map(|(i, r)| (r.id, i)).collect();
    let edges = QEdgeRow::find_by_statement(stmt(
        "SELECT e.from_id, e.to_id, e.kind FROM graph_edges e JOIN repos r ON r.id = e.repo_id
          WHERE e.workspace_id = $1 AND r.project_id = $2
         UNION ALL
         SELECT l.from_symbol_id, l.to_symbol_id, l.kind FROM graph_links l
          WHERE l.workspace_id = $1 AND l.project_id = $2 AND l.kind IN ('http', 'event')
            AND l.confidence IN ('certain', 'inferred')
            AND l.from_symbol_id IS NOT NULL AND l.to_symbol_id IS NOT NULL"
            .into(),
        vec![wid.get().into(), project.into()],
    ))
    .all(db)
    .await?;
    let g = QGraph {
        nodes: rows
            .into_iter()
            .map(|r| QNode {
                name: r.name,
                qualified: r.qualified,
                kind: SymbolKind::parse(&r.kind).unwrap_or(SymbolKind::Function),
                file: r.path,
                line: u32::try_from(r.start_line).unwrap_or(0),
                repo: r.repo,
                module: r.module,
                exported: r.exported,
                dead: !r.dead.is_empty(),
                test: r.test,
            })
            .collect(),
        edges: edges
            .into_iter()
            .filter_map(|e| {
                Some(QEdge {
                    from: *index.get(&e.from_id)?,
                    to: *index.get(&e.to_id)?,
                    kind: e.kind,
                })
            })
            .collect(),
    };
    tokio::task::spawn_blocking(move || cypher::execute(&g, &query))
        .await
        .map_err(|e| CypherError::Query(format!("query task: {e}")))?
        .map_err(|e| CypherError::Query(e.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_terms_are_tsquery_safe() {
        assert_eq!(
            terms("reserveStock"),
            vec!["reservestock", "reserve", "stock"]
        );
        assert_eq!(terms("a') | !b:*"), vec!["a", "b"]);
    }
}
