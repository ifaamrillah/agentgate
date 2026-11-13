//! `agentgate graph index|scip|status`: index code graph repo ini ke server.
//!
//! Index inkremental: CLI menghitung sha semua file, server menjawab file mana yang berubah,
//! hanya file itu yang dikirim. File rahasia (`.env`, kunci privat, kredensial) tidak pernah
//! dibaca isinya; file > 1 MB, hasil generate, dan biner dilaporkan sebagai dilewati.
use std::{
    io::Write,
    path::{Path, PathBuf},
};

use agentgate_graph::{
    Lang, history,
    protocol::{
        FinishRequest, IndexSummary, ManifestFile, ManifestRequest, ManifestResponse, ScipRequest,
        UPLOAD_BATCH, UploadFile, UploadRequest, UploadResponse,
    },
    walk::{self, Entry},
};

use crate::{
    api::Api,
    config::ProjectFile,
    error::{Error, Result, io_at},
    git, install,
};

/// Batas byte isi per batch upload (server menerima hingga 24 MB).
pub const BATCH_BYTES: usize = 8 * 1024 * 1024;

/// Repo saat ini: (root, kode project, nama repo).
fn target(project: Option<&str>, repo: Option<&str>) -> Result<(PathBuf, String, String)> {
    let root = install::repo_root()?;
    let key = match project {
        Some(p) => p.to_string(),
        None => {
            ProjectFile::load(&root)?
                .ok_or(Error::ProjectRequired)?
                .project
                .key
        }
    };
    let name = repo.map_or_else(|| git::repo_name(&root), str::to_string);
    Ok((root, key, name))
}

/// Pecah file menjadi batch (maks. [`UPLOAD_BATCH`] file atau [`BATCH_BYTES`] isi).
#[must_use]
pub fn batches(files: Vec<UploadFile>) -> Vec<Vec<UploadFile>> {
    let mut out: Vec<Vec<UploadFile>> = Vec::new();
    let mut cur: Vec<UploadFile> = Vec::new();
    let mut size = 0usize;
    for f in files {
        let n = f.content.as_ref().map_or(0, String::len);
        if !cur.is_empty() && (cur.len() >= UPLOAD_BATCH || size + n > BATCH_BYTES) {
            out.push(std::mem::take(&mut cur));
            size = 0;
        }
        size += n;
        cur.push(f);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Statistik git 90 hari terakhir (kosong bila git gagal, mis. clone dangkal).
fn git_stats(root: &Path) -> Option<history::GitStats> {
    let log = git::git(root, history::GIT_LOG_ARGS).ok()?;
    Some(history::parse(&log, |p| Lang::detect(p).is_some()))
}

/// `agentgate graph index [--full]`.
///
/// # Errors
///
/// Bukan repo git, project tidak diketahui, server menolak.
pub async fn index(
    api: &Api,
    project: Option<&str>,
    repo: Option<&str>,
    full: bool,
    out: &mut impl Write,
) -> Result<IndexSummary> {
    let (root, key, name) = target(project, repo)?;
    let _ = writeln!(out, "Membaca file repo {name}…");
    let walk_root = root.clone();
    let entries: Vec<Entry> = tokio::task::spawn_blocking(move || walk::walk(&walk_root))
        .await
        .map_err(|e| Error::Graph(e.to_string()))?;
    let commit = git::git(&root, &["rev-parse", "HEAD"]).unwrap_or_default();
    let branch = git::current_branch(&root).unwrap_or_default();
    let req = ManifestRequest {
        project: key.clone(),
        repo: name.clone(),
        commit: commit.trim().to_string(),
        branch,
        full,
        files: entries
            .iter()
            .map(|e| ManifestFile {
                path: e.path.clone(),
                sha: e.sha.clone(),
            })
            .collect(),
    };
    let diff: ManifestResponse = api.post("/api/cli/graph/manifest", &req).await?;
    let _ = writeln!(
        out,
        "{} file, {} berubah, {} tidak berubah, {} dihapus dari index.",
        entries.len(),
        diff.need.len(),
        diff.unchanged,
        diff.removed
    );
    let need: std::collections::HashSet<&str> = diff.need.iter().map(String::as_str).collect();
    let files: Vec<UploadFile> = entries
        .iter()
        .filter(|e| need.contains(e.path.as_str()))
        .map(|e| UploadFile {
            path: e.path.clone(),
            sha: e.sha.clone(),
            bytes: e.bytes,
            content: e.content.clone(),
            skip: e.skip,
        })
        .collect();
    let mut parsed = 0;
    let mut skipped = 0;
    let all = batches(files);
    let total = all.len();
    for (i, batch) in all.into_iter().enumerate() {
        let res: UploadResponse = api
            .post(
                "/api/cli/graph/upload",
                &UploadRequest {
                    project: key.clone(),
                    repo: name.clone(),
                    files: batch,
                },
            )
            .await?;
        parsed += res.parsed;
        skipped += res.skipped;
        let _ = writeln!(out, "  batch {}/{total} terkirim", i + 1);
    }
    let _ = writeln!(
        out,
        "Membangun graph ({parsed} file di-parse, {skipped} dilewati)…"
    );
    let summary: IndexSummary = api
        .post(
            "/api/cli/graph/finish",
            &FinishRequest {
                project: key,
                repo: name,
                git: git_stats(&root),
            },
        )
        .await?;
    let _ = writeln!(out, "{}", describe(&summary));
    Ok(summary)
}

/// Satu baris ringkasan index.
#[must_use]
pub fn describe(s: &IndexSummary) -> String {
    let pct = if s.edges > 0 {
        s.edges_certain * 100 / s.edges
    } else {
        0
    };
    let mut line = format!(
        "{}: {} — {} file, {} dilewati, {} simbol, {} relasi ({pct}% pasti), {} lintas repo",
        s.repo, s.status, s.files_indexed, s.files_skipped, s.symbols, s.edges, s.links
    );
    if !s.scip_indexer.is_empty() {
        line.push_str(&format!(", SCIP {}", s.scip_indexer));
    }
    if !s.error.is_empty() {
        line.push_str(&format!(" (galat: {})", s.error));
    }
    line
}

/// `agentgate graph scip [FILE]`: kirim index SCIP (`scip-typescript`, `rust-analyzer scip`, …).
///
/// # Errors
///
/// File tidak terbaca atau bukan SCIP, server menolak.
pub async fn scip(
    api: &Api,
    project: Option<&str>,
    repo: Option<&str>,
    file: Option<&Path>,
    indexer: &str,
    out: &mut impl Write,
) -> Result<IndexSummary> {
    let (root, key, name) = target(project, repo)?;
    let path = file.map_or_else(|| root.join("index.scip"), Path::to_path_buf);
    let bytes = std::fs::read(&path).map_err(io_at(&path))?;
    let indexer = indexer.to_string();
    let data = tokio::task::spawn_blocking(move || agentgate_graph::scip::read(&bytes, &indexer))
        .await
        .map_err(|e| Error::Graph(e.to_string()))?
        .map_err(|e| Error::Graph(e.to_string()))?;
    let _ = writeln!(
        out,
        "SCIP {}: {} dokumen, {} referensi call.",
        data.indexer,
        data.documents.len(),
        data.edges.len()
    );
    let summary: IndexSummary = api
        .post(
            "/api/cli/graph/scip",
            &ScipRequest {
                project: key,
                repo: name,
                scip: data,
            },
        )
        .await?;
    let _ = writeln!(out, "{}", describe(&summary));
    Ok(summary)
}

/// `agentgate graph status`.
///
/// # Errors
///
/// Project tidak diketahui, server menolak.
pub async fn status(api: &Api, project: Option<&str>, out: &mut impl Write) -> Result<()> {
    let key = match project {
        Some(p) => p.to_string(),
        None => target(None, None)?.1,
    };
    let list: Vec<IndexSummary> = api
        .get(&format!(
            "/api/cli/graph/status?project={}",
            urlencode(&key)
        ))
        .await?;
    if list.is_empty() {
        let _ = writeln!(out, "Project {key} belum punya repo.");
    }
    for s in &list {
        let _ = writeln!(out, "{}", describe(s));
    }
    Ok(())
}

fn urlencode(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c.to_string()
            } else {
                format!("%{:02X}", c as u32)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, size: usize) -> UploadFile {
        UploadFile {
            path: path.into(),
            sha: String::new(),
            bytes: size as u64,
            content: Some("x".repeat(size)),
            skip: None,
        }
    }

    #[test]
    fn batches_respect_count_and_bytes() {
        let many: Vec<UploadFile> = (0..120).map(|i| file(&format!("f{i}.ts"), 10)).collect();
        let b = batches(many);
        assert_eq!(b.iter().map(Vec::len).collect::<Vec<_>>(), vec![50, 50, 20]);
        let big = vec![
            file("a.ts", 900_000),
            file("b.ts", 900_000),
            file("c.ts", 900_000),
        ];
        let b = batches(big.into_iter().cycle().take(20).collect());
        assert!(
            b.iter()
                .all(|x| x.iter().map(|f| f.bytes as usize).sum::<usize>() <= BATCH_BYTES)
        );
        assert!(batches(Vec::new()).is_empty());
    }

    #[test]
    fn describe_reports_confidence_share() {
        let s = IndexSummary {
            repo: "kasir-api".into(),
            status: "ready".into(),
            commit: String::new(),
            files_indexed: 10,
            files_skipped: 2,
            symbols: 40,
            edges: 50,
            edges_certain: 45,
            links: 3,
            scip_indexer: String::new(),
            indexed_at: None,
            error: String::new(),
        };
        assert_eq!(
            describe(&s),
            "kasir-api: ready — 10 file, 2 dilewati, 40 simbol, 50 relasi (90% pasti), 3 lintas repo"
        );
    }
}
