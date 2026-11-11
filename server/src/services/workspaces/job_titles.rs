//! Jabatan tim (Pengaturan → Jabatan): buat, ganti nama, hapus, masing-masing dengan audit
//! `job_title.*` di transaksi yang sama. Hapus membuat member dan undangan yang memakainya tanpa
//! jabatan lewat FK `ON DELETE SET NULL (job_title_id)` (migrasi workspaces), tanpa langkah
//! tambahan di sini.

use sea_orm::{DbErr, ModelTrait, TransactionTrait};
use uuid::Uuid;

use super::Actor;
use crate::{
    models::job_titles::{self, JobTitleError},
    services::audit::{self, AuditAction, AuditSubject},
};

/// # Errors
///
/// Seperti `job_titles::ActiveModel::create`.
pub async fn create(
    db: &sea_orm::DatabaseConnection,
    actor: &Actor,
    name: &str,
) -> Result<job_titles::Model, JobTitleError> {
    let txn = db.begin().await?;
    let title = job_titles::ActiveModel::create(&txn, actor.workspace_id, name).await?;
    audit::record(
        &txn,
        actor.audit(
            AuditAction::JobTitleCreated,
            (AuditSubject::JobTitle, title.id),
            serde_json::json!({ "name": title.name }),
        ),
    )
    .await?;
    txn.commit().await?;
    Ok(title)
}

/// # Errors
///
/// `DbErr::RecordNotFound` bila jabatan `id` sudah tidak ada di workspace ini, lalu seperti
/// `job_titles::ActiveModel::rename`.
pub async fn rename(
    db: &sea_orm::DatabaseConnection,
    actor: &Actor,
    id: Uuid,
    name: &str,
) -> Result<job_titles::Model, JobTitleError> {
    let txn = db.begin().await?;
    let current = locked(&txn, actor, id).await?;
    let old_name = current.name.clone();
    let updated = job_titles::ActiveModel::rename(&txn, current, name).await?;
    let Some(change) = audit::change(&old_name, &updated.name) else {
        txn.rollback().await?;
        return Ok(updated);
    };
    audit::record(
        &txn,
        actor.audit(
            AuditAction::JobTitleUpdated,
            (AuditSubject::JobTitle, updated.id),
            serde_json::json!({ "name": change }),
        ),
    )
    .await?;
    txn.commit().await?;
    Ok(updated)
}

/// # Errors
///
/// `DbErr::RecordNotFound` bila jabatan `id` sudah tidak ada di workspace ini, error database
/// lain.
pub async fn delete(
    db: &sea_orm::DatabaseConnection,
    actor: &Actor,
    id: Uuid,
) -> Result<(), JobTitleError> {
    let txn = db.begin().await?;
    let current = locked(&txn, actor, id).await?;
    let changes = serde_json::json!({ "name": current.name });
    current.delete(&txn).await?;
    audit::record(
        &txn,
        actor.audit(
            AuditAction::JobTitleDeleted,
            (AuditSubject::JobTitle, id),
            changes,
        ),
    )
    .await?;
    txn.commit().await?;
    Ok(())
}

async fn locked(
    txn: &sea_orm::DatabaseTransaction,
    actor: &Actor,
    id: Uuid,
) -> Result<job_titles::Model, JobTitleError> {
    job_titles::Model::lock_in(txn, actor.workspace_id, id)
        .await?
        .ok_or_else(|| JobTitleError::Db(DbErr::RecordNotFound("job title".to_string())))
}
