//! Pull request yang dilacak dari provider git (M6). Diisi saat runner meminta PR dibuka dan
//! diperbarui webhook (`pull_request`, `check_run`, `check_suite`, `status`).

use agentgate_core::git::{CiState, PrState};
use sea_orm::{QueryOrder, entity::prelude::*};
use serde::{Deserialize, Serialize};

pub use super::_entities::pull_requests::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

standard_behavior!(ActiveModel);

/// Satu check CI untuk head sha PR.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Check {
    pub name: String,
    pub state: CiState,
    #[serde(default)]
    pub url: String,
}

impl Model {
    #[must_use]
    pub fn state(&self) -> PrState {
        PrState::parse(&self.state).unwrap_or(PrState::Open)
    }

    #[must_use]
    pub fn ci(&self) -> CiState {
        CiState::parse(&self.ci_state).unwrap_or(CiState::None)
    }

    #[must_use]
    pub fn checks(&self) -> Vec<Check> {
        serde_json::from_value(self.checks.clone()).unwrap_or_default()
    }

    /// PR ini dihitung gate (bukan ditutup tanpa merge).
    #[must_use]
    pub fn counts(&self) -> bool {
        self.state() != PrState::Closed
    }

    /// PR tiket (terbaru dulu).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn of_ticket<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        ticket: Uuid,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::WorkspaceId.eq(wid.get()))
            .filter(Column::TicketId.eq(ticket))
            .order_by_desc(Column::CreatedAt)
            .all(db)
            .await
    }

    /// PR project (terbaru diperbarui dulu).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn of_project<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        project: Uuid,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::WorkspaceId.eq(wid.get()))
            .filter(Column::ProjectId.eq(project))
            .order_by_desc(Column::UpdatedAt)
            .all(db)
            .await
    }

    /// PR repo × nomor.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn by_number<C: ConnectionTrait>(
        db: &C,
        repo: Uuid,
        number: i64,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find()
            .filter(Column::RepoId.eq(repo))
            .filter(Column::Number.eq(number))
            .one(db)
            .await
    }
}
