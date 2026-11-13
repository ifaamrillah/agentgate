//! Pemanggilan `git` (argv langsung, tanpa shell).

use std::{
    io,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("cannot run git: {0}")]
    Spawn(#[from] io::Error),
    #[error("git {args} failed: {stderr}")]
    Failed { args: String, stderr: String },
}

pub type Result<T> = std::result::Result<T, GitError>;

/// Jalankan `git -C dir args…`, kembalikan stdout (tanpa spasi di ujung).
///
/// # Errors
///
/// git tidak ada, atau keluar dengan status gagal.
pub fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
    } else {
        Err(GitError::Failed {
            args: args.join(" "),
            stderr: String::from_utf8_lossy(&out.stderr).trim().to_string(),
        })
    }
}

/// `git` dengan daftar argumen milik (path dinamis).
///
/// # Errors
///
/// Lihat [`git`].
pub fn git_owned(dir: &Path, args: &[String]) -> Result<String> {
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    git(dir, &refs)
}

/// Root repo yang memuat `dir`, `None` bila bukan repo git.
#[must_use]
pub fn toplevel(dir: &Path) -> Option<PathBuf> {
    git(dir, &["rev-parse", "--show-toplevel"])
        .ok()
        .map(PathBuf::from)
}

/// Nama repo: nama remote `origin` tanpa `.git`, atau nama folder.
#[must_use]
pub fn repo_name(root: &Path) -> String {
    git(root, &["remote", "get-url", "origin"])
        .ok()
        .and_then(|url| {
            let url = url.trim().trim_end_matches('/').trim_end_matches(".git");
            url.rsplit(['/', ':'])
                .next()
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        })
        .or_else(|| root.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "repo".into())
}

/// Branch aktif (kosong bila detached).
///
/// # Errors
///
/// Lihat [`git`].
pub fn current_branch(root: &Path) -> Result<String> {
    git(root, &["branch", "--show-current"])
}

/// Ada perubahan yang sudah di-stage.
///
/// # Errors
///
/// Lihat [`git`].
pub fn has_staged(root: &Path) -> Result<bool> {
    // Repo baru tanpa commit: `diff --cached` butuh HEAD; pakai status.
    let status = git(root, &["status", "--porcelain"])?;
    Ok(status
        .lines()
        .any(|l| l.chars().next().is_some_and(|c| c != ' ' && c != '?')))
}

/// Working tree bersih (tanpa perubahan maupun file untracked).
///
/// # Errors
///
/// Lihat [`git`].
pub fn is_clean(root: &Path) -> Result<bool> {
    Ok(git(root, &["status", "--porcelain"])?.is_empty())
}

/// Path tracked + untracked yang tidak di-ignore, relatif root, pemisah `/`.
///
/// # Errors
///
/// Lihat [`git`].
pub fn list_files(root: &Path) -> Result<Vec<String>> {
    Ok(git(root, &["ls-files", "-z", "-co", "--exclude-standard"])?
        .split('\0')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect())
}

/// Commit yang sudah di-stage. Tidak ada perubahan → `false`.
///
/// # Errors
///
/// Lihat [`git`].
pub fn commit(root: &Path, message: &str) -> Result<bool> {
    if git(root, &["diff", "--cached", "--quiet"]).is_ok() {
        return Ok(false);
    }
    git(root, &["commit", "-q", "--no-verify", "-m", message])?;
    Ok(true)
}

/// Ref ada (`refs/heads/x`, `origin/x`).
#[must_use]
pub fn ref_exists(root: &Path, name: &str) -> bool {
    git(root, &["rev-parse", "--verify", "-q", name]).is_ok()
}
