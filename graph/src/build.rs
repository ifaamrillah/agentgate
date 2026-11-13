//! Membangun graph satu repo dari fakta semua file: node (file + simbol + route) dan edge
//! (`calls`, `inherits`, `imports`, `handles`) dengan keyakinan.
//!
//! Keyakinan **pasti** = hasil parsing yang tidak ambigu: definisi di file yang sama, `self`/`this`,
//! nama yang diimpor dari file repo, satu paket (direktori) untuk Go/Java/Kotlin/C#/Dart, atau SCIP.
//! **Dugaan** = inferensi: satu-satunya kandidat bernama sama di repo (atau di direktori yang sama).
//! Call ke nama yang diimpor dari luar repo, atau yang kandidatnya ambigu, tidak dibuatkan edge.
use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::{
    facts::{FileFacts, SymbolKind},
    lang::Lang,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Certain,
    Inferred,
}

impl Confidence {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Certain => "certain",
            Self::Inferred => "inferred",
        }
    }

    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "certain" => Some(Self::Certain),
            "inferred" => Some(Self::Inferred),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    Calls,
    Inherits,
    Imports,
    /// Route → fungsi handler.
    Handles,
}

impl EdgeKind {
    pub const ALL: [Self; 4] = [Self::Calls, Self::Inherits, Self::Imports, Self::Handles];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Calls => "calls",
            Self::Inherits => "inherits",
            Self::Imports => "imports",
            Self::Handles => "handles",
        }
    }

    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == s)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeSource {
    TreeSitter,
    Scip,
}

impl EdgeSource {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TreeSitter => "tree_sitter",
            Self::Scip => "scip",
        }
    }
}

/// Edge hasil indexer SCIP: referensi di (`from_path`, `from_line`) ke definisi di
/// (`to_path`, `to_line`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScipEdge {
    pub from_path: String,
    pub from_line: u32,
    pub to_path: String,
    pub to_line: u32,
}

/// Index SCIP satu repo yang diunggah CLI.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScipData {
    pub indexer: String,
    /// Dokumen yang tercakup (call dari file ini memakai SCIP, bukan tree-sitter).
    pub documents: Vec<String>,
    pub edges: Vec<ScipEdge>,
}

/// Satu file masukan.
#[derive(Debug, Clone)]
pub struct SourceFile {
    pub path: String,
    pub facts: FileFacts,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    pub file: u32,
    /// Indeks simbol di `FileFacts` (`None` = node file atau route).
    pub symbol: Option<u32>,
    pub name: String,
    pub qualified: String,
    pub kind: SymbolKind,
    pub start: u32,
    pub end: u32,
    pub exported: bool,
    pub test: bool,
    pub module: String,
    pub central: bool,
    pub dead: Option<Confidence>,
    pub callers: u32,
    pub callees: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edge {
    pub from: u32,
    pub to: u32,
    pub kind: EdgeKind,
    pub confidence: Confidence,
    pub source: EdgeSource,
    pub line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleInfo {
    pub name: String,
    pub files: u32,
    pub symbols: u32,
    pub central: Option<u32>,
}

/// Statistik resolusi call.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildStats {
    pub calls: u32,
    pub certain: u32,
    pub inferred: u32,
    /// Call ke kode di luar repo (library, builtin).
    pub external: u32,
    pub ambiguous: u32,
    pub scip_edges: u32,
}

/// Permukaan repo untuk dicocokkan lintas repo (indeks node).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Surface {
    pub routes: Vec<SurfaceRoute>,
    pub http: Vec<SurfaceHttp>,
    pub events: Vec<SurfaceEvent>,
    pub packages: Vec<String>,
    pub deps: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceRoute {
    pub node: u32,
    pub method: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceHttp {
    pub node: u32,
    pub method: String,
    pub path: String,
    pub exact: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub host: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceEvent {
    pub node: u32,
    pub topic: String,
    pub publish: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoGraph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub modules: Vec<ModuleInfo>,
    pub surface: Surface,
    pub stats: BuildStats,
}

const ROOT_DIRS: &[&str] = &[
    "src", "lib", "app", "internal", "pkg", "source", "sources", "main", "java", "kotlin", "cmd",
    "Sources",
];
const DOMAIN_ROOTS: &[&str] = &["com", "org", "net", "io", "id", "co", "dev"];

/// Modul file: direktori di bawah folder akar umum (`src`, `internal`, …), maksimal dua tingkat,
/// diawali folder di atas akar (paket monorepo).
#[must_use]
pub fn module_of(path: &str) -> String {
    let segs: Vec<&str> = path.split('/').collect();
    let dirs = &segs[..segs.len().saturating_sub(1)];
    let Some(root) = dirs.iter().position(|d| ROOT_DIRS.contains(d)) else {
        return if dirs.is_empty() {
            "(root)".to_string()
        } else {
            dirs.iter().take(2).copied().collect::<Vec<_>>().join("/")
        };
    };
    let mut after = root;
    while after < dirs.len() && ROOT_DIRS.contains(&dirs[after]) {
        after += 1;
    }
    let mut rest: Vec<&str> = dirs[after..].to_vec();
    if rest.first().is_some_and(|s| DOMAIN_ROOTS.contains(s)) && rest.len() > 1 {
        let drop = (rest.len() - 1).min(3);
        rest.drain(..drop);
    }
    let mut parts: Vec<&str> = dirs[..root].to_vec();
    parts.extend(rest.iter().take(2));
    if parts.is_empty() {
        dirs[root].to_string()
    } else {
        parts.join("/")
    }
}

fn dir_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(d, _)| d)
}

/// Path tanpa ekstensi dan tanpa nama file indeks (`index`, `mod`, `__init__`, `lib`).
fn module_path(path: &str) -> String {
    let (dir, name) = path.rsplit_once('/').unwrap_or(("", path));
    let stem = name.split_once('.').map_or(name, |(s, _)| s);
    if matches!(stem, "index" | "mod" | "__init__" | "lib" | "main") {
        dir.to_string()
    } else if dir.is_empty() {
        stem.to_string()
    } else {
        format!("{dir}/{stem}")
    }
}

/// Segmen modul sebuah sumber import (`./a/b`, `pkg.mod`, `crate::a::b`, `App\Models\User`).
fn import_segments(source: &str) -> Vec<String> {
    let s = source
        .trim_start_matches("package:")
        .trim_end_matches(".dart")
        .trim_end_matches(".js")
        .trim_end_matches(".ts");
    s.split(['/', '.', ':', '\\'])
        .filter(|x| !x.is_empty() && !matches!(*x, "crate" | "self" | "super" | "@" | "~"))
        .map(str::to_string)
        .collect()
}

struct Index<'a> {
    files: &'a [SourceFile],
    /// Node per (file, simbol).
    sym_node: Vec<Vec<u32>>,
    file_node: Vec<u32>,
    by_name: HashMap<&'a str, Vec<u32>>,
    /// (tipe pemilik, nama method) → node.
    methods: HashMap<(&'a str, &'a str), Vec<u32>>,
    /// Pemilik (nama tipe) per node simbol.
    owner: Vec<&'a str>,
    /// Basis per nama tipe.
    bases: HashMap<&'a str, Vec<&'a str>>,
    /// module_path → file.
    paths: HashMap<String, Vec<u32>>,
    by_dir: HashMap<&'a str, Vec<u32>>,
    nodes: Vec<Node>,
}

impl<'a> Index<'a> {
    fn new(files: &'a [SourceFile]) -> Self {
        let mut nodes = Vec::new();
        let mut sym_node = Vec::with_capacity(files.len());
        let mut file_node = Vec::with_capacity(files.len());
        let mut owner = Vec::new();
        let mut by_name: HashMap<&str, Vec<u32>> = HashMap::new();
        let mut methods: HashMap<(&str, &str), Vec<u32>> = HashMap::new();
        let mut bases: HashMap<&str, Vec<&str>> = HashMap::new();
        let mut paths: HashMap<String, Vec<u32>> = HashMap::new();
        let mut by_dir: HashMap<&str, Vec<u32>> = HashMap::new();
        for (fi, f) in files.iter().enumerate() {
            let fi = fi as u32;
            let module = module_of(&f.path);
            let test = crate::walk::is_test_path(&f.path);
            file_node.push(nodes.len() as u32);
            nodes.push(Node {
                file: fi,
                symbol: None,
                name: f.path.rsplit('/').next().unwrap_or(&f.path).to_string(),
                qualified: f.path.clone(),
                kind: SymbolKind::File,
                start: 1,
                end: f.facts.lines.max(1),
                exported: true,
                test,
                module: module.clone(),
                central: false,
                dead: None,
                callers: 0,
                callees: 0,
            });
            owner.push("");
            paths.entry(module_path(&f.path)).or_default().push(fi);
            by_dir.entry(dir_of(&f.path)).or_default().push(fi);
            let mut ids = Vec::with_capacity(f.facts.symbols.len());
            for (si, s) in f.facts.symbols.iter().enumerate() {
                let id = nodes.len() as u32;
                ids.push(id);
                let mut qual = vec![s.name.as_str()];
                let mut p = s.parent;
                while let Some(pi) = p {
                    let ps = &f.facts.symbols[pi as usize];
                    qual.push(&ps.name);
                    p = ps.parent;
                }
                if s.parent.is_none() && !s.owner.is_empty() {
                    qual.push(&s.owner);
                }
                qual.reverse();
                nodes.push(Node {
                    file: fi,
                    symbol: Some(si as u32),
                    name: s.name.clone(),
                    qualified: qual.join("."),
                    kind: s.kind,
                    start: s.start,
                    end: s.end,
                    exported: s.exported,
                    test: s.test || test,
                    module: module.clone(),
                    central: false,
                    dead: None,
                    callers: 0,
                    callees: 0,
                });
                owner.push(s.owner.as_str());
                if s.kind.is_callable() || s.kind.is_type() {
                    by_name.entry(s.name.as_str()).or_default().push(id);
                }
                if s.kind.is_callable() && !s.owner.is_empty() {
                    methods
                        .entry((s.owner.as_str(), s.name.as_str()))
                        .or_default()
                        .push(id);
                }
            }
            for i in &f.facts.inherits {
                bases
                    .entry(i.owner.as_str())
                    .or_default()
                    .push(i.base.as_str());
            }
            sym_node.push(ids);
        }
        Self {
            files,
            sym_node,
            file_node,
            by_name,
            methods,
            owner,
            bases,
            paths,
            by_dir,
            nodes,
        }
    }

    /// File repo yang dirujuk import `source` dari file `from`.
    fn resolve_import(&self, from: usize, source: &str, lang: Lang) -> Vec<u32> {
        let from_dir = dir_of(&self.files[from].path);
        if source.starts_with('.') {
            // Relatif: ./x, ../x, .x (Python).
            let joined = if lang == Lang::Python {
                let dots = source.chars().take_while(|c| *c == '.').count();
                let mut dir: Vec<&str> = from_dir.split('/').filter(|s| !s.is_empty()).collect();
                for _ in 1..dots {
                    dir.pop();
                }
                let rest = source[dots..].replace('.', "/");
                if rest.is_empty() {
                    dir.join("/")
                } else if dir.is_empty() {
                    rest
                } else {
                    format!("{}/{rest}", dir.join("/"))
                }
            } else {
                let mut dir: Vec<&str> = from_dir.split('/').filter(|s| !s.is_empty()).collect();
                for seg in source.split('/') {
                    match seg {
                        "." | "" => {}
                        ".." => {
                            dir.pop();
                        }
                        s => dir.push(s),
                    }
                }
                dir.join("/")
            };
            let key = module_path(&format!("{joined}.x"));
            if let Some(f) = self.paths.get(&key) {
                return f.clone();
            }
            if let Some(f) = self.paths.get(&joined) {
                return f.clone();
            }
            return Vec::new();
        }
        let segs = import_segments(source);
        if segs.is_empty() {
            return Vec::new();
        }
        // Akhiran path terpanjang yang cocok (Go: direktori paket).
        if lang == Lang::Go {
            let mut best: Vec<u32> = Vec::new();
            for n in (1..=segs.len()).rev() {
                let suffix = segs[segs.len() - n..].join("/");
                for (dir, files) in &self.by_dir {
                    if *dir == suffix || dir.ends_with(&format!("/{suffix}")) {
                        best.extend(
                            files
                                .iter()
                                .filter(|&&f| self.files[f as usize].facts.lang == Lang::Go),
                        );
                    }
                }
                if !best.is_empty() {
                    break;
                }
            }
            return best;
        }
        for n in (1..=segs.len()).rev() {
            let suffix = segs[segs.len() - n..].join("/");
            let mut hits: Vec<u32> = self
                .paths
                .iter()
                .filter(|(p, _)| *p == &suffix || p.ends_with(&format!("/{suffix}")))
                .flat_map(|(_, f)| f.iter().copied())
                .collect();
            if !hits.is_empty() {
                hits.sort_unstable();
                // Satu segmen saja terlalu lemah bila cocok banyak file.
                if n == 1 && hits.len() > 1 && segs.len() > 1 {
                    return Vec::new();
                }
                return hits;
            }
        }
        Vec::new()
    }

    fn type_of_scope(&self, scope: u32) -> Option<&'a str> {
        let n = &self.nodes[scope as usize];
        let f = &self.files[n.file as usize];
        let mut sym = n.symbol;
        while let Some(si) = sym {
            let s = &f.facts.symbols[si as usize];
            if s.kind.is_type() {
                return Some(s.name.as_str());
            }
            if !s.owner.is_empty() {
                return Some(s.owner.as_str());
            }
            sym = s.parent;
        }
        None
    }
}

/// Bangun graph repo.
#[must_use]
pub fn build(files: &[SourceFile], scip: Option<&ScipData>) -> RepoGraph {
    let mut ix = Index::new(files);
    let mut edges: Vec<Edge> = Vec::new();
    let mut seen: HashSet<(u32, u32, EdgeKind)> = HashSet::new();
    let mut stats = BuildStats::default();
    let mut called_names: HashSet<&str> = HashSet::new();
    let covered: HashSet<&str> = scip
        .map(|s| s.documents.iter().map(String::as_str).collect())
        .unwrap_or_default();
    let mut push = |edges: &mut Vec<Edge>, e: Edge| {
        if seen.insert((e.from, e.to, e.kind)) {
            edges.push(e);
        } else if e.confidence == Confidence::Certain
            && let Some(x) = edges
                .iter_mut()
                .find(|x| x.from == e.from && x.to == e.to && x.kind == e.kind)
        {
            x.confidence = Confidence::Certain;
        }
    };

    // Import → file.
    let mut imported_names: Vec<HashMap<&str, Vec<u32>>> = Vec::with_capacity(files.len());
    let mut aliases: Vec<HashMap<&str, Vec<u32>>> = Vec::with_capacity(files.len());
    let mut external: Vec<HashSet<&str>> = Vec::with_capacity(files.len());
    for (fi, f) in files.iter().enumerate() {
        let mut names: HashMap<&str, Vec<u32>> = HashMap::new();
        let mut al: HashMap<&str, Vec<u32>> = HashMap::new();
        let mut ext: HashSet<&str> = HashSet::new();
        for imp in &f.facts.imports {
            // Python `from pkg import modul`: nama yang diimpor bisa berupa modul.
            if f.facts.lang == Lang::Python {
                for n in &imp.names {
                    let sub = if imp.source.ends_with('.') {
                        format!("{}{n}", imp.source)
                    } else {
                        format!("{}.{n}", imp.source)
                    };
                    let t = ix.resolve_import(fi, &sub, Lang::Python);
                    if !t.is_empty() {
                        al.entry(n.as_str()).or_default().extend(t);
                    }
                }
            }
            let targets = ix.resolve_import(fi, &imp.source, f.facts.lang);
            let alias = if imp.alias.is_empty() {
                imp.source
                    .rsplit(['/', '.', ':', '\\'])
                    .find(|s| !s.is_empty())
                    .unwrap_or("")
            } else {
                imp.alias.as_str()
            };
            if targets.is_empty() {
                for n in imp.names.iter().filter(|n| !al.contains_key(n.as_str())) {
                    ext.insert(n.as_str());
                }
                if !alias.is_empty() {
                    ext.insert(alias);
                }
                continue;
            }
            for &t in &targets {
                if t as usize != fi {
                    push(
                        &mut edges,
                        Edge {
                            from: ix.file_node[fi],
                            to: ix.file_node[t as usize],
                            kind: EdgeKind::Imports,
                            confidence: Confidence::Certain,
                            source: EdgeSource::TreeSitter,
                            line: imp.line,
                        },
                    );
                }
            }
            for n in &imp.names {
                names.entry(n.as_str()).or_default().extend(&targets);
            }
            if !alias.is_empty() {
                al.entry(alias).or_default().extend(&targets);
            }
        }
        imported_names.push(names);
        aliases.push(al);
        external.push(ext);
    }

    // Call.
    for (fi, f) in files.iter().enumerate() {
        let lang = f.facts.lang;
        let in_scip = covered.contains(f.path.as_str());
        for c in &f.facts.calls {
            called_names.insert(c.name.as_str());
            if in_scip {
                continue;
            }
            stats.calls += 1;
            let from = c
                .scope
                .map_or(ix.file_node[fi], |s| ix.sym_node[fi][s as usize]);
            let Some(cands) = ix.by_name.get(c.name.as_str()) else {
                stats.external += 1;
                continue;
            };
            let in_files = |set: &[u32]| -> Vec<u32> {
                cands
                    .iter()
                    .copied()
                    .filter(|&n| set.contains(&ix.nodes[n as usize].file))
                    .collect()
            };
            let recv = c.receiver.as_str();
            let recv_last = recv
                .rsplit(['.', ':', '>'])
                .find(|s| !s.is_empty())
                .unwrap_or(recv)
                .trim_start_matches('$');
            let fi32 = fi as u32;
            let mut found: Option<(Vec<u32>, Confidence)> = None;
            let self_recv = matches!(
                recv,
                "self" | "this" | "Self" | "$this" | "@" | "super" | "base" | "static" | "parent"
            );
            if self_recv && let Some(t) = ix.type_of_scope(from) {
                let mut ids = ix
                    .methods
                    .get(&(t, c.name.as_str()))
                    .cloned()
                    .unwrap_or_default();
                if ids.is_empty() {
                    for b in ix.bases.get(t).cloned().unwrap_or_default() {
                        ids.extend(
                            ix.methods
                                .get(&(b, c.name.as_str()))
                                .cloned()
                                .unwrap_or_default(),
                        );
                    }
                }
                if !ids.is_empty() {
                    found = Some((ids, Confidence::Certain));
                }
            }
            if found.is_none() && !recv.is_empty() && !self_recv {
                if let Some(t) = aliases[fi]
                    .get(recv_last)
                    .or_else(|| imported_names[fi].get(recv_last))
                {
                    // Modul/kelas yang diimpor: `api.validate`, `Money.zero`.
                    let mut ids = in_files(t);
                    let typed: Vec<u32> = ids
                        .iter()
                        .copied()
                        .filter(|&n| ix.owner[n as usize] == recv_last)
                        .collect();
                    if !typed.is_empty() {
                        ids = typed;
                    }
                    if !ids.is_empty() {
                        found = Some((ids, Confidence::Certain));
                    }
                } else if external[fi].contains(recv_last) {
                    stats.external += 1;
                    continue;
                } else if let Some(ids) = ix.methods.get(&(recv_last, c.name.as_str())) {
                    // Panggilan statis `Tipe::f()` / `Tipe.f()`.
                    let conf = if ids.len() == 1 {
                        Confidence::Certain
                    } else {
                        Confidence::Inferred
                    };
                    found = Some((ids.clone(), conf));
                } else if lang == Lang::Go
                    && let Some(t) = ix.type_of_scope(from)
                    && go_receiver(&f.facts, from, &ix) == Some(recv)
                {
                    let ids = ix
                        .methods
                        .get(&(t, c.name.as_str()))
                        .cloned()
                        .unwrap_or_default();
                    if !ids.is_empty() {
                        found = Some((ids, Confidence::Certain));
                    }
                }
            }
            if found.is_none() && recv.is_empty() {
                let same: Vec<u32> = in_files(&[fi32]);
                let same_owner: Vec<u32> = ix.type_of_scope(from).map_or_else(Vec::new, |t| {
                    same.iter()
                        .copied()
                        .filter(|&n| ix.owner[n as usize] == t)
                        .collect()
                });
                let free: Vec<u32> = same
                    .iter()
                    .copied()
                    .filter(|&n| {
                        ix.owner[n as usize].is_empty() || ix.nodes[n as usize].kind.is_type()
                    })
                    .collect();
                if !same_owner.is_empty() && implicit_this(lang) {
                    found = Some((same_owner, Confidence::Certain));
                } else if !free.is_empty() {
                    found = Some((free, Confidence::Certain));
                } else if let Some(t) = imported_names[fi].get(c.name.as_str()) {
                    let ids = in_files(t);
                    if !ids.is_empty() {
                        found = Some((ids, Confidence::Certain));
                    }
                } else if external[fi].contains(c.name.as_str()) {
                    stats.external += 1;
                    continue;
                }
                if found.is_none() && lang.package_is_dir() {
                    let dir = ix.by_dir.get(dir_of(&f.path)).cloned().unwrap_or_default();
                    let ids: Vec<u32> = in_files(&dir)
                        .into_iter()
                        .filter(|&n| {
                            ix.owner[n as usize].is_empty() || ix.nodes[n as usize].kind.is_type()
                        })
                        .collect();
                    if !ids.is_empty() {
                        found = Some((ids, Confidence::Certain));
                    }
                }
            }
            if found.is_none() {
                // Inferensi: kandidat unik.
                let caller_test = ix.nodes[from as usize].test;
                let pool: Vec<u32> = cands
                    .iter()
                    .copied()
                    .filter(|&n| {
                        let node = &ix.nodes[n as usize];
                        let is_method = !ix.owner[n as usize].is_empty();
                        (caller_test || !node.test)
                            && (recv.is_empty() && (!is_method || implicit_this(lang))
                                || !recv.is_empty() && is_method)
                            && compatible(lang, files[node.file as usize].facts.lang)
                    })
                    .collect();
                let pick = if pool.len() == 1 {
                    Some(pool.clone())
                } else if pool.len() > 1 {
                    let dir = dir_of(&f.path);
                    let near: Vec<u32> = pool
                        .iter()
                        .copied()
                        .filter(|&n| dir_of(&files[ix.nodes[n as usize].file as usize].path) == dir)
                        .collect();
                    (near.len() == 1).then_some(near)
                } else {
                    None
                };
                match pick {
                    Some(ids) => found = Some((ids, Confidence::Inferred)),
                    None => {
                        if pool.is_empty() {
                            stats.external += 1;
                        } else {
                            stats.ambiguous += 1;
                        }
                        continue;
                    }
                }
            }
            let Some((ids, conf)) = found else { continue };
            match conf {
                Confidence::Certain => stats.certain += 1,
                Confidence::Inferred => stats.inferred += 1,
            }
            for to in ids {
                push(
                    &mut edges,
                    Edge {
                        from,
                        to,
                        kind: EdgeKind::Calls,
                        confidence: conf,
                        source: EdgeSource::TreeSitter,
                        line: c.line,
                    },
                );
            }
        }
    }

    // SCIP.
    if let Some(s) = scip {
        let file_ix: HashMap<&str, usize> = files
            .iter()
            .enumerate()
            .map(|(i, f)| (f.path.as_str(), i))
            .collect();
        let innermost = |fi: usize, line: u32| -> Option<u32> {
            files[fi]
                .facts
                .symbols
                .iter()
                .enumerate()
                .filter(|(_, s)| s.start <= line && line <= s.end)
                .min_by_key(|(_, s)| s.end - s.start)
                .map(|(i, _)| ix.sym_node[fi][i])
        };
        for e in &s.edges {
            let (Some(&ff), Some(&tf)) = (
                file_ix.get(e.from_path.as_str()),
                file_ix.get(e.to_path.as_str()),
            ) else {
                continue;
            };
            let from = innermost(ff, e.from_line).unwrap_or(ix.file_node[ff]);
            let Some(to) = innermost(tf, e.to_line) else {
                continue;
            };
            if from == to || !ix.nodes[to as usize].kind.is_callable() {
                continue;
            }
            stats.scip_edges += 1;
            called_names.insert(
                files[tf].facts.symbols[ix.nodes[to as usize].symbol.unwrap_or(0) as usize]
                    .name
                    .as_str(),
            );
            push(
                &mut edges,
                Edge {
                    from,
                    to,
                    kind: EdgeKind::Calls,
                    confidence: Confidence::Certain,
                    source: EdgeSource::Scip,
                    line: e.from_line,
                },
            );
        }
    }

    // Pewarisan.
    for (fi, f) in files.iter().enumerate() {
        for inh in &f.facts.inherits {
            let owners: Vec<u32> = match inh.symbol {
                Some(s) => vec![ix.sym_node[fi][s as usize]],
                None => ix
                    .by_name
                    .get(inh.owner.as_str())
                    .map(|v| {
                        v.iter()
                            .copied()
                            .filter(|&n| ix.nodes[n as usize].kind.is_type())
                            .collect()
                    })
                    .unwrap_or_default(),
            };
            let Some(cands) = ix.by_name.get(inh.base.as_str()) else {
                continue;
            };
            let types: Vec<u32> = cands
                .iter()
                .copied()
                .filter(|&n| ix.nodes[n as usize].kind.is_type())
                .collect();
            let same: Vec<u32> = types
                .iter()
                .copied()
                .filter(|&n| ix.nodes[n as usize].file as usize == fi)
                .collect();
            let imported: Vec<u32> = imported_names[fi]
                .get(inh.base.as_str())
                .map(|t| {
                    types
                        .iter()
                        .copied()
                        .filter(|&n| t.contains(&ix.nodes[n as usize].file))
                        .collect()
                })
                .unwrap_or_default();
            let dir: Vec<u32> = if f.facts.lang.package_is_dir() {
                types
                    .iter()
                    .copied()
                    .filter(|&n| {
                        dir_of(&files[ix.nodes[n as usize].file as usize].path) == dir_of(&f.path)
                    })
                    .collect()
            } else {
                Vec::new()
            };
            let (targets, conf) = if !same.is_empty() {
                (same, Confidence::Certain)
            } else if !imported.is_empty() {
                (imported, Confidence::Certain)
            } else if !dir.is_empty() {
                (dir, Confidence::Certain)
            } else if types.len() == 1 && !external[fi].contains(inh.base.as_str()) {
                (types, Confidence::Inferred)
            } else {
                continue;
            };
            for &o in &owners {
                for &t in &targets {
                    if o != t {
                        push(
                            &mut edges,
                            Edge {
                                from: o,
                                to: t,
                                kind: EdgeKind::Inherits,
                                confidence: conf,
                                source: EdgeSource::TreeSitter,
                                line: inh.line,
                            },
                        );
                    }
                }
            }
        }
    }

    // Route → handler, permukaan lintas repo.
    let mut surface = Surface::default();
    for (fi, f) in files.iter().enumerate() {
        for r in &f.facts.routes {
            let id = ix.nodes.len() as u32;
            ix.nodes.push(Node {
                file: fi as u32,
                symbol: None,
                name: format!("{} {}", r.method, r.path),
                qualified: format!("{} {}", r.method, r.path),
                kind: SymbolKind::Route,
                start: r.line,
                end: r.line,
                exported: true,
                test: false,
                module: module_of(&f.path),
                central: false,
                dead: None,
                callers: 0,
                callees: 0,
            });
            ix.owner.push("");
            let handler = r
                .handler
                .map(|h| (vec![ix.sym_node[fi][h as usize]], Confidence::Certain))
                .or_else(|| {
                    if r.handler_name.is_empty() {
                        return None;
                    }
                    let c = ix.by_name.get(r.handler_name.as_str())?;
                    let same: Vec<u32> = c
                        .iter()
                        .copied()
                        .filter(|&n| ix.nodes[n as usize].file as usize == fi)
                        .collect();
                    let imported: Vec<u32> = imported_names[fi]
                        .get(r.handler_name.as_str())
                        .map(|t| {
                            c.iter()
                                .copied()
                                .filter(|&n| t.contains(&ix.nodes[n as usize].file))
                                .collect()
                        })
                        .unwrap_or_default();
                    if !same.is_empty() {
                        Some((same, Confidence::Certain))
                    } else if !imported.is_empty() {
                        Some((imported, Confidence::Certain))
                    } else if c.len() == 1 {
                        Some((c.clone(), Confidence::Inferred))
                    } else {
                        None
                    }
                });
            if let Some((hs, conf)) = handler {
                for h in hs {
                    called_names.insert(
                        files[ix.nodes[h as usize].file as usize].facts.symbols
                            [ix.nodes[h as usize].symbol.unwrap_or(0) as usize]
                            .name
                            .as_str(),
                    );
                    push(
                        &mut edges,
                        Edge {
                            from: id,
                            to: h,
                            kind: EdgeKind::Handles,
                            confidence: conf,
                            source: EdgeSource::TreeSitter,
                            line: r.line,
                        },
                    );
                }
            }
            surface.routes.push(SurfaceRoute {
                node: id,
                method: r.method.clone(),
                path: r.path.clone(),
            });
        }
        for h in &f.facts.http {
            let node = h
                .scope
                .map_or(ix.file_node[fi], |s| ix.sym_node[fi][s as usize]);
            surface.http.push(SurfaceHttp {
                node,
                method: h.method.clone(),
                path: h.path.clone(),
                exact: h.exact,
                host: h.host.clone(),
            });
        }
        for e in &f.facts.events {
            let node = e
                .scope
                .map_or(ix.file_node[fi], |s| ix.sym_node[fi][s as usize]);
            surface.events.push(SurfaceEvent {
                node,
                topic: e.topic.clone(),
                publish: e.publish,
            });
        }
        if let Some(p) = &f.facts.package {
            surface.packages.push(p.name.clone());
            surface.deps.extend(p.deps.iter().cloned());
        }
    }
    surface.packages.sort();
    surface.packages.dedup();
    surface.deps.sort();
    surface.deps.dedup();

    // Derajat.
    let mut nodes = ix.nodes;
    let mut has_caller = vec![false; nodes.len()];
    for e in &edges {
        match e.kind {
            EdgeKind::Calls => {
                if e.from != e.to {
                    nodes[e.to as usize].callers += 1;
                    nodes[e.from as usize].callees += 1;
                    has_caller[e.to as usize] = true;
                }
            }
            EdgeKind::Handles => has_caller[e.to as usize] = true,
            _ => {}
        }
    }
    // Dead code.
    let inherited: HashSet<&str> = files
        .iter()
        .flat_map(|f| f.facts.inherits.iter().map(|i| i.owner.as_str()))
        .collect();
    for (i, n) in nodes.iter_mut().enumerate() {
        if !n.kind.is_callable() || has_caller[i] || n.test || is_entry(&n.name) {
            continue;
        }
        let owner = ix.owner.get(i).copied().unwrap_or("");
        let maybe =
            n.exported || called_names.contains(n.name.as_str()) || inherited.contains(owner);
        n.dead = Some(if maybe {
            Confidence::Inferred
        } else {
            Confidence::Certain
        });
    }
    // Modul + node pusat.
    let mut mods: HashMap<String, ModuleInfo> = HashMap::new();
    for (i, n) in nodes.iter().enumerate() {
        let m = mods.entry(n.module.clone()).or_insert_with(|| ModuleInfo {
            name: n.module.clone(),
            files: 0,
            symbols: 0,
            central: None,
        });
        if n.kind == SymbolKind::File {
            m.files += 1;
        } else {
            m.symbols += 1;
        }
        if (n.kind.is_callable() || n.kind.is_type()) && n.callers + n.callees >= 2 {
            let better = m.central.is_none_or(|c| {
                let cn = &nodes[c as usize];
                n.callers + n.callees > cn.callers + cn.callees
            });
            if better {
                m.central = Some(i as u32);
            }
        }
    }
    let mut modules: Vec<ModuleInfo> = mods.into_values().collect();
    modules.sort_by(|a, b| a.name.cmp(&b.name));
    for m in &modules {
        if let Some(c) = m.central {
            nodes[c as usize].central = true;
        }
    }
    RepoGraph {
        nodes,
        edges,
        modules,
        surface,
        stats,
    }
}

fn is_entry(name: &str) -> bool {
    matches!(
        name,
        "main" | "init" | "constructor" | "setup" | "run" | "handler" | "default"
    ) || name.starts_with("__") && name.ends_with("__")
}

/// Bahasa dengan `this` implisit (method kelas dipanggil tanpa receiver).
const fn implicit_this(lang: Lang) -> bool {
    matches!(lang, Lang::Java | Lang::Kotlin | Lang::CSharp | Lang::Dart)
}

/// Call boleh lintas bahasa hanya di keluarga yang sama (TS ↔ JS).
fn compatible(a: Lang, b: Lang) -> bool {
    let fam = |l: Lang| match l {
        Lang::TypeScript | Lang::Tsx | Lang::JavaScript => 0,
        Lang::Java | Lang::Kotlin => 1,
        other => other as u8 + 10,
    };
    fam(a) == fam(b)
}

/// Nama variabel receiver method Go (`func (h *Handler)` → `h`).
fn go_receiver<'f>(facts: &'f FileFacts, scope: u32, ix: &Index<'_>) -> Option<&'f str> {
    let n = &ix.nodes[scope as usize];
    let s = &facts.symbols[n.symbol? as usize];
    let sig = s.signature.strip_prefix("func (")?;
    sig.split_whitespace().next()
}
