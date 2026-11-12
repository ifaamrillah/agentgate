//! Komentar tiket. Menulis komentar butuh `EditProject`; mengubah dan menghapus hanya komentar
//! sendiri (penulis). Komentar baru: penulis otomatis mengikuti tiket, orang yang disebut
//! (@mention, `mentions`) mendapat notifikasi `mentioned`, dan watcher lain `commented`.

use sea_orm::{DatabaseConnection, ModelTrait};
use uuid::Uuid;

use super::{TicketFailure, Working, mentions, notify};
use crate::{
    models::{
        notifications::NotificationKind,
        permission::Permission,
        ticket_comments::{self, validate_body},
        ticket_watchers,
    },
    services::workspaces::Acting,
};

/// # Errors
///
/// [`TicketFailure::Invalid`] (`body`), `NotFound`, atau error database.
pub async fn post(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    ticket_id: Uuid,
    body: &str,
) -> Result<ticket_comments::Model, TicketFailure> {
    let body = validate_body(body).map_err(|e| {
        let mut errors = validator::ValidationErrors::new();
        errors.add("body", e);
        TicketFailure::errors(errors)
    })?;
    let work = Working::begin(db, acting, ticket_id, Permission::EditProject, false).await?;
    let comment =
        ticket_comments::ActiveModel::post(&work.txn, work.wid, ticket_id, acting.user.id, body)
            .await?;
    ticket_watchers::ActiveModel::watch(&work.txn, work.wid, ticket_id, acting.user.id).await?;
    let mentioned =
        mentions::mentioned(&work.txn, work.wid, work.project.id, &comment.body).await?;
    for user in &mentioned {
        notify(
            &work.txn,
            work.wid,
            acting.user.id,
            user.id,
            NotificationKind::Mentioned,
            ticket_id,
            Some(comment.id),
        )
        .await?;
    }
    for watcher in ticket_watchers::Model::user_ids(&work.txn, work.wid, ticket_id).await? {
        if !mentioned.iter().any(|u| u.id == watcher) {
            notify(
                &work.txn,
                work.wid,
                acting.user.id,
                watcher,
                NotificationKind::Commented,
                ticket_id,
                Some(comment.id),
            )
            .await?;
        }
    }
    work.txn.commit().await?;
    Ok(comment)
}

/// Ubah isi komentar sendiri.
///
/// # Errors
///
/// [`TicketFailure::Invalid`], `NotFound` (bukan penulis, komentar tiket lain), atau error
/// database.
pub async fn edit(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    ticket_id: Uuid,
    comment_id: Uuid,
    body: &str,
) -> Result<ticket_comments::Model, TicketFailure> {
    let body = validate_body(body).map_err(|e| {
        let mut errors = validator::ValidationErrors::new();
        errors.add("body", e);
        TicketFailure::errors(errors)
    })?;
    let work = Working::begin(db, acting, ticket_id, Permission::EditProject, false).await?;
    let comment = own_comment(&work, acting, comment_id).await?;
    let comment = comment.edit(&work.txn, body).await?;
    work.txn.commit().await?;
    Ok(comment)
}

/// Hapus komentar sendiri.
///
/// # Errors
///
/// `NotFound` atau error database.
pub async fn delete(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    ticket_id: Uuid,
    comment_id: Uuid,
) -> Result<(), TicketFailure> {
    let work = Working::begin(db, acting, ticket_id, Permission::EditProject, false).await?;
    let comment = own_comment(&work, acting, comment_id).await?;
    comment.delete(&work.txn).await?;
    work.txn.commit().await?;
    Ok(())
}

/// Satu-satunya aturan "boleh mengubah komentar ini": komentar tiket ini milik pelaku.
async fn own_comment(
    work: &Working,
    acting: &Acting<'_>,
    comment_id: Uuid,
) -> Result<ticket_comments::Model, TicketFailure> {
    ticket_comments::Model::find_in(&work.txn, work.wid, work.ticket.id, comment_id)
        .await?
        .filter(|c| c.author_id == Some(acting.user.id))
        .ok_or(TicketFailure::NotFound)
}
