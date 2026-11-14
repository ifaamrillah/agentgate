//! Sampah tiket (spec: "Hapus tiket masuk sampah dulu, lalu terhapus permanen setelah 30 hari").
//!
//! - Pindah ke sampah dan pulihkan: `EditProject`; dicatat di riwayat dan audit log.
//! - Hapus permanen dari sampah: Admin project atau pembuat tiket ([`can_purge`], satu aturan);
//!   dicatat di audit log (tiketnya sendiri hilang beserta riwayatnya).
//! - [`purge_expired`]: langkah job `cleanup`, menghapus tiket yang di sampah lebih dari
//!   [`RETENTION_DAYS`] hari. File lampiran dihapus setelah commit (best-effort; key yang gagal
//!   dihapus hanya dicatat di log).

use chrono::{DateTime, Duration, Utc};
use loco_rs::storage::Storage;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait,
    IntoActiveModel, ModelTrait, QueryFilter, QueryOrder, QuerySelect, TransactionTrait,
};
use uuid::Uuid;

use super::{TicketFailure, Working, attachments::remove_file_quietly};
use crate::{
    models::{
        permission::Permission, project_access::EffectiveRole, ticket_attachments,
        ticket_events::EventKind, tickets,
    },
    services::{
        audit::{self, AuditAction, AuditSubject},
        workspaces::Acting,
    },
};

/// Lama tiket disimpan di sampah sebelum dihapus permanen.
pub const RETENTION_DAYS: i64 = 30;
/// Tiket per transaksi purge.
const PURGE_BATCH: u64 = 200;

/// Satu-satunya aturan hapus permanen: Admin project, atau pembuat tiket yang masih boleh
/// mengubah project.
#[must_use]
pub fn can_purge(access: &EffectiveRole, ticket: &tickets::Model, user_id: Uuid) -> bool {
    access.has(Permission::AdministerProject)
        || (access.has(Permission::EditProject) && ticket.created_by == Some(user_id))
}

/// Pindahkan tiket ke sampah.
///
/// # Errors
///
/// `NotFound` atau error database.
pub async fn trash(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    ticket_id: Uuid,
) -> Result<tickets::Model, TicketFailure> {
    let work = Working::begin(db, acting, ticket_id, Permission::EditProject, false).await?;
    let mut model = work.ticket.clone().into_active_model();
    model.deleted_at = Set(Some(Utc::now().into()));
    model.deleted_by = Set(Some(acting.user.id));
    let ticket = model.update(&work.txn).await?;
    work.event(acting.user.id, EventKind::Trashed, serde_json::json!({}))
        .await?;
    audit::record(
        &work.txn,
        acting.actor().audit(
            AuditAction::TicketTrashed,
            (AuditSubject::Ticket, ticket.id),
            serde_json::json!({ "code": ticket.code, "title": ticket.title }),
        ),
    )
    .await?;
    work.txn.commit().await?;
    Ok(ticket)
}

/// Pulihkan tiket dari sampah.
///
/// # Errors
///
/// `NotFound` atau error database.
pub async fn restore(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    ticket_id: Uuid,
) -> Result<tickets::Model, TicketFailure> {
    let work = Working::begin(db, acting, ticket_id, Permission::EditProject, true).await?;
    let mut model = work.ticket.clone().into_active_model();
    model.deleted_at = Set(None);
    model.deleted_by = Set(None);
    model.position = Set(tickets::bottom_position());
    let ticket = model.update(&work.txn).await?;
    work.event(acting.user.id, EventKind::Restored, serde_json::json!({}))
        .await?;
    audit::record(
        &work.txn,
        acting.actor().audit(
            AuditAction::TicketRestored,
            (AuditSubject::Ticket, ticket.id),
            serde_json::json!({ "code": ticket.code, "title": ticket.title }),
        ),
    )
    .await?;
    work.txn.commit().await?;
    Ok(ticket)
}

/// Hapus permanen satu tiket dari sampah.
///
/// # Errors
///
/// `NotFound` (bukan di sampah, atau pelaku bukan Admin project/pembuat), atau error database.
pub async fn purge(
    db: &DatabaseConnection,
    storage: &Storage,
    acting: &Acting<'_>,
    ticket_id: Uuid,
) -> Result<(), TicketFailure> {
    let work = Working::begin(db, acting, ticket_id, Permission::ViewProject, true).await?;
    if !can_purge(&work.access, &work.ticket, acting.user.id) {
        return Err(TicketFailure::NotFound);
    }
    let mut keys = ticket_attachments::Model::keys_of(&work.txn, &[ticket_id]).await?;
    keys.extend(crate::models::manual_evidence::Model::keys_of(&work.txn, &[ticket_id]).await?);
    audit::record(
        &work.txn,
        acting.actor().audit(
            AuditAction::TicketPurged,
            (AuditSubject::Ticket, ticket_id),
            serde_json::json!({ "code": work.ticket.code, "title": work.ticket.title }),
        ),
    )
    .await?;
    work.ticket.clone().delete(&work.txn).await?;
    work.txn.commit().await?;
    for key in keys {
        remove_file_quietly(storage, &key).await;
    }
    Ok(())
}

/// Hapus permanen tiket yang masuk sampah sebelum `now - RETENTION_DAYS`. `apply == false` hanya
/// menghitung. Satu transaksi per batch; mengembalikan jumlah tiket.
///
/// # Errors
///
/// Error database.
pub async fn purge_expired(
    db: &DatabaseConnection,
    storage: &Storage,
    now: DateTime<Utc>,
    apply: bool,
) -> Result<u64, sea_orm::DbErr> {
    let cutoff = now - Duration::days(RETENTION_DAYS);
    let expired = || tickets::Entity::find().filter(tickets::Column::DeletedAt.lt(cutoff));
    if !apply {
        return sea_orm::PaginatorTrait::count(expired(), db).await;
    }
    let mut total = 0;
    loop {
        let txn = db.begin().await?;
        let ids: Vec<Uuid> = expired()
            .select_only()
            .column(tickets::Column::Id)
            .order_by_asc(tickets::Column::DeletedAt)
            .limit(PURGE_BATCH)
            .lock_exclusive()
            .into_tuple()
            .all(&txn)
            .await?;
        if ids.is_empty() {
            txn.rollback().await?;
            break;
        }
        let mut keys = ticket_attachments::Model::keys_of(&txn, &ids).await?;
        keys.extend(crate::models::manual_evidence::Model::keys_of(&txn, &ids).await?);
        let deleted = tickets::Entity::delete_many()
            .filter(tickets::Column::Id.is_in(ids.iter().copied()))
            .exec(&txn)
            .await?
            .rows_affected;
        txn.commit().await?;
        for key in keys {
            remove_file_quietly(storage, &key).await;
        }
        total += deleted;
        if deleted < PURGE_BATCH {
            break;
        }
    }
    Ok(total)
}
