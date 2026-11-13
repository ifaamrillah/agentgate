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
    ensure_can_move, mentions, notify, stages, watch_if_member,
};
use crate::{
    models::{
        notifications::NotificationKind, permission::Permission, project_workflows,
        ticket_events::EventKind, tickets,
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
    let names = crate::services::planning::refs::names(
        &work.txn,
        work.wid,
        [before.sprint_id, resolved.sprint_id],
        [before.release_id, resolved.release_id],
        [before.component_id, resolved.component_id],
    )
    .await?;
    put("sprint", names.sprint);
    put("release", names.release);
    put("component", names.component);
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
    model.sprint_id = Set(resolved.sprint_id);
    model.release_id = Set(resolved.release_id);
    model.component_id = Set(resolved.component_id);
    let mut updated = model.update(&work.txn).await?;
    crate::services::planning::scope::sprint_changed(
        &work.txn,
        work.wid,
        &updated,
        before.sprint_id,
        updated.sprint_id,
    )
    .await?;
    if before.story_points != valid.story_points {
        crate::services::planning::scope::points_changed(
            &work.txn,
            work.wid,
            &updated,
            valid.story_points,
        )
        .await?;
    }
    if before.type_id != resolved.ticket_type.id {
        let workflow_id = project_workflows::Model::effective(
            &work.txn,
            work.wid,
            work.project.id,
            resolved.ticket_type.id,
        )
        .await?;
        let (ticket, moved) =
            stages::follow_workflow(&work.txn, work.wid, updated, workflow_id).await?;
        updated = ticket;
        if let Some((from, to)) = moved {
            changes.insert(
                "stage".into(),
                serde_json::json!({ "from": from, "to": to }),
            );
        }
    }
    if updated.status != valid.status.to_value() {
        let flow = stages::Flow::load(&work.txn, work.wid, &updated).await?;
        let to = stages::category(&valid.status)
            .and_then(|c| flow.current.first_in(c))
            .cloned()
            .ok_or_else(|| TicketFailure::field("status", stages::STAGE_UNKNOWN))?;
        stages::check_move(&work.txn, work.wid, &updated, &flow, &to).await?;
        let from = flow
            .rule(&updated.stage_key)
            .map_or_else(|| updated.stage_key.clone(), |s| s.name.clone());
        updated = stages::enter_stage(&work.txn, work.wid, updated, &flow, &to).await?;
        changes.insert(
            "stage".into(),
            serde_json::json!({ "from": from, "to": to.name }),
        );
    }
    let status_only = changes.keys().all(|k| k == "status" || k == "stage");
    work.event(
        acting.user.id,
        if status_only {
            EventKind::StageChanged
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
