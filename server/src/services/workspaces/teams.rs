//! Tim (Pengaturan → Tim): buat, ganti nama, hapus, tambah dan keluarkan anggota, masing-masing
//! dengan audit `team.*` di transaksi yang sama. Hapus tim mencabut keanggotaan tim dan grant
//! project tim itu lewat FK `ON DELETE CASCADE` (migrasi teams_projects), tanpa langkah
//! tambahan di sini.

use sea_orm::{DatabaseConnection, DatabaseTransaction, DbErr, ModelTrait, TransactionTrait};
use uuid::Uuid;

use super::Actor;
use crate::{
    models::{
        team_members,
        teams::{self, TeamError},
        workspace_members,
    },
    services::audit::{self, AuditAction, AuditSubject},
};

/// # Errors
///
/// Seperti `teams::ActiveModel::create`.
pub async fn create(
    db: &DatabaseConnection,
    actor: &Actor,
    name: &str,
) -> Result<teams::Model, TeamError> {
    let txn = db.begin().await?;
    let team = teams::ActiveModel::create(&txn, actor.workspace_id, name).await?;
    audit::record(
        &txn,
        actor.audit(
            AuditAction::TeamCreated,
            (AuditSubject::Team, team.id),
            serde_json::json!({ "name": team.name }),
        ),
    )
    .await?;
    txn.commit().await?;
    Ok(team)
}

/// # Errors
///
/// `DbErr::RecordNotFound` bila tim `id` sudah tidak ada di workspace ini, lalu seperti
/// `teams::ActiveModel::rename`.
pub async fn rename(
    db: &DatabaseConnection,
    actor: &Actor,
    id: Uuid,
    name: &str,
) -> Result<teams::Model, TeamError> {
    let txn = db.begin().await?;
    let current = locked(&txn, actor, id).await?;
    let old_name = current.name.clone();
    let updated = teams::ActiveModel::rename(&txn, current, name).await?;
    let Some(change) = audit::change(&old_name, &updated.name) else {
        txn.rollback().await?;
        return Ok(updated);
    };
    audit::record(
        &txn,
        actor.audit(
            AuditAction::TeamUpdated,
            (AuditSubject::Team, updated.id),
            serde_json::json!({ "name": change }),
        ),
    )
    .await?;
    txn.commit().await?;
    Ok(updated)
}

/// # Errors
///
/// `DbErr::RecordNotFound` bila tim `id` sudah tidak ada di workspace ini, error database lain.
pub async fn delete(db: &DatabaseConnection, actor: &Actor, id: Uuid) -> Result<(), TeamError> {
    let txn = db.begin().await?;
    let current = locked(&txn, actor, id).await?;
    let changes = serde_json::json!({ "name": current.name });
    current.delete(&txn).await?;
    audit::record(
        &txn,
        actor.audit(AuditAction::TeamDeleted, (AuditSubject::Team, id), changes),
    )
    .await?;
    txn.commit().await?;
    Ok(())
}

/// Masukkan anggota workspace `user_id` ke tim `team_id`. Sudah anggota tim: tanpa perubahan
/// dan tanpa audit.
///
/// # Errors
///
/// `DbErr::RecordNotFound` bila tim tidak ada atau `user_id` bukan anggota workspace ini, error
/// database lain.
pub async fn add_member(
    db: &DatabaseConnection,
    actor: &Actor,
    team_id: Uuid,
    user_id: Uuid,
) -> Result<(), TeamError> {
    let txn = db.begin().await?;
    let team = locked(&txn, actor, team_id).await?;
    if workspace_members::Model::find_with_role(&txn, actor.workspace_id, user_id)
        .await?
        .is_none()
    {
        return Err(not_found("workspace member"));
    }
    if !team_members::ActiveModel::add(&txn, actor.workspace_id, team.id, user_id).await? {
        txn.rollback().await?;
        return Ok(());
    }
    audit::record(
        &txn,
        actor.audit(
            AuditAction::TeamMemberAdded,
            (AuditSubject::Team, team.id),
            serde_json::json!({ "user_id": user_id }),
        ),
    )
    .await?;
    txn.commit().await?;
    Ok(())
}

/// Keluarkan `user_id` dari tim `team_id`. Bukan anggota tim: tanpa perubahan dan tanpa audit.
///
/// # Errors
///
/// `DbErr::RecordNotFound` bila tim tidak ada, error database lain.
pub async fn remove_member(
    db: &DatabaseConnection,
    actor: &Actor,
    team_id: Uuid,
    user_id: Uuid,
) -> Result<(), TeamError> {
    let txn = db.begin().await?;
    let team = locked(&txn, actor, team_id).await?;
    let Some(member) =
        team_members::Model::find(&txn, actor.workspace_id, team.id, user_id).await?
    else {
        txn.rollback().await?;
        return Ok(());
    };
    member.delete(&txn).await?;
    audit::record(
        &txn,
        actor.audit(
            AuditAction::TeamMemberRemoved,
            (AuditSubject::Team, team.id),
            serde_json::json!({ "user_id": user_id }),
        ),
    )
    .await?;
    txn.commit().await?;
    Ok(())
}

async fn locked(
    txn: &DatabaseTransaction,
    actor: &Actor,
    id: Uuid,
) -> Result<teams::Model, TeamError> {
    teams::Model::lock_in(txn, actor.workspace_id, id)
        .await?
        .ok_or_else(|| not_found("team"))
}

fn not_found(what: &str) -> TeamError {
    TeamError::Db(DbErr::RecordNotFound(what.to_string()))
}
