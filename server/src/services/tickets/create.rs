//! Buat tiket, dan resolusi isian form tiket yang dipakai bersama dengan [`super::update`].
//!
//! Nomor dan kode: penghitung atomik (`ticket_counters::next_number`) menurut skop template yang
//! berlaku (override project, lalu workspace). Bila kode hasilnya sudah dipakai (alias lama dari
//! template sebelumnya, atau skop yang baru diganti), nomor berikutnya diambil, paling banyak
//! [`MAX_CODE_ATTEMPTS`] kali.

use std::collections::BTreeMap;

use chrono::Datelike;
use loco_rs::validation::ModelValidationErrors;
use sea_orm::{
    ActiveEnum, ActiveModelTrait, ActiveValue::Set, ConnectionTrait, DatabaseConnection,
    DatabaseTransaction, DbErr, EntityTrait, TransactionTrait,
};
use uuid::Uuid;
use validator::{ValidationError, ValidationErrors};

use agentgate_core::workflow::Definition;

use super::{TicketFailure, access_in, mentions, notify, stages, watch_if_member};
use crate::{
    models::{
        custom_fields,
        notifications::NotificationKind,
        permission::Permission,
        project_access, project_workflows, projects,
        ticket_code::{self, CodeParts, CounterScope},
        ticket_codes, ticket_counters,
        ticket_events::{self, EventKind},
        ticket_type_fields, ticket_types, ticket_watchers,
        tickets::{self, CustomValues, TicketInput, TicketStatus, ValidTicket},
        workflows,
        workspaces::{self, WorkspaceId},
    },
    services::workspaces::Acting,
};

/// Batas percobaan mencari kode yang belum dipakai.
pub const MAX_CODE_ATTEMPTS: usize = 50;

/// Isian form tiket mentah (buat dan ubah).
#[derive(Debug, Clone, Default)]
pub struct TicketForm {
    pub input: TicketInput,
    /// Id tipe (UUID); kosong = Task.
    pub type_id: String,
    /// Id user penerima tugas; kosong = belum ada.
    pub assignee: String,
    /// Kode tiket induk (subtask/epic); kosong = tanpa induk.
    pub parent: String,
    /// Nilai field custom mentah per id field.
    pub custom: BTreeMap<Uuid, String>,
    /// Kunci stage awal (hanya saat membuat); kosong = stage pertama berkategori `status`.
    pub stage: String,
    /// Planning (M9): id sprint, rilis (Fix version), component. `None` = tidak diubah (ubah
    /// tiket) atau kosong (buat); `Some("")` = dikosongkan.
    pub sprint: Option<String>,
    pub release: Option<String>,
    pub component: Option<String>,
}

/// Isian yang sudah divalidasi dan di-resolve ke baris database.
#[derive(Debug, Clone)]
pub(crate) struct Resolved {
    pub valid: ValidTicket,
    pub ticket_type: ticket_types::Model,
    pub assignee_id: Option<Uuid>,
    pub parent_id: Option<Uuid>,
    pub custom: CustomValues,
    pub sprint_id: Option<Uuid>,
    pub release_id: Option<Uuid>,
    pub component_id: Option<Uuid>,
}

/// Validasi semua isian sekaligus. `current` = tiket yang diubah (induk tidak boleh dirinya).
pub(crate) async fn resolve<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project: &projects::Model,
    form: &TicketForm,
    current: Option<&tickets::Model>,
) -> Result<Resolved, TicketFailure> {
    let mut errors = match tickets::validate_input(&form.input) {
        Ok(_) => ValidationErrors::new(),
        Err(e) => e,
    };
    let types = ticket_types::Model::all_in(db, wid).await?;
    let ticket_type = match form.type_id.trim() {
        "" => types
            .iter()
            .find(|t| t.key == "task")
            .or(types.first())
            .cloned(),
        raw => Uuid::parse_str(raw)
            .ok()
            .and_then(|id| types.iter().find(|t| t.id == id).cloned()),
    };
    if ticket_type.is_none() {
        errors.add("type_id", ValidationError::new(tickets::TYPE_INVALID));
    }
    let assignee_id = match form.assignee.trim() {
        "" => None,
        raw => {
            let id = Uuid::parse_str(raw).ok();
            let allowed = match id {
                Some(id) => {
                    current.is_some_and(|t| t.assignee_id == Some(id))
                        || has_access(db, wid, project.id, id).await?
                }
                None => false,
            };
            if !allowed {
                errors.add("assignee", ValidationError::new(tickets::ASSIGNEE_INVALID));
            }
            id
        }
    };
    let parent_id = match form.parent.trim() {
        "" => None,
        code => {
            let parent = tickets::Model::find_by_code(db, wid, code)
                .await?
                .map(|(t, _)| t)
                .filter(|t| {
                    t.project_id == project.id
                        && !t.is_trashed()
                        && current.is_none_or(|c| c.id != t.id && t.parent_id != Some(c.id))
                });
            if parent.is_none() {
                errors.add("parent", ValidationError::new(tickets::PARENT_INVALID));
            }
            parent.map(|t| t.id)
        }
    };
    let pick = |raw: &Option<String>, now: Option<Uuid>| -> Result<Option<Uuid>, ()> {
        match raw.as_deref().map(str::trim) {
            None => Ok(now),
            Some("") => Ok(None),
            Some(v) => Uuid::parse_str(v).map(Some).map_err(|_| ()),
        }
    };
    let planning = crate::services::planning::refs::check(
        db,
        wid,
        project.id,
        pick(&form.sprint, current.and_then(|t| t.sprint_id)),
        pick(&form.release, current.and_then(|t| t.release_id)),
        pick(&form.component, current.and_then(|t| t.component_id)),
        current,
    )
    .await?;
    for (field, ok) in [
        ("sprint", planning.sprint.is_ok()),
        ("release", planning.release.is_ok()),
        ("component", planning.component.is_ok()),
    ] {
        if !ok {
            errors.add(
                field,
                ValidationError::new(crate::services::planning::TARGET_INVALID),
            );
        }
    }
    let mut custom = CustomValues::default();
    let mut custom_errors: Vec<(String, ValidationError)> = Vec::new();
    if let Some(ticket_type) = &ticket_type {
        for field in ticket_type_fields::Model::fields_of(db, wid, ticket_type.id).await? {
            let raw = form.custom.get(&field.id).map_or("", String::as_str);
            match field.parse_value(raw) {
                Ok(Some(value)) => {
                    custom.0.insert(field.id, value);
                }
                Ok(None) if field.required => custom_errors.push((
                    custom_field_key(field.id),
                    ValidationError::new(custom_fields::FIELD_VALUE_REQUIRED),
                )),
                Ok(None) => {}
                Err(e) => custom_errors.push((custom_field_key(field.id), e)),
            }
        }
    }
    if !errors.is_empty() || !custom_errors.is_empty() {
        let mut all: ModelValidationErrors = errors.into();
        for (key, e) in custom_errors {
            all.errors
                .entry(key)
                .or_default()
                .push(loco_rs::validation::ValidationError {
                    code: e.code.to_string(),
                    message: None,
                    params: e
                        .params
                        .iter()
                        .map(|(k, v)| (k.to_string(), v.clone()))
                        .collect(),
                });
        }
        return Err(TicketFailure::Invalid(all));
    }
    let valid = tickets::validate_input(&form.input).map_err(TicketFailure::errors)?;
    Ok(Resolved {
        valid,
        ticket_type: ticket_type.ok_or(TicketFailure::NotFound)?,
        assignee_id,
        parent_id,
        custom,
        sprint_id: planning.sprint.unwrap_or(None),
        release_id: planning.release.unwrap_or(None),
        component_id: planning.component.unwrap_or(None),
    })
}

/// Nama field error (dan nama input form) untuk field custom `id`.
#[must_use]
pub fn custom_field_key(id: Uuid) -> String {
    format!("custom_{id}")
}

/// `user_id` boleh membuka project (aturan `project_access`).
pub(crate) async fn has_access<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project_id: Uuid,
    user_id: Uuid,
) -> Result<bool, DbErr> {
    Ok(
        project_access::users_with_access(db, wid, project_id, None, u64::MAX >> 1)
            .await?
            .iter()
            .any(|u| u.id == user_id),
    )
}

/// Template dan skop yang berlaku untuk project (override project, lalu workspace).
#[must_use]
pub fn effective_template(
    workspace: &workspaces::Model,
    project: &projects::Model,
) -> (String, CounterScope) {
    let template = project
        .ticket_code_template
        .clone()
        .unwrap_or_else(|| workspace.ticket_code_template.clone());
    let scope = project
        .ticket_counter_scope
        .as_deref()
        .unwrap_or(&workspace.ticket_counter_scope)
        .to_string();
    (
        template,
        CounterScope::try_from_value(&scope).unwrap_or(CounterScope::Project),
    )
}

/// Nilai token untuk tiket di project ini.
#[must_use]
pub fn code_parts<'a>(
    project: &'a projects::Model,
    type_key: &'a str,
    year: i32,
    number: i32,
) -> CodeParts<'a> {
    CodeParts {
        project: &project.key,
        team: project.team_code.as_deref().unwrap_or(&project.key),
        type_key,
        year,
        number,
    }
}

/// Buat tiket di project `project_id`. Pelaku harus memegang `EditProject`.
///
/// # Errors
///
/// [`TicketFailure::Invalid`], [`TicketFailure::NotFound`], atau error database.
pub async fn run(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    project_id: Uuid,
    form: &TicketForm,
) -> Result<tickets::Model, TicketFailure> {
    let txn = db.begin().await?;
    let ticket = run_in(&txn, acting, project_id, form).await?;
    txn.commit().await?;
    Ok(ticket)
}

/// Langkah [`run`] di transaksi `txn` (juga dipakai tiket Bug lanjutan dari keputusan gate).
pub(crate) async fn run_in(
    txn: &DatabaseTransaction,
    acting: &Acting<'_>,
    project_id: Uuid,
    form: &TicketForm,
) -> Result<tickets::Model, TicketFailure> {
    let wid = acting.workspace_id();
    let project = projects::Model::find_in(txn, wid, project_id)
        .await?
        .ok_or(TicketFailure::NotFound)?;
    access_in(txn, acting, project.id, Permission::EditProject).await?;
    let resolved = resolve(txn, wid, &project, form, None).await?;
    let workflow_id =
        project_workflows::Model::effective(txn, wid, project.id, resolved.ticket_type.id).await?;
    let workflow = workflows::Model::find_in(txn, wid, workflow_id)
        .await?
        .ok_or(TicketFailure::NotFound)?;
    let flow = stages::Flow {
        current: workflow.definition(txn).await?,
        rules: Definition::default(),
        workflow,
    };
    let stage = stages::initial_stage(&flow.current, &form.stage, resolved.valid.status)?.clone();
    let status = TicketStatus::parse(stage.category.as_str()).unwrap_or(TicketStatus::Todo);
    let workspace = workspaces::Entity::find_by_id(wid.get())
        .one(txn)
        .await?
        .ok_or(TicketFailure::NotFound)?;
    let (template, scope) = effective_template(&workspace, &project);
    let year = chrono::Utc::now().year();
    let counter = scope.counter_key(resolved.ticket_type.id, year);
    let mut chosen = None;
    for _ in 0..MAX_CODE_ATTEMPTS {
        let number = ticket_counters::next_number(txn, wid, project.id, &counter).await?;
        let code = ticket_code::render(
            &template,
            &code_parts(&project, &resolved.ticket_type.key, year, number),
        );
        if ticket_codes::Model::find(txn, wid, &code).await?.is_none() {
            chosen = Some((number, code));
            break;
        }
    }
    let (number, code) =
        chosen.ok_or_else(|| TicketFailure::field("title", ticket_code::CODE_COLLISION))?;
    let valid = &resolved.valid;
    // Aturan component: pemilik jadi penerima default tiket baru tanpa penerima.
    let assignee_id = match (resolved.assignee_id, resolved.component_id) {
        (None, Some(c)) => {
            crate::services::planning::components::default_assignee(txn, wid, &project, c).await?
        }
        (a, _) => a,
    };
    let ticket = tickets::ActiveModel {
        workspace_id: Set(wid.get()),
        project_id: Set(project.id),
        type_id: Set(resolved.ticket_type.id),
        number: Set(number),
        code: Set(code.clone()),
        title: Set(valid.title.clone()),
        description: Set(valid.description.clone()),
        status: Set(status.to_value()),
        workflow_id: Set(flow.workflow.id),
        stage_key: Set(stage.key.clone()),
        stage_version: Set(flow.workflow.version),
        priority: Set(valid.priority.to_value()),
        assignee_id: Set(assignee_id),
        reporter_id: Set(Some(acting.user.id)),
        sprint_id: Set(resolved.sprint_id),
        release_id: Set(resolved.release_id),
        component_id: Set(resolved.component_id),
        parent_id: Set(resolved.parent_id),
        labels: Set(valid.labels.clone()),
        due_on: Set(valid.due_on),
        story_points: Set(valid.story_points),
        custom_values: Set(resolved.custom.to_json()),
        position: Set(tickets::bottom_position()),
        backlog_rank: Set(
            crate::services::planning::sprints::bottom_rank(txn, wid, project.id).await?,
        ),
        done_at: Set((status == TicketStatus::Done).then(|| chrono::Utc::now().into())),
        created_by: Set(Some(acting.user.id)),
        ..Default::default()
    }
    .insert(txn)
    .await?;
    if !ticket_codes::ActiveModel::claim(txn, wid, ticket.id, &code).await? {
        return Err(TicketFailure::field("title", ticket_code::CODE_COLLISION));
    }
    ticket_events::ActiveModel::record(
        txn,
        wid,
        ticket.id,
        Some(acting.user.id),
        EventKind::Created,
        serde_json::json!({ "code": code, "title": ticket.title }),
    )
    .await?;
    ticket_watchers::ActiveModel::watch(txn, wid, ticket.id, acting.user.id).await?;
    crate::services::planning::scope::sprint_changed(txn, wid, &ticket, None, ticket.sprint_id)
        .await?;
    stages::seed_people(txn, wid, ticket.id, &stage).await?;
    if let Some(assignee) = ticket.assignee_id {
        watch_if_member(txn, wid, ticket.id, assignee).await?;
        notify(
            txn,
            wid,
            acting.user.id,
            assignee,
            NotificationKind::Assigned,
            ticket.id,
            None,
        )
        .await?;
    }
    for user in mentions::mentioned(txn, wid, project.id, &ticket.description).await? {
        notify(
            txn,
            wid,
            acting.user.id,
            user.id,
            NotificationKind::Mentioned,
            ticket.id,
            None,
        )
        .await?;
    }
    Ok(ticket)
}
