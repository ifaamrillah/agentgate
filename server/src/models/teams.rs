//! Tim = kelompok anggota workspace (Tim Backend, Tim QA) yang bisa diberi akses ke project
//! sekaligus (`project_grants.team_id`). Nama unik per workspace tanpa membedakan huruf besar.
//! Dikelola di Pengaturan → Tim (`ManageTeams`).
//!
//! Hapus tim tidak dicek di sini: FK `team_members.team_id` dan `project_grants.team_id`
//! memakai `ON DELETE CASCADE`, jadi anggota tim dan akses project lewat tim ikut hilang.

use loco_rs::{
    model::{TenantEntity, TenantQueryExt},
    validation::ModelValidationErrors,
};
use sea_orm::{
    ActiveValue::Set, IntoActiveModel, QueryOrder, QuerySelect, SqlErr, entity::prelude::*,
};
use validator::{ValidationError, ValidationErrors};

pub use super::_entities::teams::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;
use crate::common::text;

pub const TEAM_NAME_TAKEN: &str = "team-name-taken";
/// Kode error field milik model ini; masing-masing punya pesan di Fluent (dijaga tes i18n).
pub const ERROR_CODES: &[&str] = &[TEAM_NAME_TAKEN];

#[derive(Debug, thiserror::Error)]
pub enum TeamError {
    /// Validasi field (`name`) gagal, termasuk nama yang sudah dipakai di workspace ini.
    #[error("invalid team: {0}")]
    Invalid(ModelValidationErrors),
    #[error(transparent)]
    Db(#[from] DbErr),
}

impl From<TeamError> for loco_rs::Error {
    fn from(err: TeamError) -> Self {
        match err {
            TeamError::Invalid(errors) => Self::Validation(errors),
            TeamError::Db(e) => Self::DB(e),
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
    /// Semua tim workspace, urut nama.
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

    /// Tim `id` di workspace ini; tim workspace lain tidak ditemukan.
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
    /// Buat tim setelah normalisasi dan validasi nama.
    ///
    /// # Errors
    ///
    /// [`TeamError::Invalid`] untuk nama tidak valid atau sudah dipakai (unique index
    /// `(workspace_id, lower(name))`), [`TeamError::Db`] untuk error lain.
    pub async fn create<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        name: &str,
    ) -> Result<Model, TeamError> {
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

    /// Ganti nama tim. Validasi sama dengan [`Self::create`].
    ///
    /// # Errors
    ///
    /// Seperti [`Self::create`].
    pub async fn rename<C: ConnectionTrait>(
        db: &C,
        team: Model,
        name: &str,
    ) -> Result<Model, TeamError> {
        let name = validated(name)?;
        let mut active = team.into_active_model();
        active.name = Set(name);
        active.update(db).await.map_err(name_taken_or_db)
    }
}

fn validated(name: &str) -> Result<String, TeamError> {
    let name = text::normalize_name(name);
    text::validate_name("team", &name).map_err(|e| {
        let mut errors = ValidationErrors::new();
        errors.add("name", e);
        TeamError::Invalid(errors.into())
    })?;
    Ok(name)
}

fn name_taken_or_db(e: DbErr) -> TeamError {
    if matches!(e.sql_err(), Some(SqlErr::UniqueConstraintViolation(_))) {
        let mut errors = ValidationErrors::new();
        errors.add("name", ValidationError::new(TEAM_NAME_TAKEN));
        TeamError::Invalid(errors.into())
    } else {
        TeamError::Db(e)
    }
}
