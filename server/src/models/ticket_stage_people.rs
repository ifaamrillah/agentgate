//! Penugasan per stage tiket (spec "Assignment per stage"): executor, approver, dan watcher,
//! satu orang atau lebih, dan satu orang boleh memegang semua peran. Disalin dari default stage
//! saat tiket masuk stage yang belum punya baris; sesudah itu diubah per tiket.

use loco_rs::model::{TenantEntity, TenantQueryExt};
use sea_orm::{ActiveValue::Set, Iterable, QueryOrder, entity::prelude::*};
use serde::Serialize;

pub use super::_entities::ticket_stage_people::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, DeriveActiveEnum, Serialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum StageRole {
    #[sea_orm(string_value = "executor")]
    Executor,
    #[sea_orm(string_value = "approver")]
    Approver,
    #[sea_orm(string_value = "watcher")]
    Watcher,
}

impl TenantEntity for Entity {
    type TenantId = WorkspaceId;

    fn tenant_column() -> Column {
        Column::WorkspaceId
    }
}

standard_behavior!(ActiveModel);

/// Penugasan satu stage per peran.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StagePeople {
    pub executors: Vec<Uuid>,
    pub approvers: Vec<Uuid>,
    pub watchers: Vec<Uuid>,
}

impl StagePeople {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.executors.is_empty() && self.approvers.is_empty() && self.watchers.is_empty()
    }

    #[must_use]
    pub fn of(&self, role: StageRole) -> &[Uuid] {
        match role {
            StageRole::Executor => &self.executors,
            StageRole::Approver => &self.approvers,
            StageRole::Watcher => &self.watchers,
        }
    }
}

impl Model {
    /// Penugasan stage `stage_key` tiket ini, urut waktu ditambah.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn of_stage<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        ticket_id: Uuid,
        stage_key: &str,
    ) -> Result<StagePeople, DbErr> {
        let rows = Entity::find()
            .in_tenant(workspace_id)
            .filter(Column::TicketId.eq(ticket_id))
            .filter(Column::StageKey.eq(stage_key))
            .order_by_asc(Column::CreatedAt)
            .order_by_asc(Column::Id)
            .all(db)
            .await?;
        let mut people = StagePeople::default();
        for r in rows {
            match StageRole::try_from_value(&r.role) {
                Ok(StageRole::Executor) => people.executors.push(r.user_id),
                Ok(StageRole::Approver) => people.approvers.push(r.user_id),
                Ok(StageRole::Watcher) => people.watchers.push(r.user_id),
                Err(_) => {}
            }
        }
        Ok(people)
    }

    /// Stage tiket ini sudah punya baris penugasan (walau kosong setelah diubah, baris
    /// sentinel tidak ada: stage tanpa baris = belum pernah disalin).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn has_stage<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        ticket_id: Uuid,
        stage_key: &str,
    ) -> Result<bool, DbErr> {
        Ok(Entity::find()
            .in_tenant(workspace_id)
            .filter(Column::TicketId.eq(ticket_id))
            .filter(Column::StageKey.eq(stage_key))
            .count(db)
            .await?
            > 0)
    }
}

impl ActiveModel {
    /// Ganti seluruh penugasan stage. User yang bukan anggota workspace dilewati pemanggil.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn replace<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        ticket_id: Uuid,
        stage_key: &str,
        people: &StagePeople,
    ) -> Result<(), DbErr> {
        Entity::delete_many()
            .filter(Column::WorkspaceId.eq(workspace_id.get()))
            .filter(Column::TicketId.eq(ticket_id))
            .filter(Column::StageKey.eq(stage_key))
            .exec(db)
            .await?;
        for role in StageRole::iter() {
            for user_id in people.of(role) {
                Self {
                    workspace_id: Set(workspace_id.get()),
                    ticket_id: Set(ticket_id),
                    stage_key: Set(stage_key.to_string()),
                    role: Set(role.to_value()),
                    user_id: Set(*user_id),
                    ..Default::default()
                }
                .insert(db)
                .await?;
            }
        }
        Ok(())
    }
}
