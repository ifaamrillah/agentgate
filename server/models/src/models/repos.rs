//! Repo milik project (frontend, backend, mobile): nama unik per project tanpa membedakan huruf
//! besar, penyedia ([`RepoProvider`], CHECK), URL clone, dan branch default.
//!
//! URL hanya dua bentuk ([`validate_url`]): `https://host/path` atau scp-like
//! `git@host:path`, maksimal [`URL_MAX_LEN`] karakter. URL https dengan kredensial
//! (`https://user:token@host/...`) ditolak: URL tampil ke semua yang bisa melihat project, jadi
//! token di dalamnya akan bocor.

use loco_rs::{
    model::{TenantEntity, TenantQueryExt},
    validation::ModelValidationErrors,
};
use sea_orm::{
    ActiveValue::Set, IntoActiveModel, QueryOrder, QuerySelect, SqlErr, entity::prelude::*,
};
use validator::{ValidationError, ValidationErrors};

pub use super::_entities::repos::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;
use crate::common::text;

pub const REPO_NAME_TAKEN: &str = "repo-name-taken";
pub const REPO_URL_INVALID: &str = "repo-url-invalid";
pub const REPO_URL_TOO_LONG: &str = "repo-url-too-long";
pub const REPO_PROVIDER_INVALID: &str = "repo-provider-invalid";
pub const REPO_BRANCH_INVALID: &str = "repo-branch-invalid";
/// Kode error field milik model ini; masing-masing punya pesan di Fluent (dijaga tes i18n).
pub const ERROR_CODES: &[&str] = &[
    REPO_NAME_TAKEN,
    REPO_URL_INVALID,
    REPO_URL_TOO_LONG,
    REPO_PROVIDER_INVALID,
    REPO_BRANCH_INVALID,
];

/// Panjang maksimal URL repo, dalam karakter.
pub const URL_MAX_LEN: usize = 500;
/// Panjang maksimal nama branch default, dalam karakter.
pub const BRANCH_MAX_LEN: usize = 100;

/// Penyedia hosting repo. Disimpan sebagai string dengan CHECK constraint (rules §4, dijaga
/// `tests/models/enums.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum RepoProvider {
    #[sea_orm(string_value = "github")]
    Github,
    #[sea_orm(string_value = "gitlab")]
    Gitlab,
    #[sea_orm(string_value = "bitbucket")]
    Bitbucket,
    #[sea_orm(string_value = "other")]
    Other,
}

#[derive(Debug, thiserror::Error)]
pub enum RepoError {
    /// Validasi field (`name`, `provider`, `url`, `default_branch`) gagal, termasuk nama yang
    /// sudah dipakai di project ini.
    #[error("invalid repo: {0}")]
    Invalid(ModelValidationErrors),
    #[error(transparent)]
    Db(#[from] DbErr),
}

impl From<RepoError> for loco_rs::Error {
    fn from(err: RepoError) -> Self {
        match err {
            RepoError::Invalid(errors) => Self::Validation(errors),
            RepoError::Db(e) => Self::DB(e),
        }
    }
}

/// Isian repo dari form (tambah dan ubah).
#[derive(Debug, Clone, Default)]
pub struct RepoInput {
    pub name: String,
    pub provider: String,
    pub url: String,
    pub default_branch: String,
}

/// Isian yang sudah dinormalisasi dan lolos validasi.
struct ValidRepo {
    name: String,
    provider: RepoProvider,
    url: String,
    default_branch: String,
}

impl TenantEntity for Entity {
    type TenantId = WorkspaceId;

    fn tenant_column() -> Column {
        Column::WorkspaceId
    }
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if insert && self.id.is_not_set() {
            self.id = Set(Uuid::now_v7());
        }
        if !insert && self.updated_at.is_unchanged() {
            self.updated_at = Set(chrono::Utc::now().into());
        }
        Ok(self)
    }
}

impl Model {
    /// Penyedia bertipe. Gagal hanya bila CHECK dilewati.
    ///
    /// # Errors
    ///
    /// Nilai kolom bukan varian [`RepoProvider`].
    pub fn provider(&self) -> Result<RepoProvider, DbErr> {
        RepoProvider::try_from_value(&self.provider)
    }

    /// Repo project `project_id`, urut nama.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn of_project<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        project_id: Uuid,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .in_tenant(workspace_id)
            .filter(Column::ProjectId.eq(project_id))
            .order_by_asc(Column::Name)
            .order_by_asc(Column::Id)
            .all(db)
            .await
    }

    /// Repo `id` di workspace.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn find_in<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        id: Uuid,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(id).in_tenant(workspace_id).one(db).await
    }

    /// Repo `id` milik project `project_id`, dikunci (`FOR UPDATE`) sampai transaksi `txn`
    /// selesai; repo project lain tidak ditemukan.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn lock_in<C: ConnectionTrait>(
        txn: &C,
        workspace_id: WorkspaceId,
        project_id: Uuid,
        id: Uuid,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(id)
            .in_tenant(workspace_id)
            .filter(Column::ProjectId.eq(project_id))
            .lock_exclusive()
            .one(txn)
            .await
    }

    /// Ganti semua field repo. Validasi sama dengan [`ActiveModel::add`].
    ///
    /// # Errors
    ///
    /// Seperti [`ActiveModel::add`].
    pub async fn update_from<C: ConnectionTrait>(
        self,
        db: &C,
        input: &RepoInput,
    ) -> Result<Self, RepoError> {
        let valid = validated(input)?;
        let mut repo = self.into_active_model();
        repo.name = Set(valid.name);
        repo.provider = Set(valid.provider.to_value());
        repo.url = Set(valid.url);
        repo.default_branch = Set(valid.default_branch);
        repo.update(db).await.map_err(name_taken_or_db)
    }
}

impl ActiveModel {
    /// Tambah repo ke project `project_id` setelah normalisasi (trim) dan validasi semua field
    /// sekaligus.
    ///
    /// # Errors
    ///
    /// [`RepoError::Invalid`] untuk field tidak valid atau nama yang sudah dipakai di project
    /// ini (unique index `(project_id, lower(name))`), [`RepoError::Db`] untuk error lain.
    pub async fn add<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        project_id: Uuid,
        input: &RepoInput,
    ) -> Result<Model, RepoError> {
        let valid = validated(input)?;
        Self {
            workspace_id: Set(workspace_id.get()),
            project_id: Set(project_id),
            name: Set(valid.name),
            provider: Set(valid.provider.to_value()),
            url: Set(valid.url),
            default_branch: Set(valid.default_branch),
            ..Default::default()
        }
        .insert(db)
        .await
        .map_err(name_taken_or_db)
    }
}

/// URL clone repo: `https://host/path` (tanpa kredensial) atau `git@host:path`, maksimal
/// [`URL_MAX_LEN`] karakter, tanpa spasi. `host` = huruf, angka, `.`, `-`, dan port opsional
/// (`:443`) untuk https. Kode [`REPO_URL_TOO_LONG`] (`max`) atau [`REPO_URL_INVALID`].
///
/// # Errors
///
/// [`ValidationError`] dengan salah satu kode di atas.
pub fn validate_url(url: &str) -> Result<(), ValidationError> {
    if url.chars().count() > URL_MAX_LEN {
        let mut e = ValidationError::new(REPO_URL_TOO_LONG);
        e.add_param("max".into(), &URL_MAX_LEN);
        return Err(e);
    }
    let valid_path = |path: &str| {
        !path.is_empty()
            && !path.starts_with('/')
            && !path.chars().any(|c| c.is_whitespace() || c.is_control())
    };
    let valid_host = |host: &str| {
        !host.is_empty()
            && !host.starts_with(['.', '-'])
            && host
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
    };
    let ok = if let Some(rest) = url.strip_prefix("https://") {
        rest.split_once('/').is_some_and(|(authority, path)| {
            let host = match authority.split_once(':') {
                Some((host, port)) => {
                    (!port.is_empty() && port.bytes().all(|b| b.is_ascii_digit())).then_some(host)
                }
                None => Some(authority),
            };
            host.is_some_and(valid_host) && valid_path(path)
        })
    } else if let Some(rest) = url.strip_prefix("git@") {
        rest.split_once(':')
            .is_some_and(|(host, path)| valid_host(host) && valid_path(path))
    } else {
        false
    };
    if ok {
        Ok(())
    } else {
        Err(ValidationError::new(REPO_URL_INVALID))
    }
}

fn validated(input: &RepoInput) -> Result<ValidRepo, RepoError> {
    let mut errors = ValidationErrors::new();
    let name = text::normalize_name(&input.name);
    if let Err(e) = text::validate_name("repo", &name) {
        errors.add("name", e);
    }
    let provider = RepoProvider::try_from_value(&input.provider.trim().to_string()).ok();
    if provider.is_none() {
        errors.add("provider", ValidationError::new(REPO_PROVIDER_INVALID));
    }
    let url = input.url.trim().to_string();
    if let Err(e) = validate_url(&url) {
        errors.add("url", e);
    }
    let default_branch = input.default_branch.trim().to_string();
    let branch_ok = !default_branch.is_empty()
        && default_branch.chars().count() <= BRANCH_MAX_LEN
        && !default_branch
            .chars()
            .any(|c| c.is_whitespace() || c.is_control());
    if !branch_ok {
        let mut e = ValidationError::new(REPO_BRANCH_INVALID);
        e.add_param("max".into(), &BRANCH_MAX_LEN);
        errors.add("default_branch", e);
    }
    match provider {
        Some(provider) if errors.is_empty() => Ok(ValidRepo {
            name,
            provider,
            url,
            default_branch,
        }),
        _ => Err(RepoError::Invalid(errors.into())),
    }
}

fn name_taken_or_db(e: DbErr) -> RepoError {
    if matches!(e.sql_err(), Some(SqlErr::UniqueConstraintViolation(_))) {
        let mut errors = ValidationErrors::new();
        errors.add("name", ValidationError::new(REPO_NAME_TAKEN));
        RepoError::Invalid(errors.into())
    } else {
        RepoError::Db(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_accepts_https_and_scp_like_ssh_only() {
        for ok in [
            "https://github.com/acme/kasir.git",
            "https://gitlab.example.co.id:8443/group/sub/repo",
            "git@github.com:acme/kasir.git",
            "git@bitbucket.org:acme/kasir",
        ] {
            assert!(validate_url(ok).is_ok(), "{ok}");
        }
        for bad in [
            "",
            "http://github.com/acme/kasir",
            "ssh://git@github.com/acme/kasir.git",
            "https://github.com",
            "https://github.com/",
            "https://user:token@github.com/acme/kasir",
            "https://github.com/acme/ka sir",
            "https://-bad.com/a",
            "https://github.com:/a",
            "git@github.com",
            "git@:acme/kasir",
            "git@github.com:",
            "javascript:alert(1)",
            "file:///etc/passwd",
        ] {
            assert!(validate_url(bad).is_err(), "{bad}");
        }
        let long = format!("https://github.com/{}", "a".repeat(URL_MAX_LEN));
        assert_eq!(validate_url(&long).unwrap_err().code, REPO_URL_TOO_LONG);
    }
}
