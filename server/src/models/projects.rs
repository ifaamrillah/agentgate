//! Project = kumpulan tiket dan repo di dalam workspace, dibuka di `/w/{slug}/p/{key}`.
//!
//! `key` adalah kode pendek project (`KSR`) yang kelak menjadi awalan nomor tiket (M2):
//! dinormalisasi ke huruf besar di sini, format `^[A-Z][A-Z0-9]{1,9}$`, unik per workspace lewat
//! expression index `upper(key)`. Pencarian dari URL juga lewat `upper(key)`, jadi `/p/ksr`
//! me-resolve `KSR`.
//!
//! Siapa boleh melihat project ditentukan `project_access` (satu aturan); di sini hanya
//! [`Model::accessible_for`], query yang memakai aturan itu untuk banyak project sekaligus.
//! Arsip (`archived_at`) tidak menutup akses: project yang diarsipkan tetap bisa dibuka dan
//! ditandai di daftar.

use loco_rs::{
    model::{TenantEntity, TenantQueryExt},
    validation::ModelValidationErrors,
};
use sea_orm::{
    ActiveValue::Set,
    IntoActiveModel, QueryOrder, QuerySelect, SqlErr,
    entity::prelude::*,
    sea_query::{ExprTrait, Func, NullOrdering, Order},
};
use validator::{ValidationError, ValidationErrors};

pub use super::_entities::projects::{ActiveModel, Column, Entity, Model};
use super::{
    project_access::{self, Accessor},
    workspaces::WorkspaceId,
};
use crate::common::text;

/// Key bukan 2–10 huruf besar/angka yang diawali huruf.
pub const PROJECT_KEY_INVALID: &str = "project-key-invalid";
pub const PROJECT_KEY_TAKEN: &str = "project-key-taken";
pub const PROJECT_DESCRIPTION_TOO_LONG: &str = "project-description-too-long";
/// Kode error field milik model ini; masing-masing punya pesan di Fluent (dijaga tes i18n).
pub const ERROR_CODES: &[&str] = &[
    PROJECT_KEY_INVALID,
    PROJECT_KEY_TAKEN,
    PROJECT_DESCRIPTION_TOO_LONG,
];

/// Panjang key project: huruf pertama ditambah 1–9 huruf/angka.
pub const KEY_MIN_LEN: usize = 2;
pub const KEY_MAX_LEN: usize = 10;
/// Panjang maksimal deskripsi project, dalam karakter.
pub const DESCRIPTION_MAX_LEN: usize = 2000;

#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    /// Validasi field (`key`, `name`, `description`) gagal, termasuk key yang sudah dipakai.
    #[error("invalid project: {0}")]
    Invalid(ModelValidationErrors),
    #[error(transparent)]
    Db(#[from] DbErr),
}

impl From<ProjectError> for loco_rs::Error {
    fn from(err: ProjectError) -> Self {
        match err {
            ProjectError::Invalid(errors) => Self::Validation(errors),
            ProjectError::Db(e) => Self::DB(e),
        }
    }
}

/// Data untuk [`ActiveModel::create`].
#[derive(Debug, Clone)]
pub struct NewProject {
    pub key: String,
    pub name: String,
    pub description: String,
    pub created_by: Option<Uuid>,
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
    #[must_use]
    pub fn is_archived(&self) -> bool {
        self.archived_at.is_some()
    }

    /// Project dengan key ini di workspace ini, tanpa membedakan huruf besar.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn find_by_key<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        key: &str,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find()
            .in_tenant(workspace_id)
            .filter(upper_key().eq(key.trim().to_uppercase()))
            .one(db)
            .await
    }

    /// Project `id` di workspace ini; project workspace lain tidak ditemukan.
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
    /// selesai: perubahan project, akses, dan repo berjalan berurutan per project.
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

    /// Project yang boleh dibuka `who` (aturan `project_access`), dalam satu query: project
    /// aktif dulu, lalu yang diarsipkan; masing-masing urut nama.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn accessible_for<C: ConnectionTrait>(
        db: &C,
        who: &Accessor<'_>,
    ) -> Result<Vec<Self>, DbErr> {
        let mut query = Entity::find().in_tenant(who.workspace_id);
        if !project_access::is_admin_everywhere(who.role) {
            query = query.filter(
                Column::Id.in_subquery(project_access::grants_with_project_permission(who)),
            );
        }
        query
            .order_by_with_nulls(Column::ArchivedAt, Order::Asc, NullOrdering::First)
            .order_by_asc(Column::Name)
            .order_by_asc(Column::Id)
            .all(db)
            .await
    }

    /// Ganti nama dan deskripsi. Key tetap (kelak menjadi awalan nomor tiket).
    ///
    /// # Errors
    ///
    /// [`ProjectError::Invalid`] untuk field tidak valid, [`ProjectError::Db`] untuk error lain.
    pub async fn update_details<C: ConnectionTrait>(
        self,
        db: &C,
        name: &str,
        description: &str,
    ) -> Result<Self, ProjectError> {
        let mut errors = ValidationErrors::new();
        let name = validated_name(name, &mut errors);
        let description = validated_description(description, &mut errors);
        if !errors.is_empty() {
            return Err(ProjectError::Invalid(errors.into()));
        }
        let mut project = self.into_active_model();
        project.name = Set(name);
        project.description = Set(description);
        Ok(project.update(db).await?)
    }

    /// Tandai project diarsipkan (sekarang). Project yang sudah diarsipkan tidak diubah.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn archive<C: ConnectionTrait>(self, db: &C) -> Result<Self, DbErr> {
        if self.is_archived() {
            return Ok(self);
        }
        let mut project = self.into_active_model();
        project.archived_at = Set(Some(chrono::Utc::now().into()));
        project.update(db).await
    }
}

impl ActiveModel {
    /// Buat project setelah normalisasi (key huruf besar, nama dan deskripsi di-trim) dan
    /// validasi semua field sekaligus.
    ///
    /// # Errors
    ///
    /// [`ProjectError::Invalid`] untuk field tidak valid atau key yang sudah dipakai di
    /// workspace ini (unique index `upper(key)`), [`ProjectError::Db`] untuk error lain.
    pub async fn create<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        new: NewProject,
    ) -> Result<Model, ProjectError> {
        let mut errors = ValidationErrors::new();
        let key = normalize_key(&new.key);
        if let Err(e) = validate_key(&key) {
            errors.add("key", e);
        }
        let name = validated_name(&new.name, &mut errors);
        let description = validated_description(&new.description, &mut errors);
        if !errors.is_empty() {
            return Err(ProjectError::Invalid(errors.into()));
        }
        Self {
            workspace_id: Set(workspace_id.get()),
            key: Set(key),
            name: Set(name),
            description: Set(description),
            created_by: Set(new.created_by),
            ..Default::default()
        }
        .insert(db)
        .await
        .map_err(|e| {
            if matches!(e.sql_err(), Some(SqlErr::UniqueConstraintViolation(_))) {
                let mut errors = ValidationErrors::new();
                errors.add("key", ValidationError::new(PROJECT_KEY_TAKEN));
                ProjectError::Invalid(errors.into())
            } else {
                ProjectError::Db(e)
            }
        })
    }
}

/// Key dari isian user: spasi dibuang, huruf besar.
#[must_use]
pub fn normalize_key(raw: &str) -> String {
    raw.trim().to_uppercase()
}

/// Key yang sudah dinormalisasi: `^[A-Z][A-Z0-9]{1,9}$`. Kode [`PROJECT_KEY_INVALID`] dengan
/// argumen `min`/`max` (panjang).
///
/// # Errors
///
/// [`ValidationError`] dengan kode di atas.
pub fn validate_key(key: &str) -> Result<(), ValidationError> {
    let bytes = key.as_bytes();
    let well_formed = (KEY_MIN_LEN..=KEY_MAX_LEN).contains(&bytes.len())
        && bytes.first().is_some_and(u8::is_ascii_uppercase)
        && bytes
            .iter()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit());
    if well_formed {
        Ok(())
    } else {
        let mut e = ValidationError::new(PROJECT_KEY_INVALID);
        e.add_param("min".into(), &KEY_MIN_LEN);
        e.add_param("max".into(), &KEY_MAX_LEN);
        Err(e)
    }
}

/// `upper(key)`: bentuk yang sama dengan unique index, sehingga pencarian memakai index itu.
fn upper_key() -> Expr {
    Expr::expr(Func::upper(Expr::col(Column::Key)))
}

fn validated_name(name: &str, errors: &mut ValidationErrors) -> String {
    let name = text::normalize_name(name);
    if let Err(e) = text::validate_name("project", &name) {
        errors.add("name", e);
    }
    name
}

fn validated_description(description: &str, errors: &mut ValidationErrors) -> String {
    let description = description.trim().to_string();
    if description.chars().count() > DESCRIPTION_MAX_LEN {
        let mut e = ValidationError::new(PROJECT_DESCRIPTION_TOO_LONG);
        e.add_param("max".into(), &DESCRIPTION_MAX_LEN);
        errors.add("description", e);
    }
    description
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_is_upper_letter_then_one_to_nine_letters_or_digits() {
        for ok in ["KS", "KSR", "A1", "ABCDEFGHIJ", "Q2W"] {
            assert!(validate_key(ok).is_ok(), "{ok}");
        }
        for bad in ["", "K", "1KS", "ABCDEFGHIJK", "K-S", "KS R", "ksr", "KÖ"] {
            assert!(validate_key(bad).is_err(), "{bad}");
        }
        assert_eq!(normalize_key(" ksr "), "KSR");
    }
}
