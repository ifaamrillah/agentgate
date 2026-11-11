//! Role = himpunan [`Permission`] per workspace. Empat role bawaan (`key` terisi, `builtin`)
//! dibuat bersama workspace; role custom (`key` NULL) dibuat lewat Pengaturan (Task 10).
//! Pertanyaan "boleh apa" ditanyakan ke sini (`has`), bukan dirangkai di controller (rules §6).
//!
//! `permissions` di entity tetap `Vec<String>` (hasil generate); [`Model::permissions`] yang
//! memetakannya ke `Vec<Permission>`, seperti kolom enum lain (`user_tokens::Model::purpose`).

use loco_rs::{
    model::{TenantEntity, TenantQueryExt},
    validation::ModelValidationErrors,
};
use sea_orm::{ActiveValue::Set, QueryOrder, SqlErr, entity::prelude::*};
use validator::{ValidationError, ValidationErrors};

pub use super::_entities::roles::{ActiveModel, Column, Entity, Model};
use super::{
    permission::{BUILTIN_ROLES, Permission, SETTINGS_PERMISSIONS},
    workspaces::WorkspaceId,
};
use crate::common::text;

pub const ROLE_NAME_TAKEN: &str = "role-name-taken";
/// Kode error field milik model ini; masing-masing punya pesan di Fluent (dijaga tes i18n).
pub const ERROR_CODES: &[&str] = &[ROLE_NAME_TAKEN];

/// Key role bawaan. Disimpan sebagai string dengan CHECK constraint (rules §4, dijaga
/// `tests/models/enums.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum RoleKey {
    #[sea_orm(string_value = "owner")]
    Owner,
    #[sea_orm(string_value = "admin")]
    Admin,
    #[sea_orm(string_value = "member")]
    Member,
    #[sea_orm(string_value = "viewer")]
    Viewer,
}

impl RoleKey {
    /// Nama role bawaan saat dibuat. Nama produk (sama di semua bahasa, seperti di spec),
    /// disimpan di `roles.name` seperti nama role custom.
    #[must_use]
    pub fn default_name(self) -> &'static str {
        match self {
            Self::Owner => "Owner",
            Self::Admin => "Admin",
            Self::Member => "Member",
            Self::Viewer => "Viewer",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RoleError {
    /// Validasi field (`name`) gagal, termasuk nama yang sudah dipakai di workspace ini.
    #[error("invalid role: {0}")]
    Invalid(ModelValidationErrors),
    #[error(transparent)]
    Db(#[from] DbErr),
}

impl From<RoleError> for loco_rs::Error {
    fn from(err: RoleError) -> Self {
        match err {
            RoleError::Invalid(errors) => Self::Validation(errors),
            RoleError::Db(e) => Self::DB(e),
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
    /// Key bawaan bertipe, `None` untuk role custom. Gagal hanya bila CHECK dilewati.
    ///
    /// # Errors
    ///
    /// Nilai kolom bukan varian [`RoleKey`].
    pub fn role_key(&self) -> Result<Option<RoleKey>, DbErr> {
        self.key.as_ref().map(RoleKey::try_from_value).transpose()
    }

    /// Izin bertipe dari kolom `text[]`. Gagal hanya bila CHECK dilewati.
    ///
    /// # Errors
    ///
    /// Elemen bukan varian [`Permission`].
    pub fn permissions(&self) -> Result<Vec<Permission>, DbErr> {
        self.permissions
            .iter()
            .map(Permission::try_from_value)
            .collect()
    }

    /// Role ini memegang `permission`.
    #[must_use]
    pub fn has(&self, permission: Permission) -> bool {
        self.permissions.contains(&permission.to_value())
    }

    /// Role ini membuka setidaknya satu halaman Pengaturan workspace.
    #[must_use]
    pub fn has_settings_access(&self) -> bool {
        SETTINGS_PERMISSIONS.iter().any(|p| self.has(*p))
    }

    /// Role bawaan Owner.
    #[must_use]
    pub fn is_owner(&self) -> bool {
        self.key.as_deref() == Some(RoleKey::Owner.to_value().as_str())
    }

    /// Semua role workspace, bawaan lebih dulu, lalu urut waktu dibuat.
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
            .order_by_desc(Column::Builtin)
            .order_by_asc(Column::CreatedAt)
            .order_by_asc(Column::Id)
            .all(db)
            .await
    }

    /// Role `id` di workspace ini; role workspace lain tidak ditemukan.
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

    /// Role bawaan `key` di workspace ini.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn find_builtin<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        key: RoleKey,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find()
            .in_tenant(workspace_id)
            .filter(Column::Key.eq(key.to_value()))
            .one(db)
            .await
    }
}

impl ActiveModel {
    /// Empat role bawaan ([`BUILTIN_ROLES`]) untuk workspace baru. Dipanggil sekali, di
    /// transaksi pembuatan workspace.
    ///
    /// # Errors
    ///
    /// Error database (termasuk unique index bila sudah ada).
    pub async fn create_builtin_set<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
    ) -> Result<Vec<Model>, DbErr> {
        let mut roles = Vec::with_capacity(BUILTIN_ROLES.len());
        for builtin in BUILTIN_ROLES {
            let role = Self {
                workspace_id: Set(workspace_id.get()),
                key: Set(Some(builtin.key.to_value())),
                name: Set(builtin.key.default_name().to_string()),
                permissions: Set(to_strings(builtin.permissions)),
                builtin: Set(true),
                ..Default::default()
            }
            .insert(db)
            .await?;
            roles.push(role);
        }
        Ok(roles)
    }

    /// Role custom dengan nama dan izin bebas.
    ///
    /// # Errors
    ///
    /// [`RoleError::Invalid`] untuk nama tidak valid atau sudah dipakai di workspace ini
    /// (unique index `(workspace_id, lower(name))`), [`RoleError::Db`] untuk error lain.
    pub async fn create_custom<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        name: &str,
        permissions: Vec<Permission>,
    ) -> Result<Model, RoleError> {
        let name = text::normalize_name(name);
        if let Err(e) = text::validate_name("role", &name) {
            let mut errors = ValidationErrors::new();
            errors.add("name", e);
            return Err(RoleError::Invalid(errors.into()));
        }
        Self {
            workspace_id: Set(workspace_id.get()),
            name: Set(name),
            permissions: Set(to_strings(&permissions)),
            ..Default::default()
        }
        .insert(db)
        .await
        .map_err(|e| {
            if matches!(e.sql_err(), Some(SqlErr::UniqueConstraintViolation(_))) {
                let mut errors = ValidationErrors::new();
                errors.add("name", ValidationError::new(ROLE_NAME_TAKEN));
                RoleError::Invalid(errors.into())
            } else {
                RoleError::Db(e)
            }
        })
    }
}

fn to_strings(permissions: &[Permission]) -> Vec<String> {
    permissions.iter().map(|p| p.to_value()).collect()
}
