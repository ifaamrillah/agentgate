//! Snapshot definisi workflow per versi (v1, v2, …). Append-only: versi lama tidak diubah,
//! sehingga aturan stage tiket yang belum pindah sejak workflow diubah tetap bisa dibaca.

use agentgate_core::workflow::Definition;
use loco_rs::model::{TenantEntity, TenantQueryExt};
use sea_orm::{ActiveValue::Set, QueryOrder, entity::prelude::*};

pub use super::_entities::workflow_versions::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

impl TenantEntity for Entity {
    type TenantId = WorkspaceId;

    fn tenant_column() -> Column {
        Column::WorkspaceId
    }
}

standard_behavior!(ActiveModel);

impl Model {
    /// Definisi versi ini. JSON yang tidak terbaca (tidak terjadi untuk data yang ditulis
    /// aplikasi) menjadi definisi kosong.
    #[must_use]
    pub fn definition(&self) -> Definition {
        serde_json::from_value(self.definition.clone()).unwrap_or_default()
    }

    /// Definisi workflow `workflow_id` versi `version`.
    ///
    /// # Errors
    ///
    /// Error database; versi tidak ada → `RecordNotFound`.
    pub async fn definition_of<C: ConnectionTrait>(
        db: &C,
        workspace_id: Uuid,
        workflow_id: Uuid,
        version: i32,
    ) -> Result<Definition, DbErr> {
        Entity::find()
            .filter(Column::WorkspaceId.eq(workspace_id))
            .filter(Column::WorkflowId.eq(workflow_id))
            .filter(Column::Version.eq(version))
            .one(db)
            .await?
            .map(|v| v.definition())
            .ok_or_else(|| DbErr::RecordNotFound(format!("workflow {workflow_id} v{version}")))
    }

    /// Semua versi workflow, terbaru dulu.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn all_of<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        workflow_id: Uuid,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .in_tenant(workspace_id)
            .filter(Column::WorkflowId.eq(workflow_id))
            .order_by_desc(Column::Version)
            .all(db)
            .await
    }
}

impl ActiveModel {
    /// Simpan `definition` sebagai versi `version` workflow.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn append<C: ConnectionTrait>(
        db: &C,
        workflow: &super::workflows::Model,
        version: i32,
        definition: &Definition,
        created_by: Option<Uuid>,
    ) -> Result<Model, DbErr> {
        Self {
            workspace_id: Set(workflow.workspace_id),
            workflow_id: Set(workflow.id),
            version: Set(version),
            definition: Set(
                serde_json::to_value(definition).map_err(|e| DbErr::Custom(e.to_string()))?
            ),
            created_by: Set(created_by),
            ..Default::default()
        }
        .insert(db)
        .await
    }
}
