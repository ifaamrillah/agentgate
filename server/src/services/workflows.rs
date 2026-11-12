//! Kelola workflow workspace (`ManageTicketSettings`) dan workflow per tipe di project
//! (`AdministerProject`). Spec "Workflow, stage & gate".
//!
//! **Draf dan versi.** Builder menyimpan perubahan ke `workflows.draft` (selalu definisi yang
//! valid). "Terbitkan" menjadikan draf versi baru (`workflow_versions`), versi lama tetap
//! tersimpan. Tiket di stage yang masih ada tidak berubah: aturan stage-nya tetap dari versi saat
//! ia masuk stage. Tiket di stage yang dihapus harus dipetakan ke stage versi baru (halaman
//! migrasi); tanpa pemetaan lengkap penerbitan ditolak (`workflow-mapping-required`).
//!
//! **Ganti workflow tipe** (default workspace atau override project): tiket yang terkena pindah ke
//! stage padanan di workflow baru (kunci sama, lalu kategori sama), lihat
//! [`stages::follow_workflow`]. Semua di satu transaksi (cukup untuk jumlah tiket sekarang;
//! workspace besar perlu job, tercatat di PROGRESS).

use std::collections::BTreeMap;

use agentgate_core::workflow::{
    Category, Definition, Issue, Stage, templates,
    yaml::{self, Document},
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseConnection,
    DatabaseTransaction, DbErr, EntityTrait, IntoActiveModel, PaginatorTrait, QueryFilter,
    QuerySelect, TransactionTrait,
};
use uuid::Uuid;
use validator::{ValidationError, ValidationErrors};

use crate::{
    models::{
        permission::Permission,
        project_workflows, projects,
        ticket_events::{self, EventKind},
        ticket_types, tickets, workflow_versions,
        workflows::{self, NewWorkflow},
        workspace_members, workspaces,
    },
    services::{
        audit::{self, AuditAction, AuditSubject},
        tickets::{TicketFailure, access_in, stages},
        workspaces::{Acting, lock_as},
    },
};

/// Kegagalan kelola workflow (bentuknya sama dengan kegagalan tiket).
pub type WorkflowFailure = TicketFailure;

pub const MAPPING_REQUIRED: &str = "workflow-mapping-required";
pub const SOURCE_INVALID: &str = "workflow-source-invalid";
pub const DRAFT_MISSING: &str = "workflow-draft-missing";
pub const STAGE_MISSING: &str = "workflow-stage-missing";
pub const LAST_STAGE: &str = "workflow-last-stage";
pub const PEOPLE_INVALID: &str = "workflow-people-invalid";
pub const ERROR_CODES: &[&str] = &[
    MAPPING_REQUIRED,
    SOURCE_INVALID,
    DRAFT_MISSING,
    STAGE_MISSING,
    LAST_STAGE,
    PEOPLE_INVALID,
];

async fn begin(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
) -> Result<(DatabaseTransaction, workspaces::Model), WorkflowFailure> {
    let txn = db.begin().await?;
    let workspace = lock_as(&txn, acting, |r| r.has(Permission::ManageTicketSettings))
        .await?
        .ok_or(TicketFailure::NotFound)?;
    Ok((txn, workspace))
}

async fn locked(
    txn: &DatabaseTransaction,
    acting: &Acting<'_>,
    id: Uuid,
) -> Result<workflows::Model, WorkflowFailure> {
    workflows::Model::lock_in(txn, acting.workspace_id(), id)
        .await?
        .ok_or(TicketFailure::NotFound)
}

fn errors_of(field: &'static str, code: &'static str, detail: Option<&str>) -> WorkflowFailure {
    let mut e = ValidationError::new(code);
    if let Some(d) = detail {
        e.add_param("detail".into(), &d);
    }
    let mut errors = ValidationErrors::new();
    errors.add(field, e);
    TicketFailure::errors(errors)
}

/// Error validasi definisi → error form. Issue stage `focus` memakai nama field-nya; issue lain
/// dikumpulkan di field `stages` dengan nama stage sebagai `detail`.
#[must_use]
pub fn issues_failure(definition: &Definition, issues: &[Issue], focus: Option<usize>) -> WorkflowFailure {
    let mut errors = ValidationErrors::new();
    for issue in issues {
        let mut e = ValidationError::new(issue.code);
        if focus.is_some() && issue.stage == focus {
            errors.add(issue.field, e);
        } else {
            let name = issue
                .stage
                .and_then(|i| definition.stages.get(i))
                .map(|s| s.name.clone())
                .unwrap_or_default();
            e.add_param("detail".into(), &name);
            errors.add("stages", e);
        }
    }
    TicketFailure::errors(errors)
}

fn checked(mut definition: Definition, focus: Option<usize>) -> Result<Definition, WorkflowFailure> {
    definition.normalize();
    definition
        .validate()
        .map_err(|issues| issues_failure(&definition, &issues, focus))?;
    Ok(definition)
}

fn identity(name: &str, description: &str) -> Result<(), WorkflowFailure> {
    workflows::validate_identity(name, description)
        .map_err(|(field, code)| errors_of(field, code, None))
}

async fn name_free<C: ConnectionTrait>(
    db: &C,
    wid: crate::models::workspaces::WorkspaceId,
    name: &str,
    except: Option<Uuid>,
) -> Result<(), WorkflowFailure> {
    let taken = workflows::Model::all_in(db, wid)
        .await?
        .iter()
        .any(|w| Some(w.id) != except && w.name.trim().to_lowercase() == name.trim().to_lowercase());
    if taken {
        Err(errors_of("name", workflows::NAME_TAKEN, None))
    } else {
        Ok(())
    }
}

/// Sumber workflow baru.
#[derive(Debug, Clone)]
pub enum Source {
    /// Satu stage To do dan satu Selesai.
    Blank,
    /// Template bawaan (kunci template).
    Template(String),
    /// Salin versi aktif workflow lain.
    Clone(Uuid),
}

impl Source {
    /// `blank`, `template:<kunci>`, atau `clone:<id>`.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "" | "blank" => Some(Self::Blank),
            v => {
                if let Some(k) = v.strip_prefix("template:") {
                    templates::find(k).map(|_| Self::Template(k.to_string()))
                } else {
                    v.strip_prefix("clone:")
                        .and_then(|id| Uuid::parse_str(id).ok())
                        .map(Self::Clone)
                }
            }
        }
    }
}

fn blank() -> Definition {
    Definition {
        stages: vec![
            Stage::new("todo", "To do", Category::Todo),
            Stage::new("in_progress", "Dikerjakan", Category::InProgress),
            Stage::new("done", "Selesai", Category::Done),
        ],
    }
}

/// Buang penugasan default yang bukan anggota workspace.
async fn members_only<C: ConnectionTrait>(
    db: &C,
    wid: crate::models::workspaces::WorkspaceId,
    mut definition: Definition,
) -> Result<Definition, DbErr> {
    for stage in &mut definition.stages {
        for list in [
            &mut stage.people.executors,
            &mut stage.people.approvers,
            &mut stage.people.watchers,
        ] {
            let mut kept = Vec::new();
            for id in list.drain(..) {
                if !kept.contains(&id)
                    && workspace_members::Model::find_with_role(db, wid, id)
                        .await?
                        .is_some()
                {
                    kept.push(id);
                }
            }
            *list = kept;
        }
    }
    Ok(definition)
}

async fn insert(
    txn: &DatabaseTransaction,
    acting: &Acting<'_>,
    name: &str,
    description: &str,
    template_key: Option<&str>,
    definition: Definition,
) -> Result<workflows::Model, WorkflowFailure> {
    identity(name, description)?;
    let wid = acting.workspace_id();
    name_free(txn, wid, name, None).await?;
    let definition = checked(members_only(txn, wid, definition).await?, None)?;
    let created = workflows::ActiveModel::create(
        txn,
        wid,
        NewWorkflow {
            name,
            description,
            template_key,
            definition: &definition,
            created_by: Some(acting.user.id),
        },
    )
    .await?;
    audit::record(
        txn,
        acting.actor().audit(
            AuditAction::WorkflowCreated,
            (AuditSubject::Workflow, created.id),
            serde_json::json!({ "name": created.name, "template": template_key }),
        ),
    )
    .await?;
    Ok(created)
}

/// Buat workflow dari kosong, template bawaan, atau salinan workflow lain.
///
/// # Errors
///
/// `Invalid` (nama, sumber), `NotFound`, atau error database.
pub async fn create(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    name: &str,
    description: &str,
    source: Option<Source>,
) -> Result<workflows::Model, WorkflowFailure> {
    let source = source.ok_or_else(|| errors_of("source", SOURCE_INVALID, None))?;
    let (txn, workspace) = begin(db, acting).await?;
    let (definition, template_key) = match &source {
        Source::Blank => (blank(), None),
        Source::Template(key) => (
            templates::find(key)
                .ok_or_else(|| errors_of("source", SOURCE_INVALID, None))?
                .document()
                .definition,
            Some(key.as_str()),
        ),
        Source::Clone(id) => (
            workflows::Model::find_in(&txn, workspace.workspace_id(), *id)
                .await?
                .ok_or_else(|| errors_of("source", SOURCE_INVALID, None))?
                .definition(&txn)
                .await?,
            None,
        ),
    };
    let created = insert(&txn, acting, name, description, template_key, definition).await?;
    txn.commit().await?;
    Ok(created)
}

/// Impor workflow dari YAML (format ekspor). Penugasan memakai email anggota workspace; email
/// yang tidak dikenal diabaikan dan dikembalikan.
///
/// # Errors
///
/// `Invalid` (field `yaml`), `NotFound`, atau error database.
pub async fn import(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    text: &str,
) -> Result<(workflows::Model, Vec<String>), WorkflowFailure> {
    let (txn, workspace) = begin(db, acting).await?;
    let emails = member_emails(&txn, workspace.workspace_id()).await?;
    let (doc, ignored) = yaml::parse(text, |email| {
        emails
            .iter()
            .find(|(e, _)| e.eq_ignore_ascii_case(email.trim()))
            .map(|(_, id)| *id)
    })
    .map_err(|e| errors_of("yaml", e.code, Some(&e.detail)))?;
    let definition = doc.definition.clone();
    let mut def = definition.clone();
    def.normalize();
    if let Err(issues) = def.validate() {
        let failure = issues_failure(&def, &issues, None);
        let TicketFailure::Invalid(mut e) = failure else {
            return Err(failure);
        };
        if let Some(list) = e.errors.remove("stages") {
            e.errors.insert("yaml".into(), list);
        }
        return Err(TicketFailure::Invalid(e));
    }
    let created = insert(&txn, acting, &doc.name, &doc.description, None, definition)
        .await
        .map_err(|f| match f {
            TicketFailure::Invalid(mut e) => {
                if let Some(list) = e.errors.remove("name") {
                    e.errors.insert("yaml".into(), list);
                }
                if let Some(list) = e.errors.remove("description") {
                    e.errors.insert("yaml".into(), list);
                }
                TicketFailure::Invalid(e)
            }
            other => other,
        })?;
    txn.commit().await?;
    Ok((created, ignored))
}

/// Email → id anggota workspace.
async fn member_emails<C: ConnectionTrait>(
    db: &C,
    wid: crate::models::workspaces::WorkspaceId,
) -> Result<Vec<(String, Uuid)>, DbErr> {
    Ok(workspace_members::Model::list_in(db, wid)
        .await?
        .into_iter()
        .map(|(_, user, _)| (user.email, user.id))
        .collect())
}

/// YAML workflow: versi aktif, atau draf bila `draft` dan ada.
///
/// # Errors
///
/// Error database.
pub async fn export<C: ConnectionTrait>(
    db: &C,
    wid: crate::models::workspaces::WorkspaceId,
    workflow: &workflows::Model,
    draft: bool,
) -> Result<String, DbErr> {
    let (definition, version) = match workflow.draft().filter(|_| draft) {
        Some(d) => (d, None),
        None => (workflow.definition(db).await?, Some(i64::from(workflow.version))),
    };
    let emails = member_emails(db, wid).await?;
    Ok(yaml::emit(
        &Document {
            key: workflow.key.clone(),
            name: workflow.name.clone(),
            description: workflow.description.clone(),
            definition,
        },
        version,
        |id| emails.iter().find(|(_, u)| *u == id).map(|(e, _)| e.clone()),
    ))
}

/// Ubah nama dan deskripsi.
///
/// # Errors
///
/// `Invalid`, `NotFound`, atau error database.
pub async fn rename(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
    name: &str,
    description: &str,
) -> Result<workflows::Model, WorkflowFailure> {
    identity(name, description)?;
    let (txn, _) = begin(db, acting).await?;
    let workflow = locked(&txn, acting, id).await?;
    name_free(&txn, acting.workspace_id(), name, Some(id)).await?;
    if workflow.name == name.trim() && workflow.description == description.trim() {
        txn.rollback().await?;
        return Ok(workflow);
    }
    let changes = serde_json::json!({
        "name": audit::change(&workflow.name, &name.trim().to_string()),
        "description": audit::change(&workflow.description, &description.trim().to_string()),
    });
    let mut model = workflow.into_active_model();
    model.name = Set(name.trim().to_string());
    model.description = Set(description.trim().to_string());
    let updated = model.update(&txn).await?;
    audit::record(
        &txn,
        acting.actor().audit(
            AuditAction::WorkflowUpdated,
            (AuditSubject::Workflow, updated.id),
            changes,
        ),
    )
    .await?;
    txn.commit().await?;
    Ok(updated)
}

/// Draf builder sekarang: draf tersimpan, atau salinan versi aktif.
///
/// # Errors
///
/// Error database.
pub async fn working_copy<C: ConnectionTrait>(
    db: &C,
    workflow: &workflows::Model,
) -> Result<Definition, DbErr> {
    match workflow.draft() {
        Some(d) => Ok(d),
        None => workflow.definition(db).await,
    }
}

/// Perubahan draf.
#[derive(Debug, Clone)]
pub enum DraftEdit {
    /// Ganti stage `key` dengan isian ini (kunci tidak berubah).
    Stage { key: String, stage: Box<Stage> },
    /// Tambah stage baru berkategori ini di akhir kategorinya. Mengembalikan kuncinya.
    Add { name: String, category: Category },
    /// Hapus stage.
    Remove { key: String },
    /// Geser stage satu langkah (`-1` naik, `1` turun).
    Move { key: String, by: i8 },
    /// Ganti seluruh definisi (tab YAML).
    Yaml(String),
}

/// Terapkan satu perubahan ke draf (draf selalu valid). Mengembalikan draf dan kunci stage yang
/// sebaiknya dipilih di builder.
///
/// # Errors
///
/// `Invalid`, `NotFound`, atau error database.
pub async fn edit_draft(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
    edit: DraftEdit,
) -> Result<(Definition, Option<String>), WorkflowFailure> {
    let (txn, workspace) = begin(db, acting).await?;
    let wid = workspace.workspace_id();
    let workflow = locked(&txn, acting, id).await?;
    let mut draft = working_copy(&txn, &workflow).await?;
    let (draft, selected) = match edit {
        DraftEdit::Stage { key, stage } => {
            let i = draft
                .index_of(&key)
                .ok_or_else(|| errors_of("stage", STAGE_MISSING, None))?;
            let mut stage = *stage;
            stage.key = key.clone();
            for id in stage
                .people
                .executors
                .iter()
                .chain(&stage.people.approvers)
                .chain(&stage.people.watchers)
            {
                if workspace_members::Model::find_with_role(&txn, wid, *id)
                    .await?
                    .is_none()
                {
                    return Err(errors_of("people", PEOPLE_INVALID, None));
                }
            }
            draft.stages[i] = stage;
            (checked(draft, Some(i))?, Some(key))
        }
        DraftEdit::Add { name, category } => {
            let key = draft.fresh_key(&name);
            let at = draft
                .stages
                .iter()
                .position(|s| s.category > category)
                .unwrap_or(draft.stages.len());
            let at = if category == Category::Done {
                draft
                    .stages
                    .iter()
                    .rposition(|s| s.category < Category::Done)
                    .map_or(0, |p| p + 1)
            } else {
                at
            };
            draft.stages.insert(at, Stage::new(&key, &name, category));
            (checked(draft, Some(at))?, Some(key))
        }
        DraftEdit::Remove { key } => {
            let i = draft
                .index_of(&key)
                .ok_or_else(|| errors_of("stage", STAGE_MISSING, None))?;
            if draft.stages.len() <= 1 {
                return Err(errors_of("stage", LAST_STAGE, None));
            }
            draft.stages.remove(i);
            for s in &mut draft.stages {
                if s.fail_to.as_deref() == Some(key.as_str()) {
                    s.fail_to = None;
                }
            }
            let next = draft.stages.get(i.min(draft.stages.len() - 1)).map(|s| s.key.clone());
            (checked(draft, None)?, next)
        }
        DraftEdit::Move { key, by } => {
            let i = draft
                .index_of(&key)
                .ok_or_else(|| errors_of("stage", STAGE_MISSING, None))?;
            let j = if by < 0 { i.checked_sub(1) } else { Some(i + 1) }
                .filter(|j| *j < draft.stages.len());
            if let Some(j) = j {
                draft.stages.swap(i, j);
            }
            (checked(draft, None)?, Some(key))
        }
        DraftEdit::Yaml(text) => {
            let emails = member_emails(&txn, wid).await?;
            let (doc, _) = yaml::parse(&text, |email| {
                emails
                    .iter()
                    .find(|(e, _)| e.eq_ignore_ascii_case(email.trim()))
                    .map(|(_, id)| *id)
            })
            .map_err(|e| errors_of("yaml", e.code, Some(&e.detail)))?;
            let def = doc.definition;
            let mut normalized = def.clone();
            normalized.normalize();
            if let Err(issues) = normalized.validate() {
                let TicketFailure::Invalid(mut e) = issues_failure(&normalized, &issues, None)
                else {
                    unreachable!("issues_failure selalu Invalid")
                };
                if let Some(list) = e.errors.remove("stages") {
                    e.errors.insert("yaml".into(), list);
                }
                return Err(TicketFailure::Invalid(e));
            }
            let first = normalized.first().map(|s| s.key.clone());
            (normalized, first)
        }
    };
    let mut model = workflow.into_active_model();
    model.draft = Set(Some(serde_json::to_value(&draft).map_err(|e| {
        DbErr::Custom(format!("draf workflow: {e}"))
    })?));
    model.update(&txn).await?;
    txn.commit().await?;
    Ok((draft, selected))
}

/// Buang draf.
///
/// # Errors
///
/// `NotFound` atau error database.
pub async fn discard_draft(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
) -> Result<(), WorkflowFailure> {
    let (txn, _) = begin(db, acting).await?;
    let workflow = locked(&txn, acting, id).await?;
    let mut model = workflow.into_active_model();
    model.draft = Set(None);
    model.update(&txn).await?;
    txn.commit().await?;
    Ok(())
}

/// Stage versi aktif yang tidak ada di draf, dengan jumlah tiket di stage itu.
#[derive(Debug, Clone)]
pub struct Removed {
    pub key: String,
    pub name: String,
    pub category: Category,
    pub tickets: u64,
}

/// Rencana penerbitan draf: stage yang dihapus (dan tiketnya), jumlah tiket lain.
#[derive(Debug, Clone)]
pub struct Plan {
    pub current: Definition,
    pub draft: Definition,
    pub removed: Vec<Removed>,
    pub others: u64,
}

impl Plan {
    /// Ada tiket yang harus dipetakan.
    #[must_use]
    pub fn needs_mapping(&self) -> bool {
        self.removed.iter().any(|r| r.tickets > 0)
    }
}

/// Hitung rencana penerbitan.
///
/// # Errors
///
/// `NotFound` (tanpa draf → `Invalid` `workflow-draft-missing`) atau error database.
pub async fn plan<C: ConnectionTrait>(
    db: &C,
    workflow: &workflows::Model,
) -> Result<Plan, WorkflowFailure> {
    let draft = workflow
        .draft()
        .ok_or_else(|| errors_of("draft", DRAFT_MISSING, None))?;
    let current = workflow.definition(db).await?;
    let counts: Vec<(String, i64)> = tickets::Entity::find()
        .select_only()
        .column(tickets::Column::StageKey)
        .column_as(tickets::Column::Id.count(), "n")
        .filter(tickets::Column::WorkspaceId.eq(workflow.workspace_id))
        .filter(tickets::Column::WorkflowId.eq(workflow.id))
        .group_by(tickets::Column::StageKey)
        .into_tuple()
        .all(db)
        .await?;
    let mut removed = Vec::new();
    let mut others = 0u64;
    for (key, n) in &counts {
        let n = u64::try_from(*n).unwrap_or(0);
        if draft.stage(key).is_some() {
            others += n;
        } else {
            let old = current.stage(key);
            removed.push(Removed {
                key: key.clone(),
                name: old.map_or_else(|| key.clone(), |s| s.name.clone()),
                category: old.map_or(Category::Todo, |s| s.category),
                tickets: n,
            });
        }
    }
    for s in &current.stages {
        if draft.stage(&s.key).is_none() && !removed.iter().any(|r| r.key == s.key) {
            removed.push(Removed {
                key: s.key.clone(),
                name: s.name.clone(),
                category: s.category,
                tickets: 0,
            });
        }
    }
    removed.sort_by_key(|r| current.index_of(&r.key).unwrap_or(usize::MAX));
    Ok(Plan {
        current,
        draft,
        removed,
        others,
    })
}

/// Terbitkan draf sebagai versi baru. `mapping` = stage lama (dihapus) → stage baru untuk tiket
/// yang ada di stage lama itu. Mengembalikan versi baru dan jumlah tiket yang dipindahkan.
///
/// # Errors
///
/// `Invalid` (`workflow-draft-missing`, `workflow-mapping-required`), `NotFound`, atau error
/// database.
pub async fn publish(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
    mapping: &BTreeMap<String, String>,
) -> Result<(i32, u64), WorkflowFailure> {
    let (txn, workspace) = begin(db, acting).await?;
    let wid = workspace.workspace_id();
    let workflow = locked(&txn, acting, id).await?;
    let plan = plan(&txn, &workflow).await?;
    let draft = checked(plan.draft.clone(), None)?;
    for r in plan.removed.iter().filter(|r| r.tickets > 0) {
        if mapping.get(&r.key).and_then(|k| draft.stage(k)).is_none() {
            return Err(errors_of("mapping", MAPPING_REQUIRED, Some(&r.name)));
        }
    }
    let version = workflow.version + 1;
    workflow_versions::ActiveModel::append(&txn, &workflow, version, &draft, Some(acting.user.id))
        .await?;
    let mut model = workflow.clone().into_active_model();
    model.version = Set(version);
    model.draft = Set(None);
    let workflow = model.update(&txn).await?;
    let mut moved = 0u64;
    let flow_current = draft.clone();
    for r in plan.removed.iter().filter(|r| r.tickets > 0) {
        let Some(to) = mapping.get(&r.key).and_then(|k| draft.stage(k)).cloned() else {
            continue;
        };
        let affected = tickets::Entity::find()
            .filter(tickets::Column::WorkspaceId.eq(wid.get()))
            .filter(tickets::Column::WorkflowId.eq(workflow.id))
            .filter(tickets::Column::StageKey.eq(r.key.clone()))
            .all(&txn)
            .await?;
        for t in affected {
            let flow = stages::Flow {
                workflow: workflow.clone(),
                current: flow_current.clone(),
                rules: flow_current.clone(),
            };
            let mut model = t.clone().into_active_model();
            model.fail_count = Set(0);
            model.fail_stage_key = Set(None);
            let t = model.update(&txn).await?;
            let t = stages::enter_stage(&txn, wid, t, &flow, &to).await?;
            ticket_events::ActiveModel::record(
                &txn,
                wid,
                t.id,
                Some(acting.user.id),
                EventKind::WorkflowMigrated,
                serde_json::json!({
                    "stage": { "from": r.name, "to": to.name },
                    "version": version,
                }),
            )
            .await?;
            moved += 1;
        }
    }
    audit::record(
        &txn,
        acting.actor().audit(
            AuditAction::WorkflowUpdated,
            (AuditSubject::Workflow, workflow.id),
            serde_json::json!({ "version": version, "name": workflow.name }),
        ),
    )
    .await?;
    if moved > 0 {
        audit::record(
            &txn,
            acting.actor().audit(
                AuditAction::WorkflowMigrated,
                (AuditSubject::Workflow, workflow.id),
                serde_json::json!({ "version": version, "tickets": moved, "mapping": mapping }),
            ),
        )
        .await?;
    }
    txn.commit().await?;
    Ok((version, moved))
}

/// Workflow dipakai: tipe tiket (default), override project, atau tiket.
///
/// # Errors
///
/// Error database.
pub async fn in_use<C: ConnectionTrait>(db: &C, workflow: &workflows::Model) -> Result<bool, DbErr> {
    let types = ticket_types::Entity::find()
        .filter(ticket_types::Column::WorkflowId.eq(workflow.id))
        .count(db)
        .await?;
    let overrides = project_workflows::Entity::find()
        .filter(project_workflows::Column::WorkflowId.eq(workflow.id))
        .count(db)
        .await?;
    let tickets = tickets::Entity::find()
        .filter(tickets::Column::WorkflowId.eq(workflow.id))
        .count(db)
        .await?;
    Ok(types + overrides + tickets > 0)
}

/// Hapus workflow yang tidak dipakai.
///
/// # Errors
///
/// `Invalid` (`workflow-in-use`), `NotFound`, atau error database.
pub async fn delete(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
) -> Result<(), WorkflowFailure> {
    let (txn, _) = begin(db, acting).await?;
    let workflow = locked(&txn, acting, id).await?;
    if in_use(&txn, &workflow).await? {
        return Err(errors_of("workflow", workflows::IN_USE, None));
    }
    workflow_versions::Entity::delete_many()
        .filter(workflow_versions::Column::WorkflowId.eq(workflow.id))
        .exec(&txn)
        .await?;
    workflows::Entity::delete_by_id(workflow.id).exec(&txn).await?;
    audit::record(
        &txn,
        acting.actor().audit(
            AuditAction::WorkflowDeleted,
            (AuditSubject::Workflow, workflow.id),
            serde_json::json!({ "name": workflow.name }),
        ),
    )
    .await?;
    txn.commit().await?;
    Ok(())
}

/// Pindahkan tiket-tiket ini ke workflow `workflow_id` (stage padanan), dengan riwayat.
async fn retarget(
    txn: &DatabaseTransaction,
    acting: &Acting<'_>,
    affected: Vec<tickets::Model>,
    workflow_id: Uuid,
) -> Result<u64, WorkflowFailure> {
    let wid = acting.workspace_id();
    let mut moved = 0;
    for t in affected {
        let (t, change) = stages::follow_workflow(txn, wid, t, workflow_id).await?;
        if let Some((from, to)) = change {
            ticket_events::ActiveModel::record(
                txn,
                wid,
                t.id,
                Some(acting.user.id),
                EventKind::WorkflowMigrated,
                serde_json::json!({ "stage": { "from": from, "to": to } }),
            )
            .await?;
            moved += 1;
        }
    }
    Ok(moved)
}

/// Ganti workflow default workspace untuk tipe tiket. Tiket tipe ini di project tanpa override
/// ikut pindah.
///
/// # Errors
///
/// `NotFound` (tipe/workflow tidak ada) atau error database.
pub async fn set_type_default(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    type_id: Uuid,
    workflow_id: Uuid,
) -> Result<u64, WorkflowFailure> {
    let (txn, workspace) = begin(db, acting).await?;
    let wid = workspace.workspace_id();
    let workflow = locked(&txn, acting, workflow_id).await?;
    let ticket_type = ticket_types::Model::all_in(&txn, wid)
        .await?
        .into_iter()
        .find(|t| t.id == type_id)
        .ok_or(TicketFailure::NotFound)?;
    if ticket_type.workflow_id == workflow.id {
        txn.rollback().await?;
        return Ok(0);
    }
    let overridden: Vec<Uuid> = project_workflows::Entity::find()
        .filter(project_workflows::Column::WorkspaceId.eq(wid.get()))
        .filter(project_workflows::Column::TypeId.eq(type_id))
        .all(&txn)
        .await?
        .into_iter()
        .map(|o| o.project_id)
        .collect();
    let mut model = ticket_type.clone().into_active_model();
    model.workflow_id = Set(workflow.id);
    model.update(&txn).await?;
    let affected = tickets::Entity::find()
        .filter(tickets::Column::WorkspaceId.eq(wid.get()))
        .filter(tickets::Column::TypeId.eq(type_id))
        .filter(tickets::Column::ProjectId.is_not_in(overridden))
        .all(&txn)
        .await?;
    let moved = retarget(&txn, acting, affected, workflow.id).await?;
    audit::record(
        &txn,
        acting.actor().audit(
            AuditAction::TicketTypeUpdated,
            (AuditSubject::TicketType, ticket_type.id),
            serde_json::json!({
                "workflow": { "from": ticket_type.workflow_id, "to": workflow.id },
                "tickets": moved,
            }),
        ),
    )
    .await?;
    txn.commit().await?;
    Ok(moved)
}

/// Pengaturan workflow project: override per tipe (`None` = ikut default workspace) dan izin
/// paksa transisi.
#[derive(Debug, Clone, Default)]
pub struct ProjectWorkflows {
    pub overrides: Vec<(Uuid, Option<Uuid>)>,
    pub allow_force_transition: bool,
}

/// Simpan pengaturan workflow project (`AdministerProject`). Tiket yang workflow-nya berubah
/// pindah ke stage padanan.
///
/// # Errors
///
/// `NotFound` atau error database.
pub async fn set_project_workflows(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    project_id: Uuid,
    input: &ProjectWorkflows,
) -> Result<u64, WorkflowFailure> {
    let wid = acting.workspace_id();
    let txn = db.begin().await?;
    let project = projects::Model::find_in(&txn, wid, project_id)
        .await?
        .ok_or(TicketFailure::NotFound)?;
    access_in(&txn, acting, project.id, Permission::AdministerProject).await?;
    let types = ticket_types::Model::all_in(&txn, wid).await?;
    let all = workflows::Model::all_in(&txn, wid).await?;
    let before = project_workflows::Model::of_project(&txn, wid, project.id).await?;
    let mut changes = serde_json::Map::new();
    let mut moved = 0;
    for (type_id, choice) in &input.overrides {
        let Some(t) = types.iter().find(|t| t.id == *type_id) else {
            return Err(TicketFailure::NotFound);
        };
        if let Some(w) = choice
            && !all.iter().any(|x| x.id == *w)
        {
            return Err(TicketFailure::NotFound);
        }
        let old = before.iter().find(|o| o.type_id == t.id).map(|o| o.workflow_id);
        let choice = choice.filter(|w| *w != t.workflow_id);
        if old == choice {
            continue;
        }
        project_workflows::ActiveModel::set(&txn, wid, project.id, t.id, choice).await?;
        let target = choice.unwrap_or(t.workflow_id);
        let affected = tickets::Entity::find()
            .filter(tickets::Column::WorkspaceId.eq(wid.get()))
            .filter(tickets::Column::ProjectId.eq(project.id))
            .filter(tickets::Column::TypeId.eq(t.id))
            .all(&txn)
            .await?;
        moved += retarget(&txn, acting, affected, target).await?;
        changes.insert(
            t.key.clone(),
            serde_json::json!({ "from": old, "to": choice }),
        );
    }
    if project.allow_force_transition != input.allow_force_transition {
        changes.insert(
            "allow_force_transition".into(),
            serde_json::json!({
                "from": project.allow_force_transition,
                "to": input.allow_force_transition,
            }),
        );
        let mut model = project.clone().into_active_model();
        model.allow_force_transition = Set(input.allow_force_transition);
        model.update(&txn).await?;
    }
    if changes.is_empty() {
        txn.rollback().await?;
        return Ok(0);
    }
    changes.insert("tickets".into(), serde_json::json!(moved));
    audit::record(
        &txn,
        acting.actor().audit(
            AuditAction::ProjectWorkflowsChanged,
            (AuditSubject::Project, project.id),
            serde_json::Value::Object(changes),
        ),
    )
    .await?;
    txn.commit().await?;
    Ok(moved)
}
