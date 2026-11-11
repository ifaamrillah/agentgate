//! Jabatan tim (PM, Dev, QA, …): label bebas per workspace untuk default assignment. Nama unik
//! per workspace tanpa membedakan huruf besar. Dikelola di Pengaturan → Jabatan.
//!
//! Hapus jabatan tidak dicek di sini: FK `(job_title_id, workspace_id)` di `workspace_members`
//! dan `invitations` memakai `ON DELETE SET NULL (job_title_id)`, sehingga member dan undangan
//! yang memakainya menjadi tanpa jabatan.

use loco_rs::{
    model::{TenantEntity, TenantQueryExt},
    validation::ModelValidationErrors,
};
use sea_orm::{
    ActiveValue::Set, IntoActiveModel, QueryOrder, QuerySelect, SqlErr, entity::prelude::*,
};
use validator::{ValidationError, ValidationErrors};

pub use super::_entities::job_titles::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;
use crate::common::text;

pub const JOB_TITLE_NAME_TAKEN: &str = "job-title-name-taken";
/// Kode error field milik model ini; masing-masing punya pesan di Fluent (dijaga tes i18n).
pub const ERROR_CODES: &[&str] = &[JOB_TITLE_NAME_TAKEN];

#[derive(Debug, thiserror::Error)]
pub enum JobTitleError {
    /// Validasi field (`name`) gagal, termasuk nama yang sudah dipakai di workspace ini.
    #[error("invalid job title: {0}")]
    Invalid(ModelValidationErrors),
    #[error(transparent)]
    Db(#[from] DbErr),
}

impl From<JobTitleError> for loco_rs::Error {
    fn from(err: JobTitleError) -> Self {
        match err {
            JobTitleError::Invalid(errors) => Self::Validation(errors),
            JobTitleError::Db(e) => Self::DB(e),
        }
    }
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
    /// Semua jabatan workspace, urut nama.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn in_workspace<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .in_tenant(workspace_id)
            .order_by_asc(Column::Name)
            .order_by_asc(Column::Id)
            .all(db)
            .await
    }

    /// Jabatan `id` di workspace ini; jabatan workspace lain tidak ditemukan.
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

    /// Seperti [`Self::find_in`], dengan baris dikunci (`FOR UPDATE`) sampai transaksi `txn`
    /// selesai.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn lock_in<C: ConnectionTrait>(
        txn: &C,
        workspace_id: WorkspaceId,
        id: Uuid,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(id)
            .in_tenant(workspace_id)
            .lock_exclusive()
            .one(txn)
            .await
    }
}

impl ActiveModel {
    /// Buat jabatan setelah normalisasi dan validasi nama.
    ///
    /// # Errors
    ///
    /// [`JobTitleError::Invalid`] untuk nama tidak valid atau sudah dipakai (unique index
    /// `(workspace_id, lower(name))`), [`JobTitleError::Db`] untuk error lain.
    pub async fn create<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        name: &str,
    ) -> Result<Model, JobTitleError> {
        let name = validated(name)?;
        Self {
            workspace_id: Set(workspace_id.get()),
            name: Set(name),
            ..Default::default()
        }
        .insert(db)
        .await
        .map_err(name_taken_or_db)
    }

    /// Ganti nama jabatan. Validasi sama dengan [`Self::create`].
    ///
    /// # Errors
    ///
    /// Seperti [`Self::create`].
    pub async fn rename<C: ConnectionTrait>(
        db: &C,
        title: Model,
        name: &str,
    ) -> Result<Model, JobTitleError> {
        let name = validated(name)?;
        let mut active = title.into_active_model();
        active.name = Set(name);
        active.update(db).await.map_err(name_taken_or_db)
    }
}

fn validated(name: &str) -> Result<String, JobTitleError> {
    let name = text::normalize_name(name);
    text::validate_name("job_title", &name).map_err(|e| {
        let mut errors = ValidationErrors::new();
        errors.add("name", e);
        JobTitleError::Invalid(errors.into())
    })?;
    Ok(name)
}

fn name_taken_or_db(e: DbErr) -> JobTitleError {
    if matches!(e.sql_err(), Some(SqlErr::UniqueConstraintViolation(_))) {
        let mut errors = ValidationErrors::new();
        errors.add("name", ValidationError::new(JOB_TITLE_NAME_TAKEN));
        JobTitleError::Invalid(errors.into())
    } else {
        JobTitleError::Db(e)
    }
}
