//! Izin di dalam workspace dan isi role bawaan.
//!
//! Role adalah himpunan [`Permission`] per workspace (`roles.permissions`, `text[]`). Setiap
//! elemen disimpan sebagai string `snake_case` dan dijaga CHECK `permissions <@ ARRAY[...]` di
//! migrasi (rules §4 untuk array): menambah varian berarti satu migrasi yang mengganti CHECK
//! itu, di commit yang sama (dijaga `tests/models/permission.rs`).
//!
//! Izin workspace (`Manage*`, `CreateProjects`, `ViewAuditLog`) dicek lewat role keanggotaan
//! workspace (`Require<P>`). Izin project (`AdministerProject`, `EditProject`, `ViewProject`)
//! berlaku per project lewat grant (`project_access`): role workspace non-admin tidak memberi
//! akses project dengan sendirinya, dan Owner/Admin workspace otomatis Admin di semua project.

use sea_orm::entity::prelude::*;

use super::roles::RoleKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum Permission {
    /// Nama, slug, 2FA wajib, hapus, dan transfer workspace.
    #[sea_orm(string_value = "manage_workspace")]
    ManageWorkspace,
    #[sea_orm(string_value = "manage_members")]
    ManageMembers,
    #[sea_orm(string_value = "manage_roles")]
    ManageRoles,
    #[sea_orm(string_value = "manage_teams")]
    ManageTeams,
    #[sea_orm(string_value = "create_projects")]
    CreateProjects,
    #[sea_orm(string_value = "view_audit_log")]
    ViewAuditLog,
    #[sea_orm(string_value = "administer_project")]
    AdministerProject,
    /// Mengubah isi project (tiket di M2).
    #[sea_orm(string_value = "edit_project")]
    EditProject,
    #[sea_orm(string_value = "view_project")]
    ViewProject,
}

impl std::fmt::Display for Permission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_value())
    }
}

/// Izin yang punya halaman di Pengaturan workspace: sidebar menampilkan Pengaturan bila role
/// memegang salah satunya.
pub const SETTINGS_PERMISSIONS: &[Permission] = &[
    Permission::ManageWorkspace,
    Permission::ManageMembers,
    Permission::ManageRoles,
    Permission::ManageTeams,
];

/// Satu role bawaan: key-nya dan izinnya.
#[derive(Debug, Clone, Copy)]
pub struct BuiltinRole {
    pub key: RoleKey,
    pub permissions: &'static [Permission],
}

/// Role bawaan yang dibuat bersama setiap workspace (`services::workspaces::create`).
pub const BUILTIN_ROLES: &[BuiltinRole] = &[
    BuiltinRole {
        key: RoleKey::Owner,
        permissions: &[
            Permission::ManageWorkspace,
            Permission::ManageMembers,
            Permission::ManageRoles,
            Permission::ManageTeams,
            Permission::CreateProjects,
            Permission::ViewAuditLog,
            Permission::AdministerProject,
            Permission::EditProject,
            Permission::ViewProject,
        ],
    },
    // Semua kecuali ManageWorkspace (hapus, 2FA wajib, transfer tetap milik Owner).
    BuiltinRole {
        key: RoleKey::Admin,
        permissions: &[
            Permission::ManageMembers,
            Permission::ManageRoles,
            Permission::ManageTeams,
            Permission::CreateProjects,
            Permission::ViewAuditLog,
            Permission::AdministerProject,
            Permission::EditProject,
            Permission::ViewProject,
        ],
    },
    BuiltinRole {
        key: RoleKey::Member,
        permissions: &[
            Permission::ViewProject,
            Permission::EditProject,
            Permission::CreateProjects,
        ],
    },
    BuiltinRole {
        key: RoleKey::Viewer,
        permissions: &[Permission::ViewProject],
    },
];

#[cfg(test)]
mod tests {
    use sea_orm::Iterable;

    use super::*;

    #[test]
    fn owner_has_every_permission_and_every_key_has_one_builtin() {
        let owner = BUILTIN_ROLES
            .iter()
            .find(|r| r.key == RoleKey::Owner)
            .unwrap();
        assert_eq!(owner.permissions.len(), Permission::iter().count());
        for key in RoleKey::iter() {
            assert_eq!(BUILTIN_ROLES.iter().filter(|r| r.key == key).count(), 1);
        }
    }
}
