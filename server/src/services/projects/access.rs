//! Akses per project (Pengaturan project → Akses, `AdministerProject`): beri akses ke tim atau
//! orang, ganti role grant, cabut grant, dengan audit `project.access_*` di transaksi yang sama.
//!
//! Aturan eskalasi (`project_access::can_grant`, lewat [`Administering::ensure_grantable`])
//! berlaku untuk role tujuan dan role asal grant: pelaku tidak bisa memberikan, mengganti, atau
//! mencabut grant ber-role di atas aksesnya sendiri, dan role Owner hanya diatur Owner. Role
//! tujuan juga harus memegang izin project (`grant-role-no-project-access`).

use sea_orm::{ModelTrait, SqlErr};
use uuid::Uuid;

use super::{Administering, ProjectFailure, grant_changes};
use crate::{
    models::{
        project_access,
        project_grants::{self, GrantSubject},
        roles, teams, workspace_members,
        workspaces::WorkspaceId,
    },
    services::{
        audit::{self, AuditAction, AuditSubject},
        workspaces::Acting,
    },
};

/// Beri `subject` (tim atau anggota workspace) akses ke project `project_id` dengan role
/// `role_id`.
///
/// # Errors
///
/// [`ProjectFailure::NotFound`] (project, izin, subjek, atau role di luar workspace),
/// [`ProjectFailure::NotAssignable`], [`ProjectFailure::GrantExists`], atau error database.
pub async fn grant(
    db: &sea_orm::DatabaseConnection,
    acting: &Acting<'_>,
    project_id: Uuid,
    subject: GrantSubject,
    role_id: Uuid,
) -> Result<project_grants::Model, ProjectFailure> {
    let wid = acting.workspace_id();
    let admin = Administering::begin(db, acting, project_id).await?;
    let role = find_role(&admin, wid, role_id).await?;
    ensure_subject(&admin, wid, subject).await?;
    admin.ensure_grantable(&role)?;
    ensure_gives_access(&role)?;
    let grant = project_grants::ActiveModel::grant(&admin.txn, wid, project_id, subject, role.id)
        .await
        .map_err(|e| {
            if matches!(e.sql_err(), Some(SqlErr::UniqueConstraintViolation(_))) {
                ProjectFailure::GrantExists
            } else {
                ProjectFailure::Db(e)
            }
        })?;
    audit::record(
        &admin.txn,
        acting.actor().audit(
            AuditAction::ProjectAccessGranted,
            (AuditSubject::ProjectGrant, grant.id),
            grant_changes(&grant, &role.name),
        ),
    )
    .await?;
    admin.txn.commit().await?;
    Ok(grant)
}

/// Ganti role grant `grant_id` menjadi `role_id`. Role yang sama tidak menulis apa pun.
///
/// # Errors
///
/// [`ProjectFailure::NotFound`], [`ProjectFailure::NotAssignable`] (role asal atau tujuan),
/// atau error database.
pub async fn change_role(
    db: &sea_orm::DatabaseConnection,
    acting: &Acting<'_>,
    project_id: Uuid,
    grant_id: Uuid,
    role_id: Uuid,
) -> Result<project_grants::Model, ProjectFailure> {
    let wid = acting.workspace_id();
    let admin = Administering::begin(db, acting, project_id).await?;
    let grant = find_grant(&admin, wid, project_id, grant_id).await?;
    let current = find_role(&admin, wid, grant.role_id).await?;
    let target = find_role(&admin, wid, role_id).await?;
    admin.ensure_grantable(&current)?;
    admin.ensure_grantable(&target)?;
    ensure_gives_access(&target)?;
    if current.id == target.id {
        admin.txn.rollback().await?;
        return Ok(grant);
    }
    let grant = grant.set_role(&admin.txn, target.id).await?;
    audit::record(
        &admin.txn,
        acting.actor().audit(
            AuditAction::ProjectAccessChanged,
            (AuditSubject::ProjectGrant, grant.id),
            serde_json::json!({
                "project_id": grant.project_id,
                "team_id": grant.team_id,
                "user_id": grant.user_id,
                "role": audit::change(&current.name, &target.name),
            }),
        ),
    )
    .await?;
    admin.txn.commit().await?;
    Ok(grant)
}

/// Cabut grant `grant_id`.
///
/// # Errors
///
/// [`ProjectFailure::NotFound`], [`ProjectFailure::NotAssignable`], atau error database.
pub async fn revoke(
    db: &sea_orm::DatabaseConnection,
    acting: &Acting<'_>,
    project_id: Uuid,
    grant_id: Uuid,
) -> Result<(), ProjectFailure> {
    let wid = acting.workspace_id();
    let admin = Administering::begin(db, acting, project_id).await?;
    let grant = find_grant(&admin, wid, project_id, grant_id).await?;
    let role = find_role(&admin, wid, grant.role_id).await?;
    admin.ensure_grantable(&role)?;
    let changes = grant_changes(&grant, &role.name);
    grant.delete(&admin.txn).await?;
    audit::record(
        &admin.txn,
        acting.actor().audit(
            AuditAction::ProjectAccessRevoked,
            (AuditSubject::ProjectGrant, grant_id),
            changes,
        ),
    )
    .await?;
    admin.txn.commit().await?;
    Ok(())
}

async fn find_role(
    admin: &Administering,
    wid: WorkspaceId,
    id: Uuid,
) -> Result<roles::Model, ProjectFailure> {
    roles::Model::find_in(&admin.txn, wid, id)
        .await?
        .ok_or(ProjectFailure::NotFound)
}

async fn find_grant(
    admin: &Administering,
    wid: WorkspaceId,
    project_id: Uuid,
    id: Uuid,
) -> Result<project_grants::Model, ProjectFailure> {
    project_grants::Model::lock_in(&admin.txn, wid, project_id, id)
        .await?
        .ok_or(ProjectFailure::NotFound)
}

/// Role grant harus memegang setidaknya satu izin project; role lain tidak memberi akses apa
/// pun (form hanya menawarkan role yang memberi akses, ini menjaga POST langsung).
fn ensure_gives_access(role: &roles::Model) -> Result<(), ProjectFailure> {
    if project_access::project_permissions_of(role).is_empty() {
        Err(ProjectFailure::NoProjectAccess)
    } else {
        Ok(())
    }
}

/// Subjek ada di workspace ini: tim milik workspace (dikunci, berurutan dengan tambah anggota
/// tim, `workspaces::teams`), atau orang yang anggota workspace.
async fn ensure_subject(
    admin: &Administering,
    wid: WorkspaceId,
    subject: GrantSubject,
) -> Result<(), ProjectFailure> {
    let found = match subject {
        GrantSubject::Team(id) => teams::Model::lock_in(&admin.txn, wid, id).await?.is_some(),
        GrantSubject::User(id) => workspace_members::Model::find_with_role(&admin.txn, wid, id)
            .await?
            .is_some(),
    };
    if found {
        Ok(())
    } else {
        Err(ProjectFailure::NotFound)
    }
}
