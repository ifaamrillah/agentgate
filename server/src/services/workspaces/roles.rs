//! Role custom (Pengaturan → Role, `ManageRoles`): buat, ubah, hapus, masing-masing dengan audit
//! `role.*` di transaksi yang sama. Buat dan ubah hanya boleh menambah atau mencabut izin yang
//! dipegang role pelaku (`roles::ensure_permissions_grantable`). Aturan role (bawaan tidak bisa diubah, nama unik, masih
//! dipakai) ada di `models::roles`; di sini hanya transaksi dan audit-nya. Ubah tanpa perubahan
//! tidak menulis apa pun, seperti Pengaturan workspace.

use sea_orm::{DbErr, TransactionTrait};
use uuid::Uuid;

use super::Actor;
use crate::{
    models::{
        invitations,
        permission::Permission,
        roles::{self, RoleError},
    },
    services::audit::{self, AuditAction, AuditSubject},
};

/// # Errors
///
/// `role-permission-not-held` bila `actor_role` tidak memegang salah satu izin, lalu seperti
/// `roles::ActiveModel::create_custom`.
pub async fn create(
    db: &sea_orm::DatabaseConnection,
    actor: &Actor,
    actor_role: &roles::Model,
    name: &str,
    permissions: Vec<Permission>,
) -> Result<roles::Model, RoleError> {
    roles::ensure_permissions_grantable(actor_role, &[], &permissions)?;
    let txn = db.begin().await?;
    let role =
        roles::ActiveModel::create_custom(&txn, actor.workspace_id, name, permissions).await?;
    audit::record(
        &txn,
        actor.audit(
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
/// `DbErr::RecordNotFound` bila role `id` sudah tidak ada di workspace ini,
/// `role-permission-not-held` bila izin yang berubah tidak dipegang `actor_role`, lalu seperti
/// `roles::ActiveModel::update_custom`.
pub async fn update(
    db: &sea_orm::DatabaseConnection,
    actor: &Actor,
    actor_role: &roles::Model,
    id: Uuid,
    name: &str,
    permissions: Vec<Permission>,
) -> Result<roles::Model, RoleError> {
    let txn = db.begin().await?;
    let current = locked(&txn, actor, id).await?;
    roles::ensure_permissions_grantable(actor_role, &current.permissions()?, &permissions)?;
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
        actor.audit(
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
/// `DbErr::RecordNotFound` bila role `id` sudah tidak ada di workspace ini, lalu seperti
/// `roles::ActiveModel::delete_custom` (`InUse`, `Builtin`). Undangan yang sudah selesai
/// dihapus lebih dulu di transaksi yang sama, jadi hanya member dan undangan aktif yang
/// menahan role; bila penghapusan gagal, undangan itu ikut kembali (rollback).
pub async fn delete(
    db: &sea_orm::DatabaseConnection,
    actor: &Actor,
    id: Uuid,
) -> Result<(), RoleError> {
    let txn = db.begin().await?;
    let current = locked(&txn, actor, id).await?;
    let changes = serde_json::json!({ "name": current.name, "permissions": current.permissions });
    roles::ensure_role_editable(&current)?;
    invitations::Model::delete_finished_for_role(&txn, actor.workspace_id, id).await?;
    roles::ActiveModel::delete_custom(&txn, current).await?;
    audit::record(
        &txn,
        actor.audit(AuditAction::RoleDeleted, (AuditSubject::Role, id), changes),
    )
    .await?;
    txn.commit().await?;
    Ok(())
}

async fn locked(
    txn: &sea_orm::DatabaseTransaction,
    actor: &Actor,
    id: Uuid,
) -> Result<roles::Model, RoleError> {
    roles::Model::lock_in(txn, actor.workspace_id, id)
        .await?
        .ok_or_else(|| RoleError::Db(DbErr::RecordNotFound("role".to_string())))
}
