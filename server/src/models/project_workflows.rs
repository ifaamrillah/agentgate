//! Workflow per tipe tiket untuk satu project (override default workspace di
//! `ticket_types.workflow_id`). Tanpa baris = ikut default workspace.

use loco_rs::model::{TenantEntity, TenantQueryExt};
use sea_orm::{ActiveValue::Set, entity::prelude::*, sea_query::OnConflict};

pub use super::_entities::project_workflows::{ActiveModel, Column, Entity, Model};
use super::{ticket_types, workspaces::WorkspaceId};

impl TenantEntity for Entity {
    type TenantId = WorkspaceId;

    fn tenant_column() -> Column {
        Column::WorkspaceId
    }
}

standard_behavior!(ActiveModel);

impl Model {
    /// Override project ini.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn of_project<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        project_id: Uuid,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .in_tenant(workspace_id)
            .filter(Column::ProjectId.eq(project_id))
            .all(db)
            .await
    }

    /// Workflow yang berlaku untuk tipe `type_id` di project ini.
    ///
    /// # Errors
    ///
    /// Error database; tipe tidak ada → `RecordNotFound`.
    pub async fn effective<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        project_id: Uuid,
        type_id: Uuid,
    ) -> Result<Uuid, DbErr> {
        if let Some(o) = Entity::find()
            .in_tenant(workspace_id)
            .filter(Column::ProjectId.eq(project_id))
            .filter(Column::TypeId.eq(type_id))
            .one(db)
            .await?
        {
            return Ok(o.workflow_id);
        }
        ticket_types::Entity::find_by_id(type_id)
            .filter(ticket_types::Column::WorkspaceId.eq(workspace_id.get()))
            .one(db)
            .await?
            .map(|t| t.workflow_id)
            .ok_or_else(|| DbErr::RecordNotFound(format!("ticket type {type_id}")))
    }
}

impl ActiveModel {
    /// Pasang override (`Some`) atau hapus (`None` = ikut default workspace).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn set<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        project_id: Uuid,
        type_id: Uuid,
        workflow_id: Option<Uuid>,
    ) -> Result<(), DbErr> {
        match workflow_id {
            None => {
                Entity::delete_many()
                    .filter(Column::ProjectId.eq(project_id))
                    .filter(Column::TypeId.eq(type_id))
                    .exec(db)
                    .await?;
            }
            Some(workflow_id) => {
                Entity::insert(Self {
                    id: Set(Uuid::now_v7()),
                    workspace_id: Set(workspace_id.get()),
                    project_id: Set(project_id),
                    type_id: Set(type_id),
                    workflow_id: Set(workflow_id),
                    ..Default::default()
                })
                .on_conflict(
                    OnConflict::columns([Column::ProjectId, Column::TypeId])
                        .update_columns([Column::WorkflowId, Column::UpdatedAt])
                        .to_owned(),
                )
                .exec(db)
                .await?;
            }
        }
        Ok(())
    }
}
