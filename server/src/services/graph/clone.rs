//! Clone dangkal untuk index server (R2): repo disimpan di volume cache
//! (`<cache_dir>/<workspace>/<repo>`), diambil dengan `git fetch --shallow-since` (default 12
//! bulan) lalu di-fetch inkremental pada index berikutnya. Statistik git (hotspot, coupling,
//! pemilik) dihitung di server dari riwayat itu.
//!
//! - Kredensial (token instalasi App / PAT) dikirim lewat header HTTP sekali pakai di env proses
//!   git (`GIT_CONFIG_*`), tidak pernah di URL remote, `.git/config`, atau log.
//! - Batas: waktu per perintah git, ukuran clone, dan ruang disk minimum (`df`). Melewati batas
//!   atau git gagal → pemanggil memakai tarball (cadangan) dan mencatat alasannya.
//! - Clone repo yang sudah dihapus dibersihkan oleh [`purge_orphans`] (job cleanup harian).
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, LazyLock, Mutex},
    time::Duration,
};

use agentgate_graph::{
    Lang,
    history::{self, GitStats},
};
use base64::Engine;
use uuid::Uuid;

use crate::common::settings::CloneSettings;

/// Alasan clone tidak dipakai (dicatat di `graph_repos.fetch_note`, lalu tarball).
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CloneError {
    #[error("clone dimatikan di konfigurasi")]
    Disabled,
    #[error("git tidak tersedia di server")]
    NoGit,
    #[error("ruang disk cache tinggal {free_mb} MB (minimum {min_mb} MB)")]
    LowDisk { free_mb: u64, min_mb: u64 },
    #[error("clone {size_mb} MB melebihi batas {max_mb} MB")]
    TooLarge { size_mb: u64, max_mb: u64 },
    #[error("git {0} melewati batas waktu")]
    Timeout(&'static str),
    #[error("git {step} gagal: {message}")]
    Git { step: &'static str, message: String },
    #[error("branch tidak valid")]
    BadBranch,
    #[error("{0}")]
    Io(String),
}

/// Satu permintaan clone/fetch.
#[derive(Debug, Clone)]
pub struct CloneJob<'a> {
    pub workspace: Uuid,
    pub repo: Uuid,
    /// URL remote tanpa kredensial (`https://github.com/o/r.git`, `file:///...`).
    pub url: String,
    pub branch: &'a str,
    /// Token untuk header `Authorization` (kosong = tanpa kredensial).
    pub token: String,
    pub settings: &'a CloneSettings,
}

/// Hasil clone siap di-index.
#[derive(Debug, Clone)]
pub struct Cloned {
    pub dir: PathBuf,
    pub sha: String,
    pub stats: Option<GitStats>,
    pub size_mb: u64,
    /// Clone baru (bukan fetch inkremental).
    pub fresh: bool,
}

/// URL remote repo dari basis (web provider, atau `graph.clone.base_url`).
#[must_use]
pub fn remote_url(base: &str, full_name: &str) -> String {
    format!(
        "{}/{}.git",
        base.trim_end_matches('/'),
        full_name.trim_matches('/')
    )
}

/// Direktori clone repo di cache.
#[must_use]
pub fn repo_dir(settings: &CloneSettings, workspace: Uuid, repo: Uuid) -> PathBuf {
    settings
        .dir()
        .join(workspace.to_string())
        .join(repo.to_string())
}

/// Branch aman dipakai di refspec (bukan opsi, tanpa karakter ref terlarang).
#[must_use]
pub fn valid_branch(b: &str) -> bool {
    !b.is_empty()
        && b.len() <= 200
        && !b.starts_with(['-', '/', '.'])
        && !b.contains("..")
        && !b.ends_with(['/', '.'])
        && b.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '_' | '.' | '+'))
}

static LOCKS: LazyLock<Mutex<HashMap<Uuid, Arc<tokio::sync::Mutex<()>>>>> =
    LazyLock::new(Mutex::default);

fn lock_for(repo: Uuid) -> Arc<tokio::sync::Mutex<()>> {
    let mut map = LOCKS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    map.entry(repo).or_default().clone()
}

/// Ruang kosong (MB) di filesystem `dir` menurut `df -Pk`. `None` bila tidak terbaca.
pub async fn free_mb(dir: &Path) -> Option<u64> {
    let out = tokio::process::Command::new("df")
        .arg("-Pk")
        .arg(dir)
        .stdin(Stdio::null())
        .output()
        .await
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().nth(1)?;
    let avail: u64 = line.split_whitespace().nth(3)?.parse().ok()?;
    Some(avail / 1024)
}

/// Ukuran direktori (byte), tanpa mengikuti symlink.
fn dir_size(dir: &Path) -> u64 {
    let mut total = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let Ok(meta) = e.path().symlink_metadata() else {
                continue;
            };
            if meta.is_dir() {
                stack.push(e.path());
            } else {
                total += meta.len();
            }
        }
    }
    total
}

struct Git<'a> {
    dir: &'a Path,
    token: &'a str,
    timeout: Duration,
}

impl Git<'_> {
    async fn run(&self, step: &'static str, args: &[&str]) -> Result<String, CloneError> {
        let mut cmd = tokio::process::Command::new("git");
        cmd.arg("-C")
            .arg(self.dir)
            .args(args)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_ASKPASS", "")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("LC_ALL", "C")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if !self.token.is_empty() {
            // Header sekali pakai: tidak ditulis ke `.git/config` dan tidak masuk URL.
            let basic = base64::engine::general_purpose::STANDARD
                .encode(format!("x-access-token:{}", self.token));
            cmd.env("GIT_CONFIG_COUNT", "1")
                .env("GIT_CONFIG_KEY_0", "http.extraHeader")
                .env(
                    "GIT_CONFIG_VALUE_0",
                    format!("Authorization: Basic {basic}"),
                );
        }
        let child = cmd.spawn().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                CloneError::NoGit
            } else {
                CloneError::Io(e.to_string())
            }
        })?;
        let out = tokio::time::timeout(self.timeout, child.wait_with_output())
            .await
            .map_err(|_| CloneError::Timeout(step))?
            .map_err(|e| CloneError::Io(e.to_string()))?;
        if !out.status.success() {
            let message: String = String::from_utf8_lossy(&out.stderr)
                .lines()
                .filter(|l| !l.trim().is_empty())
                .last()
                .unwrap_or_default()
                .chars()
                .take(200)
                .collect();
            return Err(CloneError::Git { step, message });
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }
}

/// Clone (atau fetch inkremental) branch repo ke cache, checkout, dan hitung statistik git.
///
/// # Errors
///
/// [`CloneError`]; clone yang melebihi batas ukuran dihapus.
pub async fn sync(job: CloneJob<'_>) -> Result<Cloned, CloneError> {
    let s = job.settings;
    if !s.enabled {
        return Err(CloneError::Disabled);
    }
    if !valid_branch(job.branch) {
        return Err(CloneError::BadBranch);
    }
    let lock = lock_for(job.repo);
    let _guard = lock.lock().await;
    let dir = repo_dir(s, job.workspace, job.repo);
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| CloneError::Io(e.to_string()))?;
    if let Some(free) = free_mb(&dir).await
        && free < s.min_free_mb
    {
        return Err(CloneError::LowDisk {
            free_mb: free,
            min_mb: s.min_free_mb,
        });
    }
    let git = Git {
        dir: &dir,
        token: &job.token,
        timeout: Duration::from_secs(s.timeout_seconds.max(5)),
    };
    let fresh = !dir.join(".git").is_dir();
    if fresh {
        git.run("init", &["init", "-q"]).await?;
        git.run("remote", &["remote", "add", "origin", &job.url])
            .await?;
    } else {
        git.run("remote", &["remote", "set-url", "origin", &job.url])
            .await?;
    }
    let tracking = format!("refs/remotes/origin/{}", job.branch);
    let refspec = format!("+refs/heads/{}:{tracking}", job.branch);
    let since = chrono::Utc::now() - chrono::Duration::days(i64::from(s.history_months) * 30);
    let since = format!("--shallow-since={}", since.format("%Y-%m-%d"));
    let fetch = git
        .run(
            "fetch",
            &[
                "fetch",
                "-q",
                "--no-tags",
                "--prune",
                &since,
                "origin",
                &refspec,
            ],
        )
        .await;
    if let Err(e) = fetch {
        // Tidak ada commit dalam jendela riwayat (repo sepi) → satu commit terakhir saja.
        if matches!(&e, CloneError::Git { .. }) {
            git.run(
                "fetch",
                &["fetch", "-q", "--no-tags", "--depth=1", "origin", &refspec],
            )
            .await?;
        } else {
            return Err(e);
        }
    }
    git.run(
        "checkout",
        &[
            "-c",
            "advice.detachedHead=false",
            "checkout",
            "-q",
            "-f",
            "--detach",
            &tracking,
        ],
    )
    .await?;
    git.run("clean", &["clean", "-q", "-f", "-f", "-d", "-x"])
        .await?;
    let size_dir = dir.clone();
    let size = tokio::task::spawn_blocking(move || dir_size(&size_dir))
        .await
        .unwrap_or(0);
    let size_mb = size / (1024 * 1024);
    if size_mb > s.max_size_mb {
        remove(&dir).await;
        return Err(CloneError::TooLarge {
            size_mb,
            max_mb: s.max_size_mb,
        });
    }
    let sha = git
        .run("rev-parse", &["rev-parse", "HEAD"])
        .await?
        .trim()
        .to_string();
    let stats = git
        .run("log", history::GIT_LOG_ARGS)
        .await
        .ok()
        .map(|log| history::parse(&log, |p| Lang::detect(p).is_some()));
    Ok(Cloned {
        dir,
        sha,
        stats,
        size_mb,
        fresh,
    })
}

async fn remove(dir: &Path) {
    if let Err(e) = tokio::fs::remove_dir_all(dir).await {
        tracing::warn!(error = %e, dir = %dir.display(), "remove clone failed");
    }
}

/// Hapus clone di cache yang repo-nya sudah tidak ada (`keep` = id repo yang masih ada).
/// Mengembalikan jumlah clone yang (akan) dihapus.
///
/// # Errors
///
/// Direktori cache tidak terbaca.
pub fn purge_orphans(
    settings: &CloneSettings,
    keep: &std::collections::HashSet<Uuid>,
    apply: bool,
) -> std::io::Result<u64> {
    let root = settings.dir();
    if !root.is_dir() {
        return Ok(0);
    }
    let mut count = 0;
    for ws in std::fs::read_dir(&root)?.flatten() {
        if Uuid::parse_str(&ws.file_name().to_string_lossy()).is_err() || !ws.path().is_dir() {
            continue;
        }
        for repo in std::fs::read_dir(ws.path())?.flatten() {
            let Ok(id) = Uuid::parse_str(&repo.file_name().to_string_lossy()) else {
                continue;
            };
            if keep.contains(&id) || !repo.path().is_dir() {
                continue;
            }
            count += 1;
            if apply {
                std::fs::remove_dir_all(repo.path())?;
            }
        }
        if apply && std::fs::read_dir(ws.path())?.next().is_none() {
            let _ = std::fs::remove_dir(ws.path());
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn branches_cannot_be_options_or_escape_refs() {
        assert!(valid_branch("main"));
        assert!(valid_branch("release/2026.11"));
        assert!(!valid_branch("--upload-pack=x"));
        assert!(!valid_branch("a..b"));
        assert!(!valid_branch("a b"));
        assert!(!valid_branch(""));
    }

    #[test]
    fn remote_url_never_carries_credentials() {
        assert_eq!(
            remote_url("https://github.com/", "/pt/kasir/"),
            "https://github.com/pt/kasir.git"
        );
    }
}
