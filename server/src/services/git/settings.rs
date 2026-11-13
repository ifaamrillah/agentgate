//! Konvensi git project (`projects.git`, Admin project): pola branch, gaya dan bahasa commit/PR,
//! hasil kode stage Development, mode merge, dan gate CI. Audit `project.git_updated` di
//! transaksi yang sama.

use agentgate_core::git::GitSettings;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, DatabaseConnection, TransactionTrait};
use uuid::Uuid;

use crate::{
    models::{permission::Permission, projects},
    services::{
        audit::{self, AuditAction, AuditSubject},
        tickets::{TicketFailure, access_in},
        workspaces::Acting,
    },
};

/// Simpan konvensi git project. Tanpa perubahan → tanpa audit.
///
/// # Errors
///
/// `NotFound` (project/izin), `Invalid` (`git-branch-pattern-invalid`, `git-check-invalid`),
/// error database.
pub async fn update(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    project_id: Uuid,
    mut input: GitSettings,
) -> Result<GitSettings, TicketFailure> {
    input
        .normalize()
        .map_err(|(field, code)| TicketFailure::field(field, code))?;
    let txn = db.begin().await?;
    let project = projects::Model::lock_in(&txn, acting.workspace_id(), project_id)
        .await?
        .ok_or(TicketFailure::NotFound)?;
    access_in(&txn, acting, project.id, Permission::AdministerProject).await?;
    let before = project.git_settings();
    if before == input {
        txn.rollback().await?;
        return Ok(input);
    }
    let after = serde_json::to_value(&input).unwrap_or_default();
    let mut m: projects::ActiveModel = project.clone().into();
    m.git = Set(after.clone());
    m.update(&txn).await?;
    audit::record(
        &txn,
        acting.actor().audit(
            AuditAction::ProjectGitUpdated,
            (AuditSubject::Project, project.id),
            serde_json::json!({
                "from": serde_json::to_value(&before).unwrap_or_default(),
                "to": after,
            }),
        ),
    )
    .await?;
    txn.commit().await?;
    Ok(input)
}
