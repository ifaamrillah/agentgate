//! Lokasi dan isi berkas lokal CLI.
//!
//! - **Config dir** (`~/.config/agentgate`, atau `AGENTGATE_CONFIG_DIR`): `credentials.json`
//!   (token login), `runner.json` (id runner + rahasia Ed25519), `repos.json` (registry repo yang
//!   di-`init`, dipakai runner mencari repo lokal untuk job). Berkas rahasia selalu 0600.
//! - **State dir** (`~/.local/state/agentgate`, atau `AGENTGATE_STATE_DIR`): worktree job
//!   (`wt/<repo>/<branch>`), state job untuk hook (`jobs/<id>.json`), launcher terminal.
//!
//! Semua fungsi menerima [`Paths`] eksplisit (bukan membaca env di tempat) supaya tes bisa
//! memakai direktori sementara tanpa mengubah environment proses.

use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use agentgate_core::runner::protocol::{DangerRule, JobSpec};
use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;

/// Error membaca/menulis berkas lokal.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("io error at {path}: {source}")]
    Io { path: PathBuf, source: io::Error },
    #[error("invalid json in {path}: {source}")]
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("invalid toml in {path}: {message}")]
    Toml { path: PathBuf, message: String },
    #[error("invalid runner secret in {0}")]
    Secret(PathBuf),
}

pub type Result<T> = std::result::Result<T, ConfigError>;

fn io_err(path: &Path) -> impl FnOnce(io::Error) -> ConfigError + '_ {
    move |source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// Direktori kerja CLI.
#[derive(Debug, Clone)]
pub struct Paths {
    pub config: PathBuf,
    pub state: PathBuf,
}

fn env_dir(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

fn home() -> PathBuf {
    env_dir("HOME")
        .or_else(|| env_dir("USERPROFILE"))
        .unwrap_or_else(|| PathBuf::from("."))
}

impl Paths {
    /// Dari environment: `AGENTGATE_CONFIG_DIR`/`AGENTGATE_STATE_DIR`, lalu XDG, lalu bawaan OS.
    #[must_use]
    pub fn from_env() -> Self {
        let config = env_dir("AGENTGATE_CONFIG_DIR").unwrap_or_else(|| {
            if cfg!(windows) {
                env_dir("APPDATA").unwrap_or_else(home).join("agentgate")
            } else {
                env_dir("XDG_CONFIG_HOME")
                    .unwrap_or_else(|| home().join(".config"))
                    .join("agentgate")
            }
        });
        let state = env_dir("AGENTGATE_STATE_DIR").unwrap_or_else(|| {
            if cfg!(windows) {
                env_dir("LOCALAPPDATA")
                    .unwrap_or_else(home)
                    .join("agentgate")
            } else {
                env_dir("XDG_STATE_HOME")
                    .unwrap_or_else(|| home().join(".local").join("state"))
                    .join("agentgate")
            }
        });
        Self { config, state }
    }

    #[must_use]
    pub fn credentials(&self) -> PathBuf {
        self.config.join("credentials.json")
    }

    #[must_use]
    pub fn runner(&self) -> PathBuf {
        self.config.join("runner.json")
    }

    #[must_use]
    pub fn repos(&self) -> PathBuf {
        self.config.join("repos.json")
    }

    /// State job untuk hook (`jobs/<id>.json`).
    #[must_use]
    pub fn job_state(&self, job: Uuid) -> PathBuf {
        self.state.join("jobs").join(format!("{job}.json"))
    }
}

/// Satuan waktu tunggu (polling device flow, event runner, izin). Bawaan 1 detik;
/// `AGENTGATE_TIME_SCALE_MS` memperkecilnya untuk tes integrasi.
#[must_use]
pub fn seconds(n: u64) -> Duration {
    let unit = std::env::var("AGENTGATE_TIME_SCALE_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(1000);
    Duration::from_millis(n.saturating_mul(unit))
}

/// Tulis berkas secara atomik (tmp + rename) dengan izin 0600 di Unix.
///
/// # Errors
///
/// Error IO.
pub fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(io_err(dir))?;
    }
    let tmp = path.with_extension("tmp");
    {
        let mut opts = fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(&tmp).map_err(io_err(&tmp))?;
        f.write_all(bytes).map_err(io_err(&tmp))?;
        f.sync_all().map_err(io_err(&tmp))?;
    }
    #[cfg(unix)]
    {
        // Berkas tmp sisa proses lama bisa punya izin lebih longgar.
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600)).map_err(io_err(&tmp))?;
    }
    fs::rename(&tmp, path).map_err(io_err(path))
}

/// Baca JSON; berkas tidak ada → `None`.
///
/// # Errors
///
/// Error IO selain "tidak ada", JSON tidak valid.
pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|source| ConfigError::Json {
                path: path.to_path_buf(),
                source,
            }),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(io_err(path)(e)),
    }
}

/// Tulis JSON rapi dengan izin 0600.
///
/// # Errors
///
/// Error IO.
pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|source| ConfigError::Json {
        path: path.to_path_buf(),
        source,
    })?;
    bytes.push(b'\n');
    write_private(path, &bytes)
}

/// Hapus berkas; tidak ada → `false`.
///
/// # Errors
///
/// Error IO selain "tidak ada".
pub fn remove(path: &Path) -> Result<bool> {
    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(io_err(path)(e)),
    }
}

/// Token login (`agentgate login`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credentials {
    pub server: String,
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub user: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub workspace: String,
}

impl Credentials {
    /// Token kedaluwarsa (atau tinggal < 60 dtk) pada `now`.
    #[must_use]
    pub fn expiring(&self, now: DateTime<Utc>) -> bool {
        self.expires_at
            .is_some_and(|t| t - chrono::Duration::seconds(60) <= now)
    }

    /// # Errors
    ///
    /// Error IO/JSON.
    pub fn load(paths: &Paths) -> Result<Option<Self>> {
        read_json(&paths.credentials())
    }

    /// # Errors
    ///
    /// Error IO.
    pub fn save(&self, paths: &Paths) -> Result<()> {
        write_json(&paths.credentials(), self)
    }
}

/// Identitas runner perangkat ini: id dari server dan rahasia Ed25519 (tidak pernah dikirim).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerIdentity {
    #[serde(default)]
    pub runner_id: Option<Uuid>,
    /// Server tempat `runner_id` terdaftar (id runner tidak berlaku di server lain).
    #[serde(default)]
    pub server: String,
    pub name: String,
    /// Rahasia Ed25519 32 byte (base64).
    pub secret: String,
}

impl RunnerIdentity {
    /// Muat identitas, atau buat rahasia baru bila belum ada.
    ///
    /// # Errors
    ///
    /// Error IO/JSON.
    pub fn load_or_create(paths: &Paths) -> Result<Self> {
        if let Some(id) = read_json::<Self>(&paths.runner())? {
            id.secret_bytes(paths)?;
            return Ok(id);
        }
        let secret: [u8; 32] = rand::random();
        let id = Self {
            runner_id: None,
            server: String::new(),
            name: device_name(),
            secret: STANDARD.encode(secret),
        };
        id.save(paths)?;
        Ok(id)
    }

    /// # Errors
    ///
    /// Error IO/JSON.
    pub fn load(paths: &Paths) -> Result<Option<Self>> {
        read_json(&paths.runner())
    }

    /// # Errors
    ///
    /// Error IO.
    pub fn save(&self, paths: &Paths) -> Result<()> {
        write_json(&paths.runner(), self)
    }

    /// # Errors
    ///
    /// Rahasia bukan base64 32 byte.
    pub fn secret_bytes(&self, paths: &Paths) -> Result<[u8; 32]> {
        STANDARD
            .decode(self.secret.trim())
            .ok()
            .and_then(|b| <[u8; 32]>::try_from(b).ok())
            .ok_or_else(|| ConfigError::Secret(paths.runner()))
    }
}

/// Nama perangkat untuk runner dan device flow.
#[must_use]
pub fn device_name() -> String {
    hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .filter(|h| !h.trim().is_empty())
        .unwrap_or_else(|| "perangkat".into())
}

/// Repo lokal yang sudah di-`init`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoEntry {
    /// Nama repo (nama remote `origin` tanpa `.git`, atau nama folder).
    pub name: String,
    pub path: PathBuf,
    pub project: String,
    pub server: String,
}

/// Registry repo lokal (`repos.json`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Repos {
    #[serde(default)]
    pub repos: Vec<RepoEntry>,
}

impl Repos {
    /// # Errors
    ///
    /// Error IO/JSON.
    pub fn load(paths: &Paths) -> Result<Self> {
        Ok(read_json(&paths.repos())?.unwrap_or_default())
    }

    /// # Errors
    ///
    /// Error IO.
    pub fn save(&self, paths: &Paths) -> Result<()> {
        write_json(&paths.repos(), self)
    }

    /// Tambah atau ganti entri dengan path yang sama.
    pub fn upsert(&mut self, entry: RepoEntry) {
        self.repos.retain(|r| r.path != entry.path);
        self.repos.push(entry);
    }

    pub fn remove_path(&mut self, path: &Path) {
        self.repos.retain(|r| r.path != path);
    }

    /// Repo lokal untuk job: project sama, nama cocok dengan salah satu repo project bila server
    /// menyebutkannya, lalu repo project pertama yang masih ada di disk.
    #[must_use]
    pub fn find_for(&self, project: &str, names: &[String]) -> Option<&RepoEntry> {
        let candidates: Vec<&RepoEntry> = self
            .repos
            .iter()
            .filter(|r| r.project.eq_ignore_ascii_case(project) && r.path.is_dir())
            .collect();
        candidates
            .iter()
            .find(|r| names.iter().any(|n| repo_name_matches(n, &r.name)))
            .or_else(|| candidates.first())
            .copied()
    }
}

/// Nama repo server (`org/kasir`, `kasir.git`) cocok dengan nama lokal (`kasir`).
fn repo_name_matches(server: &str, local: &str) -> bool {
    let short = server
        .trim()
        .trim_end_matches(".git")
        .rsplit('/')
        .next()
        .unwrap_or(server);
    short.eq_ignore_ascii_case(local)
}

/// State job yang ditulis runner untuk hook (`agentgate hook …` berjalan di proses client).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobState {
    pub job_id: Uuid,
    pub runner_id: Uuid,
    pub ticket: String,
    pub title: String,
    pub url: String,
    pub stage_name: String,
    pub persona_name: String,
    pub danger: Vec<DangerRule>,
    pub allowed_actions: Vec<String>,
}

impl JobState {
    #[must_use]
    pub fn from_spec(spec: &JobSpec, runner_id: Uuid) -> Self {
        Self {
            job_id: spec.id,
            runner_id,
            ticket: spec.ticket.clone(),
            title: spec.title.clone(),
            url: spec.url.clone(),
            stage_name: spec.stage_name.clone(),
            persona_name: spec.persona_name.clone(),
            danger: spec.danger.clone(),
            allowed_actions: spec.allowed_actions.clone(),
        }
    }

    /// # Errors
    ///
    /// Error IO/JSON.
    pub fn load(paths: &Paths, job: Uuid) -> Result<Option<Self>> {
        read_json(&paths.job_state(job))
    }

    /// # Errors
    ///
    /// Error IO.
    pub fn save(&self, paths: &Paths) -> Result<()> {
        write_json(&paths.job_state(self.job_id), self)
    }
}

/// `.agentgate/project.toml` di repo.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectFile {
    pub project: ProjectSection,
    #[serde(default)]
    pub test: TestSection,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectSection {
    pub key: String,
    #[serde(default)]
    pub server: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestSection {
    /// Perintah tes yang dijalankan runner untuk evidence `test_report` (kosong = tidak ada).
    #[serde(default)]
    pub command: String,
}

/// Path `project.toml` di repo `root`.
#[must_use]
pub fn project_file_path(root: &Path) -> PathBuf {
    root.join(".agentgate").join("project.toml")
}

impl ProjectFile {
    /// # Errors
    ///
    /// Error IO selain "tidak ada", TOML tidak valid.
    pub fn load(root: &Path) -> Result<Option<Self>> {
        let path = project_file_path(root);
        match fs::read_to_string(&path) {
            Ok(text) => toml::from_str(&text)
                .map(Some)
                .map_err(|e| ConfigError::Toml {
                    path,
                    message: e.to_string(),
                }),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(io_err(&path)(e)),
        }
    }

    /// Isi berkas (dengan komentar penjelasan).
    #[must_use]
    pub fn render(&self) -> String {
        let quote = |s: &str| toml::Value::String(s.to_string()).to_string();
        format!(
            "# Dibuat oleh `agentgate init`. Ikut di-commit supaya seluruh tim memakai project yang sama.\n\
             [project]\n\
             key = {}\n\
             server = {}\n\
             \n\
             [test]\n\
             # Perintah tes yang dijalankan runner setelah agent selesai (evidence test_report).\n\
             # Contoh: \"cargo test\", \"npm test\". Kosong = tidak ada evidence tes.\n\
             command = {}\n",
            quote(&self.project.key),
            quote(&self.project.server),
            quote(&self.test.command),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::temp_dir;

    fn paths(tag: &str) -> Paths {
        let root = temp_dir(tag);
        Paths {
            config: root.join("config"),
            state: root.join("state"),
        }
    }

    #[test]
    fn credentials_round_trip_with_private_permissions() {
        let paths = paths("creds");
        assert_eq!(Credentials::load(&paths).unwrap(), None);
        let creds = Credentials {
            server: "https://ag.example".into(),
            access_token: "at".into(),
            refresh_token: Some("rt".into()),
            expires_at: Some(Utc::now()),
            user: "Ani".into(),
            email: "ani@example.com".into(),
            workspace: "Kasir".into(),
        };
        creds.save(&paths).unwrap();
        assert_eq!(Credentials::load(&paths).unwrap(), Some(creds.clone()));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(paths.credentials())
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        assert!(creds.expiring(Utc::now()));
        assert!(remove(&paths.credentials()).unwrap());
        assert!(!remove(&paths.credentials()).unwrap());
    }

    #[test]
    fn runner_identity_is_created_once() {
        let paths = paths("runner");
        let a = RunnerIdentity::load_or_create(&paths).unwrap();
        let b = RunnerIdentity::load_or_create(&paths).unwrap();
        assert_eq!(a.secret, b.secret);
        assert_eq!(a.secret_bytes(&paths).unwrap().len(), 32);
    }

    #[test]
    fn repo_lookup_matches_project_and_name() {
        let dir = temp_dir("repos");
        let entry = |name: &str, project: &str| RepoEntry {
            name: name.into(),
            path: dir.clone(),
            project: project.into(),
            server: String::new(),
        };
        let repos = Repos {
            repos: vec![entry("web", "KSR"), entry("api", "KSR"), entry("x", "OTH")],
        };
        let found = repos.find_for("ksr", &["org/api.git".into()]).unwrap();
        assert_eq!(found.name, "api");
        assert_eq!(repos.find_for("KSR", &[]).unwrap().name, "web");
        assert!(repos.find_for("NOPE", &[]).is_none());
    }

    #[test]
    fn project_file_renders_parseable_toml() {
        let root = temp_dir("project");
        let file = ProjectFile {
            project: ProjectSection {
                key: "KSR".into(),
                server: "https://ag.example".into(),
            },
            test: TestSection {
                command: "cargo test \"x\"".into(),
            },
        };
        fs::create_dir_all(root.join(".agentgate")).unwrap();
        fs::write(project_file_path(&root), file.render()).unwrap();
        assert_eq!(ProjectFile::load(&root).unwrap(), Some(file));
    }
}
