//! Membuat workspace baru: satu transaksi berisi workspace, empat role bawaan, workflow bawaan
//! (Kanban, Subtask ringkas), tipe tiket bawaan, pembuatnya
//! sebagai Owner, dan audit `workspace.created`. Dipanggil `POST /w/new` dan seed dev. Siapa
//! pun yang login boleh membuat workspace (spec: pendaftaran terbuka seperti Jira).

use loco_rs::app::AppContext;
use sea_orm::{ConnectionTrait, TransactionTrait};

use agentgate_core::workflow::templates::{self, Template};

use crate::{
    models::{
        roles::{self, RoleKey},
        ticket_types, users, workflows, workspace_members,
        workspaces::{self, CreateWorkspace, WorkspaceError},
    },
    services::audit::{self, AuditAction, AuditEntry, AuditSubject},
};

/// Isian form. `slug` kosong = dibentuk dari nama.
#[derive(Debug)]
pub struct NewWorkspace {
    pub name: String,
    pub slug: String,
}

/// # Errors
///
/// [`WorkspaceError::Invalid`] untuk nama/slug tidak valid atau slug terpakai,
/// [`WorkspaceError::Db`] untuk error database lain. Tidak ada yang tersimpan bila gagal.
pub async fn run(
    ctx: &AppContext,
    owner: &users::Model,
    new: NewWorkspace,
    request_id: Option<String>,
) -> Result<workspaces::Model, WorkspaceError> {
    let txn = ctx.db.begin().await?;
    let workspace = run_in(&txn, owner, new, request_id).await?;
    txn.commit().await?;
    Ok(workspace)
}

/// Langkah [`run`] di dalam transaksi milik pemanggil `txn` (mis. seed dev, yang menambah
/// anggota lain di transaksi yang sama). Tidak commit sendiri.
///
/// # Errors
///
/// Sama dengan [`run`].
pub async fn run_in<C: ConnectionTrait>(
    txn: &C,
    owner: &users::Model,
    new: NewWorkspace,
    request_id: Option<String>,
) -> Result<workspaces::Model, WorkspaceError> {
    let workspace = workspaces::ActiveModel::create(
        txn,
        CreateWorkspace {
            name: new.name,
            slug: new.slug,
            created_by: Some(owner.id),
        },
    )
    .await?;
    let wid = workspace.workspace_id();
    let roles = roles::ActiveModel::create_builtin_set(txn, wid).await?;
    let owner_role = roles
        .iter()
        .find(|r| r.role_key().ok().flatten() == Some(RoleKey::Owner))
        .ok_or_else(|| sea_orm::DbErr::RecordNotFound("owner role".to_string()))?;
    workspace_members::ActiveModel::add(txn, wid, owner.id, owner_role.id).await?;
    let kanban = create_builtin_workflow(txn, wid, templates::kanban(), owner.id).await?;
    let subtask = create_builtin_workflow(txn, wid, templates::SUBTASK, owner.id).await?;
    ticket_types::ActiveModel::create_builtin_set(txn, wid, kanban.id, subtask.id).await?;
    audit::record(
        txn,
        AuditEntry {
            workspace_id: wid,
            actor_id: Some(owner.id),
            action: AuditAction::WorkspaceCreated,
            subject: (AuditSubject::Workspace, workspace.id),
            changes: serde_json::json!({ "name": workspace.name, "slug": workspace.slug }),
            request_id,
        },
    )
    .await?;
    Ok(workspace)
}

/// Workflow bawaan dari template (Kanban untuk semua tipe, Subtask ringkas untuk Subtask).
async fn create_builtin_workflow<C: ConnectionTrait>(
    txn: &C,
    wid: workspaces::WorkspaceId,
    template: Template,
    owner: uuid::Uuid,
) -> Result<workflows::Model, sea_orm::DbErr> {
    let doc = template.document();
    workflows::ActiveModel::create(
        txn,
        wid,
        workflows::NewWorkflow {
            name: &doc.name,
            description: &doc.description,
            template_key: Some(template.key),
            definition: &doc.definition,
            created_by: Some(owner),
        },
    )
    .await
}
