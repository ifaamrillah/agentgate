//! Workflow workspace yang menyentuh beberapa tabel sekaligus (workspace, role, member, audit).
pub mod accept_invite;
pub mod create;
pub mod deletion;
pub mod invite;
pub mod job_titles;
pub mod membership;
pub mod require_two_factor;
pub mod roles;
pub mod teams;
pub mod update;

use sea_orm::{ConnectionTrait, DbErr};
use uuid::Uuid;

use crate::{
    models::{
        project_access::Accessor,
        roles as role_model, users, workspace_members,
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
    pub(crate) fn audit(
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

    /// Penanya aturan akses project (`project_access`), dengan role workspace di sini.
    #[must_use]
    pub fn accessor(&self) -> Accessor<'_> {
        Accessor {
            workspace_id: self.workspace_id(),
            user_id: self.user.id,
            role: self.role,
        }
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

/// Kunci baris workspace `acting` di `txn`, lalu baca ulang role pelaku sesudah kunci (role
/// dari awal request bisa sudah basi). Mengembalikan workspace terbaru bila pelaku masih
/// anggota dan role terbarunya memenuhi `allowed`, `None` bila tidak atau workspace sudah tidak
/// ada (pemanggil menjawab 404).
pub(crate) async fn lock_as<C: ConnectionTrait>(
    txn: &C,
    acting: &Acting<'_>,
    allowed: impl Fn(&role_model::Model) -> bool,
) -> Result<Option<workspaces::Model>, DbErr> {
    Ok(lock_with_role(txn, acting, allowed)
        .await?
        .map(|(workspace, _)| workspace))
}

/// Seperti [`lock_as`], juga mengembalikan role terbaru pelaku, untuk aturan yang
/// membandingkan role pelaku dengan yang diubahnya (mis. izin yang boleh diberikan).
pub(crate) async fn lock_with_role<C: ConnectionTrait>(
    txn: &C,
    acting: &Acting<'_>,
    allowed: impl Fn(&role_model::Model) -> bool,
) -> Result<Option<(workspaces::Model, role_model::Model)>, DbErr> {
    let wid = acting.workspace_id();
    // Kalah balapan dengan job pembersihan (workspace dihapus permanen setelah `Tenant`
    // memuatnya): sama dengan workspace yang tidak ada, 404 bukan 500.
    let workspace = match workspaces::Model::lock_for_membership(txn, wid).await {
        Ok(workspace) => workspace,
        Err(DbErr::RecordNotFound(_)) => return Ok(None),
        Err(e) => return Err(e),
    };
    let role = workspace_members::Model::find_with_role(txn, wid, acting.user.id)
        .await?
        .map(|(_, role)| role);
    Ok(role.filter(|r| allowed(r)).map(|role| (workspace, role)))
}
