//! Versi dokumen knowledge: setiap simpan, usulan yang disetujui, pemulihan, atau impor dari
//! repo menjadi satu versi bernomor (riwayat + diff).
use sea_orm::{
    ColumnTrait, ConnectionTrait, DbErr, DeriveActiveEnum, EntityTrait, EnumIter, QueryFilter,
    QueryOrder,
};
use uuid::Uuid;

pub use super::_entities::knowledge_versions::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

standard_behavior!(ActiveModel, append_only);

/// Asal versi (CHECK migrasi, dijaga `tests/models/enums.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum VersionSource {
    #[sea_orm(string_value = "create")]
    Create,
    #[sea_orm(string_value = "edit")]
    Edit,
    #[sea_orm(string_value = "proposal")]
    Proposal,
    #[sea_orm(string_value = "restore")]
    Restore,
    #[sea_orm(string_value = "repo")]
    Repo,
}

impl Model {
    /// Id pendek gaya commit untuk tampilan.
    #[must_use]
    pub fn short(&self) -> String {
        self.id.simple().to_string().chars().rev().take(7).collect()
    }

    /// Semua versi dokumen, terbaru dulu.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn of_doc<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        doc_id: Uuid,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::WorkspaceId.eq(wid.get()))
            .filter(Column::DocId.eq(doc_id))
            .order_by_desc(Column::Number)
            .all(db)
            .await
    }

    /// Versi nomor `number`.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn number<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        doc_id: Uuid,
        number: i32,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find()
            .filter(Column::WorkspaceId.eq(wid.get()))
            .filter(Column::DocId.eq(doc_id))
            .filter(Column::Number.eq(number))
            .one(db)
            .await
    }
}
