//! Pemasangan aturan Agent Gate di repo: `init`, `uninstall`, `rules check`, `rules sync`.
//!
//! **Backup aturan lama**: semua file aturan client lain
//! ([`agentgate_core::runner::rules::is_legacy_rule_path`]) diarsipkan ke
//! `.agentgate/backup/<ts>.tar.gz` beserta `<ts>.manifest.json`, lalu dihapus. Bentuk arsip
//! mencegah aturan lama ikut terbaca client. Arsip ikut di-commit supaya anggota tim lain juga
//! bisa `uninstall`; `.agentgate/evidence/` (screenshot lokal) di-ignore.

use std::{
    collections::BTreeSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use agentgate_core::runner::{
    protocol::{DriftReport, RuleFile},
    rules::{self, is_legacy_rule_path},
    sha256_hex,
};
use chrono::Utc;
use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use serde::{Deserialize, Serialize};

use crate::{
    api::Api,
    config::{Paths, ProjectFile, ProjectSection, RepoEntry, Repos, project_file_path},
    error::{Error, Result, io_at},
    git, ui,
};

/// Branch PR sinkronisasi aturan.
pub const SYNC_BRANCH: &str = "agentgate/sync-rules";
const BACKUP_DIR: &str = ".agentgate/backup";
const GITIGNORE: &str = ".agentgate/.gitignore";
const GITIGNORE_BODY: &str =
    "# Screenshot bukti lokal; dikirim runner sebagai evidence, tidak di-commit.\nevidence/\n";

/// Direktori/berkas di root yang diperiksa langsung di disk (termasuk yang di-ignore git).
const ROOT_SCAN_DIRS: &[&str] = &[
    ".claude",
    ".cursor",
    ".codex",
    ".gemini",
    ".github",
    ".agents",
    ".opencode",
];
const ROOT_SCAN_FILES: &[&str] = &[
    "CLAUDE.md",
    "CLAUDE.local.md",
    "AGENTS.md",
    "GEMINI.md",
    ".mcp.json",
    ".cursorrules",
    "opencode.json",
];

/// `GET /api/cli/rules?project=KEY`.
#[derive(Debug, Clone, Deserialize)]
pub struct ServerRules {
    pub project: String,
    #[serde(default)]
    pub project_name: String,
    pub checksum: String,
    pub files: Vec<RuleFile>,
}

/// Isi manifest backup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub created_at: String,
    pub project: String,
    pub archive: String,
    pub files: Vec<ManifestFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestFile {
    pub path: String,
    pub sha256: String,
    pub size: u64,
}

/// Root repo git yang memuat folder kerja.
///
/// # Errors
///
/// Bukan repo git.
pub fn repo_root() -> Result<PathBuf> {
    let cwd = std::env::current_dir().map_err(io_at("."))?;
    git::toplevel(&cwd).ok_or(Error::NotGitRepo)
}

/// Ambil file aturan project dari server.
///
/// # Errors
///
/// Lihat [`crate::api::ApiError`].
pub async fn fetch_rules(api: &Api, project: &str) -> Result<ServerRules> {
    Ok(api
        .get(&format!(
            "/api/cli/rules?project={}",
            percent_encode(project.trim())
        ))
        .await?)
}

fn percent_encode(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

fn walk(root: &Path, rel: &str, out: &mut BTreeSet<String>) {
    let Ok(entries) = fs::read_dir(root.join(rel)) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let child = format!("{rel}/{name}");
        match entry.file_type() {
            Ok(t) if t.is_dir() => walk(root, &child, out),
            Ok(t) if t.is_file() => {
                out.insert(child);
            }
            _ => {}
        }
    }
}

/// File aturan lama di repo (relatif root, urut), kecuali file yang isinya sudah sama dengan
/// aturan Agent Gate `current` (init ulang tidak mem-backup aturan Agent Gate sendiri).
///
/// # Errors
///
/// Lihat [`git::list_files`].
pub fn legacy_files(root: &Path, current: &[RuleFile]) -> Result<Vec<String>> {
    let mut all: BTreeSet<String> = git::list_files(root)?.into_iter().collect();
    for dir in ROOT_SCAN_DIRS {
        walk(root, dir, &mut all);
    }
    for file in ROOT_SCAN_FILES {
        if root.join(file).is_file() {
            all.insert((*file).to_string());
        }
    }
    Ok(all
        .into_iter()
        .filter(|p| is_legacy_rule_path(p) && root.join(p).is_file())
        .filter(|p| {
            !current.iter().any(|f| {
                f.path == *p && fs::read_to_string(root.join(p)).is_ok_and(|b| b == f.body)
            })
        })
        .collect())
}

/// Arsipkan `files` ke `.agentgate/backup/<ts>.tar.gz` + manifest.
///
/// # Errors
///
/// Error IO/arsip.
pub fn backup(root: &Path, files: &[String], project: &str) -> Result<Manifest> {
    let dir = root.join(BACKUP_DIR);
    fs::create_dir_all(&dir).map_err(io_at(&dir))?;
    let now = Utc::now();
    let mut ts = now.format("%Y%m%dT%H%M%SZ").to_string();
    // Dua init dalam satu detik tidak boleh menimpa arsip.
    let mut n = 1;
    while dir.join(format!("{ts}.tar.gz")).exists() {
        n += 1;
        ts = format!("{}-{n}", now.format("%Y%m%dT%H%M%SZ"));
    }
    let archive_name = format!("{ts}.tar.gz");
    let archive_path = dir.join(&archive_name);
    let file = fs::File::create(&archive_path).map_err(io_at(&archive_path))?;
    let mut tar = tar::Builder::new(GzEncoder::new(file, Compression::default()));
    let mut entries = Vec::new();
    for rel in files {
        let path = root.join(rel);
        let bytes = fs::read(&path).map_err(io_at(&path))?;
        tar.append_path_with_name(&path, rel)
            .map_err(|e| Error::Archive(e.to_string()))?;
        entries.push(ManifestFile {
            path: rel.clone(),
            sha256: sha256_hex(&bytes),
            size: bytes.len() as u64,
        });
    }
    tar.into_inner()
        .and_then(GzEncoder::finish)
        .and_then(|mut f| f.flush())
        .map_err(|e| Error::Archive(e.to_string()))?;
    let manifest = Manifest {
        created_at: now.to_rfc3339(),
        project: project.to_string(),
        archive: archive_name,
        files: entries,
    };
    let manifest_path = dir.join(format!("{ts}.manifest.json"));
    let mut body =
        serde_json::to_vec_pretty(&manifest).map_err(|e| Error::Archive(e.to_string()))?;
    body.push(b'\n');
    fs::write(&manifest_path, body).map_err(io_at(&manifest_path))?;
    Ok(manifest)
}

/// Manifest backup terbaru.
///
/// # Errors
///
/// Tidak ada backup, manifest rusak.
pub fn latest_backup(root: &Path) -> Result<Manifest> {
    let dir = root.join(BACKUP_DIR);
    let mut names: Vec<String> = fs::read_dir(&dir)
        .map_err(|_| Error::NoBackup)?
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".manifest.json"))
        .collect();
    // Nama `<ts>[-n]` berurutan leksikal = urutan waktu (sufiks -n untuk detik yang sama).
    names.sort_by_key(|n| {
        let stem = n.trim_end_matches(".manifest.json");
        let (ts, seq) = stem.split_once('-').unwrap_or((stem, "1"));
        (ts.to_string(), seq.parse::<u32>().unwrap_or(1))
    });
    let name = names.pop().ok_or(Error::NoBackup)?;
    let path = dir.join(name);
    let bytes = fs::read(&path).map_err(io_at(&path))?;
    serde_json::from_slice(&bytes).map_err(|e| Error::Archive(e.to_string()))
}

/// Pulihkan isi arsip backup ke root (menimpa file yang ada).
///
/// # Errors
///
/// Error IO/arsip.
pub fn restore(root: &Path, manifest: &Manifest) -> Result<()> {
    let path = root.join(BACKUP_DIR).join(&manifest.archive);
    let file = fs::File::open(&path).map_err(io_at(&path))?;
    let mut archive = tar::Archive::new(GzDecoder::new(file));
    // `unpack` menolak entri yang keluar dari root (`..`, path absolut).
    archive
        .unpack(root)
        .map_err(|e| Error::Archive(e.to_string()))
}

/// Hapus file (relatif root) beserta folder induk yang menjadi kosong.
///
/// # Errors
///
/// Error IO.
pub fn remove_files(root: &Path, files: &[String]) -> Result<()> {
    for rel in files {
        let path = root.join(rel);
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(io_at(&path)(e)),
        }
        let mut dir = path.parent();
        while let Some(d) = dir {
            if d == root || fs::remove_dir(d).is_err() {
                break;
            }
            dir = d.parent();
        }
    }
    Ok(())
}

/// Tulis file aturan ke `dir`.
///
/// # Errors
///
/// Error IO.
pub fn write_rules(dir: &Path, files: &[RuleFile]) -> Result<()> {
    for f in files {
        let rel = f.path.trim_start_matches('/');
        if rel.split('/').any(|seg| seg == "..") {
            continue;
        }
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(io_at(parent))?;
        }
        fs::write(&path, &f.body).map_err(io_at(&path))?;
    }
    Ok(())
}

/// Stage perubahan `paths` (ada → add, hilang → hapus dari index).
///
/// # Errors
///
/// Lihat [`git::git`].
pub fn stage(root: &Path, paths: &[String]) -> Result<()> {
    let (present, gone): (Vec<&String>, Vec<&String>) =
        paths.iter().partition(|p| root.join(p).exists());
    if !present.is_empty() {
        let mut args = vec!["add".to_string(), "-f".into(), "-A".into(), "--".into()];
        args.extend(present.into_iter().cloned());
        git::git_owned(root, &args)?;
    }
    if !gone.is_empty() {
        let mut args = vec![
            "rm".to_string(),
            "-q".into(),
            "-r".into(),
            "--cached".into(),
            "--ignore-unmatch".into(),
            "--".into(),
        ];
        args.extend(gone.into_iter().cloned());
        git::git_owned(root, &args)?;
    }
    Ok(())
}

/// Baca file aturan lokal untuk checksum/drift.
fn read_local(root: &Path, expected: &[RuleFile]) -> Vec<RuleFile> {
    expected
        .iter()
        .map(|f| RuleFile {
            path: f.path.clone(),
            body: fs::read_to_string(root.join(&f.path)).unwrap_or_default(),
        })
        .collect()
}

/// Hasil cek drift satu repo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drift {
    pub local_checksum: String,
    pub server_checksum: String,
    pub changed: Vec<String>,
}

/// Bandingkan file aturan di `root` dengan versi server.
#[must_use]
pub fn drift(root: &Path, server: &ServerRules) -> Drift {
    let changed = rules::changed_files(&server.files, |p| fs::read_to_string(root.join(p)).ok());
    Drift {
        local_checksum: rules::checksum(&read_local(root, &server.files)),
        server_checksum: server.checksum.clone(),
        changed,
    }
}

/// Paket hasil `init` (untuk ringkasan dan tes).
#[derive(Debug)]
pub struct InitOutcome {
    pub backed_up: Vec<String>,
    pub written: Vec<String>,
    pub committed: bool,
}

/// `agentgate init`: backup + hapus aturan lama, tulis aturan server, commit.
///
/// # Errors
///
/// Bukan repo git, belum login, project tidak ditemukan, git gagal.
pub async fn init(
    paths: &Paths,
    api: &Api,
    project: Option<&str>,
    yes: bool,
    out: &mut impl Write,
) -> Result<InitOutcome> {
    let root = repo_root()?;
    let existing = ProjectFile::load(&root)?;
    let key = project
        .map(str::to_string)
        .or_else(|| existing.as_ref().map(|p| p.project.key.clone()))
        .filter(|k| !k.trim().is_empty())
        .ok_or(Error::ProjectRequired)?;
    if git::has_staged(&root)? {
        return Err(Error::StagedChanges);
    }
    let server_rules = fetch_rules(api, &key).await?;
    let legacy = legacy_files(&root, &server_rules.files)?;
    let _ = writeln!(
        out,
        "Repo: {}\nProject: {} {}\n",
        root.display(),
        server_rules.project,
        server_rules.project_name
    );
    if legacy.is_empty() {
        let _ = writeln!(out, "Tidak ada file aturan lama.");
    } else {
        let _ = writeln!(
            out,
            "File aturan lama yang akan di-backup ke {BACKUP_DIR}/ lalu dihapus:"
        );
        for f in &legacy {
            let _ = writeln!(out, "  - {f}");
        }
    }
    let _ = writeln!(out, "\nFile aturan Agent Gate yang akan ditulis:");
    for f in &server_rules.files {
        let _ = writeln!(out, "  + {}", f.path);
    }
    let _ = writeln!(
        out,
        "  + .agentgate/project.toml\n\nSemua perubahan langsung di-commit."
    );
    let _ = out.flush();
    if !yes && !ui::confirm("Lanjutkan?") {
        return Err(Error::Aborted);
    }
    let mut touched: Vec<String> = Vec::new();
    if !legacy.is_empty() {
        backup(&root, &legacy, &server_rules.project)?;
        remove_files(&root, &legacy)?;
        touched.extend(legacy.iter().cloned());
        touched.push(BACKUP_DIR.into());
    }
    write_rules(&root, &server_rules.files)?;
    touched.extend(server_rules.files.iter().map(|f| f.path.clone()));
    let project_file = ProjectFile {
        project: ProjectSection {
            key: server_rules.project.clone(),
            server: api.base().to_string(),
        },
        test: existing.map(|p| p.test).unwrap_or_default(),
    };
    let toml_path = project_file_path(&root);
    if let Some(dir) = toml_path.parent() {
        fs::create_dir_all(dir).map_err(io_at(dir))?;
    }
    fs::write(&toml_path, project_file.render()).map_err(io_at(&toml_path))?;
    fs::write(root.join(GITIGNORE), GITIGNORE_BODY).map_err(io_at(root.join(GITIGNORE)))?;
    touched.push(".agentgate/project.toml".into());
    touched.push(GITIGNORE.into());
    touched.sort();
    touched.dedup();
    stage(&root, &touched)?;
    let committed = git::commit(&root, "chore: pasang aturan Agent Gate")?;
    let mut repos = Repos::load(paths)?;
    repos.upsert(RepoEntry {
        name: git::repo_name(&root),
        path: root.clone(),
        project: server_rules.project.clone(),
        server: api.base().to_string(),
    });
    repos.save(paths)?;
    Ok(InitOutcome {
        backed_up: legacy,
        written: server_rules.files.iter().map(|f| f.path.clone()).collect(),
        committed,
    })
}

/// `agentgate uninstall`: pulihkan backup terbaru, hapus aturan Agent Gate, commit.
///
/// # Errors
///
/// Bukan repo git, belum di-init, tidak ada backup, git gagal.
pub fn uninstall(paths: &Paths, yes: bool, out: &mut impl Write) -> Result<Manifest> {
    let root = repo_root()?;
    if ProjectFile::load(&root)?.is_none() {
        return Err(Error::NotInitialized);
    }
    if git::has_staged(&root)? {
        return Err(Error::StagedChanges);
    }
    let manifest = latest_backup(&root)?;
    let _ = writeln!(
        out,
        "File aturan lama dari backup {} ({} file) akan dipulihkan, aturan Agent Gate dihapus, \
         lalu di-commit.",
        manifest.archive,
        manifest.files.len()
    );
    if !yes && !ui::confirm("Lanjutkan?") {
        return Err(Error::Aborted);
    }
    // Path file aturan Agent Gate selalu sama untuk server/project mana pun.
    let ours: Vec<String> = rules::agent_gate_files("", "")
        .into_iter()
        .map(|f| f.path)
        .filter(|p| !manifest.files.iter().any(|m| m.path == *p))
        .collect();
    remove_files(&root, &ours)?;
    let meta = vec![".agentgate/project.toml".to_string(), GITIGNORE.into()];
    remove_files(&root, &meta)?;
    restore(&root, &manifest)?;
    let mut touched = ours;
    touched.extend(meta);
    touched.extend(manifest.files.iter().map(|f| f.path.clone()));
    stage(&root, &touched)?;
    git::commit(
        &root,
        "chore: copot aturan Agent Gate, pulihkan aturan lama",
    )?;
    let mut repos = Repos::load(paths)?;
    repos.remove_path(&root);
    repos.save(paths)?;
    Ok(manifest)
}

/// Konteks repo yang sudah di-init.
fn initialized() -> Result<(PathBuf, ProjectFile)> {
    let root = repo_root()?;
    let project = ProjectFile::load(&root)?.ok_or(Error::NotInitialized)?;
    Ok((root, project))
}

/// `agentgate rules check`: cek drift dan laporkan ke server.
///
/// # Errors
///
/// Belum di-init, server gagal; [`Error::Drift`] bila berbeda.
pub async fn check(api: &Api, out: &mut impl Write) -> Result<()> {
    let (root, project) = initialized()?;
    let server_rules = fetch_rules(api, &project.project.key).await?;
    let d = drift(&root, &server_rules);
    let report = DriftReport {
        repo: git::repo_name(&root),
        checksum: d.local_checksum.clone(),
        changed: d.changed.clone(),
    };
    if let Err(e) = api.post_unit("/api/runner/drift", &report).await {
        let _ = writeln!(out, "Peringatan: hasil cek tidak terkirim ke server ({e}).");
    }
    if d.changed.is_empty() {
        let _ = writeln!(
            out,
            "Aturan sesuai versi server (checksum {}).",
            short(&d.server_checksum)
        );
        Ok(())
    } else {
        let _ = writeln!(out, "Aturan berbeda dari versi server:");
        for f in &d.changed {
            let _ = writeln!(out, "  ~ {f}");
        }
        let _ = writeln!(out, "Jalankan `agentgate rules sync` untuk memperbarui.");
        Err(Error::Drift)
    }
}

fn short(checksum: &str) -> &str {
    checksum.get(..12).unwrap_or(checksum)
}

/// `agentgate rules sync`: branch [`SYNC_BRANCH`] berisi aturan versi server + commit.
///
/// # Errors
///
/// Belum di-init, working tree kotor, git gagal.
pub async fn sync(api: &Api, out: &mut impl Write) -> Result<()> {
    let (root, project) = initialized()?;
    let server_rules = fetch_rules(api, &project.project.key).await?;
    let d = drift(&root, &server_rules);
    if d.changed.is_empty() {
        let _ = writeln!(
            out,
            "Aturan sudah sesuai versi server. Tidak ada yang disinkronkan."
        );
        return Ok(());
    }
    if !git::is_clean(&root)? {
        return Err(Error::DirtyTree);
    }
    let original = git::current_branch(&root)?;
    git::git(&root, &["switch", "-q", "-C", SYNC_BRANCH])?;
    let result = (|| -> Result<bool> {
        write_rules(&root, &server_rules.files)?;
        stage(&root, &d.changed)?;
        Ok(git::commit(&root, "chore: update aturan Agent Gate")?)
    })();
    if !original.is_empty() {
        git::git(&root, &["switch", "-q", &original])?;
    }
    result?;
    let _ = writeln!(
        out,
        "Commit aturan baru dibuat di branch {SYNC_BRANCH}:\n{}\n\nPembuatan PR otomatis belum \
         tersedia. Push lalu buat PR \"Update aturan Agent Gate\":\n  git push -u origin {SYNC_BRANCH}",
        d.changed
            .iter()
            .map(|f| format!("  ~ {f}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    Ok(())
}

/// Status aturan repo `root` (untuk `status`/`doctor`): `None` bila belum di-init.
///
/// # Errors
///
/// Server gagal.
pub async fn repo_status(api: &Api, root: &Path) -> Result<Option<(ProjectFile, Drift)>> {
    let Some(project) = ProjectFile::load(root)? else {
        return Ok(None);
    };
    let server_rules = fetch_rules(api, &project.project.key).await?;
    let d = drift(root, &server_rules);
    Ok(Some((project, d)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{git_repo, temp_dir};

    #[test]
    fn backup_remove_and_restore_round_trip() {
        let root = git_repo("backup");
        fs::create_dir_all(root.join(".claude/commands")).unwrap();
        fs::create_dir_all(root.join("web")).unwrap();
        fs::write(root.join("CLAUDE.md"), "lama").unwrap();
        fs::write(root.join(".claude/commands/x.md"), "cmd").unwrap();
        fs::write(root.join("web/AGENTS.md"), "web").unwrap();
        fs::write(root.join("README.md"), "readme").unwrap();
        // Diabaikan git, tetap dibackup karena tetap dibaca client.
        fs::write(root.join(".gitignore"), "CLAUDE.local.md\n").unwrap();
        fs::write(root.join("CLAUDE.local.md"), "pribadi").unwrap();
        let ours = rules::agent_gate_files("http://x", "KSR");
        fs::write(root.join(".mcp.json"), &ours[2].body).unwrap();

        let legacy = legacy_files(&root, &ours).unwrap();
        assert_eq!(
            legacy,
            [
                ".claude/commands/x.md",
                "CLAUDE.local.md",
                "CLAUDE.md",
                "web/AGENTS.md"
            ]
        );
        let manifest = backup(&root, &legacy, "KSR").unwrap();
        assert_eq!(manifest.files.len(), 4);
        remove_files(&root, &legacy).unwrap();
        assert!(!root.join("CLAUDE.md").exists());
        assert!(!root.join(".claude").exists(), "folder kosong ikut dihapus");

        let latest = latest_backup(&root).unwrap();
        assert_eq!(latest, manifest);
        restore(&root, &latest).unwrap();
        assert_eq!(fs::read_to_string(root.join("CLAUDE.md")).unwrap(), "lama");
        assert_eq!(
            fs::read_to_string(root.join(".claude/commands/x.md")).unwrap(),
            "cmd"
        );
    }

    #[test]
    fn latest_backup_orders_same_second_suffixes() {
        let root = temp_dir("order");
        let dir = root.join(BACKUP_DIR);
        fs::create_dir_all(&dir).unwrap();
        for (name, archive) in [
            ("20261113T010101Z", "a"),
            ("20261113T010101Z-2", "b"),
            ("20261113T010101Z-10", "c"),
            ("20261112T235959Z", "d"),
        ] {
            let m = Manifest {
                created_at: String::new(),
                project: String::new(),
                archive: archive.into(),
                files: vec![],
            };
            fs::write(
                dir.join(format!("{name}.manifest.json")),
                serde_json::to_vec(&m).unwrap(),
            )
            .unwrap();
        }
        assert_eq!(latest_backup(&root).unwrap().archive, "c");
        assert!(matches!(
            latest_backup(&temp_dir("none")),
            Err(Error::NoBackup)
        ));
    }

    #[test]
    fn drift_reports_changed_rule_files() {
        let root = temp_dir("drift");
        let files = rules::agent_gate_files("http://x", "KSR");
        write_rules(&root, &files).unwrap();
        let server = ServerRules {
            project: "KSR".into(),
            project_name: String::new(),
            checksum: rules::checksum(&files),
            files: files.clone(),
        };
        let d = drift(&root, &server);
        assert!(d.changed.is_empty());
        assert_eq!(d.local_checksum, d.server_checksum);
        fs::write(root.join("CLAUDE.md"), "diubah").unwrap();
        fs::remove_file(root.join(".mcp.json")).unwrap();
        let d = drift(&root, &server);
        assert_eq!(d.changed, ["CLAUDE.md", ".mcp.json"]);
        assert_ne!(d.local_checksum, d.server_checksum);
    }

    #[test]
    fn rule_paths_cannot_escape_the_target_dir() {
        let root = temp_dir("escape");
        let inner = root.join("inner");
        fs::create_dir_all(&inner).unwrap();
        write_rules(
            &inner,
            &[RuleFile {
                path: "../evil.md".into(),
                body: "x".into(),
            }],
        )
        .unwrap();
        assert!(!root.join("evil.md").exists());
    }
}
