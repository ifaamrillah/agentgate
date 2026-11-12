//! Lampiran tiket (rules §8 storage): file di `ctx.storage` (S3/SeaweedFS di dev, memori di tes)
//! dengan key `tickets/<ticket>/<uuid>`, database menyimpan key, nama, tipe, dan ukuran.

use loco_rs::model::{TenantEntity, TenantQueryExt};
use sea_orm::{ActiveValue::Set, QueryOrder, entity::prelude::*};

pub use super::_entities::ticket_attachments::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

pub const ATTACHMENT_EMPTY: &str = "ticket-attachment-empty";
pub const ATTACHMENT_TOO_LARGE: &str = "ticket-attachment-too-large";
pub const ERROR_CODES: &[&str] = &[ATTACHMENT_EMPTY, ATTACHMENT_TOO_LARGE];
/// Ukuran maksimal satu lampiran (10 MB).
pub const MAX_BYTES: usize = 10 * 1024 * 1024;

impl TenantEntity for Entity {
    type TenantId = WorkspaceId;

    fn tenant_column() -> Column {
        Column::WorkspaceId
    }
}

standard_behavior!(ActiveModel);

/// Data lampiran baru.
#[derive(Debug, Clone)]
pub struct NewAttachment {
    pub ticket_id: Uuid,
    pub uploader_id: Uuid,
    pub filename: String,
    pub content_type: String,
    pub size: i64,
    pub storage_key: String,
}

/// Nama file aman untuk disimpan dan untuk header `Content-Disposition`: hanya bagian setelah
/// `/` atau `\` terakhir, karakter kontrol dan kutip dibuang, maksimal 120 karakter.
#[must_use]
pub fn sanitize_filename(raw: &str) -> String {
    let base = raw.rsplit(['/', '\\']).next().unwrap_or("");
    let cleaned: String = base
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, '"' | ';'))
        .take(120)
        .collect();
    let cleaned = cleaned.trim().to_string();
    if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
        "lampiran".to_string()
    } else {
        cleaned
    }
}

impl Model {
    /// Lampiran tiket, terlama dulu.
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

    /// Key storage lampiran milik tiket-tiket `ticket_ids` (untuk dihapus setelah purge).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn keys_of<C: ConnectionTrait>(
        db: &C,
        ticket_ids: &[Uuid],
    ) -> Result<Vec<String>, DbErr> {
        Ok(Entity::find()
            .filter(Column::TicketId.is_in(ticket_ids.iter().copied()))
            .all(db)
            .await?
            .into_iter()
            .map(|a| a.storage_key)
            .collect())
    }
}

impl ActiveModel {
    /// # Errors
    ///
    /// Error database.
    pub async fn add<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        new: NewAttachment,
    ) -> Result<Model, DbErr> {
        Self {
            workspace_id: Set(workspace_id.get()),
            ticket_id: Set(new.ticket_id),
            uploader_id: Set(Some(new.uploader_id)),
            filename: Set(new.filename),
            content_type: Set(new.content_type),
            size: Set(new.size),
            storage_key: Set(new.storage_key),
            ..Default::default()
        }
        .insert(db)
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filenames_are_sanitized() {
        assert_eq!(sanitize_filename("../../etc/passwd"), "passwd");
        assert_eq!(sanitize_filename("C:\\a\\b.log"), "b.log");
        assert_eq!(sanitize_filename("a\"b;c\n.txt"), "abc.txt");
        assert_eq!(sanitize_filename(""), "lampiran");
        assert_eq!(sanitize_filename(".."), "lampiran");
    }
}
