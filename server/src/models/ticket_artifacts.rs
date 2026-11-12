//! Artifact tiket (spec "Alur review"): dokumen Markdown hasil satu stage (mis. `prd.md`).
//! Isinya ada di [`super::artifact_versions`]; setiap simpan menjadi versi baru (v1, v2, v3)
//! yang bisa dibandingkan. Nama unik per tiket tanpa membedakan huruf besar. Artifact tetap
//! tersimpan walau stage-nya dihapus dari workflow.

use agentgate_core::workflow::definition::valid_artifact_name;
use loco_rs::model::{TenantEntity, TenantQueryExt};
use sea_orm::{ActiveValue::Set, QueryOrder, entity::prelude::*};

pub use super::_entities::ticket_artifacts::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

pub const NAME_INVALID: &str = "artifact-name-invalid";
pub const NAME_TAKEN: &str = "artifact-name-taken";
pub const BODY_TOO_LONG: &str = "artifact-body-too-long";
pub const UNCHANGED: &str = "artifact-unchanged";
pub const ERROR_CODES: &[&str] = &[NAME_INVALID, NAME_TAKEN, BODY_TOO_LONG, UNCHANGED];
/// Batas isi satu versi (karakter).
pub const BODY_MAX_LEN: usize = 200_000;

impl TenantEntity for Entity {
    type TenantId = WorkspaceId;

    fn tenant_column() -> Column {
        Column::WorkspaceId
    }
}

standard_behavior!(ActiveModel);

/// Nama artifact dari isian: di-trim; `None` bila tidak valid.
#[must_use]
pub fn parse_name(raw: &str) -> Option<String> {
    let name = raw.trim();
    valid_artifact_name(name).then(|| name.to_string())
}

impl Model {
    /// Artifact tiket, urut dibuat.
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
            .filter(Column::TicketId.eq(ticket_id))
            .order_by_asc(Column::CreatedAt)
            .order_by_asc(Column::Id)
            .all(db)
            .await
    }

    /// Artifact `id` milik tiket `ticket_id`.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn find_of<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        ticket_id: Uuid,
        id: Uuid,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(id)
            .in_tenant(workspace_id)
            .filter(Column::TicketId.eq(ticket_id))
            .one(db)
            .await
    }

    /// Artifact bernama `name` (tanpa beda huruf besar) di tiket ini.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn find_named<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        ticket_id: Uuid,
        name: &str,
    ) -> Result<Option<Self>, DbErr> {
        Ok(Self::of_ticket(db, workspace_id, ticket_id)
            .await?
            .into_iter()
            .find(|a| a.name.eq_ignore_ascii_case(name.trim())))
    }
}

impl ActiveModel {
    /// # Errors
    ///
    /// Error database (nama kembar → unique violation).
    pub async fn create<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        ticket_id: Uuid,
        stage_key: &str,
        name: &str,
        created_by: Uuid,
    ) -> Result<Model, DbErr> {
        Self {
            workspace_id: Set(workspace_id.get()),
            ticket_id: Set(ticket_id),
            stage_key: Set(stage_key.to_string()),
            name: Set(name.to_string()),
            created_by: Set(Some(created_by)),
            ..Default::default()
        }
        .insert(db)
        .await
    }
}
