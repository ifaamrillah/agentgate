//! Repo project (Pengaturan project → Repo, `AdministerProject`): tambah, ubah, hapus, dengan
//! audit `repo.*` di transaksi yang sama.

use sea_orm::{DatabaseConnection, ModelTrait};
use uuid::Uuid;

use super::{Administering, ProjectFailure};
use crate::{
    models::repos::{self, RepoInput},
    services::{
        audit::{self, AuditAction, AuditSubject},
        workspaces::Acting,
    },
};

/// # Errors
///
/// [`ProjectFailure::NotFound`] (project atau izin), [`ProjectFailure::Invalid`] (termasuk
/// `repo-name-taken`), atau error database.
pub async fn add(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    project_id: Uuid,
    input: &RepoInput,
) -> Result<repos::Model, ProjectFailure> {
    let admin = Administering::begin(db, acting, project_id).await?;
    let repo =
        repos::ActiveModel::add(&admin.txn, acting.workspace_id(), project_id, input).await?;
    audit::record(
        &admin.txn,
        acting.actor().audit(
            AuditAction::RepoAdded,
            (AuditSubject::Repo, repo.id),
            serde_json::json!({
                "project_id": project_id,
                "name": repo.name,
                "provider": repo.provider,
                "url": repo.url,
                "default_branch": repo.default_branch,
            }),
        ),
    )
    .await?;
    admin.txn.commit().await?;
    Ok(repo)
}

/// Ubah repo `repo_id`. Tanpa perubahan: tidak menulis apa pun.
///
/// # Errors
///
/// [`ProjectFailure::NotFound`] (project, izin, atau repo di project lain),
/// [`ProjectFailure::Invalid`], atau error database.
pub async fn update(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    project_id: Uuid,
    repo_id: Uuid,
    input: &RepoInput,
) -> Result<repos::Model, ProjectFailure> {
    let admin = Administering::begin(db, acting, project_id).await?;
    let before = repos::Model::lock_in(&admin.txn, acting.workspace_id(), project_id, repo_id)
        .await?
        .ok_or(ProjectFailure::NotFound)?;
    let after = before.clone().update_from(&admin.txn, input).await?;
    let mut changes = serde_json::Map::new();
    for (field, from, to) in [
        ("name", &before.name, &after.name),
        ("provider", &before.provider, &after.provider),
        ("url", &before.url, &after.url),
        (
            "default_branch",
            &before.default_branch,
            &after.default_branch,
        ),
    ] {
        if let Some(c) = audit::change(from, to) {
            changes.insert(field.to_string(), c);
        }
    }
    if changes.is_empty() {
        admin.txn.rollback().await?;
        return Ok(before);
    }
    audit::record(
        &admin.txn,
        acting.actor().audit(
            AuditAction::RepoUpdated,
            (AuditSubject::Repo, after.id),
            serde_json::Value::Object(changes),
        ),
    )
    .await?;
    admin.txn.commit().await?;
    Ok(after)
}

/// Hapus repo `repo_id`.
///
/// # Errors
///
/// [`ProjectFailure::NotFound`] (project, izin, atau repo di project lain), atau error
/// database.
pub async fn remove(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    project_id: Uuid,
    repo_id: Uuid,
) -> Result<(), ProjectFailure> {
    let admin = Administering::begin(db, acting, project_id).await?;
    let repo = repos::Model::lock_in(&admin.txn, acting.workspace_id(), project_id, repo_id)
        .await?
        .ok_or(ProjectFailure::NotFound)?;
    let changes =
        serde_json::json!({ "project_id": project_id, "name": repo.name, "url": repo.url });
    repo.delete(&admin.txn).await?;
    audit::record(
        &admin.txn,
        acting.actor().audit(
            AuditAction::RepoRemoved,
            (AuditSubject::Repo, repo_id),
            changes,
        ),
    )
    .await?;
    admin.txn.commit().await?;
    Ok(())
}
