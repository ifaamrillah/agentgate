//! Workflow workspace yang menyentuh beberapa tabel sekaligus (workspace, role, member, audit).
pub mod create;
pub mod job_titles;
pub mod roles;
pub mod update;

use uuid::Uuid;

use crate::{
    models::workspaces::WorkspaceId,
    services::audit::{AuditAction, AuditEntry, AuditSubject},
};

/// Siapa melakukan perubahan di workspace mana, untuk baris audit-nya (Pengaturan role dan
/// jabatan). Dibangun controller dari `Tenant` dan `RequestId`.
#[derive(Debug, Clone)]
pub struct Actor {
    pub workspace_id: WorkspaceId,
    pub user_id: Uuid,
    pub request_id: Option<String>,
}

impl Actor {
    fn audit(
        &self,
        action: AuditAction,
        subject: (AuditSubject, Uuid),
        changes: serde_json::Value,
    ) -> AuditEntry {
        AuditEntry {
            workspace_id: self.workspace_id,
            actor_id: Some(self.user_id),
            action,
            subject,
            changes,
            request_id: self.request_id.clone(),
        }
    }
}
