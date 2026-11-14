//! `agentgate knowledge pull|status|sync`: dokumen knowledge project di
//! `.agentgate/knowledge/<slug>.md` (front matter: judul, jenis, kelompok, versi server).
//!
//! - `pull` menulis versi server (menimpa file lokal; file lokal yang berubah dilaporkan dulu
//!   dan dilewati kecuali `--force`).
//! - `status` membandingkan file lokal dengan server.
//! - `sync` mengunggah file lokal yang berubah (versi sama → versi baru di server; versi
//!   berbeda atau dokumen baru → usulan untuk pemilik knowledge), lalu `pull` dan commit.
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    api::Api,
    config::ProjectFile,
    error::{Error, Result, io_at},
    git, install,
};

pub const DIR: &str = agentgate_core::knowledge::DIR;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServerDoc {
    pub slug: String,
    pub title: String,
    #[serde(rename = "type")]
    pub type_key: String,
    #[serde(default)]
    pub group: String,
    pub version: i32,
    pub body: String,
}

#[derive(Debug, Deserialize)]
struct Export {
    docs: Vec<ServerDoc>,
}

/// File lokal hasil parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalDoc {
    pub slug: String,
    pub title: String,
    pub type_key: String,
    pub version: i32,
    pub body: String,
}

#[derive(Debug, Serialize)]
struct ImportDoc<'a> {
    slug: &'a str,
    title: &'a str,
    #[serde(rename = "type")]
    type_key: &'a str,
    base_version: i32,
    body: &'a str,
}

#[derive(Debug, Deserialize)]
struct ImportResult {
    slug: String,
    result: String,
    #[serde(default)]
    version: i32,
    #[serde(default)]
    error: String,
}

#[derive(Debug, Deserialize)]
struct ImportResponse {
    results: Vec<ImportResult>,
}

/// Isi file `.md` untuk dokumen server.
#[must_use]
pub fn render(doc: &ServerDoc) -> String {
    agentgate_core::knowledge::render(
        &doc.title,
        &doc.type_key,
        &doc.group,
        doc.version,
        &doc.body,
    )
}

/// Parse file `.md` (front matter opsional; tanpa front matter = dokumen baru versi 0).
#[must_use]
pub fn parse(slug: &str, src: &str) -> LocalDoc {
    let mut doc = LocalDoc {
        slug: slug.to_string(),
        title: String::new(),
        type_key: String::new(),
        version: 0,
        body: src.trim().to_string(),
    };
    let Some(rest) = src.strip_prefix("---\n") else {
        return doc;
    };
    let Some(end) = rest.find("\n---") else {
        return doc;
    };
    for line in rest[..end].lines() {
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        let v = v.trim().to_string();
        match k.trim() {
            "title" => doc.title = v,
            "type" => doc.type_key = v,
            "version" => doc.version = v.parse().unwrap_or(0),
            _ => {}
        }
    }
    doc.body = rest[end + 4..].trim().to_string();
    doc
}

/// Status satu dokumen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Same,
    /// Diubah di repo (versi dasar = versi server).
    Local,
    /// Versi lokal lebih lama dari server dan isinya berbeda (tertinggal, atau diubah di repo
    /// dan server sekaligus; server yang memutuskan saat `sync`).
    Conflict,
    /// Hanya di repo.
    New,
    /// Hanya di server.
    Missing,
}

impl State {
    const fn label(self) -> &'static str {
        match self {
            Self::Same => "sama",
            Self::Local => "diubah di repo",
            Self::Conflict => "versi server lebih baru",
            Self::New => "baru di repo",
            Self::Missing => "belum ditarik",
        }
    }
}

/// Bandingkan file lokal dengan dokumen server (urut slug).
#[must_use]
pub fn compare(local: &[LocalDoc], server: &[ServerDoc]) -> Vec<(String, State)> {
    let mut out: BTreeMap<String, State> = BTreeMap::new();
    for s in server {
        let state = match local.iter().find(|l| l.slug == s.slug) {
            None => State::Missing,
            Some(l) => {
                let edited =
                    l.body.trim() != s.body.trim() || (!l.title.is_empty() && l.title != s.title);
                match (edited, l.version == s.version) {
                    (false, _) => State::Same,
                    (true, true) => State::Local,
                    (true, false) => State::Conflict,
                }
            }
        };
        out.insert(s.slug.clone(), state);
    }
    for l in local {
        out.entry(l.slug.clone()).or_insert(State::New);
    }
    out.into_iter().collect()
}

fn read_local(dir: &Path) -> Result<Vec<LocalDoc>> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(out);
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "md") {
            continue;
        }
        let Some(slug) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let src = fs::read_to_string(&path).map_err(io_at(&path))?;
        out.push(parse(slug, &src));
    }
    out.sort_by(|a, b| a.slug.cmp(&b.slug));
    Ok(out)
}

fn target(project: Option<&str>) -> Result<(PathBuf, String)> {
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
    Ok((root, key))
}

async fn export(api: &Api, key: &str) -> Result<Vec<ServerDoc>> {
    let e: Export = api
        .get(&format!("/api/cli/knowledge/export?project={key}"))
        .await?;
    Ok(e.docs)
}

/// Tulis dokumen server ke folder; file yang `skip` tidak disentuh. Hasil: path yang ditulis.
fn write_all(root: &Path, docs: &[ServerDoc], skip: &[String]) -> Result<Vec<String>> {
    let dir = root.join(DIR);
    fs::create_dir_all(&dir).map_err(io_at(&dir))?;
    let mut written = Vec::new();
    for d in docs {
        if skip.contains(&d.slug) || d.slug.contains('/') || d.slug.contains("..") {
            continue;
        }
        let path = dir.join(format!("{}.md", d.slug));
        let body = render(d);
        if fs::read_to_string(&path).ok().as_deref() != Some(body.as_str()) {
            fs::write(&path, body).map_err(io_at(&path))?;
            written.push(format!("{DIR}/{}.md", d.slug));
        }
    }
    Ok(written)
}

/// `agentgate knowledge status`.
///
/// # Errors
///
/// Bukan repo/project, server menolak.
pub async fn status(api: &Api, project: Option<&str>, out: &mut impl Write) -> Result<()> {
    let (root, key) = target(project)?;
    let server = export(api, &key).await?;
    let local = read_local(&root.join(DIR))?;
    let rows = compare(&local, &server);
    if rows.is_empty() {
        let _ = writeln!(out, "Project {key} belum punya dokumen knowledge.");
        return Ok(());
    }
    for (slug, state) in &rows {
        let _ = writeln!(out, "{:<28} {}", slug, state.label());
    }
    Ok(())
}

/// `agentgate knowledge pull`.
///
/// # Errors
///
/// Bukan repo/project, server menolak, gagal menulis.
pub async fn pull(
    api: &Api,
    project: Option<&str>,
    force: bool,
    out: &mut impl Write,
) -> Result<Vec<String>> {
    let (root, key) = target(project)?;
    let server = export(api, &key).await?;
    let local = read_local(&root.join(DIR))?;
    let skip: Vec<String> = if force {
        Vec::new()
    } else {
        compare(&local, &server)
            .into_iter()
            .filter(|(_, s)| matches!(s, State::Local | State::Conflict))
            .map(|(slug, _)| slug)
            .collect()
    };
    for s in &skip {
        let _ = writeln!(
            out,
            "Dilewati {s}: diubah di repo (jalankan `agentgate knowledge sync` atau --force)."
        );
    }
    let written = write_all(&root, &server, &skip)?;
    let _ = writeln!(
        out,
        "{} dokumen ditulis ke {DIR}/ ({} dokumen di server).",
        written.len(),
        server.len()
    );
    Ok(written)
}

/// `agentgate knowledge sync`.
///
/// # Errors
///
/// Bukan repo/project, ada perubahan staged, server menolak, git gagal.
pub async fn sync(api: &Api, project: Option<&str>, out: &mut impl Write) -> Result<bool> {
    let (root, key) = target(project)?;
    if git::has_staged(&root)? {
        return Err(Error::StagedChanges);
    }
    let server = export(api, &key).await?;
    let local = read_local(&root.join(DIR))?;
    let changed: Vec<&LocalDoc> = compare(&local, &server)
        .into_iter()
        .filter(|(_, s)| matches!(s, State::Local | State::Conflict | State::New))
        .filter_map(|(slug, _)| local.iter().find(|l| l.slug == slug))
        .collect();
    let mut proposed: Vec<String> = Vec::new();
    if !changed.is_empty() {
        let docs: Vec<ImportDoc<'_>> = changed
            .iter()
            .map(|l| ImportDoc {
                slug: &l.slug,
                title: &l.title,
                type_key: &l.type_key,
                base_version: l.version,
                body: &l.body,
            })
            .collect();
        let res: ImportResponse = api
            .post(
                "/api/cli/knowledge/import",
                &serde_json::json!({ "project": key, "docs": docs }),
            )
            .await?;
        for r in &res.results {
            let msg = match r.result.as_str() {
                "saved" => format!("v{} disimpan di server", r.version),
                "proposed" => {
                    proposed.push(r.slug.clone());
                    "dikirim sebagai usulan (menunggu pemilik knowledge)".to_string()
                }
                "unchanged" => "tidak berubah".to_string(),
                "behind" => "tertinggal; ditarik dari server".to_string(),
                _ => format!("gagal: {}", r.error),
            };
            let _ = writeln!(out, "{:<28} {msg}", r.slug);
        }
    }
    // Tarik ulang; file yang menjadi usulan tetap versi lokal sampai diputuskan.
    let server = export(api, &key).await?;
    let mut written = write_all(&root, &server, &proposed)?;
    written.retain(|p| !p.is_empty());
    let mut touched: Vec<String> = written;
    touched.extend(changed.iter().map(|l| format!("{DIR}/{}.md", l.slug)));
    touched.sort();
    touched.dedup();
    if touched.is_empty() {
        let _ = writeln!(out, "Knowledge {key} sudah sinkron.");
        return Ok(false);
    }
    install::stage(&root, &[DIR.to_string()])?;
    let committed = git::commit(&root, "docs(knowledge): sinkron dengan Agent Gate")?;
    let _ = writeln!(
        out,
        "{} file knowledge diperbarui{}.",
        touched.len(),
        if committed { " dan di-commit" } else { "" }
    );
    Ok(committed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(slug: &str, version: i32, body: &str) -> ServerDoc {
        ServerDoc {
            slug: slug.into(),
            title: "Judul".into(),
            type_key: "overview".into(),
            group: "gambaran-umum".into(),
            version,
            body: body.into(),
        }
    }

    #[test]
    fn render_and_parse_round_trip() {
        let d = server("overview", 3, "# Isi\n\nTeks");
        let l = parse("overview", &render(&d));
        assert_eq!(l.title, "Judul");
        assert_eq!(l.type_key, "overview");
        assert_eq!(l.version, 3);
        assert_eq!(l.body, "# Isi\n\nTeks");
        let bare = parse("baru", "# Tanpa front matter");
        assert_eq!(bare.version, 0);
        assert_eq!(bare.body, "# Tanpa front matter");
    }

    #[test]
    fn compare_detects_each_state() {
        let srv = vec![
            server("a", 2, "sama"),
            server("b", 2, "server"),
            server("c", 3, "server baru"),
            server("d", 1, "x"),
        ];
        let local = vec![
            parse("a", &render(&server("a", 2, "sama"))),
            parse("b", &render(&server("b", 2, "lokal"))),
            parse("c", &render(&server("c", 2, "lokal lama"))),
            parse("e", "baru"),
        ];
        let rows = compare(&local, &srv);
        let get = |s: &str| rows.iter().find(|(k, _)| k == s).unwrap().1;
        assert_eq!(get("a"), State::Same);
        assert_eq!(get("b"), State::Local);
        assert_eq!(get("c"), State::Conflict);
        assert_eq!(get("d"), State::Missing);
        assert_eq!(get("e"), State::New);
    }
}
