//! Simbol code graph (M7): node file, fungsi, method, tipe, dan route. Ditulis ulang setiap
//! build repo.
use agentgate_graph::{SymbolKind, build::Confidence};
use sea_orm::entity::prelude::*;

pub use super::_entities::graph_symbols::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

standard_behavior!(ActiveModel, append_only);

impl Model {
    #[must_use]
    pub fn kind(&self) -> SymbolKind {
        SymbolKind::parse(&self.kind).unwrap_or(SymbolKind::Function)
    }

    #[must_use]
    pub fn dead(&self) -> Option<Confidence> {
        Confidence::parse(&self.dead)
    }

    /// Simbol `id` di workspace.
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
