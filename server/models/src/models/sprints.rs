//! Sprint project (M9): rencana → aktif → selesai. Isi sprint = `tickets.sprint_id`; perubahan isi
//! dicatat `sprint_scope_events` untuk burndown/burnup. Laporan beku disimpan di `report` saat
//! sprint diselesaikan (`services::planning::sprints::complete`).
use sea_orm::{
    ActiveEnum, ColumnTrait, ConnectionTrait, DbErr, DeriveActiveEnum, EntityTrait, EnumIter,
    QueryFilter, QueryOrder,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use super::_entities::sprints::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

standard_behavior!(ActiveModel);

/// Batas panjang nama dan tujuan sprint.
pub const NAME_MAX_LEN: usize = 80;
pub const GOAL_MAX_LEN: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum SprintState {
    #[sea_orm(string_value = "planned")]
    Planned,
    #[sea_orm(string_value = "active")]
    Active,
    #[sea_orm(string_value = "completed")]
    Completed,
}

/// Laporan beku sprint yang sudah selesai.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SprintReport {
    /// Poin di awal sprint.
    #[serde(default)]
    pub committed: i32,
    /// Poin tiket yang selesai.
    #[serde(default)]
    pub completed: i32,
    /// Poin yang ditambahkan setelah sprint mulai.
    #[serde(default)]
    pub added: i32,
    #[serde(default)]
    pub done: Vec<ReportTicket>,
    #[serde(default)]
    pub not_done: Vec<ReportTicket>,
    /// Nama sprint/backlog tujuan tiket yang belum selesai.
    #[serde(default)]
    pub moved_to: String,
}

/// Satu tiket di laporan sprint (jam estimasi/aktual opsional).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ReportTicket {
    pub id: Uuid,
    pub code: String,
    pub title: String,
    pub type_key: String,
    #[serde(default)]
    pub points: Option<i32>,
    #[serde(default)]
    pub estimate_hours: Option<f64>,
    #[serde(default)]
    pub actual_hours: Option<f64>,
}

impl Model {
    #[must_use]
    pub fn state(&self) -> SprintState {
        SprintState::try_from_value(&self.state).unwrap_or(SprintState::Planned)
    }

    #[must_use]
    pub fn report(&self) -> SprintReport {
        serde_json::from_value(self.report.clone()).unwrap_or_default()
    }

    /// Sprint di workspace berdasarkan id.
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

    /// Semua sprint project, urut tanggal mulai.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn of_project<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        project_id: Uuid,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::WorkspaceId.eq(wid.get()))
            .filter(Column::ProjectId.eq(project_id))
            .order_by_asc(Column::StartsOn)
            .order_by_asc(Column::CreatedAt)
            .order_by_asc(Column::Id)
            .all(db)
            .await
    }
}
