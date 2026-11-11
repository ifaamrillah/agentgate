//! Workflow workspace yang menyentuh beberapa tabel sekaligus (workspace, role, member, audit).
pub mod accept_invite;
pub mod create;
pub mod invite;
pub mod job_titles;
pub mod membership;
pub mod roles;
pub mod update;

use uuid::Uuid;

use crate::{
    models::{
        roles as role_model, users,
        workspaces::{self, WorkspaceId},
    },
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

/// Anggota yang bertindak di workspace-nya: padanan `Tenant` controller untuk service (service
/// tidak boleh memakai extractor, rules §3). Dipakai alur anggota dan undangan, yang butuh
/// role pelaku (aturan penugasan role) dan nama workspace/pelaku (email undangan).
#[derive(Debug, Clone)]
pub struct Acting<'a> {
    pub workspace: &'a workspaces::Model,
    pub user: &'a users::Model,
    pub role: &'a role_model::Model,
    pub request_id: Option<String>,
}

impl Acting<'_> {
    #[must_use]
    pub fn workspace_id(&self) -> WorkspaceId {
        self.workspace.workspace_id()
    }

    /// Pelaku untuk baris audit.
    #[must_use]
    pub fn actor(&self) -> Actor {
        Actor {
            workspace_id: self.workspace_id(),
            user_id: self.user.id,
            request_id: self.request_id.clone(),
        }
    }
}
