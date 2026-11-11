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
use sea_orm::{
    ActiveValue::Set, IntoActiveModel, Iterable, QueryOrder, QuerySelect, SqlErr,
    entity::prelude::*,
};
use validator::{ValidationError, ValidationErrors};

pub use super::_entities::roles::{ActiveModel, Column, Entity, Model};
use super::{
    permission::{BUILTIN_ROLES, Permission, SETTINGS_PERMISSIONS},
    workspaces::WorkspaceId,
};
use crate::common::text;

pub const ROLE_NAME_TAKEN: &str = "role-name-taken";
/// Role custom tanpa izin sama sekali (field `permissions`).
pub const ROLE_PERMISSIONS_EMPTY: &str = "role-permissions-empty";
/// Isian izin bukan varian [`Permission`] (form yang diubah di luar browser).
pub const ROLE_PERMISSION_UNKNOWN: &str = "role-permission-unknown";
/// Pelaku memberi atau mencabut izin yang tidak ia pegang ([`ensure_permissions_grantable`]).
pub const ROLE_PERMISSION_NOT_HELD: &str = "role-permission-not-held";
/// Role bawaan tidak bisa diubah atau dihapus ([`RoleError::Builtin`]).
pub const ROLE_BUILTIN: &str = "role-builtin";
/// Role masih dipegang member atau undangan aktif ([`RoleError::InUse`]).
pub const ROLE_IN_USE: &str = "role-in-use";
/// Pelaku memberi atau mencabut role yang tidak boleh ia berikan ([`can_assign`]): mengundang
/// dengan role itu, mengganti role member dari/ke role itu, atau mengeluarkan pemegangnya.
pub const MEMBER_ROLE_NOT_ASSIGNABLE: &str = "member-role-not-assignable";
/// Kode error milik model ini; masing-masing punya pesan di Fluent (dijaga tes i18n).
pub const ERROR_CODES: &[&str] = &[
    MEMBER_ROLE_NOT_ASSIGNABLE,
    ROLE_NAME_TAKEN,
    ROLE_PERMISSIONS_EMPTY,
    ROLE_PERMISSION_UNKNOWN,
    ROLE_PERMISSION_NOT_HELD,
    ROLE_BUILTIN,
    ROLE_IN_USE,
];

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
    /// Validasi field (`name`, `permissions`) gagal, termasuk nama yang sudah dipakai di
    /// workspace ini.
    #[error("invalid role: {0}")]
    Invalid(ModelValidationErrors),
    /// Role bawaan: nama dan izinnya tetap ([`ensure_role_editable`]).
    #[error("built-in role is immutable")]
    Builtin,
    /// Masih dipakai member, undangan aktif, atau grant project (FK `role_id`), jadi tidak bisa
    /// dihapus.
    #[error("role is in use")]
    InUse,
    #[error(transparent)]
    Db(#[from] DbErr),
}

impl RoleError {
    /// Kode Fluent untuk penolakan yang bukan error field (`Builtin`, `InUse`).
    #[must_use]
    pub fn code(&self) -> Option<&'static str> {
        match self {
            Self::Builtin => Some(ROLE_BUILTIN),
            Self::InUse => Some(ROLE_IN_USE),
            Self::Invalid(_) | Self::Db(_) => None,
        }
    }
}

impl From<RoleError> for loco_rs::Error {
    fn from(err: RoleError) -> Self {
        match err {
            RoleError::Invalid(errors) => Self::Validation(errors),
            RoleError::Builtin | RoleError::InUse => Self::BadRequest(err.to_string()),
            RoleError::Db(e) => Self::DB(e),
        }
    }
}

/// Satu-satunya aturan "role ini boleh diubah/dihapus": role bawaan tidak pernah (namanya dan
/// izinnya tetap; ini juga yang menjaga Owner tidak kehilangan `ManageWorkspace`). Dipanggil
/// setiap handler ubah/hapus role dan oleh [`ActiveModel::update_custom`]/
/// [`ActiveModel::delete_custom`].
///
/// # Errors
///
/// [`RoleError::Builtin`] untuk role bawaan.
pub fn ensure_role_editable(role: &Model) -> Result<(), RoleError> {
    if role.builtin {
        return Err(RoleError::Builtin);
    }
    Ok(())
}

/// Satu-satunya aturan "izin apa yang boleh diubah pelaku": setiap izin yang ditambah atau
/// dicabut (`before` → `after`) harus dipegang role pelaku (`actor`). Mencegah eskalasi lewat
/// role custom (pemegang `ManageRoles` menambah `ManageWorkspace` ke role-nya sendiri). Owner
/// memegang semua izin, jadi tidak dibatasi.
///
/// # Errors
///
/// [`RoleError::Invalid`] dengan [`ROLE_PERMISSION_NOT_HELD`] di field `permissions`.
pub fn ensure_permissions_grantable(
    actor: &Model,
    before: &[Permission],
    after: &[Permission],
) -> Result<(), RoleError> {
    let changed = before
        .iter()
        .filter(|p| !after.contains(p))
        .chain(after.iter().filter(|p| !before.contains(p)));
    if changed.into_iter().all(|p| actor.has(*p)) {
        Ok(())
    } else {
        Err(field_error("permissions", ROLE_PERMISSION_NOT_HELD))
    }
}

/// Satu-satunya aturan "role ini boleh diberikan atau dicabut oleh pelaku": role Owner hanya
/// oleh Owner, role lain hanya bila setiap izinnya dipegang role pelaku (`actor`), aturan yang
/// sama dengan [`ensure_permissions_grantable`] untuk penugasan. Berlaku untuk role tujuan
/// (undang, ganti role) dan role asal member (ganti role, keluarkan), sehingga non-Owner tidak
/// bisa menurunkan atau mengeluarkan Owner maupun pemegang izin yang tidak ia punya.
#[must_use]
pub fn can_assign(actor: &Model, role: &Model) -> bool {
    if role.is_owner() {
        return actor.is_owner();
    }
    role.permissions
        .iter()
        .all(|p| actor.permissions.contains(p))
}

/// Isian izin dari form menjadi [`Permission`]. Nilai tak dikenal ditolak (bukan diabaikan),
/// karena form yang sah hanya mengirim nilai dari daftar checkbox.
///
/// # Errors
///
/// [`RoleError::Invalid`] dengan [`ROLE_PERMISSION_UNKNOWN`] di field `permissions`.
pub fn parse_permissions(raw: &[String]) -> Result<Vec<Permission>, RoleError> {
    raw.iter()
        .map(Permission::try_from_value)
        .collect::<Result<_, _>>()
        .map_err(|_| field_error("permissions", ROLE_PERMISSION_UNKNOWN))
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

    /// Seperti [`Self::find_in`], dengan baris dikunci (`FOR UPDATE`) sampai transaksi `txn`
    /// selesai, supaya perubahan dan audit-nya dihitung dari nilai terbaru.
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

    /// Role `id` di workspace ini dengan kunci baca (`FOR SHARE`) sampai transaksi `txn`
    /// selesai: perubahan izin role itu menunggu, tetapi pembaca lain tidak saling menunggu.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn lock_shared_in<C: ConnectionTrait>(
        txn: &C,
        workspace_id: WorkspaceId,
        id: Uuid,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(id)
            .in_tenant(workspace_id)
            .lock_shared()
            .one(txn)
            .await
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

    /// Role custom dengan nama dan izin bebas (minimal satu izin; urutan dan duplikat
    /// dinormalkan ke urutan [`Permission`]).
    ///
    /// # Errors
    ///
    /// [`RoleError::Invalid`] untuk nama tidak valid atau sudah dipakai di workspace ini
    /// (unique index `(workspace_id, lower(name))`, termasuk nama role bawaan) dan izin kosong,
    /// [`RoleError::Db`] untuk error lain.
    pub async fn create_custom<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        name: &str,
        permissions: Vec<Permission>,
    ) -> Result<Model, RoleError> {
        let (name, permissions) = validated(name, permissions)?;
        Self {
            workspace_id: Set(workspace_id.get()),
            name: Set(name),
            permissions: Set(permissions),
            ..Default::default()
        }
        .insert(db)
        .await
        .map_err(name_taken_or_db)
    }

    /// Ganti nama dan izin role custom. Validasi sama dengan [`Self::create_custom`].
    ///
    /// # Errors
    ///
    /// [`RoleError::Builtin`] untuk role bawaan ([`ensure_role_editable`]), lalu seperti
    /// [`Self::create_custom`].
    pub async fn update_custom<C: ConnectionTrait>(
        db: &C,
        role: Model,
        name: &str,
        permissions: Vec<Permission>,
    ) -> Result<Model, RoleError> {
        ensure_role_editable(&role)?;
        let (name, permissions) = validated(name, permissions)?;
        let mut active = role.into_active_model();
        active.name = Set(name);
        active.permissions = Set(permissions);
        active.update(db).await.map_err(name_taken_or_db)
    }

    /// Hapus role custom. Role yang masih dipegang member atau undangan ditolak oleh FK
    /// `role_id` (NO ACTION) di database, bukan oleh SELECT lebih dulu, jadi tidak ada celah
    /// race. Undangan yang sudah selesai dihapus pemanggil lebih dulu
    /// (`services::workspaces::roles::delete`), jadi hanya undangan aktif yang menahan. Grant
    /// project (`project_grants.role_id`, FK tanpa ON DELETE) ikut menahan lewat jalur yang sama.
    ///
    /// # Errors
    ///
    /// [`RoleError::Builtin`] untuk role bawaan, [`RoleError::InUse`] bila masih dipakai,
    /// [`RoleError::Db`] untuk error lain. Pelanggaran FK membatalkan transaksi `db`.
    pub async fn delete_custom<C: ConnectionTrait>(db: &C, role: Model) -> Result<(), RoleError> {
        ensure_role_editable(&role)?;
        role.delete(db).await.map_err(|e| {
            if matches!(e.sql_err(), Some(SqlErr::ForeignKeyConstraintViolation(_))) {
                RoleError::InUse
            } else {
                RoleError::Db(e)
            }
        })?;
        Ok(())
    }
}

/// Nama ternormalisasi dan izin kanonis (urut [`Permission`], tanpa duplikat), atau error
/// semua field yang tidak valid sekaligus.
fn validated(name: &str, permissions: Vec<Permission>) -> Result<(String, Vec<String>), RoleError> {
    let name = text::normalize_name(name);
    let mut errors = ValidationErrors::new();
    if let Err(e) = text::validate_name("role", &name) {
        errors.add("name", e);
    }
    if permissions.is_empty() {
        errors.add("permissions", ValidationError::new(ROLE_PERMISSIONS_EMPTY));
    }
    if !errors.is_empty() {
        return Err(RoleError::Invalid(errors.into()));
    }
    let canonical: Vec<Permission> = Permission::iter()
        .filter(|p| permissions.contains(p))
        .collect();
    Ok((name, to_strings(&canonical)))
}

fn name_taken_or_db(e: DbErr) -> RoleError {
    if matches!(e.sql_err(), Some(SqlErr::UniqueConstraintViolation(_))) {
        field_error("name", ROLE_NAME_TAKEN)
    } else {
        RoleError::Db(e)
    }
}

fn field_error(field: &'static str, code: &'static str) -> RoleError {
    let mut errors = ValidationErrors::new();
    errors.add(field, ValidationError::new(code));
    RoleError::Invalid(errors.into())
}

fn to_strings(permissions: &[Permission]) -> Vec<String> {
    permissions.iter().map(|p| p.to_value()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn role_holding(permissions: &[Permission]) -> Model {
        let now = chrono::Utc::now().into();
        Model {
            id: Uuid::now_v7(),
            workspace_id: Uuid::now_v7(),
            key: None,
            name: "x".to_string(),
            permissions: to_strings(permissions),
            builtin: false,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn only_held_permissions_can_be_granted_or_revoked() {
        use Permission::{ManageRoles, ManageWorkspace, ViewAuditLog, ViewProject};
        let actor = role_holding(&[ManageRoles, ViewProject]);
        let check = |before: &[Permission], after: &[Permission]| {
            ensure_permissions_grantable(&actor, before, after).is_ok()
        };
        assert!(check(&[], &[ViewProject, ManageRoles]));
        assert!(check(&[ViewAuditLog, ViewProject], &[ViewAuditLog]));
        assert!(!check(&[], &[ManageWorkspace]));
        assert!(!check(&[ViewAuditLog, ViewProject], &[ViewProject]));
        let owner = role_holding(&Permission::iter().collect::<Vec<_>>());
        assert!(ensure_permissions_grantable(&owner, &[ViewAuditLog], &[ManageWorkspace]).is_ok());
    }

    #[test]
    fn only_owner_assigns_owner_and_others_assign_held_permissions() {
        use Permission::{ManageMembers, ManageWorkspace, ViewProject};
        let all: Vec<Permission> = Permission::iter().collect();
        let mut owner = role_holding(&all);
        owner.key = Some(RoleKey::Owner.to_value());
        // Role custom berizin lengkap tetap bukan Owner.
        let full_custom = role_holding(&all);
        let admin = role_holding(&[ManageMembers, ViewProject]);
        let viewer = role_holding(&[ViewProject]);

        assert!(can_assign(&owner, &owner));
        assert!(!can_assign(&full_custom, &owner));
        assert!(can_assign(&full_custom, &full_custom));
        assert!(can_assign(&admin, &viewer));
        assert!(can_assign(&admin, &admin));
        assert!(!can_assign(&admin, &role_holding(&[ManageWorkspace])));
        assert!(!can_assign(&viewer, &admin));
    }
}
