//! Bacaan audit log untuk tes: kejadian terbaru satu workspace.

use agentgate_server::models::{
    audit_events::{Column, Entity, Model},
    workspaces::WorkspaceId,
};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
};

/// `limit` kejadian terbaru di workspace ini (total order `created_at`, lalu `id`).
pub async fn newest_audit_events<C: ConnectionTrait>(
    db: &C,
    workspace_id: WorkspaceId,
    limit: u64,
) -> Result<Vec<Model>, DbErr> {
    Entity::find()
        .filter(Column::WorkspaceId.eq(workspace_id.get()))
        .order_by_desc(Column::CreatedAt)
        .order_by_desc(Column::Id)
        .limit(limit)
        .all(db)
        .await
}
