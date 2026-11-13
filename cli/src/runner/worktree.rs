//! Satu git worktree per tiket: `<state>/wt/<repo>/<branch>` dari repo lokal yang di-`init`.
//! Branch tiket (`ag/KSR-26-slug`) diambil dari `origin/<branch>` bila sudah ada di remote, atau
//! dibuat baru dari base branch repo (`origin/<base>` setelah fetch, lalu `<base>` lokal, lalu
//! HEAD). Setelah job, branch di-push dan server membuka PR (hasil kode "PR otomatis").

use std::path::{Path, PathBuf};

use agentgate_core::runner::protocol::RuleFile;

use crate::{
    error::{Error, Result, io_at},
    git, install,
};

/// Komponen path aman: hanya `[A-Za-z0-9._-]`, tanpa `.`/`..`.
fn safe_segment(s: &str) -> Option<String> {
    let clean: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '-'
            }
        })
        .collect();
    let clean = clean.trim_matches('.').to_string();
    (!clean.is_empty()).then_some(clean)
}

/// Path worktree untuk `repo` dan `branch` (`ag/KSR-1-x` → `wt/<repo>/ag/KSR-1-x`).
#[must_use]
pub fn path_for(state: &Path, repo: &str, branch: &str) -> PathBuf {
    let mut p = state
        .join("wt")
        .join(safe_segment(repo).unwrap_or_else(|| "repo".into()));
    let mut any = false;
    for seg in branch.split('/').filter_map(safe_segment) {
        p.push(seg);
        any = true;
    }
    if !any {
        p.push("branch");
    }
    p
}

/// Siapkan (atau pakai ulang) worktree branch `branch` dari repo `repo_root`.
///
/// # Errors
///
/// git gagal, IO.
pub fn prepare(repo_root: &Path, target: &Path, branch: &str, base: &str) -> Result<()> {
    if target.join(".git").exists() {
        return Ok(());
    }
    let has_origin = git::git(repo_root, &["remote", "get-url", "origin"]).is_ok();
    if has_origin {
        // Gagal fetch (offline) bukan alasan membatalkan job: pakai ref yang ada.
        let _ = git::git(repo_root, &["fetch", "-q", "origin"]);
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(io_at(parent))?;
    }
    let _ = git::git(repo_root, &["worktree", "prune"]);
    let t = target.to_string_lossy().into_owned();
    let local = format!("refs/heads/{branch}");
    let remote = format!("refs/remotes/origin/{branch}");
    let args: Vec<String> = if git::ref_exists(repo_root, &local) {
        vec!["worktree".into(), "add".into(), t, branch.into()]
    } else if git::ref_exists(repo_root, &remote) {
        vec![
            "worktree".into(),
            "add".into(),
            "-b".into(),
            branch.into(),
            t,
            format!("origin/{branch}"),
        ]
    } else {
        vec![
            "worktree".into(),
            "add".into(),
            "-b".into(),
            branch.into(),
            t,
            base_ref(repo_root, base),
        ]
    };
    git::git_owned(repo_root, &args)?;
    Ok(())
}

/// Titik awal branch baru: `origin/<base>`, `<base>` lokal, atau HEAD.
fn base_ref(repo_root: &Path, base: &str) -> String {
    let base = base.trim();
    if base.is_empty() || base.starts_with('-') {
        return "HEAD".into();
    }
    if git::ref_exists(repo_root, &format!("refs/remotes/origin/{base}")) {
        format!("origin/{base}")
    } else if git::ref_exists(repo_root, &format!("refs/heads/{base}")) {
        base.to_string()
    } else {
        "HEAD".into()
    }
}

/// Jumlah commit branch worktree yang belum ada di base (`origin/<base>` atau `<base>`).
#[must_use]
pub fn ahead_of(worktree: &Path, base: &str) -> u64 {
    let base = base_ref(worktree, base);
    if base == "HEAD" {
        return 0;
    }
    git::git(worktree, &["rev-list", "--count", &format!("{base}..HEAD")])
        .ok()
        .and_then(|n| n.trim().parse().ok())
        .unwrap_or(0)
}

/// `git push -u origin <branch>` dari worktree.
///
/// # Errors
///
/// Tanpa remote `origin`, push ditolak, jaringan.
pub fn push(worktree: &Path, branch: &str) -> Result<()> {
    if branch.starts_with('-') {
        return Err(Error::Git(git::GitError::Failed {
            args: "push".into(),
            stderr: format!("invalid branch name: {branch}"),
        }));
    }
    git::git(worktree, &["push", "-q", "-u", "origin", branch])?;
    Ok(())
}

/// Tulis aturan versi server ke worktree (job tidak menunggu PR sinkronisasi aturan).
///
/// # Errors
///
/// IO.
pub fn write_rules(worktree: &Path, rules: &[RuleFile]) -> Result<()> {
    install::write_rules(worktree, rules)
}

/// Commit hasil kerja agent di branch tiket (tanpa file aturan yang ditulis runner dan tanpa
/// folder bukti). Tidak ada perubahan → `false`.
///
/// # Errors
///
/// git gagal.
pub fn commit_work(worktree: &Path, rules: &[RuleFile], message: &str) -> Result<bool> {
    let mut args: Vec<String> = vec!["add".into(), "-A".into(), "--".into(), ".".into()];
    for r in rules {
        args.push(format!(":(exclude){}", r.path));
    }
    // Folder bukti tidak pernah di-commit. Exclude pathspec gagal bila path itu sudah di-ignore
    // `.agentgate/.gitignore`, jadi folder diberi `.gitignore` sendiri.
    let evidence = worktree.join(super::evidence::EVIDENCE_DIR);
    if evidence.is_dir() {
        let ignore = evidence.join(".gitignore");
        std::fs::write(&ignore, "*\n").map_err(io_at(&ignore))?;
    }
    git::git_owned(worktree, &args).map_err(Error::Git)?;
    Ok(git::commit(worktree, message)?)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::testutil::{git_repo, temp_dir};

    #[test]
    fn worktree_paths_follow_repo_and_branch() {
        let state = Path::new("/s");
        assert_eq!(
            path_for(state, "kasir", "ag/KSR-26-validasi-stok"),
            Path::new("/s/wt/kasir/ag/KSR-26-validasi-stok")
        );
        assert_eq!(
            path_for(state, "../x", "ag/../../etc"),
            Path::new("/s/wt/-x/ag/etc")
        );
        assert_eq!(path_for(state, "", ""), Path::new("/s/wt/repo/branch"));
    }

    #[test]
    fn prepare_creates_branch_worktree_and_commits_work() {
        let repo = git_repo("wt");
        fs::write(repo.join("README.md"), "x").unwrap();
        git::git(&repo, &["add", "."]).unwrap();
        git::git(&repo, &["commit", "-qm", "init"]).unwrap();
        let state = temp_dir("wt-state");
        let target = path_for(&state, "kasir", "ag/KSR-1-x");
        prepare(&repo, &target, "ag/KSR-1-x", "main").unwrap();
        assert!(target.join("README.md").is_file());
        // Dipakai ulang tanpa error.
        prepare(&repo, &target, "ag/KSR-1-x", "main").unwrap();

        let rules = agentgate_core::runner::rules::agent_gate_files("http://x", "KSR");
        write_rules(&target, &rules).unwrap();
        fs::create_dir_all(target.join(".agentgate/evidence")).unwrap();
        fs::write(target.join(".agentgate/evidence/s.png"), "png").unwrap();
        assert!(!commit_work(&target, &rules, "kosong").unwrap());
        fs::write(target.join("stok.rs"), "fn main() {}").unwrap();
        assert!(commit_work(&target, &rules, "feat(KSR-1): x").unwrap());
        let files = git::git(&target, &["show", "--name-only", "--format=", "HEAD"]).unwrap();
        assert_eq!(files, "stok.rs");
        assert_eq!(git::current_branch(&target).unwrap(), "ag/KSR-1-x");
    }
}
