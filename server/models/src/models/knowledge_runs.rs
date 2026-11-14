//! Run scan/update/audit knowledge: satu job knowledge di runner (tanpa tiket) yang menghasilkan
//! usulan lewat MCP `ag_knowledge propose`.
use sea_orm::{
    ActiveEnum, ColumnTrait, ConnectionTrait, DbErr, DeriveActiveEnum, EntityTrait, EnumIter,
    QueryFilter,
};
use uuid::Uuid;

pub use super::_entities::knowledge_runs::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

standard_behavior!(ActiveModel);

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum RunKind {
    /// Scan awal / scan ulang (membuat dan memperbarui dokumen).
    #[sea_orm(string_value = "scan")]
    Scan,
    /// Perbarui dokumen tertentu (tiket Done, merge, manual).
    #[sea_orm(string_value = "update")]
    Update,
    /// Audit penuh mingguan (cari dokumen basi).
    #[sea_orm(string_value = "audit")]
    Audit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum RunTrigger {
    #[sea_orm(string_value = "manual")]
    Manual,
    #[sea_orm(string_value = "done")]
    Done,
    #[sea_orm(string_value = "merge")]
    Merge,
    #[sea_orm(string_value = "audit")]
    Audit,
    #[sea_orm(string_value = "init")]
    Init,
}

/// Kedalaman scan (spec: cepat, standar, mendalam).
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum Depth {
    #[sea_orm(string_value = "cepat")]
    Cepat,
    #[sea_orm(string_value = "standar")]
    Standar,
    #[sea_orm(string_value = "mendalam")]
    Mendalam,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum RunStatus {
    #[sea_orm(string_value = "queued")]
    Queued,
    #[sea_orm(string_value = "running")]
    Running,
    #[sea_orm(string_value = "done")]
    Done,
    #[sea_orm(string_value = "failed")]
    Failed,
    #[sea_orm(string_value = "cancelled")]
    Cancelled,
}

impl Model {
    #[must_use]
    pub fn kind(&self) -> RunKind {
        RunKind::try_from_value(&self.kind).unwrap_or(RunKind::Update)
    }

    #[must_use]
    pub fn depth(&self) -> Depth {
        Depth::try_from_value(&self.depth).unwrap_or(Depth::Standar)
    }

    #[must_use]
    pub fn status(&self) -> RunStatus {
        RunStatus::try_from_value(&self.status).unwrap_or(RunStatus::Failed)
    }

    #[must_use]
    pub fn trigger(&self) -> RunTrigger {
        RunTrigger::try_from_value(&self.trigger).unwrap_or(RunTrigger::Manual)
    }

    #[must_use]
    pub fn is_open(&self) -> bool {
        matches!(self.status(), RunStatus::Queued | RunStatus::Running)
    }

    /// Run di workspace berdasarkan id.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn find_in<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        id: Uuid,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(id)
            .filter(Column::WorkspaceId.eq(wid.get()))
            .one(db)
            .await
    }
}
