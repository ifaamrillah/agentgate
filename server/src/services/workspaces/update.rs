//! Mengubah nama dan slug workspace (Pengaturan, `ManageWorkspace`) beserta audit
//! `workspace.updated` di transaksi yang sama. Tanpa perubahan: tidak ada tulis dan tidak ada
//! audit. Baris workspace dikunci dulu (`lock_as`), lalu role pelaku dibaca ulang, supaya
//! `changes` dihitung dari nilai terbaru dan pelaku yang kehilangan `ManageWorkspace` di tengah
//! jalan ditolak.

use loco_rs::app::AppContext;
use sea_orm::TransactionTrait;

use super::{Acting, lock_as};
use crate::{
    models::{
        permission::Permission,
        workspaces::{self, WorkspaceError},
    },
    services::audit::{self, AuditAction, AuditSubject},
};

/// Isian form Pengaturan.
#[derive(Debug)]
pub struct WorkspaceDetails {
    pub name: String,
    pub slug: String,
}

/// # Errors
///
/// [`WorkspaceError::Invalid`] untuk nama/slug tidak valid atau slug terpakai,
/// [`WorkspaceError::NotFound`] bila workspace sudah dihapus permanen atau role terbaru pelaku
/// tidak memegang `ManageWorkspace`, [`WorkspaceError::Db`] untuk error database lain.
pub async fn run(
    ctx: &AppContext,
    acting: &Acting<'_>,
    details: WorkspaceDetails,
) -> Result<workspaces::Model, WorkspaceError> {
    let txn = ctx.db.begin().await?;
    let current = lock_as(&txn, acting, |r| r.has(Permission::ManageWorkspace))
        .await?
        .ok_or(WorkspaceError::NotFound)?;
    let (old_name, old_slug) = (current.name.clone(), current.slug.clone());
    let updated = current
        .update_details(&txn, &details.name, &details.slug)
        .await?;
    let mut changes = serde_json::Map::new();
    if let Some(c) = audit::change(&old_name, &updated.name) {
        changes.insert("name".to_string(), c);
    }
    if let Some(c) = audit::change(&old_slug, &updated.slug) {
        changes.insert("slug".to_string(), c);
    }
    if changes.is_empty() {
        // Tidak ada yang berubah: `update` tanpa kolom berubah tidak menulis apa pun.
        txn.rollback().await?;
        return Ok(updated);
    }
    audit::record(
        &txn,
        acting.actor().audit(
            AuditAction::WorkspaceUpdated,
            (AuditSubject::Workspace, updated.id),
            serde_json::Value::Object(changes),
        ),
    )
    .await?;
    txn.commit().await?;
    Ok(updated)
}
