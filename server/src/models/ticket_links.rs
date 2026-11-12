//! Relasi antar tiket (spec: blocks, duplicates, relates to). Disimpan satu arah
//! (`from` *kind* `to`); "diblokir oleh" adalah sisi `to` dari `blocks`. Relasi hanya antar
//! tiket satu project (dicek `services::tickets::links`), tidak ke dirinya sendiri (CHECK).
//! [`open_blockers`] adalah satu-satunya aturan "tiket ini belum boleh mulai".

use loco_rs::model::{TenantEntity, TenantQueryExt};
use sea_orm::{ActiveValue::Set, Condition, QueryOrder, QuerySelect, SqlErr, entity::prelude::*};
use serde::Serialize;

pub use super::_entities::ticket_links::{ActiveModel, Column, Entity, Model};
use super::{
    tickets::{self, TicketStatus},
    workspaces::WorkspaceId,
};

pub const LINK_EXISTS: &str = "ticket-link-exists";
pub const LINK_TARGET_INVALID: &str = "ticket-link-target-invalid";
pub const LINK_KIND_INVALID: &str = "ticket-link-kind-invalid";
pub const ERROR_CODES: &[&str] = &[LINK_EXISTS, LINK_TARGET_INVALID, LINK_KIND_INVALID];

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum LinkKind {
    #[sea_orm(string_value = "blocks")]
    Blocks,
    #[sea_orm(string_value = "duplicates")]
    Duplicates,
    #[sea_orm(string_value = "relates")]
    Relates,
}

/// Pilihan relasi di form, dari sudut pandang tiket yang dibuka: `blocked_by` disimpan
/// sebagai `blocks` dengan arah dibalik.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkChoice {
    Blocks,
    BlockedBy,
    Duplicates,
    Relates,
}

impl LinkChoice {
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "blocks" => Some(Self::Blocks),
            "blocked_by" => Some(Self::BlockedBy),
            "duplicates" => Some(Self::Duplicates),
            "relates" => Some(Self::Relates),
            _ => None,
        }
    }

    /// (kind, tiket yang dibuka adalah `from`).
    #[must_use]
    pub fn stored(self) -> (LinkKind, bool) {
        match self {
            Self::Blocks => (LinkKind::Blocks, true),
            Self::BlockedBy => (LinkKind::Blocks, false),
            Self::Duplicates => (LinkKind::Duplicates, true),
            Self::Relates => (LinkKind::Relates, true),
        }
    }
}

impl TenantEntity for Entity {
    type TenantId = WorkspaceId;

    fn tenant_column() -> Column {
        Column::WorkspaceId
    }
}

standard_behavior!(ActiveModel);

impl Model {
    /// # Errors
    ///
    /// Nilai kolom bukan varian [`LinkKind`].
    pub fn kind(&self) -> Result<LinkKind, DbErr> {
        LinkKind::try_from_value(&self.kind)
    }

    /// Semua relasi yang menyentuh tiket `ticket_id` (kedua arah), terlama dulu.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn of_ticket<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        ticket_id: Uuid,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .in_tenant(workspace_id)
            .filter(
                Condition::any()
                    .add(Column::FromId.eq(ticket_id))
                    .add(Column::ToId.eq(ticket_id)),
            )
            .order_by_asc(Column::CreatedAt)
            .order_by_asc(Column::Id)
            .all(db)
            .await
    }
}

impl ActiveModel {
    /// `Ok(None)` = relasi yang sama sudah ada.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn link<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        from_id: Uuid,
        to_id: Uuid,
        kind: LinkKind,
        created_by: Uuid,
    ) -> Result<Option<Model>, DbErr> {
        let result = Self {
            workspace_id: Set(workspace_id.get()),
            from_id: Set(from_id),
            to_id: Set(to_id),
            kind: Set(kind.to_value()),
            created_by: Set(Some(created_by)),
            ..Default::default()
        }
        .insert(db)
        .await;
        match result {
            Ok(m) => Ok(Some(m)),
            Err(e) if matches!(e.sql_err(), Some(SqlErr::UniqueConstraintViolation(_))) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

/// Tiket pemblokir `ticket_id` yang belum selesai (dan tidak di sampah). Kosong = boleh mulai.
///
/// # Errors
///
/// Error database.
pub async fn open_blockers<C: ConnectionTrait>(
    db: &C,
    workspace_id: WorkspaceId,
    ticket_id: Uuid,
) -> Result<Vec<tickets::Model>, DbErr> {
    tickets::Entity::find()
        .in_tenant(workspace_id)
        .filter(tickets::active())
        .filter(tickets::Column::Status.ne(TicketStatus::Done.to_value()))
        .filter(
            tickets::Column::Id.in_subquery(sea_orm::QueryTrait::into_query(
                Entity::find()
                    .select_only()
                    .column(Column::FromId)
                    .filter(Column::ToId.eq(ticket_id))
                    .filter(Column::Kind.eq(LinkKind::Blocks.to_value())),
            )),
        )
        .order_by_asc(tickets::Column::Code)
        .all(db)
        .await
}
