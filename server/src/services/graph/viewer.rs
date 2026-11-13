//! Data graph viewer (M10) satu project: memuat node + relasi satu level zoom dari tabel code
//! graph, lalu diserahkan ke `agentgate_graph::viewer` (pemangkasan, layout, dampak).
//!
//! Level ditentukan lingkup (`Scope`): tanpa apa-apa = Project (repo, layanan eksternal, dokumen
//! knowledge), `repo` = modul, `+module` = file, `+file` = simbol, `+symbol` = pemanggil dan
//! yang dipanggil (termasuk rute lintas repo). Tetangga di luar lingkup tampil sebagai node "luar".
//! Sorot tiket memakai `jobs.changed_files` semua job tiket itu.
use std::collections::{BTreeMap, HashMap, HashSet};

use agentgate_graph::{
    build::module_of,
    viewer::{Edge, Graph, Layer, Level, Node, guess_layer},
};
use sea_orm::{ConnectionTrait, DbBackend, DbErr, EntityTrait, FromQueryResult, Statement, Value};
use serde::Serialize;
use uuid::Uuid;

use crate::models::{graph_repos, repos, workspaces::WorkspaceId};

/// Lingkup level (query string halaman viewer).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scope {
    pub repo: Option<String>,
    pub module: Option<String>,
    pub file: Option<String>,
    pub symbol: Option<Uuid>,
}

impl Scope {
    #[must_use]
    pub fn level(&self) -> Level {
        match (&self.repo, &self.module, &self.file, &self.symbol) {
            (Some(_), Some(_), Some(_), Some(_)) => Level::Symbol,
            (Some(_), Some(_), Some(_), None) => Level::File,
            (Some(_), Some(_), None, _) => Level::Module,
            (Some(_), None, _, _) => Level::Repo,
            _ => Level::Project,
        }
    }

    /// Query string lingkup ini (tanpa `?`).
    #[must_use]
    pub fn query(&self) -> String {
        let mut pairs: Vec<(&str, String)> = Vec::new();
        if let Some(r) = &self.repo {
            pairs.push(("repo", r.clone()));
        }
        if let Some(m) = &self.module {
            pairs.push(("module", m.clone()));
        }
        if let Some(f) = &self.file {
            pairs.push(("file", f.clone()));
        }
        if let Some(s) = self.symbol {
            pairs.push(("sym", s.to_string()));
        }
        serde_urlencoded::to_string(pairs).unwrap_or_default()
    }

    fn repo(repo: &str) -> Self {
        Self {
            repo: Some(repo.to_string()),
            ..Self::default()
        }
    }

    fn module(repo: &str, module: &str) -> Self {
        Self {
            module: Some(module.to_string()),
            ..Self::repo(repo)
        }
    }

    fn file(repo: &str, path: &str) -> Self {
        Self {
            file: Some(path.to_string()),
            ..Self::module(repo, &module_of(path))
        }
    }

    fn symbol(repo: &str, path: &str, id: Uuid) -> Self {
        Self {
            symbol: Some(id),
            ..Self::file(repo, path)
        }
    }
}

/// Satu langkah breadcrumb level.
#[derive(Debug, Clone, Serialize)]
pub struct Crumb {
    pub level: Level,
    pub label: String,
    pub query: String,
}

/// Hasil muat satu level (belum dipangkas/di-layout).
#[derive(Debug, Clone)]
pub struct Loaded {
    pub level: Level,
    pub graph: Graph,
    pub crumbs: Vec<Crumb>,
    /// Node yang dipilih bawaan (simbol pusat di level Fungsi).
    pub focus: Option<String>,
}

/// File yang diubah job satu tiket.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TicketFiles {
    pub code: String,
    pub title: String,
    pub files: Vec<(String, String)>,
}

/// Dokumen knowledge project (untuk node dokumen dan panel).
#[derive(Debug, Clone, FromQueryResult)]
pub struct Doc {
    pub slug: String,
    pub title: String,
    pub body: String,
}

fn stmt(sql: &str, values: Vec<Value>) -> Statement {
    Statement::from_sql_and_values(DbBackend::Postgres, sql, values)
}

fn short(s: &str, max: usize) -> String {
    if s.chars().count() > max {
        format!("{}…", s.chars().take(max - 1).collect::<String>())
    } else {
        s.to_string()
    }
}

fn base_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn kind_word(kind: &str) -> &'static str {
    match kind {
        "function" => "fungsi",
        "method" => "method",
        "class" => "kelas",
        "interface" => "interface",
        "struct" => "struct",
        "enum" => "enum",
        "trait" => "trait",
        "type" => "tipe",
        "module" => "modul",
        "route" => "rute",
        _ => "simbol",
    }
}

#[derive(Debug, Clone, FromQueryResult)]
struct Sym {
    id: Uuid,
    repo: String,
    path: String,
    lang: String,
    name: String,
    kind: String,
    start_line: i32,
    end_line: i32,
    test: bool,
    dead: String,
    callers: i32,
}

const SYM_SELECT: &str = "SELECT s.id, r.name AS repo, f.path, f.lang, s.name, s.kind,
    s.start_line, s.end_line, s.test, s.dead, s.callers
  FROM graph_symbols s
  JOIN graph_files f ON f.id = s.file_id
  JOIN repos r ON r.id = s.repo_id
  WHERE s.workspace_id = $1 AND r.project_id = $2";

impl Sym {
    fn node(&self, out: bool, here_repo: &str) -> Node {
        let callable = matches!(self.kind.as_str(), "function" | "method");
        let mut n = Node::new(
            format!("s:{}", self.id),
            if callable {
                format!("{}()", self.name)
            } else {
                self.name.clone()
            },
            guess_layer(&self.path, self.test),
        );
        n.sub = if out {
            let place = if self.repo == here_repo {
                base_name(&self.path).to_string()
            } else {
                format!("{}/{}", self.repo, base_name(&self.path))
            };
            format!("luar · {place}")
        } else {
            format!(
                "{} · {} baris",
                kind_word(&self.kind),
                (self.end_line - self.start_line + 1).max(1)
            )
        };
        if self.kind == "route" {
            n.layer = Layer::Api;
        }
        n.lang.clone_from(&self.lang);
        n.kind_key = format!("graph-kind-{}", self.kind);
        n.path = format!("{}/{}:{}", self.repo, self.path, self.start_line);
        n.out = out;
        n.dead = !self.dead.is_empty();
        n.weight = i64::from(self.callers);
        n.drill = Some(Scope::symbol(&self.repo, &self.path, self.id).query());
        n.symbol = Some(self.id.to_string());
        n
    }
}

async fn syms<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project: Uuid,
    extra: &str,
    mut values: Vec<Value>,
) -> Result<Vec<Sym>, DbErr> {
    let mut all = vec![wid.get().into(), project.into()];
    all.append(&mut values);
    Sym::find_by_statement(stmt(&format!("{SYM_SELECT} {extra}"), all))
        .all(db)
        .await
}

#[derive(Debug, FromQueryResult)]
struct Pair {
    a: String,
    b: String,
    n: i64,
    certain: bool,
}

#[derive(Debug, FromQueryResult)]
struct IdPair {
    from_id: Uuid,
    to_id: Uuid,
    certain: bool,
}

#[derive(Debug, FromQueryResult)]
struct LinkRow {
    from_repo: String,
    to_repo: Option<String>,
    kind: String,
    confidence: String,
    detail: String,
}

#[derive(Debug, FromQueryResult)]
struct LangRow {
    key: String,
    lang: String,
    n: i64,
}

fn top_lang(rows: &[LangRow]) -> HashMap<String, String> {
    let mut best: HashMap<String, (i64, String)> = HashMap::new();
    for r in rows {
        let e = best.entry(r.key.clone()).or_insert((0, String::new()));
        if r.n > e.0 || (r.n == e.0 && r.lang < e.1) {
            *e = (r.n, r.lang.clone());
        }
    }
    best.into_iter().map(|(k, v)| (k, v.1)).collect()
}

/// Dokumen knowledge project yang aktif (maks. 60).
///
/// # Errors
///
/// Error database.
pub async fn docs<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project: Uuid,
) -> Result<Vec<Doc>, DbErr> {
    Doc::find_by_statement(stmt(
        "SELECT slug, title, body FROM knowledge_docs WHERE workspace_id = $1 AND project_id = $2 \
         AND archived_at IS NULL ORDER BY title, id LIMIT 60",
        vec![wid.get().into(), project.into()],
    ))
    .all(db)
    .await
}

#[derive(Debug, FromQueryResult)]
struct TicketFileRow {
    code: String,
    title: String,
    files: serde_json::Value,
}

/// Tiket project yang job-nya melaporkan file berubah (terbaru dulu, maks. 50).
///
/// # Errors
///
/// Error database.
pub async fn ticket_files<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project: Uuid,
) -> Result<Vec<TicketFiles>, DbErr> {
    let rows = TicketFileRow::find_by_statement(stmt(
        "SELECT t.code, t.title, jsonb_agg(j.changed_files) AS files
           FROM jobs j JOIN tickets t ON t.id = j.ticket_id AND t.workspace_id = j.workspace_id
          WHERE j.workspace_id = $1 AND j.project_id = $2 AND j.changed_files <> '[]'::jsonb
            AND t.deleted_at IS NULL
          GROUP BY t.id, t.code, t.title
          ORDER BY max(j.finished_at) DESC NULLS LAST, t.code LIMIT 50",
        vec![wid.get().into(), project.into()],
    ))
    .all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| {
            let mut files: Vec<(String, String)> = r
                .files
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(serde_json::Value::as_array)
                .flatten()
                .filter_map(|f| {
                    Some((
                        f.get("repo")?.as_str()?.to_string(),
                        f.get("path")?.as_str()?.to_string(),
                    ))
                })
                .collect();
            files.sort();
            files.dedup();
            TicketFiles {
                code: r.code,
                title: r.title,
                files,
            }
        })
        .collect())
}

/// Node yang disentuh file berubah (repo, path) di level ini.
#[must_use]
pub fn touched(graph: &Graph, files: &[(String, String)], level: Level) -> HashSet<String> {
    graph
        .nodes
        .iter()
        .filter(|n| {
            files.iter().any(|(repo, path)| match level {
                Level::Project => n.id == format!("r:{repo}"),
                Level::Repo => n.id == format!("m:{}", module_of(path)) && !n.out,
                Level::Module => n.id == format!("f:{path}"),
                Level::File | Level::Symbol => {
                    n.symbol.is_some() && n.path.starts_with(&format!("{repo}/{path}:"))
                }
            })
        })
        .map(|n| n.id.clone())
        .collect()
}

/// Muat satu level. `None` = lingkup tidak ada di project (repo/modul/file/simbol tak dikenal).
///
/// # Errors
///
/// Error database.
pub async fn load<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project: Uuid,
    project_name: &str,
    scope: &Scope,
) -> Result<Option<Loaded>, DbErr> {
    let level = scope.level();
    let mut crumbs = vec![Crumb {
        level: Level::Project,
        label: project_name.to_string(),
        query: String::new(),
    }];
    if level == Level::Project {
        return Ok(Some(Loaded {
            level,
            graph: project_level(db, wid, project).await?,
            crumbs,
            focus: None,
        }));
    }
    let repo_name = scope.repo.clone().unwrap_or_default();
    let Some(repo) = repos::Model::of_project(db, wid, project)
        .await?
        .into_iter()
        .find(|r| r.name == repo_name)
    else {
        return Ok(None);
    };
    crumbs.push(Crumb {
        level: Level::Repo,
        label: repo.name.clone(),
        query: Scope::repo(&repo.name).query(),
    });
    if let Some(m) = &scope.module {
        crumbs.push(Crumb {
            level: Level::Module,
            label: m.clone(),
            query: Scope::module(&repo.name, m).query(),
        });
    }
    if let Some(f) = &scope.file {
        crumbs.push(Crumb {
            level: Level::File,
            label: base_name(f).to_string(),
            query: Scope::file(&repo.name, f).query(),
        });
    }
    let (graph, focus) = match level {
        Level::Repo => (repo_level(db, wid, &repo).await?, None),
        Level::Module => {
            let module = scope.module.clone().unwrap_or_default();
            let g = module_level(db, wid, &repo, &module).await?;
            if g.nodes.iter().all(|n| n.out) {
                return Ok(None);
            }
            (g, None)
        }
        Level::File => {
            let path = scope.file.clone().unwrap_or_default();
            let g = file_level(db, wid, project, &repo, &path).await?;
            if g.nodes.is_empty() {
                return Ok(None);
            }
            (g, None)
        }
        _ => {
            let id = scope.symbol.unwrap_or_default();
            let Some((g, center)) = symbol_level(db, wid, project, &repo, id).await? else {
                return Ok(None);
            };
            crumbs.push(Crumb {
                level: Level::Symbol,
                label: center.clone(),
                query: scope.query(),
            });
            (g, Some(format!("s:{id}")))
        }
    };
    Ok(Some(Loaded {
        level,
        graph,
        crumbs,
        focus,
    }))
}

async fn project_level<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project: Uuid,
) -> Result<Graph, DbErr> {
    let repo_rows = repos::Model::of_project(db, wid, project).await?;
    let indexed = graph_repos::Model::of_project(db, wid, project).await?;
    let mut nodes = Vec::new();
    for r in &repo_rows {
        let g = indexed.iter().find(|g| g.repo_id == r.id);
        let mut n = Node::new(
            format!("r:{}", r.name),
            r.name.clone(),
            guess_layer(&r.name, false),
        );
        if n.layer == Layer::Logic && r.name.contains("web") {
            n.layer = Layer::Ui;
        }
        n.sub = g.map_or_else(
            || "repo · belum di-index".to_string(),
            |g| format!("repo · {} file", g.files_indexed),
        );
        n.lang = g
            .and_then(|g| {
                g.languages()
                    .into_iter()
                    .max_by_key(|(_, c)| c.files)
                    .map(|(l, _)| l)
            })
            .unwrap_or_default();
        n.kind_key = "gv-kind-repo".into();
        n.path.clone_from(&r.url);
        n.weight = g.map_or(0, |g| i64::from(g.symbols));
        n.drill = g.map(|_| Scope::repo(&r.name).query());
        nodes.push(n);
    }
    let links = LinkRow::find_by_statement(stmt(
        "SELECT ra.name AS from_repo, rb.name AS to_repo, l.kind, l.confidence, l.detail
           FROM graph_links l JOIN repos ra ON ra.id = l.from_repo_id
           LEFT JOIN repos rb ON rb.id = l.to_repo_id
          WHERE l.workspace_id = $1 AND l.project_id = $2 LIMIT 5000",
        vec![wid.get().into(), project.into()],
    ))
    .all(db)
    .await?;
    let mut pairs: BTreeMap<(String, String), (i64, bool, String)> = BTreeMap::new();
    let mut ext: BTreeMap<String, usize> = BTreeMap::new();
    for l in &links {
        let to = match (&l.to_repo, l.confidence.as_str()) {
            (Some(r), _) => format!("r:{r}"),
            (None, "external") => {
                let host = l
                    .detail
                    .split_whitespace()
                    .nth(1)
                    .and_then(|u| u.split('/').next())
                    .unwrap_or("")
                    .to_string();
                if host.is_empty() {
                    continue;
                }
                let next = ext.len();
                let i = *ext.entry(host).or_insert(next);
                format!("x:{i}")
            }
            _ => continue,
        };
        let e =
            pairs
                .entry((format!("r:{}", l.from_repo), to))
                .or_insert((0, false, l.kind.clone()));
        e.0 += 1;
        e.1 |= l.confidence == "certain";
    }
    for (host, i) in &ext {
        let mut n = Node::new(format!("x:{i}"), host.clone(), Layer::Ext);
        n.sub = "layanan eksternal".into();
        n.kind_key = "gv-kind-external".into();
        n.path.clone_from(host);
        nodes.push(n);
    }
    let mut edges: Vec<Edge> = pairs
        .into_iter()
        .map(|((from, to), (n, certain, kind))| Edge {
            from,
            to,
            certain,
            label: match kind.as_str() {
                "http" => format!("HTTP · {n}"),
                "event" => format!("event · {n}"),
                _ => "paket".to_string(),
            },
        })
        .collect();
    for d in docs(db, wid, project).await?.into_iter().take(6) {
        let id = format!("d:{}", d.slug);
        let body = d.body.to_lowercase();
        for r in &repo_rows {
            if body.contains(&r.name.to_lowercase()) {
                edges.push(Edge {
                    from: id.clone(),
                    to: format!("r:{}", r.name),
                    certain: false,
                    label: "dokumentasi".into(),
                });
            }
        }
        let mut n = Node::new(id, d.title, Layer::Doc);
        n.sub = "knowledge".into();
        n.kind_key = "gv-kind-doc".into();
        n.path = d.slug;
        n.lang = "Markdown".into();
        nodes.push(n);
    }
    Ok(Graph::new(nodes, edges))
}

async fn repo_level<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    repo: &repos::Model,
) -> Result<Graph, DbErr> {
    let modules = graph_repos::Entity::find_by_id(repo.id)
        .one(db)
        .await?
        .map(|g| g.modules())
        .unwrap_or_default();
    let langs = top_lang(
        &LangRow::find_by_statement(stmt(
            "SELECT s.module AS key, f.lang, COUNT(*)::bigint AS n FROM graph_symbols s
               JOIN graph_files f ON f.id = s.file_id
              WHERE s.workspace_id = $1 AND s.repo_id = $2 AND s.kind = 'file' GROUP BY 1, 2",
            vec![wid.get().into(), repo.id.into()],
        ))
        .all(db)
        .await?,
    );
    let nodes = modules
        .iter()
        .map(|m| {
            let mut n = Node::new(
                format!("m:{}", m.name),
                m.name.clone(),
                guess_layer(&m.name, false),
            );
            n.sub = if m.central.is_some() {
                format!("node pusat · {} file", m.files)
            } else {
                format!("modul · {} file", m.files)
            };
            n.lang = langs.get(&m.name).cloned().unwrap_or_default();
            n.kind_key = "gv-kind-module".into();
            n.path = format!("{}/{}", repo.name, m.name);
            n.weight = i64::from(m.symbols);
            n.drill = Some(Scope::module(&repo.name, &m.name).query());
            n
        })
        .collect();
    let pairs = Pair::find_by_statement(stmt(
        "SELECT a.module AS a, b.module AS b, COUNT(*)::bigint AS n,
                bool_or(e.confidence = 'certain') AS certain
           FROM graph_edges e
           JOIN graph_symbols a ON a.id = e.from_id
           JOIN graph_symbols b ON b.id = e.to_id
          WHERE e.workspace_id = $1 AND e.repo_id = $2 AND a.module <> b.module
          GROUP BY 1, 2",
        vec![wid.get().into(), repo.id.into()],
    ))
    .all(db)
    .await?;
    let edges = pairs
        .into_iter()
        .map(|p| Edge {
            from: format!("m:{}", p.a),
            to: format!("m:{}", p.b),
            certain: p.certain,
            label: if p.n > 1 {
                p.n.to_string()
            } else {
                String::new()
            },
        })
        .collect();
    Ok(Graph::new(nodes, edges))
}

#[derive(Debug, FromQueryResult)]
struct FileRow {
    path: String,
    lang: String,
    lines: i32,
    test: bool,
    symbols: i64,
    callers: i64,
}

async fn module_level<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    repo: &repos::Model,
    module: &str,
) -> Result<Graph, DbErr> {
    let files = FileRow::find_by_statement(stmt(
        "SELECT f.path, f.lang, f.lines, bool_or(s.test) AS test,
                COUNT(*) FILTER (WHERE s.kind <> 'file')::bigint AS symbols,
                COALESCE(SUM(s.callers), 0)::bigint AS callers
           FROM graph_symbols s JOIN graph_files f ON f.id = s.file_id
          WHERE s.workspace_id = $1 AND s.repo_id = $2 AND s.module = $3
          GROUP BY f.path, f.lang, f.lines",
        vec![wid.get().into(), repo.id.into(), module.into()],
    ))
    .all(db)
    .await?;
    let inside: HashSet<String> = files.iter().map(|f| f.path.clone()).collect();
    let mut nodes: Vec<Node> = files
        .iter()
        .map(|f| {
            let mut n = Node::new(
                format!("f:{}", f.path),
                base_name(&f.path).to_string(),
                guess_layer(&f.path, f.test),
            );
            n.sub = format!("{} simbol · {} baris", f.symbols, f.lines);
            n.lang.clone_from(&f.lang);
            n.kind_key = "gv-kind-file".into();
            n.path = format!("{}/{}", repo.name, f.path);
            n.weight = f.callers;
            n.drill = Some(Scope::file(&repo.name, &f.path).query());
            n
        })
        .collect();
    let pairs = Pair::find_by_statement(stmt(
        "SELECT fa.path AS a, fb.path AS b, COUNT(*)::bigint AS n,
                bool_or(e.confidence = 'certain') AS certain
           FROM graph_edges e
           JOIN graph_symbols a ON a.id = e.from_id JOIN graph_files fa ON fa.id = a.file_id
           JOIN graph_symbols b ON b.id = e.to_id JOIN graph_files fb ON fb.id = b.file_id
          WHERE e.workspace_id = $1 AND e.repo_id = $2 AND (a.module = $3 OR b.module = $3)
            AND fa.id <> fb.id
          GROUP BY 1, 2",
        vec![wid.get().into(), repo.id.into(), module.into()],
    ))
    .all(db)
    .await?;
    let mut outside: BTreeMap<String, i64> = BTreeMap::new();
    for p in &pairs {
        for path in [&p.a, &p.b] {
            if !inside.contains(path) {
                *outside.entry(path.clone()).or_default() += p.n;
            }
        }
    }
    // Tetangga luar modul: 8 yang paling sering berelasi.
    let mut outside: Vec<(String, i64)> = outside.into_iter().collect();
    outside.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    for (path, w) in outside.into_iter().take(8) {
        let mut n = Node::new(
            format!("f:{path}"),
            base_name(&path).to_string(),
            guess_layer(&path, false),
        );
        n.sub = format!("luar modul · {}", module_of(&path));
        n.kind_key = "gv-kind-file".into();
        n.path = format!("{}/{}", repo.name, path);
        n.out = true;
        n.weight = w;
        n.drill = Some(Scope::file(&repo.name, &path).query());
        nodes.push(n);
    }
    let edges = pairs
        .into_iter()
        .map(|p| Edge {
            from: format!("f:{}", p.a),
            to: format!("f:{}", p.b),
            certain: p.certain,
            label: if p.n > 1 {
                p.n.to_string()
            } else {
                String::new()
            },
        })
        .collect();
    Ok(Graph::new(nodes, edges))
}

async fn edges_of<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    repo: Uuid,
    filter: &str,
    value: Value,
) -> Result<Vec<IdPair>, DbErr> {
    IdPair::find_by_statement(stmt(
        &format!(
            "SELECT e.from_id, e.to_id, bool_or(e.confidence = 'certain') AS certain
               FROM graph_edges e
               JOIN graph_symbols a ON a.id = e.from_id JOIN graph_files fa ON fa.id = a.file_id
               JOIN graph_symbols b ON b.id = e.to_id JOIN graph_files fb ON fb.id = b.file_id
              WHERE e.workspace_id = $1 AND e.repo_id = $2 AND a.kind <> 'file'
                AND b.kind <> 'file' AND ({filter})
              GROUP BY 1, 2 LIMIT 400"
        ),
        vec![wid.get().into(), repo.into(), value],
    ))
    .all(db)
    .await
}

async fn file_level<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project: Uuid,
    repo: &repos::Model,
    path: &str,
) -> Result<Graph, DbErr> {
    let own = syms(
        db,
        wid,
        project,
        "AND s.repo_id = $3 AND f.path = $4 AND s.kind <> 'file' ORDER BY s.start_line LIMIT 200",
        vec![repo.id.into(), path.into()],
    )
    .await?;
    if own.is_empty() {
        return Ok(Graph::default());
    }
    let pairs = edges_of(
        db,
        wid,
        repo.id,
        "fa.path = $3 OR fb.path = $3",
        path.into(),
    )
    .await?;
    let inside: HashSet<Uuid> = own.iter().map(|s| s.id).collect();
    let mut weight: HashMap<Uuid, i64> = HashMap::new();
    for p in &pairs {
        for id in [p.from_id, p.to_id] {
            if !inside.contains(&id) {
                *weight.entry(id).or_default() += 1;
            }
        }
    }
    let mut outside: Vec<(Uuid, i64)> = weight.into_iter().collect();
    outside.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let outside_ids: Vec<Uuid> = outside.into_iter().take(10).map(|o| o.0).collect();
    let neighbors = if outside_ids.is_empty() {
        Vec::new()
    } else {
        syms(
            db,
            wid,
            project,
            "AND s.id = ANY($3)",
            vec![Value::Array(
                sea_orm::sea_query::ArrayType::Uuid,
                Some(Box::new(
                    outside_ids.iter().map(|u| Value::Uuid(Some(*u))).collect(),
                )),
            )],
        )
        .await?
    };
    let mut nodes: Vec<Node> = own.iter().map(|s| s.node(false, &repo.name)).collect();
    nodes.extend(neighbors.iter().map(|s| s.node(true, &repo.name)));
    let edges = pairs
        .into_iter()
        .map(|p| Edge {
            from: format!("s:{}", p.from_id),
            to: format!("s:{}", p.to_id),
            certain: p.certain,
            label: String::new(),
        })
        .collect();
    Ok(Graph::new(nodes, edges))
}

#[derive(Debug, FromQueryResult)]
struct SymLink {
    from_symbol_id: Option<Uuid>,
    to_symbol_id: Option<Uuid>,
    confidence: String,
    detail: String,
}

async fn symbol_level<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project: Uuid,
    repo: &repos::Model,
    id: Uuid,
) -> Result<Option<(Graph, String)>, DbErr> {
    let Some(center) = syms(db, wid, project, "AND s.id = $3", vec![id.into()])
        .await?
        .into_iter()
        .next()
        .filter(|s| s.repo == repo.name)
    else {
        return Ok(None);
    };
    let pairs = edges_of(db, wid, repo.id, "a.id = $3 OR b.id = $3", id.into()).await?;
    let links = SymLink::find_by_statement(stmt(
        "SELECT from_symbol_id, to_symbol_id, confidence, detail FROM graph_links
          WHERE workspace_id = $1 AND project_id = $2
            AND (from_symbol_id = $3 OR to_symbol_id = $3) LIMIT 40",
        vec![wid.get().into(), project.into(), id.into()],
    ))
    .all(db)
    .await?;
    let mut others: Vec<Uuid> = pairs
        .iter()
        .flat_map(|p| [p.from_id, p.to_id])
        .chain(
            links
                .iter()
                .flat_map(|l| [l.from_symbol_id, l.to_symbol_id])
                .flatten(),
        )
        .filter(|o| *o != id)
        .collect();
    others.sort();
    others.dedup();
    others.truncate(60);
    let rows = if others.is_empty() {
        Vec::new()
    } else {
        syms(
            db,
            wid,
            project,
            "AND s.id = ANY($3)",
            vec![Value::Array(
                sea_orm::sea_query::ArrayType::Uuid,
                Some(Box::new(
                    others.iter().map(|u| Value::Uuid(Some(*u))).collect(),
                )),
            )],
        )
        .await?
    };
    let mut nodes = vec![center.node(false, &repo.name)];
    nodes[0].drill = None;
    nodes.extend(rows.iter().map(|s| {
        let mut n = s.node(true, &repo.name);
        n.out = s.path != center.path || s.repo != center.repo;
        if !n.out {
            n.sub = format!("{} · baris {}", kind_word(&s.kind), s.start_line);
        }
        n
    }));
    let mut edges: Vec<Edge> = pairs
        .into_iter()
        .map(|p| Edge {
            from: format!("s:{}", p.from_id),
            to: format!("s:{}", p.to_id),
            certain: p.certain,
            label: String::new(),
        })
        .collect();
    edges.extend(links.into_iter().filter_map(|l| {
        Some(Edge {
            from: format!("s:{}", l.from_symbol_id?),
            to: format!("s:{}", l.to_symbol_id?),
            certain: l.confidence == "certain",
            label: short(&l.detail, 28),
        })
    }));
    let label = nodes[0].label.clone();
    Ok(Some((Graph::new(nodes, edges), label)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_level_and_query_round_trip() {
        let s = Scope::symbol("kasir-web", "src/features/cart/promo.ts", Uuid::nil());
        assert_eq!(s.level(), Level::Symbol);
        assert_eq!(s.module.as_deref(), Some("features/cart"));
        let q = s.query();
        assert!(
            q.starts_with("repo=kasir-web&module=features%2Fcart&file=src%2F"),
            "{q}"
        );
        assert_eq!(Scope::default().level(), Level::Project);
        assert_eq!(Scope::repo("a").level(), Level::Repo);
    }

    #[test]
    fn touched_matches_files_per_level() {
        let mut file = Node::new("f:src/a.ts", "a.ts", Layer::Logic);
        file.path = "web/src/a.ts".into();
        let mut sym = Node::new("s:1", "x()", Layer::Logic);
        sym.path = "web/src/a.ts:3".into();
        sym.symbol = Some("1".into());
        let g = Graph::new(vec![file, sym], vec![]);
        let files = vec![("web".to_string(), "src/a.ts".to_string())];
        assert!(touched(&g, &files, Level::Module).contains("f:src/a.ts"));
        assert!(touched(&g, &files, Level::File).contains("s:1"));
    }
}
