//! Hapus workspace dengan masa tenggang (spec "Keamanan, privasi & akun"): Owner mengetik slug
//! untuk konfirmasi, workspace ditandai `deletion_scheduled_at = now()`, lalu job pembersihan
//! (`services::cleanup`) menghapusnya permanen setelah `DELETION_GRACE_DAYS` hari. Selama
//! tenggang, workspace tersembunyi dari semua anggota kecuali Owner (`is_open_to`), dan Owner
//! bisa membatalkan.
//!
//! Hanya role bawaan Owner, bukan sekadar `ManageWorkspace`: spec menyebut Owner, dan role
//! custom yang diberi `ManageWorkspace` tidak boleh bisa menghapus seluruh workspace. Role
//! pelaku dibaca ulang setelah baris workspace dikunci. Menjadwalkan yang sudah terjadwal atau
//! membatalkan yang tidak terjadwal tidak menulis apa pun (tenggat tidak diperpanjang).

use sea_orm::{DatabaseConnection, DbErr, TransactionTrait};

use super::{Acting, lock_as};
use crate::{
    models::{roles, workspaces},
    services::audit::{self, AuditAction, AuditSubject},
};

/// Slug yang diketik tidak sama dengan slug workspace (field `confirm_slug`).
pub const DELETE_CONFIRM_MISMATCH: &str = "workspace-delete-confirm-mismatch";
/// Kode error milik service ini (dijaga tes i18n).
pub const ERROR_CODES: &[&str] = &[DELETE_CONFIRM_MISMATCH];

#[derive(Debug, thiserror::Error)]
pub enum DeletionError {
    /// Pelaku sudah bukan anggota, bukan Owner, atau workspace sudah dihapus permanen.
    #[error("not found")]
    NotFound,
    #[error("confirmation does not match the slug")]
    ConfirmMismatch,
    #[error(transparent)]
    Db(#[from] DbErr),
}

/// Jadwalkan hapus bila `confirm_slug` (spasi di tepi diabaikan) sama persis dengan slug
/// workspace. Audit `workspace.deletion_scheduled`.
///
/// # Errors
///
/// [`DeletionError::ConfirmMismatch`], [`DeletionError::NotFound`], atau error database.
pub async fn schedule(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    confirm_slug: &str,
) -> Result<workspaces::Model, DeletionError> {
    if confirm_slug.trim() != acting.workspace.slug {
        return Err(DeletionError::ConfirmMismatch);
    }
    set(db, acting, true).await
}

/// Batalkan jadwal hapus. Audit `workspace.deletion_cancelled`.
///
/// # Errors
///
/// [`DeletionError::NotFound`] atau error database.
pub async fn cancel(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
) -> Result<workspaces::Model, DeletionError> {
    set(db, acting, false).await
}

async fn set(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    scheduled: bool,
) -> Result<workspaces::Model, DeletionError> {
    let txn = db.begin().await?;
    let workspace = lock_as(&txn, acting, roles::Model::is_owner)
        .await?
        .ok_or(DeletionError::NotFound)?;
    if workspace.deletion_scheduled_at.is_some() == scheduled {
        txn.rollback().await?;
        return Ok(workspace);
    }
    let workspace = workspace.set_deletion_scheduled(&txn, scheduled).await?;
    let action = if scheduled {
        AuditAction::WorkspaceDeletionScheduled
    } else {
        AuditAction::WorkspaceDeletionCancelled
    };
    // Tanggal hapus permanen tidak dicatat: selalu `created_at` baris ini + masa tenggang.
    audit::record(
        &txn,
        acting.actor().audit(
            action,
            (AuditSubject::Workspace, workspace.id),
            serde_json::json!({ "slug": workspace.slug }),
        ),
    )
    .await?;
    txn.commit().await?;
    Ok(workspace)
}
