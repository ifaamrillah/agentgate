//! Role custom (Pengaturan → Role, `ManageRoles`): buat, ubah, hapus, masing-masing dengan audit
//! `role.*` di transaksi yang sama. Buat dan ubah hanya boleh menambah atau mencabut izin yang
//! dipegang role pelaku (`roles::ensure_permissions_grantable`), dan izin role yang dipakai grant
//! project hanya diubah Owner atau Admin bawaan (`roles::ensure_grant_role_editable`). Aturan
//! role (bawaan tidak bisa diubah, nama unik, masih dipakai) ada di `models::roles`; di sini
//! hanya transaksi dan audit-nya. Ubah tanpa perubahan tidak menulis apa pun, seperti
//! Pengaturan workspace.
//!
//! Setiap operasi mengunci baris workspace lalu membaca ulang role pelaku (`lock_with_role`),
//! bukan memakai role dari awal request: pelaku yang tidak lagi memegang `ManageRoles`, role
//! yang sudah dihapus, atau workspace yang sudah dihapus permanen → [`RoleError::NotFound`].

use sea_orm::{DatabaseConnection, DatabaseTransaction, TransactionTrait};
use uuid::Uuid;

use super::{Acting, lock_with_role};
use crate::{
    models::{
        invitations,
        permission::Permission,
        project_grants,
        roles::{self, RoleError},
    },
    services::audit::{self, AuditAction, AuditSubject},
};

/// # Errors
///
/// [`RoleError::NotFound`], `role-permission-not-held` bila role pelaku tidak memegang salah
/// satu izin, lalu seperti `roles::ActiveModel::create_custom`.
pub async fn create(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    name: &str,
    permissions: Vec<Permission>,
) -> Result<roles::Model, RoleError> {
    let txn = db.begin().await?;
    let actor_role = lock_actor(&txn, acting).await?;
    roles::ensure_permissions_grantable(&actor_role, &[], &permissions)?;
    let role =
        roles::ActiveModel::create_custom(&txn, acting.workspace_id(), name, permissions).await?;
    audit::record(
        &txn,
        acting.actor().audit(
            AuditAction::RoleCreated,
            (AuditSubject::Role, role.id),
            serde_json::json!({ "name": role.name, "permissions": role.permissions }),
        ),
    )
    .await?;
    txn.commit().await?;
    Ok(role)
}

/// # Errors
///
/// [`RoleError::NotFound`], `role-permission-not-held` bila izin yang berubah tidak dipegang
/// role pelaku, `role-permissions-granted` bila izin role yang dipakai grant project diubah
/// pelaku yang bukan Owner/Admin bawaan, lalu seperti `roles::ActiveModel::update_custom`.
pub async fn update(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
    name: &str,
    permissions: Vec<Permission>,
) -> Result<roles::Model, RoleError> {
    let txn = db.begin().await?;
    let actor_role = lock_actor(&txn, acting).await?;
    // `FOR UPDATE` pada role juga menahan grant baru yang memakainya (FK `role_id` mengambil
    // `FOR KEY SHARE`), jadi pemeriksaan grant di bawah tidak basi.
    let current = lock_target(&txn, acting, id).await?;
    let before = current.permissions()?;
    roles::ensure_permissions_grantable(&actor_role, &before, &permissions)?;
    let in_grants =
        project_grants::Model::exists_for_role(&txn, acting.workspace_id(), current.id).await?;
    roles::ensure_grant_role_editable(&actor_role, in_grants, &before, &permissions)?;
    let (old_name, old_permissions) = (current.name.clone(), current.permissions.clone());
    let updated = roles::ActiveModel::update_custom(&txn, current, name, permissions).await?;
    let mut changes = serde_json::Map::new();
    if let Some(c) = audit::change(&old_name, &updated.name) {
        changes.insert("name".to_string(), c);
    }
    if let Some(c) = audit::change(&old_permissions, &updated.permissions) {
        changes.insert("permissions".to_string(), c);
    }
    if changes.is_empty() {
        txn.rollback().await?;
        return Ok(updated);
    }
    audit::record(
        &txn,
        acting.actor().audit(
            AuditAction::RoleUpdated,
            (AuditSubject::Role, updated.id),
            serde_json::Value::Object(changes),
        ),
    )
    .await?;
    txn.commit().await?;
    Ok(updated)
}

/// # Errors
///
/// [`RoleError::NotFound`], lalu seperti `roles::ActiveModel::delete_custom` (`InUse`,
/// `Builtin`). Undangan yang sudah selesai dihapus lebih dulu di transaksi yang sama, jadi hanya
/// member dan undangan aktif yang menahan role; bila penghapusan gagal, undangan itu ikut
/// kembali (rollback).
pub async fn delete(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
) -> Result<(), RoleError> {
    let txn = db.begin().await?;
    lock_actor(&txn, acting).await?;
    let current = lock_target(&txn, acting, id).await?;
    let changes = serde_json::json!({ "name": current.name, "permissions": current.permissions });
    roles::ensure_role_editable(&current)?;
    invitations::Model::delete_finished_for_role(&txn, acting.workspace_id(), id).await?;
    roles::ActiveModel::delete_custom(&txn, current).await?;
    audit::record(
        &txn,
        acting
            .actor()
            .audit(AuditAction::RoleDeleted, (AuditSubject::Role, id), changes),
    )
    .await?;
    txn.commit().await?;
    Ok(())
}

/// Role terbaru pelaku setelah kunci workspace, bila masih memegang `ManageRoles`.
async fn lock_actor(
    txn: &DatabaseTransaction,
    acting: &Acting<'_>,
) -> Result<roles::Model, RoleError> {
    lock_with_role(txn, acting, |r| r.has(Permission::ManageRoles))
        .await?
        .map(|(_, role)| role)
        .ok_or(RoleError::NotFound)
}

async fn lock_target(
    txn: &DatabaseTransaction,
    acting: &Acting<'_>,
    id: Uuid,
) -> Result<roles::Model, RoleError> {
    roles::Model::lock_in(txn, acting.workspace_id(), id)
        .await?
        .ok_or(RoleError::NotFound)
}
