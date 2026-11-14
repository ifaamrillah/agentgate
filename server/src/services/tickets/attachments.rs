//! Lampiran tiket di storage S3 (SeaweedFS di dev, memori di tes; rules §8).
//!
//! Urutan tulis seperti logo project: upload dulu, lalu baris DB + riwayat di transaksi; bila
//! transaksi gagal, file baru dihapus. Hapus lampiran: baris dulu (commit), file sesudahnya
//! (best-effort: gagal hapus file hanya dicatat di log, karena baris sudah tidak menunjuknya).

use std::path::Path;

use axum::body::Bytes;
use loco_rs::storage::Storage;
use sea_orm::{DatabaseConnection, ModelTrait};
use uuid::Uuid;

use super::{TicketFailure, Working};
use crate::models::{
    permission::Permission,
    ticket_attachments::{self, ATTACHMENT_EMPTY, ATTACHMENT_TOO_LARGE, MAX_BYTES, NewAttachment},
    ticket_events::EventKind,
};
use crate::services::workspaces::Acting;

/// Content type yang disimpan: dari header unggahan bila berbentuk `type/subtype` sederhana,
/// selain itu `application/octet-stream`. Unduhan selalu `attachment` + `nosniff`, jadi tipe ini
/// tidak pernah membuat browser menjalankan isi berkas.
#[must_use]
pub fn content_type(declared: Option<&str>) -> String {
    let declared = declared.unwrap_or("").trim().to_ascii_lowercase();
    let ok = declared.len() <= 100
        && declared.split_once('/').is_some_and(|(a, b)| {
            !a.is_empty()
                && !b.is_empty()
                && declared
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'/' | b'.' | b'+' | b'-'))
        });
    if ok {
        declared
    } else {
        "application/octet-stream".to_string()
    }
}

/// Hapus file di storage; kegagalan hanya dicatat.
pub async fn remove_file_quietly(storage: &Storage, key: &str) {
    if let Err(e) = storage.delete(Path::new(key)).await {
        tracing::warn!(error = %e, key, "gagal menghapus file lampiran");
    }
}

/// Unggah lampiran ke tiket.
///
/// # Errors
///
/// [`TicketFailure::Invalid`] (`file` kosong atau terlalu besar), `NotFound`, error storage
/// (sebagai `Db` custom), atau error database.
pub async fn upload(
    db: &DatabaseConnection,
    storage: &Storage,
    acting: &Acting<'_>,
    ticket_id: Uuid,
    filename: &str,
    declared_type: Option<&str>,
    data: Bytes,
) -> Result<ticket_attachments::Model, TicketFailure> {
    upload_limited(
        db,
        storage,
        acting,
        ticket_id,
        filename,
        declared_type,
        data,
        MAX_BYTES,
    )
    .await
}

/// [`upload`] dengan batas ukuran `max_bytes` sendiri (impor Jira memakai batas 25 MB spec,
/// lebih besar dari batas unggah form).
///
/// # Errors
///
/// Sama dengan [`upload`].
#[allow(clippy::too_many_arguments)]
pub async fn upload_limited(
    db: &DatabaseConnection,
    storage: &Storage,
    acting: &Acting<'_>,
    ticket_id: Uuid,
    filename: &str,
    declared_type: Option<&str>,
    data: Bytes,
    max_bytes: usize,
) -> Result<ticket_attachments::Model, TicketFailure> {
    if data.is_empty() {
        return Err(TicketFailure::field("file", ATTACHMENT_EMPTY));
    }
    if data.len() > max_bytes {
        let mut errors = validator::ValidationErrors::new();
        let mut e = validator::ValidationError::new(ATTACHMENT_TOO_LARGE);
        e.add_param("max".into(), &(max_bytes / 1024 / 1024));
        errors.add("file", e);
        return Err(TicketFailure::errors(errors));
    }
    let work = Working::begin(db, acting, ticket_id, Permission::EditProject, false).await?;
    let key = format!("tickets/{ticket_id}/{}", Uuid::now_v7());
    storage
        .upload(Path::new(&key), &data)
        .await
        .map_err(|e| TicketFailure::Db(sea_orm::DbErr::Custom(format!("storage: {e}"))))?;
    let result = async {
        let attachment = ticket_attachments::ActiveModel::add(
            &work.txn,
            work.wid,
            NewAttachment {
                ticket_id,
                uploader_id: acting.user.id,
                filename: ticket_attachments::sanitize_filename(filename),
                content_type: content_type(declared_type),
                size: i64::try_from(data.len()).unwrap_or(i64::MAX),
                storage_key: key.clone(),
            },
        )
        .await?;
        work.event(
            acting.user.id,
            EventKind::AttachmentAdded,
            serde_json::json!({ "filename": attachment.filename, "size": attachment.size }),
        )
        .await?;
        Ok::<_, sea_orm::DbErr>(attachment)
    }
    .await;
    match result {
        Ok(attachment) => match work.txn.commit().await {
            Ok(()) => Ok(attachment),
            Err(e) => {
                remove_file_quietly(storage, &key).await;
                Err(e.into())
            }
        },
        Err(e) => {
            remove_file_quietly(storage, &key).await;
            Err(e.into())
        }
    }
}

/// Hapus lampiran `attachment_id` dari tiket.
///
/// # Errors
///
/// `NotFound` atau error database.
pub async fn remove(
    db: &DatabaseConnection,
    storage: &Storage,
    acting: &Acting<'_>,
    ticket_id: Uuid,
    attachment_id: Uuid,
) -> Result<(), TicketFailure> {
    let work = Working::begin(db, acting, ticket_id, Permission::EditProject, false).await?;
    let attachment =
        ticket_attachments::Model::find_in(&work.txn, work.wid, ticket_id, attachment_id)
            .await?
            .ok_or(TicketFailure::NotFound)?;
    let key = attachment.storage_key.clone();
    work.event(
        acting.user.id,
        EventKind::AttachmentRemoved,
        serde_json::json!({ "filename": attachment.filename }),
    )
    .await?;
    attachment.delete(&work.txn).await?;
    work.txn.commit().await?;
    remove_file_quietly(storage, &key).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_type_is_sanitized() {
        assert_eq!(content_type(Some("image/png")), "image/png");
        assert_eq!(
            content_type(Some("text/html; charset=x")),
            "application/octet-stream"
        );
        assert_eq!(content_type(None), "application/octet-stream");
    }
}
