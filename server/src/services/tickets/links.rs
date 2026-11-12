//! Relasi tiket (blocks, blocked by, duplicates, relates to). Hanya antar tiket satu project
//! yang tidak di sampah. Menambah/menghapus relasi tercatat di riwayat kedua tiket.

use sea_orm::{DatabaseConnection, EntityTrait, ModelTrait};
use uuid::Uuid;

use super::{TicketFailure, Working};
use crate::{
    models::{
        permission::Permission,
        ticket_events::{self, EventKind},
        ticket_links::{self, LINK_EXISTS, LINK_KIND_INVALID, LINK_TARGET_INVALID, LinkChoice},
        tickets,
    },
    services::workspaces::Acting,
};

/// Tambah relasi dari tiket `ticket_id` ke tiket berkode `target_code`.
///
/// # Errors
///
/// [`TicketFailure::Invalid`] (`kind`/`target`), `NotFound`, atau error database.
pub async fn add(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    ticket_id: Uuid,
    kind: &str,
    target_code: &str,
) -> Result<ticket_links::Model, TicketFailure> {
    let choice = LinkChoice::parse(kind.trim())
        .ok_or_else(|| TicketFailure::field("kind", LINK_KIND_INVALID))?;
    let work = Working::begin(db, acting, ticket_id, Permission::EditProject, false).await?;
    let target = tickets::Model::find_by_code(&work.txn, work.wid, target_code)
        .await?
        .map(|(t, _)| t)
        .filter(|t| t.id != ticket_id && t.project_id == work.ticket.project_id && !t.is_trashed())
        .ok_or_else(|| TicketFailure::field("target", LINK_TARGET_INVALID))?;
    let (kind, this_is_from) = choice.stored();
    let (from, to) = if this_is_from {
        (ticket_id, target.id)
    } else {
        (target.id, ticket_id)
    };
    let link = ticket_links::ActiveModel::link(&work.txn, work.wid, from, to, kind, acting.user.id)
        .await?
        .ok_or_else(|| TicketFailure::field("target", LINK_EXISTS))?;
    record_both(&work, acting, &link, &target, EventKind::Linked).await?;
    work.txn.commit().await?;
    Ok(link)
}

/// Hapus relasi `link_id` yang menyentuh tiket `ticket_id`.
///
/// # Errors
///
/// `NotFound` atau error database.
pub async fn remove(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    ticket_id: Uuid,
    link_id: Uuid,
) -> Result<(), TicketFailure> {
    let work = Working::begin(db, acting, ticket_id, Permission::EditProject, false).await?;
    let link = ticket_links::Entity::find_by_id(link_id)
        .one(&work.txn)
        .await?
        .filter(|l| {
            l.workspace_id == work.wid.get() && (l.from_id == ticket_id || l.to_id == ticket_id)
        })
        .ok_or(TicketFailure::NotFound)?;
    let other_id = if link.from_id == ticket_id {
        link.to_id
    } else {
        link.from_id
    };
    let other = tickets::Model::find_in(&work.txn, work.wid, other_id)
        .await?
        .ok_or(TicketFailure::NotFound)?;
    record_both(&work, acting, &link, &other, EventKind::Unlinked).await?;
    link.delete(&work.txn).await?;
    work.txn.commit().await?;
    Ok(())
}

async fn record_both(
    work: &Working,
    acting: &Acting<'_>,
    link: &ticket_links::Model,
    other: &tickets::Model,
    kind: EventKind,
) -> Result<(), TicketFailure> {
    let data = |this: &tickets::Model, that: &tickets::Model| {
        serde_json::json!({
            "kind": link.kind,
            "direction": if link.from_id == this.id { "out" } else { "in" },
            "code": that.code,
        })
    };
    work.event(acting.user.id, kind, data(&work.ticket, other))
        .await?;
    ticket_events::ActiveModel::record(
        &work.txn,
        work.wid,
        other.id,
        Some(acting.user.id),
        kind,
        data(other, &work.ticket),
    )
    .await?;
    Ok(())
}
