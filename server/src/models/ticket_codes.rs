//! Kode tiket: kode sekarang dan alias lama (spec "Saat format diubah, kode lama tetap berlaku
//! sebagai alias"). Unik per workspace tanpa membedakan huruf besar (index `upper(code)`), jadi
//! satu kode tidak pernah menunjuk dua tiket, termasuk alias lama.

use loco_rs::model::{TenantEntity, TenantQueryExt};
use sea_orm::{
    ActiveValue::Set,
    entity::prelude::*,
    sea_query::{ExprTrait, Func, OnConflict},
};

pub use super::_entities::ticket_codes::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

impl TenantEntity for Entity {
    type TenantId = WorkspaceId;

    fn tenant_column() -> Column {
        Column::WorkspaceId
    }
}

standard_behavior!(ActiveModel);

impl Model {
    /// Baris kode `code` (tanpa membedakan huruf besar) di workspace ini.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn find<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        code: &str,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find()
            .in_tenant(workspace_id)
            .filter(Expr::expr(Func::upper(Expr::col(Column::Code))).eq(code.trim().to_uppercase()))
            .one(db)
            .await
    }
}

impl ActiveModel {
    /// Daftarkan `code` untuk tiket `ticket_id`. `Ok(false)` = kode sudah dipakai (tiket ini
    /// atau tiket lain): pemanggil memutuskan. `ON CONFLICT DO NOTHING`, jadi bentrok tidak
    /// membatalkan transaksi pemanggil.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn claim<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        ticket_id: Uuid,
        code: &str,
    ) -> Result<bool, DbErr> {
        let row = Self {
            id: Set(Uuid::now_v7()),
            workspace_id: Set(workspace_id.get()),
            ticket_id: Set(ticket_id),
            code: Set(code.to_string()),
            ..Default::default()
        };
        let inserted = Entity::insert(row)
            .on_conflict(OnConflict::new().do_nothing().to_owned())
            .exec_without_returning(db)
            .await?;
        Ok(inserted == 1)
    }
}
