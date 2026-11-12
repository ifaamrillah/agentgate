//! Skill pack workspace (spec "Persona agent, skill pack & model"): kumpulan skill format
//! `SKILL.md` + `scripts/` + `references/`, dari repo Git publik atau ditulis sendiri.
//!
//! Siklus: `draft` (bisa diubah) → `pending` (dikirim ke review, ada laporan pindai) →
//! `approved` (bisa dipasang ke persona) atau `rejected` (kembali bisa diubah dan dikirim ulang).
//! Mengubah file skill pack yang sudah `approved` tidak bisa (versi baru = utang M5).

use loco_rs::model::{TenantEntity, TenantQueryExt};
use sea_orm::{QueryOrder, QuerySelect, entity::prelude::*};
use uuid::Uuid;

pub use super::_entities::skill_packs::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

pub const NAME_TAKEN: &str = "skill-name-taken";
pub const DESCRIPTION_TOO_LONG: &str = "skill-description-too-long";
pub const NOT_APPROVED: &str = "skill-not-approved";
pub const NOT_EDITABLE: &str = "skill-not-editable";
pub const PATH_INVALID: &str = "skill-path-invalid";
pub const FILE_TOO_LARGE: &str = "skill-file-too-large";
pub const TOO_MANY_FILES: &str = "skill-too-many-files";
pub const SKILL_MD_REQUIRED: &str = "skill-skill-md-required";
pub const REPO_URL_INVALID: &str = "skill-repo-url-invalid";
pub const FETCH_FAILED: &str = "skill-fetch-failed";
pub const NOTE_REQUIRED: &str = "skill-review-note-required";
pub const CONFIRM_MISMATCH: &str = "skill-confirm-mismatch";
pub const NOT_PENDING: &str = "skill-not-pending";
pub const ERROR_CODES: &[&str] = &[
    NAME_TAKEN,
    DESCRIPTION_TOO_LONG,
    NOT_APPROVED,
    NOT_EDITABLE,
    PATH_INVALID,
    FILE_TOO_LARGE,
    TOO_MANY_FILES,
    SKILL_MD_REQUIRED,
    REPO_URL_INVALID,
    FETCH_FAILED,
    NOTE_REQUIRED,
    CONFIRM_MISMATCH,
    NOT_PENDING,
];

pub const DESCRIPTION_MAX_LEN: usize = 500;
/// Batas file per skill pack dan ukuran satu file (byte).
pub const MAX_FILES: usize = 60;
pub const MAX_FILE_BYTES: usize = 64 * 1024;
/// Batas total isi skill pack (byte).
pub const MAX_TOTAL_BYTES: usize = 512 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum SkillSource {
    #[sea_orm(string_value = "custom")]
    Custom,
    #[sea_orm(string_value = "repo")]
    Repo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum SkillStatus {
    #[sea_orm(string_value = "draft")]
    Draft,
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "approved")]
    Approved,
    #[sea_orm(string_value = "rejected")]
    Rejected,
}

/// Hasil pindai statis: Aman, Perlu perhatian, Berbahaya.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum SkillRisk {
    #[sea_orm(string_value = "safe")]
    Safe,
    #[sea_orm(string_value = "attention")]
    Attention,
    #[sea_orm(string_value = "danger")]
    Danger,
}

impl TenantEntity for Entity {
    type TenantId = WorkspaceId;

    fn tenant_column() -> Column {
        Column::WorkspaceId
    }
}

standard_behavior!(ActiveModel);

impl Model {
    #[must_use]
    pub fn status(&self) -> SkillStatus {
        SkillStatus::try_from_value(&self.status).unwrap_or(SkillStatus::Draft)
    }

    #[must_use]
    pub fn risk(&self) -> Option<SkillRisk> {
        self.risk
            .as_deref()
            .and_then(|r| SkillRisk::try_from_value(&r.to_string()).ok())
    }

    #[must_use]
    pub fn source(&self) -> SkillSource {
        SkillSource::try_from_value(&self.source).unwrap_or(SkillSource::Custom)
    }

    /// Draf dan yang ditolak masih bisa diubah lalu dikirim (ulang) ke review.
    #[must_use]
    pub fn editable(&self) -> bool {
        matches!(self.status(), SkillStatus::Draft | SkillStatus::Rejected)
    }

    /// Semua skill pack workspace, terbaru dulu.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn all_in<C: ConnectionTrait>(db: &C, wid: WorkspaceId) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .in_tenant(wid)
            .order_by_asc(Column::Name)
            .all(db)
            .await
    }

    /// Skill pack yang sudah di-approve (boleh dipasang ke persona).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn approved_in<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .in_tenant(wid)
            .filter(Column::Status.eq(SkillStatus::Approved.to_value()))
            .order_by_asc(Column::Name)
            .all(db)
            .await
    }

    /// # Errors
    ///
    /// Error database.
    pub async fn find_in<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        id: Uuid,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(id).in_tenant(wid).one(db).await
    }

    /// # Errors
    ///
    /// Error database.
    pub async fn lock_in<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        id: Uuid,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(id)
            .in_tenant(wid)
            .lock_exclusive()
            .one(db)
            .await
    }
}
