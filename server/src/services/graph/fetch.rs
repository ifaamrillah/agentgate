//! Index oleh server (tombol Index ulang, PR di-merge ke branch utama): unduh arsip repo lewat
//! koneksi git M6 (token tersimpan terenkripsi), bongkar ke direktori sementara, lalu
//! [`ingest::index_dir`] dengan aturan file yang sama dengan CLI.
//!
//! Server tidak menjalankan `git clone`: arsip tidak membawa riwayat, jadi statistik git
//! (hotspot, coupling, pemilik) hanya dari `agentgate graph index` di CLI dan dipertahankan
//! di antara index server (keputusan sementara M7).
use std::{
    io::Read,
    path::{Path, PathBuf},
};

use agentgate_graph::walk::MAX_BYTES;
use sea_orm::{DatabaseConnection, DbErr};
use uuid::Uuid;

use super::ingest::{self, DirIndex};
use crate::{
    common::settings::Settings,
    models::{graph_repos, repos, workspaces::WorkspaceId},
    services::git::{self, provider_code},
};

/// Batas arsip yang diunduh.
pub const MAX_ARCHIVE_BYTES: usize = 300 * 1024 * 1024;
/// Batas file di arsip.
pub const MAX_ENTRIES: usize = 100_000;

/// Pesan repo tanpa koneksi git (index server tidak bisa mengunduh).
pub const NOT_CONNECTED_MESSAGE: &str = "Repo belum terhubung ke provider git. Hubungkan di \
    Pengaturan → Git, atau jalankan `agentgate graph index` dari laptop.";

/// Galat index server (kode untuk UI, disimpan di `graph_repos.error`).
#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("{0}")]
    Code(&'static str),
    #[error("archive: {0}")]
    Archive(String),
    #[error(transparent)]
    Db(#[from] DbErr),
}

impl FetchError {
    /// Pesan untuk UI (Bahasa Indonesia).
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::Code("git-not-connected") => NOT_CONNECTED_MESSAGE.into(),
            Self::Code(code) => format!("Provider git menolak unduhan ({code})."),
            Self::Archive(e) => format!("Arsip repo tidak bisa dibongkar: {e}"),
            Self::Db(_) => "Galat database saat index.".into(),
        }
    }
}

/// Bongkar `.tar.gz` GitHub ke `dir` (folder teratas dibuang). File di atas batas ditulis
/// sebagai file jarang berukuran sama supaya tercatat "terlalu besar" tanpa disimpan isinya.
///
/// # Errors
///
/// Arsip rusak atau melampaui batas.
pub fn unpack(archive: &[u8], dir: &Path) -> Result<Option<String>, FetchError> {
    let bad = |e: std::io::Error| FetchError::Archive(e.to_string());
    let gz = flate2::read::GzDecoder::new(archive);
    let mut tar = tar::Archive::new(gz);
    let mut sha = None;
    let mut count = 0usize;
    for entry in tar.entries().map_err(bad)? {
        let mut entry = entry.map_err(bad)?;
        let kind = entry.header().entry_type();
        if kind.is_pax_global_extensions() {
            if let Ok(Some(exts)) = entry.pax_extensions() {
                for ext in exts.flatten() {
                    if ext.key() == Ok("comment")
                        && let Ok(v) = ext.value()
                        && v.trim().len() == 40
                    {
                        sha = Some(v.trim().to_string());
                    }
                }
            }
            continue;
        }
        if !kind.is_file() {
            continue;
        }
        count += 1;
        if count > MAX_ENTRIES {
            return Err(FetchError::Archive("terlalu banyak file".into()));
        }
        let raw = entry.path().map_err(bad)?.to_string_lossy().to_string();
        let Some((_, rel)) = raw.split_once('/') else {
            continue;
        };
        if rel.is_empty()
            || rel.starts_with('/')
            || rel.contains('\\')
            || rel
                .split('/')
                .any(|s| s.is_empty() || s == ".." || s == ".")
        {
            continue;
        }
        let target: PathBuf = dir.join(rel);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(bad)?;
        }
        let size = entry.header().size().map_err(bad)?;
        if size > MAX_BYTES {
            let f = std::fs::File::create(&target).map_err(bad)?;
            f.set_len(size).map_err(bad)?;
            continue;
        }
        let mut buf = Vec::with_capacity(usize::try_from(size).unwrap_or(0));
        entry.read_to_end(&mut buf).map_err(bad)?;
        std::fs::write(&target, buf).map_err(bad)?;
    }
    Ok(sha)
}

/// Unduh dan index repo dari branch default-nya.
///
/// # Errors
///
/// [`FetchError`]; status repo diset `failed` oleh pemanggil.
pub async fn fetch_and_index(
    db: &DatabaseConnection,
    settings: &Settings,
    wid: WorkspaceId,
    repo: &repos::Model,
    by: Option<Uuid>,
    full: bool,
) -> Result<graph_repos::Model, FetchError> {
    let conn = git::pulls::connection_of(db, wid, repo)
        .await?
        .ok_or(FetchError::Code("git-not-connected"))?;
    let provider = git::provider_for(&conn, settings)
        .await
        .map_err(|_| FetchError::Code("token"))?;
    let bytes = provider
        .tarball(&repo.full_name, &repo.default_branch, MAX_ARCHIVE_BYTES)
        .await
        .map_err(|e| FetchError::Code(provider_code(&e)))?;
    let dir = std::env::temp_dir().join(format!("agentgate-graph-{}", Uuid::now_v7()));
    let unpack_dir = dir.clone();
    let sha = tokio::task::spawn_blocking(move || {
        std::fs::create_dir_all(&unpack_dir).map_err(|e| FetchError::Archive(e.to_string()))?;
        unpack(&bytes, &unpack_dir)
    })
    .await
    .map_err(|e| FetchError::Archive(e.to_string()))?;
    let result = match sha {
        Ok(sha) => ingest::index_dir(
            db,
            wid,
            repo,
            DirIndex {
                root: &dir,
                full,
                commit: sha.as_deref().unwrap_or_default(),
                git: None,
                by,
            },
        )
        .await
        .map_err(FetchError::from),
        Err(e) => Err(e),
    };
    let cleanup = dir.clone();
    let _ = tokio::task::spawn_blocking(move || std::fs::remove_dir_all(cleanup)).await;
    result
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn archive(files: &[(&str, &[u8])]) -> Vec<u8> {
        let enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        let mut b = tar::Builder::new(enc);
        for (path, body) in files {
            let mut h = tar::Header::new_gnu();
            h.set_size(body.len() as u64);
            h.set_mode(0o644);
            h.set_cksum();
            b.append_data(&mut h, format!("repo-abc/{path}"), *body)
                .unwrap();
        }
        b.into_inner().unwrap().finish().unwrap()
    }

    #[test]
    fn unpack_strips_top_folder() {
        let dir = std::env::temp_dir().join(format!("ag-unpack-{}", Uuid::now_v7()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = archive(&[("src/a.ts", b"export function a() {}"), ("README.md", b"x")]);
        unpack(&a, &dir).unwrap();
        assert!(dir.join("src/a.ts").is_file());
        assert!(dir.join("README.md").is_file());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
