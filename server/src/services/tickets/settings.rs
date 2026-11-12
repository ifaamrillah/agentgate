//! Pengaturan tiket workspace (`ManageTicketSettings`): tipe tiket, field custom, dan template kode
//! tiket; serta override template per project (`AdministerProject`).
//!
//! Izin dibaca ulang di dalam transaksi setelah baris workspace dikunci (`lock_as`), seperti
//! service workspace lain. Setiap perubahan menulis audit di transaksi yang sama.
//!
//! **Ganti template kode** mengubah kode semua tiket yang terkena (project tanpa override untuk
//! template workspace, atau satu project untuk override-nya): kode baru menjadi kode sekarang,
//! kode lama tetap terdaftar di `ticket_codes` sebagai alias (spec). Bila kode baru sudah dimiliki
//! tiket lain, seluruh perubahan dibatalkan dengan `ticket-code-collision`. Dikerjakan di satu
//! transaksi (cukup untuk jumlah tiket M2; workspace besar perlu job, tercatat di PROGRESS).

use chrono::Datelike;
use sea_orm::{
    ActiveEnum, ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait,
    DatabaseConnection, DbErr, EntityTrait, IntoActiveModel, ModelTrait, QueryFilter,
    TransactionTrait,
};
use uuid::Uuid;
use validator::ValidationErrors;

use super::{
    TicketFailure, access_in,
    create::{code_parts, effective_template},
};
use crate::{
    models::{
        custom_fields::{self, CustomFieldError, FieldInput},
        permission::Permission,
        projects,
        ticket_code::{self, CODE_COLLISION, CounterScope},
        ticket_codes, ticket_type_fields,
        ticket_types::{self, TYPE_BUILTIN, TYPE_IN_USE, TicketTypeError, TypeInput},
        tickets, workspaces,
    },
    services::{
        audit::{self, AuditAction, AuditSubject},
        workspaces::{Acting, lock_as},
    },
};

impl From<TicketTypeError> for TicketFailure {
    fn from(e: TicketTypeError) -> Self {
        match e {
            TicketTypeError::Invalid(errors) => Self::Invalid(errors),
            TicketTypeError::Db(e) => Self::Db(e),
        }
    }
}

impl From<CustomFieldError> for TicketFailure {
    fn from(e: CustomFieldError) -> Self {
        match e {
            CustomFieldError::Invalid(errors) => Self::Invalid(errors),
            CustomFieldError::Db(e) => Self::Db(e),
        }
    }
}

async fn begin(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
) -> Result<(sea_orm::DatabaseTransaction, workspaces::Model), TicketFailure> {
    let txn = db.begin().await?;
    let workspace = lock_as(&txn, acting, |r| r.has(Permission::ManageTicketSettings))
        .await?
        .ok_or(TicketFailure::NotFound)?;
    Ok((txn, workspace))
}

/// Buat tipe custom.
///
/// # Errors
///
/// `Invalid`, `NotFound`, atau error database.
pub async fn create_type(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    input: &TypeInput,
) -> Result<ticket_types::Model, TicketFailure> {
    let (txn, workspace) = begin(db, acting).await?;
    let position = i32::try_from(
        ticket_types::Model::all_in(&txn, workspace.workspace_id())
            .await?
            .len(),
    )
    .unwrap_or(i32::MAX);
    let created =
        ticket_types::ActiveModel::create(&txn, workspace.workspace_id(), input, false, position)
            .await?;
    audit::record(
        &txn,
        acting.actor().audit(
            AuditAction::TicketTypeCreated,
            (AuditSubject::TicketType, created.id),
            serde_json::json!({ "key": created.key, "name": created.name }),
        ),
    )
    .await?;
    txn.commit().await?;
    Ok(created)
}

/// Ubah tipe (bawaan maupun custom) dan field yang dipakainya.
///
/// # Errors
///
/// `Invalid`, `NotFound`, atau error database.
pub async fn update_type(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    type_id: Uuid,
    input: &TypeInput,
    field_ids: Option<&[Uuid]>,
) -> Result<ticket_types::Model, TicketFailure> {
    let (txn, workspace) = begin(db, acting).await?;
    let wid = workspace.workspace_id();
    let current = ticket_types::Model::find_in(&txn, wid, type_id)
        .await?
        .ok_or(TicketFailure::NotFound)?;
    let updated = current.update_details(&txn, input).await?;
    let mut changes = serde_json::json!({ "name": updated.name });
    if let Some(ids) = field_ids {
        let known: Vec<Uuid> = custom_fields::Model::all_in(&txn, wid)
            .await?
            .into_iter()
            .map(|f| f.id)
            .filter(|id| ids.contains(id))
            .collect();
        ticket_type_fields::Model::replace_for_type(&txn, wid, type_id, &known).await?;
        changes["fields"] = serde_json::json!(known.len());
    }
    audit::record(
        &txn,
        acting.actor().audit(
            AuditAction::TicketTypeUpdated,
            (AuditSubject::TicketType, updated.id),
            changes,
        ),
    )
    .await?;
    txn.commit().await?;
    Ok(updated)
}

/// Hapus tipe custom yang tidak dipakai tiket.
///
/// # Errors
///
/// `Invalid` (`ticket-type-builtin`, `ticket-type-in-use`), `NotFound`, atau error database.
pub async fn delete_type(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    type_id: Uuid,
) -> Result<(), TicketFailure> {
    let (txn, workspace) = begin(db, acting).await?;
    let current = ticket_types::Model::find_in(&txn, workspace.workspace_id(), type_id)
        .await?
        .ok_or(TicketFailure::NotFound)?;
    if current.builtin {
        return Err(TicketFailure::field("type", TYPE_BUILTIN));
    }
    let in_use = sea_orm::PaginatorTrait::count(
        tickets::Entity::find().filter(tickets::Column::TypeId.eq(type_id)),
        &txn,
    )
    .await?;
    if in_use > 0 {
        return Err(TicketFailure::field("type", TYPE_IN_USE));
    }
    audit::record(
        &txn,
        acting.actor().audit(
            AuditAction::TicketTypeDeleted,
            (AuditSubject::TicketType, current.id),
            serde_json::json!({ "key": current.key, "name": current.name }),
        ),
    )
    .await?;
    current.delete(&txn).await?;
    txn.commit().await?;
    Ok(())
}

/// Buat field custom dan pasang ke tipe `type_ids`.
///
/// # Errors
///
/// `Invalid`, `NotFound`, atau error database.
pub async fn create_field(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    input: &FieldInput,
    type_ids: &[Uuid],
) -> Result<custom_fields::Model, TicketFailure> {
    let (txn, workspace) = begin(db, acting).await?;
    let wid = workspace.workspace_id();
    let field = custom_fields::ActiveModel::create(&txn, wid, input).await?;
    let types = known_types(&txn, &workspace, type_ids).await?;
    ticket_type_fields::Model::replace_for_field(&txn, wid, field.id, &types).await?;
    audit::record(
        &txn,
        acting.actor().audit(
            AuditAction::CustomFieldCreated,
            (AuditSubject::CustomField, field.id),
            serde_json::json!({ "name": field.name, "kind": field.kind }),
        ),
    )
    .await?;
    txn.commit().await?;
    Ok(field)
}

/// Ubah field custom dan tipe yang memakainya.
///
/// # Errors
///
/// `Invalid`, `NotFound`, atau error database.
pub async fn update_field(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    field_id: Uuid,
    input: &FieldInput,
    type_ids: &[Uuid],
) -> Result<custom_fields::Model, TicketFailure> {
    let (txn, workspace) = begin(db, acting).await?;
    let wid = workspace.workspace_id();
    let current = custom_fields::Model::find_in(&txn, wid, field_id)
        .await?
        .ok_or(TicketFailure::NotFound)?;
    let field = current.update_details(&txn, input).await?;
    let types = known_types(&txn, &workspace, type_ids).await?;
    ticket_type_fields::Model::replace_for_field(&txn, wid, field.id, &types).await?;
    audit::record(
        &txn,
        acting.actor().audit(
            AuditAction::CustomFieldUpdated,
            (AuditSubject::CustomField, field.id),
            serde_json::json!({ "name": field.name }),
        ),
    )
    .await?;
    txn.commit().await?;
    Ok(field)
}

/// Hapus field custom. Nilai lama di tiket tidak lagi ditampilkan (kunci tanpa field diabaikan).
///
/// # Errors
///
/// `NotFound` atau error database.
pub async fn delete_field(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    field_id: Uuid,
) -> Result<(), TicketFailure> {
    let (txn, workspace) = begin(db, acting).await?;
    let current = custom_fields::Model::find_in(&txn, workspace.workspace_id(), field_id)
        .await?
        .ok_or(TicketFailure::NotFound)?;
    audit::record(
        &txn,
        acting.actor().audit(
            AuditAction::CustomFieldDeleted,
            (AuditSubject::CustomField, current.id),
            serde_json::json!({ "name": current.name }),
        ),
    )
    .await?;
    current.delete(&txn).await?;
    txn.commit().await?;
    Ok(())
}

async fn known_types<C: ConnectionTrait>(
    db: &C,
    workspace: &workspaces::Model,
    ids: &[Uuid],
) -> Result<Vec<Uuid>, DbErr> {
    Ok(ticket_types::Model::all_in(db, workspace.workspace_id())
        .await?
        .into_iter()
        .map(|t| t.id)
        .filter(|id| ids.contains(id))
        .collect())
}

/// Validasi pasangan template + skop dari isian.
fn parse_template(template: &str, scope: &str) -> Result<(String, CounterScope), TicketFailure> {
    let template = template.trim().to_string();
    let scope = CounterScope::try_from_value(&scope.trim().to_string())
        .map_err(|_| TicketFailure::field("scope", ticket_code::CODE_TEMPLATE_INVALID))?;
    ticket_code::validate(&template, scope).map_err(|e| {
        let mut errors = ValidationErrors::new();
        errors.add("template", e);
        TicketFailure::errors(errors)
    })?;
    Ok((template, scope))
}

/// Ganti template kode default workspace; kode tiket project tanpa override ikut berubah.
///
/// # Errors
///
/// `Invalid` (termasuk `ticket-code-collision`), `NotFound`, atau error database.
pub async fn update_workspace_code(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    template: &str,
    scope: &str,
) -> Result<u64, TicketFailure> {
    let (template, scope) = parse_template(template, scope)?;
    let (txn, workspace) = begin(db, acting).await?;
    let before = (
        workspace.ticket_code_template.clone(),
        workspace.ticket_counter_scope.clone(),
    );
    if before == (template.clone(), scope.to_value()) {
        txn.rollback().await?;
        return Ok(0);
    }
    let wid = workspace.workspace_id();
    let mut model = workspace.into_active_model();
    model.ticket_code_template = Set(template.clone());
    model.ticket_counter_scope = Set(scope.to_value());
    let workspace = model.update(&txn).await?;
    let projects = projects::Entity::find()
        .filter(projects::Column::WorkspaceId.eq(wid.get()))
        .filter(projects::Column::TicketCodeTemplate.is_null())
        .all(&txn)
        .await?;
    let mut recoded = 0;
    for project in &projects {
        recoded += recode(&txn, &workspace, project).await?;
    }
    audit::record(
        &txn,
        acting.actor().audit(
            AuditAction::TicketCodeUpdated,
            (AuditSubject::Workspace, workspace.id),
            serde_json::json!({
                "template": { "from": before.0, "to": template },
                "scope": { "from": before.1, "to": scope.to_value() },
                "recoded": recoded,
            }),
        ),
    )
    .await?;
    txn.commit().await?;
    Ok(recoded)
}

/// Isian override kode tiket project.
#[derive(Debug, Clone, Default)]
pub struct ProjectCodeInput {
    /// `false` = ikut template workspace (template dan skop di bawah diabaikan).
    pub custom: bool,
    pub template: String,
    pub scope: String,
    /// Nilai `{TEAM}`; kosong = key project.
    pub team_code: String,
}

/// Override (atau lepas override) template kode tiket satu project; kode tiketnya ikut berubah.
///
/// # Errors
///
/// `Invalid`, `NotFound`, atau error database.
pub async fn update_project_code(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    project_id: Uuid,
    input: &ProjectCodeInput,
) -> Result<u64, TicketFailure> {
    let team_code = ticket_code::normalize_team_code(&input.team_code).map_err(|e| {
        let mut errors = ValidationErrors::new();
        errors.add("team_code", e);
        TicketFailure::errors(errors)
    })?;
    let override_ = if input.custom {
        Some(parse_template(&input.template, &input.scope)?)
    } else {
        None
    };
    let txn = db.begin().await?;
    let wid = acting.workspace_id();
    let project = projects::Model::lock_in(&txn, wid, project_id)
        .await?
        .ok_or(TicketFailure::NotFound)?;
    access_in(&txn, acting, project.id, Permission::AdministerProject).await?;
    let workspace = workspaces::Entity::find_by_id(wid.get())
        .one(&txn)
        .await?
        .ok_or(TicketFailure::NotFound)?;
    let before = (
        project.ticket_code_template.clone(),
        project.ticket_counter_scope.clone(),
        project.team_code.clone(),
    );
    let after = (
        override_.as_ref().map(|(t, _)| t.clone()),
        override_.as_ref().map(|(_, s)| s.to_value()),
        team_code,
    );
    if before == after {
        txn.rollback().await?;
        return Ok(0);
    }
    let mut model = project.into_active_model();
    model.ticket_code_template = Set(after.0.clone());
    model.ticket_counter_scope = Set(after.1.clone());
    model.team_code = Set(after.2.clone());
    let project = model.update(&txn).await?;
    let recoded = recode(&txn, &workspace, &project).await?;
    audit::record(
        &txn,
        acting.actor().audit(
            AuditAction::TicketCodeUpdated,
            (AuditSubject::Project, project.id),
            serde_json::json!({
                "template": { "from": before.0, "to": after.0 },
                "scope": { "from": before.1, "to": after.1 },
                "team_code": { "from": before.2, "to": after.2 },
                "recoded": recoded,
            }),
        ),
    )
    .await?;
    txn.commit().await?;
    Ok(recoded)
}

/// Hitung ulang kode semua tiket project (termasuk di sampah) dengan template yang berlaku. Kode
/// lama tetap menjadi alias.
async fn recode<C: ConnectionTrait>(
    txn: &C,
    workspace: &workspaces::Model,
    project: &projects::Model,
) -> Result<u64, TicketFailure> {
    let wid = workspace.workspace_id();
    let (template, _) = effective_template(workspace, project);
    let types = ticket_types::Model::all_in(txn, wid).await?;
    let tickets = tickets::Entity::find()
        .filter(tickets::Column::WorkspaceId.eq(wid.get()))
        .filter(tickets::Column::ProjectId.eq(project.id))
        .all(txn)
        .await?;
    let mut count = 0;
    for ticket in tickets {
        let type_key = types
            .iter()
            .find(|t| t.id == ticket.type_id)
            .map_or("task", |t| t.key.as_str());
        let code = ticket_code::render(
            &template,
            &code_parts(project, type_key, ticket.created_at.year(), ticket.number),
        );
        if code == ticket.code {
            continue;
        }
        if !ticket_codes::ActiveModel::claim(txn, wid, ticket.id, &code).await? {
            let owner = ticket_codes::Model::find(txn, wid, &code).await?;
            if owner.is_none_or(|o| o.ticket_id != ticket.id) {
                let mut errors = ValidationErrors::new();
                let mut e = validator::ValidationError::new(CODE_COLLISION);
                e.add_param("code".into(), &code);
                errors.add("template", e);
                return Err(TicketFailure::errors(errors));
            }
        }
        let mut model = ticket.into_active_model();
        model.code = Set(code);
        model.update(txn).await?;
        count += 1;
    }
    Ok(count)
}
