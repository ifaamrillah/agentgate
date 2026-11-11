//! Satu-satunya jalur menulis audit log (rules "Audit log"): dipanggil dengan transaksi yang
//! sama dengan perubahannya, sehingga tidak ada perubahan tanpa jejak dan tidak ada jejak
//! untuk perubahan yang di-rollback. Bukan lewat job.

use sea_orm::{ConnectionTrait, DbErr};
use uuid::Uuid;

pub use crate::models::audit_events::{AuditAction, AuditSubject};
use crate::models::{
    audit_events::{self, NewAuditEvent},
    workspaces::WorkspaceId,
};

/// Satu kejadian. `subject` = (jenis, id) yang diubah, mis. `(AuditSubject::Workspace, id)`. `changes`
/// berisi field yang berubah (`{"name": {"from": …, "to": …}}`) atau data kejadian; tidak
/// pernah berisi rahasia (password, token). `request_id` dari header `x-request-id`
/// (extractor `RequestId`), untuk mencocokkan baris dengan log request.
#[derive(Debug)]
pub struct AuditEntry {
    pub workspace_id: WorkspaceId,
    pub actor_id: Option<Uuid>,
    pub action: AuditAction,
    pub subject: (AuditSubject, Uuid),
    pub changes: serde_json::Value,
    pub request_id: Option<String>,
}

/// Tulis `entry` lewat `txn` (transaksi milik perubahan yang dicatat).
///
/// # Errors
///
/// Error database; pemanggil membatalkan transaksinya.
pub async fn record<C: ConnectionTrait>(txn: &C, entry: AuditEntry) -> Result<(), DbErr> {
    let (subject_type, subject_id) = entry.subject;
    audit_events::ActiveModel::append(
        txn,
        NewAuditEvent {
            workspace_id: entry.workspace_id,
            actor_id: entry.actor_id,
            action: entry.action,
            subject_type,
            subject_id,
            changes: entry.changes,
            request_id: entry.request_id,
        },
    )
    .await?;
    Ok(())
}

/// `{"from": lama, "to": baru}` untuk satu field yang berubah, atau `None` bila sama.
#[must_use]
pub fn change<T: serde::Serialize + PartialEq>(from: &T, to: &T) -> Option<serde_json::Value> {
    (from != to).then(|| serde_json::json!({ "from": from, "to": to }))
}
