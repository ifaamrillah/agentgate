//! Audit log append-only: siapa mengubah apa di workspace, kapan. Baris hanya ditambah lewat
//! [`ActiveModel::append`], dipanggil `services::audit::record` di transaksi yang sama dengan
//! perubahannya; model ini sengaja tidak punya jalur update atau delete (rules "Audit log").
//! Halaman audit (Task 13) membaca lewat [`Model::newest_first`] dan keyset di atasnya.

use loco_rs::model::{TenantEntity, TenantQueryExt};
use sea_orm::{ActiveValue::Set, QueryOrder, QuerySelect, entity::prelude::*};

pub use super::_entities::audit_events::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

/// Jenis kejadian. Disimpan sebagai string `<subjek>.<aksi>` dengan CHECK constraint (rules §4,
/// dijaga `tests/models/enums.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum AuditAction {
    #[sea_orm(string_value = "workspace.created")]
    WorkspaceCreated,
    #[sea_orm(string_value = "workspace.updated")]
    WorkspaceUpdated,
    #[sea_orm(string_value = "workspace.require_2fa_changed")]
    WorkspaceRequire2faChanged,
    #[sea_orm(string_value = "workspace.deletion_scheduled")]
    WorkspaceDeletionScheduled,
    #[sea_orm(string_value = "workspace.deletion_cancelled")]
    WorkspaceDeletionCancelled,
    #[sea_orm(string_value = "member.invited")]
    MemberInvited,
    #[sea_orm(string_value = "member.joined")]
    MemberJoined,
    #[sea_orm(string_value = "member.role_changed")]
    MemberRoleChanged,
    #[sea_orm(string_value = "member.removed")]
    MemberRemoved,
    #[sea_orm(string_value = "member.left")]
    MemberLeft,
    #[sea_orm(string_value = "member.job_title_changed")]
    MemberJobTitleChanged,
    /// Undangan dibatalkan sebelum diterima (Pengaturan → Anggota).
    #[sea_orm(string_value = "member.invitation_revoked")]
    MemberInvitationRevoked,
    #[sea_orm(string_value = "role.created")]
    RoleCreated,
    #[sea_orm(string_value = "role.updated")]
    RoleUpdated,
    #[sea_orm(string_value = "role.deleted")]
    RoleDeleted,
    #[sea_orm(string_value = "job_title.created")]
    JobTitleCreated,
    #[sea_orm(string_value = "job_title.updated")]
    JobTitleUpdated,
    #[sea_orm(string_value = "job_title.deleted")]
    JobTitleDeleted,
    #[sea_orm(string_value = "team.created")]
    TeamCreated,
    #[sea_orm(string_value = "team.updated")]
    TeamUpdated,
    #[sea_orm(string_value = "team.deleted")]
    TeamDeleted,
    #[sea_orm(string_value = "team.member_added")]
    TeamMemberAdded,
    #[sea_orm(string_value = "team.member_removed")]
    TeamMemberRemoved,
    #[sea_orm(string_value = "project.created")]
    ProjectCreated,
    #[sea_orm(string_value = "project.updated")]
    ProjectUpdated,
    #[sea_orm(string_value = "project.archived")]
    ProjectArchived,
    #[sea_orm(string_value = "project.access_granted")]
    ProjectAccessGranted,
    #[sea_orm(string_value = "project.access_changed")]
    ProjectAccessChanged,
    #[sea_orm(string_value = "project.access_revoked")]
    ProjectAccessRevoked,
    #[sea_orm(string_value = "repo.added")]
    RepoAdded,
    #[sea_orm(string_value = "repo.updated")]
    RepoUpdated,
    #[sea_orm(string_value = "repo.removed")]
    RepoRemoved,
}

/// Jenis subjek yang diubah (`subject_type`). Disimpan sebagai string dengan CHECK constraint
/// (rules §4, dijaga `tests/models/enums.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum AuditSubject {
    #[sea_orm(string_value = "workspace")]
    Workspace,
    #[sea_orm(string_value = "member")]
    Member,
    #[sea_orm(string_value = "role")]
    Role,
    #[sea_orm(string_value = "job_title")]
    JobTitle,
    #[sea_orm(string_value = "team")]
    Team,
    #[sea_orm(string_value = "project")]
    Project,
    #[sea_orm(string_value = "project_grant")]
    ProjectGrant,
    #[sea_orm(string_value = "repo")]
    Repo,
    #[sea_orm(string_value = "invitation")]
    Invitation,
}

/// Data satu baris audit untuk [`ActiveModel::append`].
#[derive(Debug)]
pub struct NewAuditEvent {
    pub workspace_id: WorkspaceId,
    pub actor_id: Option<Uuid>,
    pub action: AuditAction,
    pub subject_type: AuditSubject,
    pub subject_id: Uuid,
    pub changes: serde_json::Value,
    pub request_id: Option<String>,
}

impl TenantEntity for Entity {
    type TenantId = WorkspaceId;

    fn tenant_column() -> Column {
        Column::WorkspaceId
    }
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if !insert {
            // Append-only: tidak ada jalur aplikasi yang boleh mengubah baris audit.
            return Err(DbErr::Custom("audit_events is append-only".to_string()));
        }
        if self.id.is_not_set() {
            self.id = Set(Uuid::now_v7());
        }
        Ok(self)
    }
}

impl Model {
    /// Aksi bertipe dari kolom string. Gagal hanya bila CHECK constraint dilewati.
    ///
    /// # Errors
    ///
    /// Nilai kolom bukan varian [`AuditAction`].
    pub fn action(&self) -> Result<AuditAction, DbErr> {
        AuditAction::try_from_value(&self.action)
    }

    /// Jenis subjek bertipe dari kolom string. Gagal hanya bila CHECK constraint dilewati.
    ///
    /// # Errors
    ///
    /// Nilai kolom bukan varian [`AuditSubject`].
    pub fn subject(&self) -> Result<AuditSubject, DbErr> {
        AuditSubject::try_from_value(&self.subject_type)
    }

    /// `limit` kejadian terbaru di workspace ini (total order `created_at`, lalu `id`).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn newest_first<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        limit: u64,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .in_tenant(workspace_id)
            .order_by_desc(Column::CreatedAt)
            .order_by_desc(Column::Id)
            .limit(limit)
            .all(db)
            .await
    }
}

impl ActiveModel {
    /// Tambahkan satu baris audit.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn append<C: ConnectionTrait>(db: &C, new: NewAuditEvent) -> Result<Model, DbErr> {
        Self {
            workspace_id: Set(new.workspace_id.get()),
            actor_id: Set(new.actor_id),
            action: Set(new.action.to_value()),
            subject_type: Set(new.subject_type.to_value()),
            subject_id: Set(new.subject_id),
            changes: Set(new.changes),
            request_id: Set(new.request_id),
            ..Default::default()
        }
        .insert(db)
        .await
    }
}
