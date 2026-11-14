//! Git tag saat "Rilis sekarang" (R2): tag ringan `refs/tags/<nama>` dibuat lewat provider di
//! setiap repo project yang tertaut, di HEAD branch utama (default) atau commit pilihan.
//!
//! Provider yang tidak tertaut atau gagal tidak menggagalkan rilis: hasil per repo disimpan di
//! `releases.git_tags` dan halaman rilis menampilkan peringatan.
use sea_orm::{ActiveModelTrait, ActiveValue::Set, DatabaseConnection, DbErr, IntoActiveModel};
use serde::{Deserialize, Serialize};

use super::{NOT_CONNECTED, provider_code, provider_for, pulls::connection_of};
use crate::{
    common::settings::Settings,
    models::{releases, repos, workspaces::WorkspaceId},
};

/// Nama tag tidak valid sebagai ref git.
pub const TAG_INVALID: &str = "git-tag-invalid";
/// Commit target bukan SHA heksadesimal 7–40 karakter.
pub const SHA_INVALID: &str = "git-sha-invalid";
/// Tag dengan nama itu sudah ada di repo.
pub const TAG_EXISTS: &str = "git-tag-exists";

/// Target commit tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagTarget {
    /// HEAD branch utama tiap repo (default).
    Head,
    /// Commit tertentu (dipakai di semua repo; repo tanpa commit itu gagal sendiri).
    Commit(String),
}

/// Hasil per repo (disimpan di `releases.git_tags`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagResult {
    pub repo: String,
    pub tag: String,
    #[serde(default)]
    pub sha: String,
    #[serde(default)]
    pub url: String,
    /// Kode Fluent bila gagal (kosong = sukses).
    #[serde(default)]
    pub error: String,
}

/// Nama tag aman (aturan `git check-ref-format` yang relevan): huruf/angka/`._-/+`, tanpa
/// `..`, tidak diawali `-`/`.`/`/`, tidak diakhiri `.`/`/`/`.lock`, maks. 100 karakter.
#[must_use]
pub fn valid_tag(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 100
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/' | '+'))
        && !name.contains("..")
        && !name.contains("//")
        && !name.starts_with(['-', '.', '/'])
        && !name.ends_with(['.', '/'])
        && !name.ends_with(".lock")
}

/// Nama tag bawaan dari nama rilis (spasi → `-`).
#[must_use]
pub fn default_tag(release_name: &str) -> String {
    release_name
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
}

/// SHA heksadesimal 7–40 karakter.
#[must_use]
pub fn valid_sha(sha: &str) -> bool {
    (7..=40).contains(&sha.len()) && sha.chars().all(|c| c.is_ascii_hexdigit())
}

/// Buat tag di semua repo project lalu simpan hasilnya di rilis. Tanpa repo tertaut → daftar
/// berisi satu hasil `git-not-connected` per repo (atau kosong bila project tanpa repo).
///
/// # Errors
///
/// Error database (kegagalan provider dicatat per repo).
pub async fn tag_release(
    db: &DatabaseConnection,
    settings: &Settings,
    wid: WorkspaceId,
    release: releases::Model,
    tag: &str,
    target: &TagTarget,
) -> Result<Vec<TagResult>, DbErr> {
    let mut out = Vec::new();
    for repo in repos::Model::of_project(db, wid, release.project_id).await? {
        out.push(tag_repo(db, settings, wid, &repo, tag, target).await?);
    }
    let mut m = release.into_active_model();
    m.git_tags = Set(serde_json::to_value(&out).unwrap_or_default());
    m.update(db).await?;
    Ok(out)
}

async fn tag_repo(
    db: &DatabaseConnection,
    settings: &Settings,
    wid: WorkspaceId,
    repo: &repos::Model,
    tag: &str,
    target: &TagTarget,
) -> Result<TagResult, DbErr> {
    let mut res = TagResult {
        repo: repo.name.clone(),
        tag: tag.to_string(),
        sha: String::new(),
        url: String::new(),
        error: String::new(),
    };
    let Some(conn) = connection_of(db, wid, repo).await? else {
        res.error = NOT_CONNECTED.into();
        return Ok(res);
    };
    let provider = match provider_for(&conn, settings).await {
        Ok(p) => p,
        Err(_) => {
            res.error = super::TOKEN_INVALID.into();
            return Ok(res);
        }
    };
    let sha = match target {
        TagTarget::Commit(sha) => sha.clone(),
        TagTarget::Head => match provider
            .branch_head(&repo.full_name, &repo.default_branch)
            .await
        {
            Ok(sha) => sha,
            Err(e) => {
                res.error = provider_code(&e).into();
                return Ok(res);
            }
        },
    };
    res.sha = sha.clone();
    match provider
        .create_ref(&repo.full_name, &format!("refs/tags/{tag}"), &sha)
        .await
    {
        Ok(()) => {
            res.url = format!(
                "{}/{}/releases/tag/{tag}",
                conn.web_base.trim_end_matches('/'),
                repo.full_name
            );
        }
        Err(super::provider::ProviderError::Rejected(m)) if m == "already-exists" => {
            res.error = TAG_EXISTS.into();
        }
        Err(e) => res.error = provider_code(&e).into(),
    }
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_names_follow_ref_rules() {
        for ok in ["v1.4", "rilis/2026-11", "v1.4.0+build.7", "Sprint_4"] {
            assert!(valid_tag(ok), "{ok}");
        }
        for bad in [
            "", "-x", ".x", "a..b", "a b", "x.lock", "a/", "v1~2", "a//b",
        ] {
            assert!(!valid_tag(bad), "{bad}");
        }
        assert_eq!(default_tag(" Rilis  Lebaran 2026 "), "Rilis-Lebaran-2026");
        assert!(valid_sha("abc1234"));
        assert!(!valid_sha("abc12"));
        assert!(!valid_sha("zzzzzzz"));
    }
}
