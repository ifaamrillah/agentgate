//! Estimasi tiket (M9): usulan AI (MCP `ag_planning estimate`, draft tiket) atau ubahan manusia.
//! Baris terbaru per tiket berlaku; baris AI terbaru dipakai sebagai "usulan AI".
use sea_orm::{
    ActiveEnum, ColumnTrait, ConnectionTrait, DbErr, DeriveActiveEnum, EntityTrait, EnumIter,
    QueryFilter, QueryOrder,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use super::_entities::ticket_estimates::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

#[async_trait::async_trait]
impl sea_orm::ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if insert && self.id.is_not_set() {
            self.id = sea_orm::ActiveValue::Set(Uuid::now_v7());
        }
        Ok(self)
    }
}

pub const BASIS_MAX_LEN: usize = 2000;
/// Batas jam per stage per jenis kerja.
pub const HOURS_MAX: f64 = 1000.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum EstimateSource {
    #[sea_orm(string_value = "ai")]
    Ai,
    #[sea_orm(string_value = "human")]
    Human,
}

/// Jam kerja satu stage (AI dan manusia).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StageHours {
    pub stage: String,
    #[serde(default)]
    pub ai_hours: f64,
    #[serde(default)]
    pub human_hours: f64,
}

impl Model {
    #[must_use]
    pub fn source(&self) -> EstimateSource {
        EstimateSource::try_from_value(&self.source).unwrap_or(EstimateSource::Human)
    }

    #[must_use]
    pub fn stages(&self) -> Vec<StageHours> {
        serde_json::from_value(self.stages.clone()).unwrap_or_default()
    }

    /// Total jam (AI, manusia).
    #[must_use]
    pub fn totals(&self) -> (f64, f64) {
        self.stages()
            .iter()
            .fold((0.0, 0.0), |(a, h), s| (a + s.ai_hours, h + s.human_hours))
    }

    /// Semua estimasi untuk tiket-tiket ini (lama → baru).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn of_tickets<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        ticket_ids: &[Uuid],
    ) -> Result<Vec<Self>, DbErr> {
        if ticket_ids.is_empty() {
            return Ok(Vec::new());
        }
        Entity::find()
            .filter(Column::WorkspaceId.eq(wid.get()))
            .filter(Column::TicketId.is_in(ticket_ids.to_vec()))
            .order_by_asc(Column::CreatedAt)
            .order_by_asc(Column::Id)
            .all(db)
            .await
    }
}

/// Estimasi terbaru per tiket: usulan AI terakhir dan estimasi yang berlaku (baris terakhir).
#[derive(Debug, Clone, Default)]
pub struct Latest {
    pub ai: Option<Model>,
    pub current: Option<Model>,
}

impl Latest {
    /// Jam (AI, manusia) estimasi yang berlaku.
    #[must_use]
    pub fn hours(&self) -> Option<(f64, f64)> {
        self.current.as_ref().map(Model::totals)
    }
}
