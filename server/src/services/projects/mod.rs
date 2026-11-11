//! Project: buat (pembuat langsung menjadi Admin project lewat grant), ubah nama/deskripsi, dan
//! arsipkan; akses per project di [`access`], repo di [`repos`]. Setiap perubahan menulis audit
//! `project.*`/`repo.*` di transaksi yang sama.
//!
//! **Izin pelaku dibaca ulang di dalam transaksi** ([`Administering::begin`]), bukan dari awal
//! request: role workspace dan grant pelaku dimuat lagi setelah baris project dikunci, sehingga
//! pelaku yang aksesnya dicabut di tengah jalan mendapat `NotFound` (sama dengan extractor
//! `ProjectAdmin`), dan perubahan akses pada satu project berjalan berurutan.

pub mod access;
pub mod repos;

use loco_rs::validation::ModelValidationErrors;
use sea_orm::{DatabaseConnection, DatabaseTransaction, DbErr, TransactionTrait};
use uuid::Uuid;

use crate::{
    models::{
        permission::Permission,
        project_access::{self, Accessor, EffectiveRole},
        project_grants::{self, GrantSubject, PROJECT_GRANT_EXISTS},
        projects::{self, NewProject, ProjectError},
        repos::RepoError,
        roles::{self, MEMBER_ROLE_NOT_ASSIGNABLE, RoleKey},
        workspace_members,
    },
    services::{
        audit::{self, AuditAction, AuditSubject},
        workspaces::Acting,
    },
};

#[derive(Debug, thiserror::Error)]
pub enum ProjectFailure {
    /// Project, grant, repo, role, tim, atau orang tidak ada di workspace/project ini, atau
    /// pelaku tidak (lagi) memegang izin yang dibutuhkan.
    #[error("not found")]
    NotFound,
    /// Pelaku tidak boleh memberikan atau mencabut role itu (`member-role-not-assignable`).
    #[error("role not grantable by actor")]
    NotAssignable,
    /// Subjek sudah punya grant di project ini (`project-grant-exists`).
    #[error("grant already exists")]
    GrantExists,
    /// Validasi field gagal.
    #[error("invalid: {0}")]
    Invalid(ModelValidationErrors),
    #[error(transparent)]
    Db(#[from] DbErr),
}

impl ProjectFailure {
    /// Kode Fluent untuk penolakan aturan yang ditampilkan sebagai pesan halaman (bukan error
    /// field).
    #[must_use]
    pub fn code(&self) -> Option<&'static str> {
        match self {
            Self::NotAssignable => Some(MEMBER_ROLE_NOT_ASSIGNABLE),
            Self::GrantExists => Some(PROJECT_GRANT_EXISTS),
            Self::NotFound | Self::Invalid(_) | Self::Db(_) => None,
        }
    }
}

impl From<ProjectError> for ProjectFailure {
    fn from(err: ProjectError) -> Self {
        match err {
            ProjectError::Invalid(errors) => Self::Invalid(errors),
            ProjectError::Db(e) => Self::Db(e),
        }
    }
}

impl From<RepoError> for ProjectFailure {
    fn from(err: RepoError) -> Self {
        match err {
            RepoError::Invalid(errors) => Self::Invalid(errors),
            RepoError::Db(e) => Self::Db(e),
        }
    }
}

impl From<ProjectFailure> for loco_rs::Error {
    fn from(err: ProjectFailure) -> Self {
        match err {
            ProjectFailure::NotFound => Self::NotFound,
            ProjectFailure::Invalid(errors) => Self::Validation(errors),
            ProjectFailure::NotAssignable | ProjectFailure::GrantExists => {
                Self::BadRequest(err.to_string())
            }
            ProjectFailure::Db(e) => Self::DB(e),
        }
    }
}

/// Isian form project baru.
#[derive(Debug, Clone)]
pub struct ProjectInput {
    pub key: String,
    pub name: String,
    pub description: String,
}

/// Buat project. Pelaku harus (masih) memegang `CreateProjects`; ia mendapat grant langsung
/// dengan role bawaan Admin, supaya pembuat yang bukan Owner/Admin workspace bisa mengatur
/// project-nya.
///
/// # Errors
///
/// [`ProjectFailure::NotFound`] bila pelaku tidak lagi memegang `CreateProjects`,
/// [`ProjectFailure::Invalid`] (termasuk `project-key-taken`), atau error database.
pub async fn create(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    input: ProjectInput,
) -> Result<projects::Model, ProjectFailure> {
    let wid = acting.workspace_id();
    let txn = db.begin().await?;
    let (_, role) = workspace_members::Model::find_with_role(&txn, wid, acting.user.id)
        .await?
        .ok_or(ProjectFailure::NotFound)?;
    if !role.has(Permission::CreateProjects) {
        return Err(ProjectFailure::NotFound);
    }
    let project = projects::ActiveModel::create(
        &txn,
        wid,
        NewProject {
            key: input.key,
            name: input.name,
            description: input.description,
            created_by: Some(acting.user.id),
        },
    )
    .await?;
    audit::record(
        &txn,
        acting.actor().audit(
            AuditAction::ProjectCreated,
            (AuditSubject::Project, project.id),
            serde_json::json!({ "key": project.key, "name": project.name }),
        ),
    )
    .await?;
    let admin = roles::Model::find_builtin(&txn, wid, RoleKey::Admin)
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("builtin admin role".to_string()))?;
    let grant = project_grants::ActiveModel::grant(
        &txn,
        wid,
        project.id,
        GrantSubject::User(acting.user.id),
        admin.id,
    )
    .await?;
    audit::record(
        &txn,
        acting.actor().audit(
            AuditAction::ProjectAccessGranted,
            (AuditSubject::ProjectGrant, grant.id),
            grant_changes(&grant, &admin.name),
        ),
    )
    .await?;
    txn.commit().await?;
    Ok(project)
}

/// Ganti nama dan deskripsi project. Tanpa perubahan: tidak menulis apa pun.
///
/// # Errors
///
/// [`ProjectFailure::NotFound`] (project atau izin), [`ProjectFailure::Invalid`], atau error
/// database.
pub async fn update(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    project_id: Uuid,
    name: &str,
    description: &str,
) -> Result<projects::Model, ProjectFailure> {
    let admin = Administering::begin(db, acting, project_id).await?;
    let before = admin.project.clone();
    let after = admin
        .project
        .update_details(&admin.txn, name, description)
        .await?;
    let mut changes = serde_json::Map::new();
    if let Some(c) = audit::change(&before.name, &after.name) {
        changes.insert("name".to_string(), c);
    }
    if let Some(c) = audit::change(&before.description, &after.description) {
        changes.insert("description".to_string(), c);
    }
    if changes.is_empty() {
        admin.txn.rollback().await?;
        return Ok(before);
    }
    audit::record(
        &admin.txn,
        acting.actor().audit(
            AuditAction::ProjectUpdated,
            (AuditSubject::Project, after.id),
            serde_json::Value::Object(changes),
        ),
    )
    .await?;
    admin.txn.commit().await?;
    Ok(after)
}

/// Arsipkan project. Project yang sudah diarsipkan tidak diubah dan tidak diaudit lagi.
///
/// # Errors
///
/// [`ProjectFailure::NotFound`] (project atau izin), atau error database.
pub async fn archive(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    project_id: Uuid,
) -> Result<projects::Model, ProjectFailure> {
    let admin = Administering::begin(db, acting, project_id).await?;
    if admin.project.is_archived() {
        admin.txn.rollback().await?;
        return Ok(admin.project);
    }
    let project = admin.project.archive(&admin.txn).await?;
    audit::record(
        &admin.txn,
        acting.actor().audit(
            AuditAction::ProjectArchived,
            (AuditSubject::Project, project.id),
            serde_json::json!({ "key": project.key }),
        ),
    )
    .await?;
    admin.txn.commit().await?;
    Ok(project)
}

/// Data audit satu grant: project, subjek, dan nama role.
fn grant_changes(grant: &project_grants::Model, role_name: &str) -> serde_json::Value {
    serde_json::json!({
        "project_id": grant.project_id,
        "team_id": grant.team_id,
        "user_id": grant.user_id,
        "role": role_name,
    })
}

/// Transaksi dengan baris project terkunci, beserta role workspace dan akses project pelaku
/// yang dibaca setelah kunci. Ada hanya bila pelaku memegang `AdministerProject`.
pub(crate) struct Administering {
    txn: DatabaseTransaction,
    project: projects::Model,
    actor_role: roles::Model,
    access: EffectiveRole,
}

impl Administering {
    /// `NotFound` bila project tidak ada di workspace pelaku, pelaku bukan lagi anggota, atau
    /// akses project terbarunya tidak memegang `AdministerProject`.
    async fn begin(
        db: &DatabaseConnection,
        acting: &Acting<'_>,
        project_id: Uuid,
    ) -> Result<Self, ProjectFailure> {
        let wid = acting.workspace_id();
        let txn = db.begin().await?;
        let project = projects::Model::lock_in(&txn, wid, project_id)
            .await?
            .ok_or(ProjectFailure::NotFound)?;
        let (_, actor_role) = workspace_members::Model::find_with_role(&txn, wid, acting.user.id)
            .await?
            .ok_or(ProjectFailure::NotFound)?;
        let who = Accessor {
            workspace_id: wid,
            user_id: acting.user.id,
            role: &actor_role,
        };
        let access = project_access::effective_role(&txn, &who, project.id)
            .await?
            .filter(|access| access.has(Permission::AdministerProject))
            .ok_or(ProjectFailure::NotFound)?;
        Ok(Self {
            txn,
            project,
            actor_role,
            access,
        })
    }

    /// Satu-satunya aturan pemberian role grant (`project_access::can_grant`).
    fn ensure_grantable(&self, role: &roles::Model) -> Result<(), ProjectFailure> {
        if project_access::can_grant(&self.actor_role, &self.access, role) {
            Ok(())
        } else {
            Err(ProjectFailure::NotAssignable)
        }
    }
}
