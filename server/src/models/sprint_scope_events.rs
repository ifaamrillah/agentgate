//! Log isi sprint (M9): tiket masuk/keluar, selesai/dibuka lagi, poin berubah. Ditulis di
//! transaksi yang sama dengan perubahannya; direplay laporan burndown/burnup.
use sea_orm::{
    ActiveEnum, ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DbErr,
    DeriveActiveEnum, EntityTrait, EnumIter, QueryFilter, QueryOrder,
};
use uuid::Uuid;

pub use super::_entities::sprint_scope_events::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

#[async_trait::async_trait]
impl sea_orm::ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if insert && self.id.is_not_set() {
            self.id = Set(Uuid::now_v7());
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum ScopeKind {
    /// Tiket masuk sprint (`points` = poin saat itu).
    #[sea_orm(string_value = "added")]
    Added,
    /// Tiket keluar sprint.
    #[sea_orm(string_value = "removed")]
    Removed,
    /// Tiket masuk kategori Selesai.
    #[sea_orm(string_value = "done")]
    Done,
    /// Tiket keluar dari Selesai.
    #[sea_orm(string_value = "reopened")]
    Reopened,
    /// Story point berubah (`points` = nilai baru).
    #[sea_orm(string_value = "points")]
    Points,
}

impl Model {
    #[must_use]
    pub fn kind(&self) -> ScopeKind {
        ScopeKind::try_from_value(&self.kind).unwrap_or(ScopeKind::Points)
    }

    /// Log satu sprint, urut waktu.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn of_sprint<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        sprint_id: Uuid,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::WorkspaceId.eq(wid.get()))
            .filter(Column::SprintId.eq(sprint_id))
            .order_by_asc(Column::CreatedAt)
            .order_by_asc(Column::Id)
            .all(db)
            .await
    }
}

impl ActiveModel {
    /// Catat satu kejadian.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn log<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        sprint_id: Uuid,
        ticket_id: Uuid,
        kind: ScopeKind,
        points: Option<i32>,
    ) -> Result<Model, DbErr> {
        Self {
            workspace_id: Set(wid.get()),
            sprint_id: Set(sprint_id),
            ticket_id: Set(ticket_id),
            kind: Set(kind.to_value()),
            points: Set(points.unwrap_or(0)),
            ..Default::default()
        }
        .insert(db)
        .await
    }
}
