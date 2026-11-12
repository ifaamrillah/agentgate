//! Komentar tiket (Markdown). Penulis bisa mengubah dan menghapus komentarnya sendiri; @mention
//! di isinya diproses `services::tickets::mentions` saat dibuat.

use loco_rs::model::{TenantEntity, TenantQueryExt};
use sea_orm::{ActiveValue::Set, IntoActiveModel, QueryOrder, entity::prelude::*};
use validator::ValidationError;

pub use super::_entities::ticket_comments::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

pub const COMMENT_REQUIRED: &str = "ticket-comment-required";
pub const COMMENT_TOO_LONG: &str = "ticket-comment-too-long";
pub const ERROR_CODES: &[&str] = &[COMMENT_REQUIRED, COMMENT_TOO_LONG];
pub const BODY_MAX_LEN: usize = 20_000;

impl TenantEntity for Entity {
    type TenantId = WorkspaceId;

    fn tenant_column() -> Column {
        Column::WorkspaceId
    }
}

standard_behavior!(ActiveModel);

/// Isi komentar: di-trim, wajib, maksimal [`BODY_MAX_LEN`].
///
/// # Errors
///
/// [`COMMENT_REQUIRED`] atau [`COMMENT_TOO_LONG`].
pub fn validate_body(raw: &str) -> Result<String, ValidationError> {
    let body = raw.trim();
    if body.is_empty() {
        return Err(ValidationError::new(COMMENT_REQUIRED));
    }
    if body.chars().count() > BODY_MAX_LEN {
        let mut e = ValidationError::new(COMMENT_TOO_LONG);
        e.add_param("max".into(), &BODY_MAX_LEN);
        return Err(e);
    }
    Ok(body.to_string())
}

impl Model {
    /// Komentar tiket, terlama dulu.
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

    /// # Errors
    ///
    /// Error database.
    pub async fn find_in<C: ConnectionTrait>(
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

    /// Ganti isi (sudah divalidasi), tandai diedit.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn edit<C: ConnectionTrait>(self, db: &C, body: String) -> Result<Self, DbErr> {
        let mut model = self.into_active_model();
        model.body = Set(body);
        model.edited_at = Set(Some(chrono::Utc::now().into()));
        model.update(db).await
    }
}

impl ActiveModel {
    /// # Errors
    ///
    /// Error database.
    pub async fn post<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        ticket_id: Uuid,
        author_id: Uuid,
        body: String,
    ) -> Result<Model, DbErr> {
        Self {
            workspace_id: Set(workspace_id.get()),
            ticket_id: Set(ticket_id),
            author_id: Set(Some(author_id)),
            body: Set(body),
            ..Default::default()
        }
        .insert(db)
        .await
    }
}
