//! Penghitung nomor tiket per project per skop (`ticket_code::CounterScope`). Satu statement
//! `INSERT … ON CONFLICT DO UPDATE … RETURNING` mengambil nomor berikutnya secara atomik, jadi
//! dua tiket yang dibuat bersamaan tidak pernah mendapat nomor sama.

use sea_orm::{DbBackend, Statement, entity::prelude::*};

pub use super::_entities::ticket_counters::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

standard_behavior!(ActiveModel);

/// Ambil nomor berikutnya untuk `(project_id, scope)` dan naikkan penghitung.
///
/// # Errors
///
/// Error database.
pub async fn next_number<C: ConnectionTrait>(
    db: &C,
    workspace_id: WorkspaceId,
    project_id: Uuid,
    scope: &str,
) -> Result<i32, DbErr> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO ticket_counters (id, workspace_id, project_id, scope, next_number)
             VALUES ($1, $2, $3, $4, 2)
             ON CONFLICT (project_id, scope)
             DO UPDATE SET next_number = ticket_counters.next_number + 1, updated_at = now()
             RETURNING next_number - 1",
            [
                Uuid::now_v7().into(),
                workspace_id.get().into(),
                project_id.into(),
                scope.into(),
            ],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("ticket counter".into()))?;
    row.try_get_by_index::<i32>(0)
}
