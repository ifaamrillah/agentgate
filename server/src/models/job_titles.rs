//! Jabatan tim (PM, Dev, QA, …): label bebas per workspace untuk default assignment. Nama unik
//! per workspace tanpa membedakan huruf besar. Halaman pengelolaannya di Task 10.

use loco_rs::{
    model::{TenantEntity, TenantQueryExt},
    validation::ModelValidationErrors,
};
use sea_orm::{ActiveValue::Set, QueryOrder, SqlErr, entity::prelude::*};
use validator::{ValidationError, ValidationErrors};

pub use super::_entities::job_titles::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;
use crate::common::text;

pub const JOB_TITLE_TAKEN: &str = "job-title-taken";
/// Kode error field milik model ini; masing-masing punya pesan di Fluent (dijaga tes i18n).
pub const ERROR_CODES: &[&str] = &[JOB_TITLE_TAKEN];

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
        let name = text::normalize_name(name);
        if let Err(e) = text::validate_name("job_title", &name) {
            let mut errors = ValidationErrors::new();
            errors.add("name", e);
            return Err(JobTitleError::Invalid(errors.into()));
        }
        Self {
            workspace_id: Set(workspace_id.get()),
            name: Set(name),
            ..Default::default()
        }
        .insert(db)
        .await
        .map_err(|e| {
            if matches!(e.sql_err(), Some(SqlErr::UniqueConstraintViolation(_))) {
                let mut errors = ValidationErrors::new();
                errors.add("name", ValidationError::new(JOB_TITLE_TAKEN));
                JobTitleError::Invalid(errors.into())
            } else {
                JobTitleError::Db(e)
            }
        })
    }
}
