//! Workspace = tenant (perusahaan/organisasi). Setiap tabel domain punya `workspace_id`, dan
//! setiap query model atas tabel itu menerima [`WorkspaceId`] sebagai parameter wajib
//! (`TenantEntity` Loco, rules "Ditunda → Multi-tenancy"). Workspace di-resolve sekali per
//! request oleh extractor `Tenant` dari path `/w/{slug}`.
//!
//! Slug dinormalisasi ke huruf kecil di sini dan unik lewat expression index `lower(slug)`;
//! pencarian juga case-insensitive, jadi `/w/ACME` me-resolve workspace `acme`. Slug baru tanpa
//! isian dibentuk dari nama (`text::slugify`). Daftar slug terlarang menjaga path statis di
//! bawah `/w/` (`/w/new`) dan path yang mungkin dipakai kelak.

use loco_rs::validation::ModelValidationErrors;
use sea_orm::{
    ActiveValue::Set,
    IntoActiveModel, SqlErr,
    entity::prelude::*,
    sea_query::{ExprTrait, Func},
};
use serde::{Deserialize, Serialize};
use validator::{ValidationError, ValidationErrors};

pub use super::_entities::workspaces::{ActiveModel, Column, Entity, Model};
use super::{roles, users};
use crate::common::text;

/// Slug yang tidak boleh dipakai workspace: bentrok dengan route statis di bawah `/w/`, atau
/// dicadangkan untuk area aplikasi.
pub const RESERVED_SLUGS: &[&str] = &["new", "settings", "admin", "api", "auth"];

pub const SLUG_TAKEN: &str = "slug-taken";
pub const SLUG_RESERVED: &str = "slug-reserved";
/// Kode error field milik model ini (kode slug umum ada di `common::text`).
pub const ERROR_CODES: &[&str] = &[SLUG_TAKEN, SLUG_RESERVED];

/// Id workspace bertipe: parameter wajib setiap query tabel milik workspace, sehingga query
/// tanpa tenant tidak bisa ditulis tanpa sengaja melewatkan argumen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkspaceId(pub Uuid);

impl From<WorkspaceId> for sea_orm::Value {
    fn from(id: WorkspaceId) -> Self {
        id.0.into()
    }
}

impl std::fmt::Display for WorkspaceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Data untuk [`ActiveModel::create`]. Slug kosong = dibentuk dari nama.
#[derive(Debug)]
pub struct CreateWorkspace {
    pub name: String,
    pub slug: String,
    pub created_by: Option<Uuid>,
}

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    /// Validasi field (`name`, `slug`) gagal, termasuk slug yang sudah dipakai.
    #[error("invalid workspace: {0}")]
    Invalid(ModelValidationErrors),
    #[error(transparent)]
    Db(#[from] DbErr),
}

impl From<WorkspaceError> for loco_rs::Error {
    fn from(err: WorkspaceError) -> Self {
        match err {
            WorkspaceError::Invalid(errors) => Self::Validation(errors),
            WorkspaceError::Db(e) => Self::DB(e),
        }
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
    #[must_use]
    pub fn workspace_id(&self) -> WorkspaceId {
        WorkspaceId(self.id)
    }

    /// Workspace dengan slug ini, tanpa membedakan huruf besar.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn find_by_slug<C: ConnectionTrait>(
        db: &C,
        slug: &str,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find()
            .filter(lower_slug().eq(slug.trim().to_lowercase()))
            .one(db)
            .await
    }

    /// Workspace boleh dibuka anggota dengan role `role`: selama dijadwalkan hapus, hanya
    /// Owner yang masih melihatnya (untuk membatalkan). Satu aturan untuk extractor `Tenant`
    /// dan daftar workspace user.
    #[must_use]
    pub fn is_open_to(&self, role: &roles::Model) -> bool {
        self.deletion_scheduled_at.is_none() || role.is_owner()
    }

    /// Workspace mewajibkan 2FA dan `user` belum mengaktifkannya.
    #[must_use]
    pub fn requires_two_factor_from(&self, user: &users::Model) -> bool {
        self.require_2fa_at.is_some() && user.totp_enabled_at.is_none()
    }

    /// Ganti nama dan slug setelah normalisasi dan validasi yang sama dengan
    /// [`ActiveModel::create`]. Slug kosong berarti tetap.
    ///
    /// # Errors
    ///
    /// [`WorkspaceError::Invalid`] untuk field tidak valid atau slug terpakai,
    /// [`WorkspaceError::Db`] untuk error database lain.
    pub async fn update_details<C: ConnectionTrait>(
        self,
        db: &C,
        name: &str,
        slug: &str,
    ) -> Result<Self, WorkspaceError> {
        let slug = if slug.trim().is_empty() {
            self.slug.clone()
        } else {
            slug.to_string()
        };
        let (name, slug) = normalize_and_validate(name, &slug)?;
        let mut workspace = self.into_active_model();
        workspace.name = Set(name);
        workspace.slug = Set(slug);
        workspace.update(db).await.map_err(slug_taken_or_db)
    }
}

impl ActiveModel {
    /// Buat workspace (tanpa role dan anggota; workflow lengkapnya
    /// `services::workspaces::create`).
    ///
    /// # Errors
    ///
    /// [`WorkspaceError::Invalid`] untuk field tidak valid, slug terlarang, atau slug yang
    /// sudah dipakai (unique index `lower(slug)`), [`WorkspaceError::Db`] untuk error lain.
    pub async fn create<C: ConnectionTrait>(
        db: &C,
        new: CreateWorkspace,
    ) -> Result<Model, WorkspaceError> {
        let slug = if new.slug.trim().is_empty() {
            text::slugify(&new.name)
        } else {
            new.slug
        };
        let (name, slug) = normalize_and_validate(&new.name, &slug)?;
        Self {
            name: Set(name),
            slug: Set(slug),
            created_by: Set(new.created_by),
            ..Default::default()
        }
        .insert(db)
        .await
        .map_err(slug_taken_or_db)
    }
}

/// `lower(slug)`: bentuk yang sama dengan unique index, sehingga pencarian memakai index itu.
fn lower_slug() -> Expr {
    Expr::expr(Func::lower(Expr::col(Column::Slug)))
}

/// Normalisasi nama (`trim`) dan slug (`trim` + huruf kecil), lalu validasi keduanya sekaligus
/// supaya form menampilkan semua error.
fn normalize_and_validate(name: &str, slug: &str) -> Result<(String, String), WorkspaceError> {
    let name = text::normalize_name(name);
    let slug = slug.trim().to_lowercase();
    let mut errors = ValidationErrors::new();
    if let Err(e) = text::validate_name("workspace", &name) {
        errors.add("name", e);
    }
    if let Err(e) = text::validate_slug(&slug) {
        errors.add("slug", e);
    } else if RESERVED_SLUGS.contains(&slug.as_str()) {
        errors.add("slug", ValidationError::new(SLUG_RESERVED));
    }
    if errors.is_empty() {
        Ok((name, slug))
    } else {
        Err(WorkspaceError::Invalid(errors.into()))
    }
}

fn slug_taken_or_db(e: DbErr) -> WorkspaceError {
    if matches!(e.sql_err(), Some(SqlErr::UniqueConstraintViolation(_))) {
        let mut errors = ValidationErrors::new();
        errors.add("slug", ValidationError::new(SLUG_TAKEN));
        WorkspaceError::Invalid(errors.into())
    } else {
        WorkspaceError::Db(e)
    }
}
