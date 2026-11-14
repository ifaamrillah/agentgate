//! Data tampilan Code graph (M7): status index repo, hasil cari simbol, trace, relasi lintas repo,
//! riwayat git, kualitas kode. Angka diformat di sini (pemisah ribuan Indonesia).
use agentgate_graph::{Lang, SymbolKind};
use chrono::Utc;
use serde::Serialize;
use uuid::Uuid;

use crate::{
    models::{graph_repos, repos},
    views::runner::{Age, age},
};

/// `18420` → `18.420`.
#[must_use]
pub fn num(n: i64) -> String {
    let s = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push('.');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

/// `96.43` → `96,4`.
#[must_use]
pub fn pct(x: f64) -> String {
    format!("{x:.1}").replace('.', ",")
}

/// Warna bahasa untuk `lang_bar` (data, bukan gaya template).
#[must_use]
pub fn lang_color(lang: &str) -> &'static str {
    match lang {
        "typescript" | "tsx" => "#3178c6",
        "javascript" => "#e2b714",
        "python" => "#3572a5",
        "go" => "#00add8",
        "rust" => "#c4703a",
        "java" => "#b07219",
        "kotlin" => "#a97bff",
        "php" => "#4f5d95",
        "csharp" => "#178600",
        "dart" => "#00b4ab",
        _ => "#8d8d99",
    }
}

/// Nama bahasa untuk tampilan.
#[must_use]
pub fn lang_label(lang: &str) -> String {
    Lang::parse(lang).map_or_else(|| lang.to_string(), |l| l.label().to_string())
}

/// Satu segmen bar bahasa.
#[derive(Debug, Clone, Serialize)]
pub struct LangPart {
    pub label: String,
    pub percent: u32,
    pub color: &'static str,
}

#[must_use]
pub fn lang_parts(row: &graph_repos::Model) -> Vec<LangPart> {
    let langs = row.languages();
    let total: u32 = langs.iter().map(|(_, c)| c.lines).sum::<u32>().max(1);
    langs
        .iter()
        .map(|(l, c)| LangPart {
            label: lang_label(l),
            percent: (c.lines * 100 / total).max(1),
            color: lang_color(l),
        })
        .collect()
}

/// Status index satu repo untuk tabel "Repo & index".
#[derive(Debug, Clone, Serialize)]
pub struct RepoIndexView {
    pub id: Uuid,
    pub name: String,
    pub branch: String,
    pub connected: bool,
    pub files: String,
    pub skipped: i32,
    pub status: String,
    pub status_key: &'static str,
    pub status_variant: &'static str,
    pub source_key: &'static str,
    /// Cara index server mengambil kode (`clone`/`tarball`) dan alasannya (R2).
    pub fetch_via: String,
    pub fetch_note: String,
    pub commit: String,
    pub indexed: Option<Age>,
    pub error: String,
    pub scip: Option<ScipView>,
    pub symbols: String,
    pub edges: String,
    pub certain_pct: String,
    pub coverage: String,
    pub coverage_width: u32,
    pub langs: Vec<LangPart>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScipView {
    pub indexer: String,
    pub by: String,
    pub age: Option<Age>,
    /// Lebih dari 7 hari dan index tree-sitter lebih baru.
    pub stale: bool,
}

impl RepoIndexView {
    #[must_use]
    pub fn of(repo: &repos::Model, row: Option<&graph_repos::Model>) -> Self {
        let now = Utc::now();
        let empty = graph_repos::Model::empty();
        let r = row.unwrap_or(&empty);
        let (status_key, status_variant) = match r.status() {
            graph_repos::IndexStatus::None => ("graph-status-none", "neutral"),
            graph_repos::IndexStatus::Queued => ("graph-status-queued", "info"),
            graph_repos::IndexStatus::Running => ("graph-status-running", "ai"),
            graph_repos::IndexStatus::Ready => ("graph-status-ready", "success"),
            graph_repos::IndexStatus::Failed => ("graph-status-failed", "destructive"),
        };
        let scip = (!r.scip_indexer.is_empty()).then(|| ScipView {
            indexer: r.scip_indexer.clone(),
            by: r.scip_by.clone(),
            age: r.scip_at.map(|t| age(&t, now)),
            stale: r
                .scip_at
                .zip(r.indexed_at)
                .is_some_and(|(s, i)| i > s && (now - s.with_timezone(&Utc)).num_days() > 7),
        });
        let certain = if r.edges > 0 {
            f64::from(r.edges_certain) * 100.0 / f64::from(r.edges)
        } else {
            0.0
        };
        Self {
            id: repo.id,
            name: repo.name.clone(),
            branch: if r.branch.is_empty() {
                repo.default_branch.clone()
            } else {
                r.branch.clone()
            },
            connected: repo.connection_id.is_some(),
            files: num(i64::from(r.files_indexed)),
            skipped: r.files_skipped,
            status: r.status.clone(),
            status_key,
            status_variant,
            source_key: match r.source() {
                Some(graph_repos::IndexSource::Cli) => "graph-source-cli",
                Some(graph_repos::IndexSource::Server) => match r.fetch_via.as_str() {
                    "clone" => "graph-source-clone",
                    "tarball" => "graph-source-tarball",
                    _ => "graph-source-server",
                },
                None => "",
            },
            fetch_via: r.fetch_via.clone(),
            fetch_note: r.fetch_note.clone(),
            commit: r.commit_sha.chars().take(7).collect(),
            indexed: r.indexed_at.map(|t| age(&t, now)),
            error: r.error.clone(),
            scip,
            symbols: num(i64::from(r.symbols)),
            edges: num(i64::from(r.edges)),
            certain_pct: pct(certain),
            coverage: pct(r.coverage()),
            coverage_width: r.coverage().round() as u32,
            langs: lang_parts(r),
        }
    }
}

/// Satu simbol dalam daftar.
#[derive(Debug, Clone, Serialize)]
pub struct SymbolRow {
    pub id: Uuid,
    pub href: String,
    pub name: String,
    pub qualified: String,
    pub kind: String,
    pub kind_key: String,
    pub icon: &'static str,
    pub repo: String,
    pub path: String,
    pub line: i32,
    pub end: i32,
    pub signature: String,
    pub doc: String,
    pub dead: String,
    pub test: bool,
    pub exported: bool,
    pub callers: i32,
    pub callees: i32,
    pub complexity: i32,
    pub module: String,
    pub lang: String,
}

#[must_use]
pub fn kind_icon(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::File => "file-code",
        SymbolKind::Function => "function",
        SymbolKind::Method => "math-function",
        SymbolKind::Class | SymbolKind::Struct => "box",
        SymbolKind::Interface | SymbolKind::Trait => "puzzle",
        SymbolKind::Enum => "list-details",
        SymbolKind::Type => "braces",
        SymbolKind::Module => "folder",
        SymbolKind::Route => "route",
    }
}

/// Satu langkah trace.
#[derive(Debug, Clone, Serialize)]
pub struct HopRow {
    pub symbol: SymbolRow,
    pub depth: u8,
    pub confidence: &'static str,
    pub via: String,
    pub via_key: String,
}

/// Satu relasi lintas repo.
#[derive(Debug, Clone, Serialize)]
pub struct LinkView {
    pub kind: String,
    pub kind_key: String,
    pub confidence: String,
    pub confidence_key: String,
    pub from_repo: String,
    pub from_name: String,
    pub from_href: Option<String>,
    pub to_repo: String,
    pub to_name: String,
    pub to_href: Option<String>,
}

/// Baris kode dengan nomor (`code_view`).
#[derive(Debug, Clone, Serialize)]
pub struct CodeLine {
    pub n: u32,
    pub text: String,
}

#[must_use]
pub fn code_lines(code: &str, start: u32) -> Vec<CodeLine> {
    code.lines()
        .enumerate()
        .map(|(i, l)| CodeLine {
            n: start + u32::try_from(i).unwrap_or(0),
            text: l.replace('\t', "    "),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_use_indonesian_separators() {
        assert_eq!(num(18_420), "18.420");
        assert_eq!(num(999), "999");
        assert_eq!(num(1_234_567), "1.234.567");
        assert_eq!(pct(96.43), "96,4");
    }
}
