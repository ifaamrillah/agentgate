//! Usulan perubahan knowledge dari AI (atau impor repo yang bentrok). Tidak pernah menimpa
//! dokumen sebelum disetujui pemilik knowledge.
use sea_orm::{
    ActiveEnum, ColumnTrait, ConnectionTrait, DbErr, DeriveActiveEnum, EntityTrait, EnumIter,
    QueryFilter, QueryOrder,
};
use uuid::Uuid;

pub use super::_entities::knowledge_proposals::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

standard_behavior!(ActiveModel);

/// Pemicu usulan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum ProposalTrigger {
    #[sea_orm(string_value = "scan")]
    Scan,
    #[sea_orm(string_value = "done")]
    Done,
    #[sea_orm(string_value = "merge")]
    Merge,
    #[sea_orm(string_value = "audit")]
    Audit,
    #[sea_orm(string_value = "manual")]
    Manual,
    #[sea_orm(string_value = "init")]
    Init,
    #[sea_orm(string_value = "repo")]
    Repo,
}

impl ProposalTrigger {
    /// Ikon Tabler pemicu.
    #[must_use]
    pub fn icon(self) -> &'static str {
        match self {
            Self::Scan => "sparkles",
            Self::Done => "circle-check",
            Self::Merge => "git-merge",
            Self::Audit => "calendar",
            Self::Manual => "hand-stop",
            Self::Init => "file-import",
            Self::Repo => "git-branch",
        }
    }
}

/// Status usulan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum ProposalStatus {
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "approved")]
    Approved,
    #[sea_orm(string_value = "rejected")]
    Rejected,
    /// Digantikan usulan lebih baru untuk dokumen yang sama.
    #[sea_orm(string_value = "superseded")]
    Superseded,
}

impl Model {
    #[must_use]
    pub fn status(&self) -> ProposalStatus {
        ProposalStatus::try_from_value(&self.status).unwrap_or(ProposalStatus::Pending)
    }

    #[must_use]
    pub fn trigger(&self) -> ProposalTrigger {
        ProposalTrigger::try_from_value(&self.trigger).unwrap_or(ProposalTrigger::Manual)
    }

    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.status() == ProposalStatus::Pending
    }

    /// Usulan project dengan status tertentu; pending urut terlama dulu (antrean), selain itu
    /// terbaru dulu.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn of_project<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        project_id: Uuid,
        status: ProposalStatus,
        limit: u64,
    ) -> Result<Vec<Self>, DbErr> {
        use sea_orm::QuerySelect;
        let q = Entity::find()
            .filter(Column::WorkspaceId.eq(wid.get()))
            .filter(Column::ProjectId.eq(project_id))
            .filter(Column::Status.eq(status.to_value()));
        let q = if status == ProposalStatus::Pending {
            q.order_by_asc(Column::CreatedAt)
        } else {
            q.order_by_desc(Column::DecidedAt)
                .order_by_desc(Column::UpdatedAt)
        };
        q.limit(limit).all(db).await
    }

    /// Usulan di workspace berdasarkan id.
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
