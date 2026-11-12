//! Impor skill pack dari repo GitHub publik: unduh tarball (`codeload.github.com`), ambil hanya
//! `SKILL.md`, `scripts/`, dan `references/` (teks UTF-8), dengan batas jumlah dan ukuran. Host
//! lain dan repo privat lewat runner (M5).

use std::{io::Read, time::Duration};

use crate::{
    common::settings::SkillSettings,
    models::skill_packs::{
        FETCH_FAILED, FILE_TOO_LARGE, MAX_FILE_BYTES, MAX_FILES, MAX_TOTAL_BYTES, PATH_INVALID,
        REPO_URL_INVALID, SKILL_MD_REQUIRED, TOO_MANY_FILES,
    },
};

/// Batas ukuran tarball yang diunduh (byte).
const MAX_ARCHIVE_BYTES: usize = 20 * 1024 * 1024;

/// Repo GitHub yang akan diimpor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoRef {
    pub owner: String,
    pub repo: String,
    pub git_ref: String,
    /// Folder di dalam repo (`""` = akar).
    pub path: String,
}

impl RepoRef {
    #[must_use]
    pub fn url(&self) -> String {
        format!("https://github.com/{}/{}", self.owner, self.repo)
    }
}

fn name_ok(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 100
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        && s != "."
        && s != ".."
}

/// `https://github.com/owner/repo[.git][/]` + ref + folder.
///
/// # Errors
///
/// Kode error (`skill-repo-url-invalid`, `skill-path-invalid`).
pub fn parse(url: &str, git_ref: &str, path: &str) -> Result<RepoRef, &'static str> {
    let rest = url
        .trim()
        .strip_prefix("https://github.com/")
        .ok_or(REPO_URL_INVALID)?;
    let rest = rest.trim_end_matches('/');
    let rest = rest.strip_suffix(".git").unwrap_or(rest);
    let mut parts = rest.split('/');
    let (Some(owner), Some(repo), None) = (parts.next(), parts.next(), parts.next()) else {
        return Err(REPO_URL_INVALID);
    };
    if !name_ok(owner) || !name_ok(repo) {
        return Err(REPO_URL_INVALID);
    }
    let git_ref = match git_ref.trim() {
        "" => "main",
        r => r,
    };
    if git_ref.len() > 100
        || git_ref.starts_with(['-', '/'])
        || git_ref.contains("..")
        || !git_ref
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'))
    {
        return Err(REPO_URL_INVALID);
    }
    let path = path.trim().trim_matches('/');
    if path.split('/').any(|s| s == ".." || s == ".") || path.len() > 200 || path.contains('\\') {
        return Err(PATH_INVALID);
    }
    Ok(RepoRef {
        owner: owner.to_string(),
        repo: repo.to_string(),
        git_ref: git_ref.to_string(),
        path: path.to_string(),
    })
}

/// Isi skill pack hasil unduhan.
#[derive(Debug, Clone, Default)]
pub struct Fetched {
    pub files: Vec<(String, String)>,
    pub commit_sha: Option<String>,
}

/// Path di dalam skill pack yang disimpan: `SKILL.md` di mana saja, atau berkas di bawah folder
/// `scripts/` / `references/`.
#[must_use]
pub fn wanted(path: &str) -> bool {
    let segments: Vec<&str> = path.split('/').collect();
    segments.last() == Some(&"SKILL.md")
        || segments
            .iter()
            .rev()
            .skip(1)
            .any(|s| *s == "scripts" || *s == "references")
}

/// Ambil berkas skill dari tarball `.tar.gz` GitHub (folder teratas `repo-ref/` dibuang).
///
/// # Errors
///
/// Kode error skill pack.
pub fn extract(archive: &[u8], folder: &str) -> Result<Fetched, &'static str> {
    let gz = flate2::read::GzDecoder::new(archive);
    let mut tar = tar::Archive::new(gz);
    let mut out = Fetched::default();
    let mut total = 0usize;
    let prefix = if folder.is_empty() {
        String::new()
    } else {
        format!("{folder}/")
    };
    for entry in tar.entries().map_err(|_| FETCH_FAILED)? {
        let mut entry = entry.map_err(|_| FETCH_FAILED)?;
        let kind = entry.header().entry_type();
        if kind.is_pax_global_extensions() {
            // `git archive` menulis sha commit di header global (`comment`).
            if let Ok(Some(exts)) = entry.pax_extensions() {
                for ext in exts.flatten() {
                    if ext.key() == Ok("comment")
                        && let Ok(v) = ext.value()
                        && v.trim().len() == 40
                        && v.trim().chars().all(|c| c.is_ascii_hexdigit())
                    {
                        out.commit_sha = Some(v.trim().to_string());
                    }
                }
            }
            continue;
        }
        if !kind.is_file() {
            continue;
        }
        let raw = entry.path().map_err(|_| FETCH_FAILED)?;
        let raw = raw.to_string_lossy().to_string();
        // Buang folder teratas `repo-ref/`.
        let Some((_, inner)) = raw.split_once('/') else {
            continue;
        };
        let Some(rel) = inner.strip_prefix(&prefix) else {
            continue;
        };
        if rel.is_empty() || !wanted(rel) || rel.split('/').any(|s| s == ".." || s.is_empty()) {
            continue;
        }
        let size = usize::try_from(entry.header().size().unwrap_or(0)).unwrap_or(usize::MAX);
        if size > MAX_FILE_BYTES {
            return Err(FILE_TOO_LARGE);
        }
        let mut body = Vec::with_capacity(size);
        entry
            .by_ref()
            .take(MAX_FILE_BYTES as u64 + 1)
            .read_to_end(&mut body)
            .map_err(|_| FETCH_FAILED)?;
        if body.len() > MAX_FILE_BYTES {
            return Err(FILE_TOO_LARGE);
        }
        // Berkas biner dilewati: skill pack hanya teks.
        let Ok(text) = String::from_utf8(body) else {
            continue;
        };
        total += text.len();
        if total > MAX_TOTAL_BYTES {
            return Err(FILE_TOO_LARGE);
        }
        out.files.push((rel.to_string(), text));
        if out.files.len() > MAX_FILES {
            return Err(TOO_MANY_FILES);
        }
    }
    if !out.files.iter().any(|(p, _)| p.ends_with("SKILL.md")) {
        return Err(SKILL_MD_REQUIRED);
    }
    out.files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// Unduh tarball repo lalu ambil berkas skill.
///
/// # Errors
///
/// `skill-fetch-failed` (jaringan, 404, terlalu besar) atau kode dari [`extract`].
pub async fn fetch(settings: &SkillSettings, repo: &RepoRef) -> Result<Fetched, &'static str> {
    let url = format!(
        "{}/{}/{}/tar.gz/{}",
        settings.fetch_base.trim_end_matches('/'),
        repo.owner,
        repo.repo,
        repo.git_ref
    );
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(settings.fetch_timeout_seconds))
        .redirect(reqwest::redirect::Policy::limited(3))
        .user_agent("agentgate-skill-import")
        .build()
        .map_err(|_| FETCH_FAILED)?;
    let mut res = client.get(&url).send().await.map_err(|e| {
        tracing::warn!(error = %e, url, "unduh skill pack gagal");
        FETCH_FAILED
    })?;
    if !res.status().is_success() {
        tracing::info!(status = %res.status(), url, "repo skill pack tidak bisa diunduh");
        return Err(FETCH_FAILED);
    }
    let mut archive: Vec<u8> = Vec::new();
    while let Some(chunk) = res.chunk().await.map_err(|_| FETCH_FAILED)? {
        archive.extend_from_slice(&chunk);
        if archive.len() > MAX_ARCHIVE_BYTES {
            return Err(FILE_TOO_LARGE);
        }
    }
    extract(&archive, &repo.path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_public_github_urls_are_accepted() {
        let r = parse("https://github.com/obra/superpowers.git/", "", "skills/").unwrap();
        assert_eq!(r.owner, "obra");
        assert_eq!(r.repo, "superpowers");
        assert_eq!(r.git_ref, "main");
        assert_eq!(r.path, "skills");
        for bad in [
            "http://github.com/a/b",
            "https://gitlab.com/a/b",
            "https://github.com/a",
            "https://github.com/a/b/c",
            "https://github.com/../b",
            "https://evil.example/github.com/a/b",
        ] {
            assert_eq!(parse(bad, "", ""), Err(REPO_URL_INVALID), "{bad}");
        }
        assert_eq!(
            parse("https://github.com/a/b", "--upload", ""),
            Err(REPO_URL_INVALID)
        );
        assert_eq!(
            parse("https://github.com/a/b", "main", "../x"),
            Err(PATH_INVALID)
        );
    }

    #[test]
    fn only_skill_files_are_wanted() {
        assert!(wanted("SKILL.md"));
        assert!(wanted("skills/tdd/SKILL.md"));
        assert!(wanted("skills/tdd/scripts/run.sh"));
        assert!(wanted("references/a.md"));
        assert!(!wanted("README.md"));
        assert!(!wanted("src/main.rs"));
        assert!(!wanted("scripts"));
    }
}
