//! Mengubah nama dan slug workspace (Pengaturan, `ManageWorkspace`) beserta audit
//! `workspace.updated` di transaksi yang sama. Tanpa perubahan: tidak ada tulis dan tidak ada
//! audit. Baris workspace dikunci dulu supaya `changes` dihitung dari nilai terbaru.

use loco_rs::app::AppContext;
use sea_orm::{EntityTrait, QuerySelect, TransactionTrait};
use uuid::Uuid;

use crate::{
    models::workspaces::{self, WorkspaceError},
    services::audit::{self, AuditAction, AuditEntry, AuditSubject},
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
/// [`WorkspaceError::Db`] untuk error database lain (termasuk workspace sudah terhapus).
pub async fn run(
    ctx: &AppContext,
    workspace_id: workspaces::WorkspaceId,
    actor_id: Uuid,
    details: WorkspaceDetails,
    request_id: Option<String>,
) -> Result<workspaces::Model, WorkspaceError> {
    let txn = ctx.db.begin().await?;
    let current = workspaces::Entity::find_by_id(workspace_id.get())
        .lock_exclusive()
        .one(&txn)
        .await?
        .ok_or_else(|| sea_orm::DbErr::RecordNotFound("workspace".to_string()))?;
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
        AuditEntry {
            workspace_id,
            actor_id: Some(actor_id),
            action: AuditAction::WorkspaceUpdated,
            subject: (AuditSubject::Workspace, updated.id),
            changes: serde_json::Value::Object(changes),
            request_id,
        },
    )
    .await?;
    txn.commit().await?;
    Ok(updated)
}
