//! Ubah tiket dari form detail: semua field sekaligus. Perubahan status lewat jalur ini ikut
//! aturan *blocks* yang sama dengan board (`ensure_can_move`). Tanpa perubahan: tidak menulis
//! apa pun. Penerima tugas baru mendapat notifikasi dan otomatis mengikuti tiket.

use sea_orm::{
    ActiveEnum, ActiveModelTrait, ActiveValue::Set, DatabaseConnection, IntoActiveModel,
};
use uuid::Uuid;

use super::{
    TicketFailure, Working,
    create::{TicketForm, resolve},
    ensure_can_move, mentions, notify, watch_if_member,
};
use crate::{
    models::{
        notifications::NotificationKind, permission::Permission, ticket_events::EventKind, tickets,
    },
    services::{audit, workspaces::Acting},
};

/// # Errors
///
/// [`TicketFailure::Invalid`] (termasuk `ticket-blocked`), [`TicketFailure::NotFound`], atau
/// error database.
pub async fn run(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    ticket_id: Uuid,
    form: &TicketForm,
) -> Result<tickets::Model, TicketFailure> {
    let work = Working::begin(db, acting, ticket_id, Permission::EditProject, false).await?;
    let before = work.ticket.clone();
    let resolved = resolve(&work.txn, work.wid, &work.project, form, Some(&before)).await?;
    let valid = &resolved.valid;
    ensure_can_move(&work.txn, work.wid, &before, valid.status).await?;

    let mut changes = serde_json::Map::new();
    let mut put = |key: &str, change: Option<serde_json::Value>| {
        if let Some(c) = change {
            changes.insert(key.to_string(), c);
        }
    };
    put("title", audit::change(&before.title, &valid.title));
    put(
        "description",
        (before.description != valid.description).then(|| serde_json::json!({})),
    );
    put(
        "status",
        audit::change(&before.status, &valid.status.to_value()),
    );
    put(
        "priority",
        audit::change(&before.priority, &valid.priority.to_value()),
    );
    put(
        "type_id",
        audit::change(&before.type_id, &resolved.ticket_type.id),
    );
    put(
        "assignee_id",
        audit::change(&before.assignee_id, &resolved.assignee_id),
    );
    put(
        "parent_id",
        audit::change(&before.parent_id, &resolved.parent_id),
    );
    put("labels", audit::change(&before.labels, &valid.labels));
    put("due_on", audit::change(&before.due_on, &valid.due_on));
    put(
        "story_points",
        audit::change(&before.story_points, &valid.story_points),
    );
    put(
        "custom_values",
        (before.custom_values() != resolved.custom).then(|| serde_json::json!({})),
    );
    if changes.is_empty() {
        work.txn.rollback().await?;
        return Ok(before);
    }

    let mut model = before.clone().into_active_model();
    model.title = Set(valid.title.clone());
    model.description = Set(valid.description.clone());
    model.priority = Set(valid.priority.to_value());
    model.type_id = Set(resolved.ticket_type.id);
    model.assignee_id = Set(resolved.assignee_id);
    model.parent_id = Set(resolved.parent_id);
    model.labels = Set(valid.labels.clone());
    model.due_on = Set(valid.due_on);
    model.story_points = Set(valid.story_points);
    model.custom_values = Set(resolved.custom.to_json());
    let updated = model.update(&work.txn).await?;
    let updated = if updated.status == valid.status.to_value() {
        updated
    } else {
        updated
            .set_status(&work.txn, valid.status, tickets::bottom_position())
            .await?
    };
    work.event(
        acting.user.id,
        if changes.len() == 1 && changes.contains_key("status") {
            EventKind::StatusChanged
        } else {
            EventKind::Updated
        },
        serde_json::Value::Object(changes),
    )
    .await?;
    if let Some(assignee) = resolved
        .assignee_id
        .filter(|a| before.assignee_id != Some(*a))
    {
        watch_if_member(&work.txn, work.wid, ticket_id, assignee).await?;
        notify(
            &work.txn,
            work.wid,
            acting.user.id,
            assignee,
            NotificationKind::Assigned,
            ticket_id,
            None,
        )
        .await?;
    }
    if before.description != valid.description {
        let old =
            mentions::mentioned(&work.txn, work.wid, work.project.id, &before.description).await?;
        for user in
            mentions::mentioned(&work.txn, work.wid, work.project.id, &valid.description).await?
        {
            if !old.iter().any(|o| o.id == user.id) {
                notify(
                    &work.txn,
                    work.wid,
                    acting.user.id,
                    user.id,
                    NotificationKind::Mentioned,
                    ticket_id,
                    None,
                )
                .await?;
            }
        }
    }
    work.txn.commit().await?;
    Ok(updated)
}
