//! Error perintah CLI. `Display` untuk log developer (Inggris); teks untuk user lewat
//! [`Error::user_message`] (Bahasa Indonesia), ditulis sekali di sini.

use std::{io, path::PathBuf};

use crate::{api::ApiError, config::ConfigError, git::GitError};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Api(#[from] ApiError),
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Git(#[from] GitError),
    #[error("io error at {path}: {source}")]
    Io { path: PathBuf, source: io::Error },
    #[error("not logged in")]
    NotLoggedIn,
    #[error("server url required")]
    ServerRequired,
    #[error("device login failed: {0}")]
    Login(String),
    #[error("not inside a git repository")]
    NotGitRepo,
    #[error("repository not initialized")]
    NotInitialized,
    #[error("project key required")]
    ProjectRequired,
    #[error("no backup found")]
    NoBackup,
    #[error("index has staged changes")]
    StagedChanges,
    #[error("working tree not clean")]
    DirtyTree,
    #[error("aborted by user")]
    Aborted,
    #[error("job not found: {0}")]
    JobNotFound(String),
    #[error("ambiguous job id: {0}")]
    JobAmbiguous(String),
    #[error("runner not registered")]
    NoRunner,
    #[error("local repository for project {0} not found")]
    RepoNotFound(String),
    #[error("archive error: {0}")]
    Archive(String),
    #[error("code graph error: {0}")]
    Graph(String),
    #[error("failed to start {program}: {source}")]
    Spawn { program: String, source: io::Error },
    #[error("rules drift detected")]
    Drift,
    #[error("installer exited with status {0}")]
    UpdateFailed(i32),
    #[error("server closed the runner connection: {0}")]
    ServerClosed(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Pembungkus error IO dengan path.
pub fn io_at(path: impl Into<PathBuf>) -> impl FnOnce(io::Error) -> Error {
    let path = path.into();
    move |source| Error::Io { path, source }
}

impl Error {
    /// Pesan untuk user.
    #[must_use]
    pub fn user_message(&self) -> String {
        match self {
            Self::Api(e) => api_message(e),
            Self::Config(e) => format!("Berkas konfigurasi lokal tidak bisa dibaca/ditulis: {e}"),
            Self::Git(e) => format!("Perintah git gagal: {e}"),
            Self::Io { path, source } => {
                format!("Berkas {} tidak bisa diakses: {source}", path.display())
            }
            Self::NotLoggedIn => {
                "Anda belum login. Jalankan `agentgate login --server <URL>` dulu.".into()
            }
            Self::ServerRequired => {
                "Alamat server belum diketahui. Jalankan `agentgate login --server <URL>`.".into()
            }
            Self::Login(code) => match code.as_str() {
                "access_denied" => "Login ditolak di browser.".into(),
                "expired_token" => {
                    "Kode login kedaluwarsa. Jalankan `agentgate login` lagi.".into()
                }
                other => format!("Login gagal ({other})."),
            },
            Self::NotGitRepo => "Folder ini bukan repo git.".into(),
            Self::NotInitialized => {
                "Repo ini belum memakai Agent Gate. Jalankan `agentgate init --project <KODE>`."
                    .into()
            }
            Self::ProjectRequired => {
                "Sebutkan kode project: `agentgate init --project <KODE>`.".into()
            }
            Self::NoBackup => "Tidak ada backup aturan lama di .agentgate/backup/.".into(),
            Self::StagedChanges => {
                "Ada perubahan yang sudah di-stage. Commit atau batalkan dulu supaya commit Agent \
                 Gate tidak tercampur."
                    .into()
            }
            Self::DirtyTree => {
                "Ada perubahan yang belum di-commit. Commit atau simpan dulu, lalu ulangi.".into()
            }
            Self::Aborted => "Dibatalkan.".into(),
            Self::JobNotFound(id) => format!("Job {id} tidak ditemukan."),
            Self::JobAmbiguous(id) => {
                format!("Id job {id} cocok dengan beberapa job. Tulis id lebih panjang.")
            }
            Self::NoRunner => {
                "Runner perangkat ini belum terdaftar. Jalankan `agentgate runner start` dulu."
                    .into()
            }
            Self::RepoNotFound(project) => format!(
                "Repo lokal untuk project {project} tidak ditemukan. Jalankan `agentgate init \
                 --project {project}` di repo tersebut."
            ),
            Self::Archive(e) => format!("Arsip backup tidak bisa diproses: {e}"),
            Self::Graph(e) => format!("Code graph tidak bisa diproses: {e}"),
            Self::Spawn { program, source } => {
                format!("{program} tidak bisa dijalankan: {source}")
            }
            Self::Drift => "File aturan berbeda dari versi server.".into(),
            Self::UpdateFailed(code) => format!(
                "Installer gagal (status {code}). Cek koneksi ke server dan halaman /install."
            ),
            Self::ServerClosed(msg) => format!(
                "Server menutup koneksi runner ({msg}). Login ulang atau nyalakan ulang runner."
            ),
        }
    }
}

fn api_message(e: &ApiError) -> String {
    match e {
        ApiError::Unauthorized => {
            "Sesi login tidak berlaku lagi. Jalankan `agentgate login` lagi.".into()
        }
        ApiError::NotFound(code) if code == "project_not_found" => {
            "Project tidak ditemukan atau Anda tidak punya akses.".into()
        }
        ApiError::NotFound(_) => "Data tidak ditemukan di server (atau bukan milik Anda).".into(),
        ApiError::Rejected(code) => match code.as_str() {
            "job-not-queued" => "Job sudah tidak di antrean.".into(),
            "job-not-executor" => "Anda bukan executor stage job ini.".into(),
            "job-not-holder" => "Job ini tidak sedang Anda pegang.".into(),
            "job-closed" => "Job sudah selesai atau dibatalkan.".into(),
            "job-runner-invalid" => "Runner tidak valid.".into(),
            "job-action-blocked" => "Aksi ini selalu diblokir.".into(),
            "job-runner-busy" => "Runner penuh (batas job paralel).".into(),
            other => format!("Server menolak permintaan ({other})."),
        },
        ApiError::Status { status, code } => format!("Server membalas {status} {code}."),
        ApiError::Network(err) => format!("Server tidak bisa dihubungi: {err}"),
        ApiError::Decode(err) => format!("Balasan server tidak dikenali: {err}"),
    }
}
