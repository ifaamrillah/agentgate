//! Draft tiket brainstorming: disimpan agent lewat `ag_draft_tickets`, ditinjau manusia di
//! halaman "Draft tiket" project (centang, ubah, hapus), lalu "Buat N tiket" membuat tiket
//! sungguhan dengan hierarki induknya. Semua langkah butuh `EditProject` di project itu.
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, DatabaseConnection, EntityTrait, IntoActiveModel,
    TransactionTrait,
};
use uuid::Uuid;

use crate::{
    models::{
        permission::Permission,
        ticket_drafts::{self, MAX_PER_CALL, MAX_PER_PROJECT},
        ticket_types,
        tickets::{self, TicketInput},
    },
    services::{
        tickets::{
            TicketFailure, access_in,
            create::{self, TicketForm},
        },
        workspaces::Acting,
    },
};

pub const TOO_MANY: &str = "draft-too-many";
pub const TYPE_INVALID: &str = "draft-type-invalid";
pub const PARENT_INVALID: &str = "draft-parent-invalid";
pub const NONE_SELECTED: &str = "draft-none-selected";
pub const ERROR_CODES: &[&str] = &[TOO_MANY, TYPE_INVALID, PARENT_INVALID, NONE_SELECTED];

/// Satu draft dari agent. `reference` = id sementara agar draft lain bisa menunjuknya sebagai
/// induk (`parent`).
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct DraftInput {
    #[serde(default, rename = "ref")]
    pub reference: String,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default, rename = "type")]
    pub type_key: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub story_points: Option<i32>,
    /// Estimasi AI per stage (disalin ke `ticket_estimates` saat tiket dibuat).
    #[serde(default)]
    pub estimate: Option<DraftEstimate>,
}

/// Estimasi AI pada draft: jam per stage dan dasar estimasinya.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct DraftEstimate {
    #[serde(default)]
    pub stages: Vec<crate::models::ticket_estimates::StageHours>,
    #[serde(default)]
    pub basis: String,
    #[serde(default)]
    pub persona: String,
}

/// Isian ubah draft dari halaman review.
#[derive(Debug, Clone, Default)]
pub struct DraftEdit {
    pub title: String,
    pub description: String,
    pub type_key: String,
    pub story_points: String,
}

fn validate(
    title: &str,
    description: &str,
    story_points: &str,
) -> Result<tickets::ValidTicket, TicketFailure> {
    tickets::validate_input(&TicketInput {
        title: title.to_string(),
        description: description.to_string(),
        story_points: story_points.to_string(),
        ..TicketInput::default()
    })
    .map_err(TicketFailure::errors)
}

/// Simpan draft dari agent (ditambahkan setelah draft yang sudah ada).
///
/// # Errors
///
/// `NotFound` (tanpa `EditProject`), `Invalid` (judul, tipe, induk, jumlah), error database.
pub async fn save_from_agent(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    project_id: Uuid,
    source_ticket_id: Option<Uuid>,
    inputs: &[DraftInput],
) -> Result<Vec<ticket_drafts::Model>, TicketFailure> {
    let wid = acting.workspace_id();
    let txn = db.begin().await?;
    access_in(&txn, acting, project_id, Permission::EditProject).await?;
    let existing = ticket_drafts::Model::count_in_project(&txn, wid, project_id).await?;
    if inputs.is_empty()
        || inputs.len() > MAX_PER_CALL
        || existing + inputs.len() as u64 > MAX_PER_PROJECT
    {
        return Err(TicketFailure::field("drafts", TOO_MANY));
    }
    let types = ticket_types::Model::all_in(&txn, wid).await?;
    let mut saved: Vec<(String, ticket_drafts::Model)> = Vec::new();
    for (i, input) in inputs.iter().enumerate() {
        let points = input
            .story_points
            .map(|p| p.to_string())
            .unwrap_or_default();
        let valid = validate(&input.title, &input.description, &points)?;
        let type_key = input.type_key.trim().to_lowercase();
        let type_key = if type_key.is_empty() {
            "task".to_string()
        } else {
            type_key
        };
        if !types.iter().any(|t| t.key == type_key) {
            return Err(TicketFailure::field("type", TYPE_INVALID));
        }
        let parent_id = match input
            .parent
            .as_deref()
            .map(str::trim)
            .filter(|p| !p.is_empty())
        {
            None => None,
            Some(r) => Some(
                saved
                    .iter()
                    .find(|(reference, _)| reference == r)
                    .map(|(_, d)| d.id)
                    .ok_or_else(|| TicketFailure::field("parent", PARENT_INVALID))?,
            ),
        };
        let estimate = match &input.estimate {
            Some(e) => {
                let valid = crate::services::planning::estimates::validate(
                    &crate::services::planning::estimates::EstimateInput {
                        points: valid.story_points,
                        stages: e.stages.clone(),
                        basis: e.basis.clone(),
                    },
                )?;
                serde_json::to_value(DraftEstimate {
                    stages: valid.stages,
                    basis: valid.basis,
                    persona: e.persona.chars().take(60).collect(),
                })
                .unwrap_or_default()
            }
            None => serde_json::json!({}),
        };
        let draft = ticket_drafts::ActiveModel {
            estimate: Set(estimate),
            workspace_id: Set(wid.get()),
            project_id: Set(project_id),
            source_ticket_id: Set(source_ticket_id),
            parent_id: Set(parent_id),
            type_key: Set(type_key),
            title: Set(valid.title),
            description: Set(valid.description),
            story_points: Set(valid.story_points),
            position: Set(i32::try_from(existing).unwrap_or(0) + i32::try_from(i).unwrap_or(0)),
            approved: Set(true),
            created_by: Set(Some(acting.user.id)),
            ..Default::default()
        }
        .insert(&txn)
        .await?;
        saved.push((input.reference.trim().to_string(), draft));
    }
    txn.commit().await?;
    Ok(saved.into_iter().map(|(_, d)| d).collect())
}

/// Ubah satu draft (judul, deskripsi, tipe, poin).
///
/// # Errors
///
/// `NotFound`, `Invalid`, error database.
pub async fn update(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    project_id: Uuid,
    id: Uuid,
    edit: &DraftEdit,
) -> Result<ticket_drafts::Model, TicketFailure> {
    let wid = acting.workspace_id();
    access_in(db, acting, project_id, Permission::EditProject).await?;
    let draft = ticket_drafts::Model::find_in_project(db, wid, project_id, id)
        .await?
        .ok_or(TicketFailure::NotFound)?;
    let valid = validate(&edit.title, &edit.description, &edit.story_points)?;
    let types = ticket_types::Model::all_in(db, wid).await?;
    if !types.iter().any(|t| t.key == edit.type_key) {
        return Err(TicketFailure::field("type", TYPE_INVALID));
    }
    let mut model = draft.into_active_model();
    model.title = Set(valid.title);
    model.description = Set(valid.description);
    model.story_points = Set(valid.story_points);
    model.type_key = Set(edit.type_key.clone());
    Ok(model.update(db).await?)
}

/// Hapus draft (beserta turunannya, FK cascade).
///
/// # Errors
///
/// `NotFound`, error database.
pub async fn delete(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    project_id: Uuid,
    id: Uuid,
) -> Result<(), TicketFailure> {
    let wid = acting.workspace_id();
    access_in(db, acting, project_id, Permission::EditProject).await?;
    let draft = ticket_drafts::Model::find_in_project(db, wid, project_id, id)
        .await?
        .ok_or(TicketFailure::NotFound)?;
    draft.into_active_model().delete(db).await?;
    Ok(())
}

/// Buat tiket dari draft yang dicentang `ids` (urutan induk dulu), hapus draftnya. Draft yang
/// induknya tidak ikut dibuat menjadi tiket tanpa induk. Mengembalikan tiket yang dibuat.
///
/// # Errors
///
/// `NotFound`, `Invalid` (tiket tidak valid, tidak ada yang dicentang), error database.
pub async fn create_tickets(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    project_id: Uuid,
    ids: &[Uuid],
) -> Result<Vec<tickets::Model>, TicketFailure> {
    let wid = acting.workspace_id();
    let txn = db.begin().await?;
    access_in(&txn, acting, project_id, Permission::EditProject).await?;
    let all = ticket_drafts::Model::of_project(&txn, wid, project_id).await?;
    let chosen: Vec<&ticket_drafts::Model> = all.iter().filter(|d| ids.contains(&d.id)).collect();
    if chosen.is_empty() {
        return Err(TicketFailure::field("drafts", NONE_SELECTED));
    }
    let types = ticket_types::Model::all_in(&txn, wid).await?;
    // Urutan topologis sederhana: induk sebelum anak (kedalaman hierarki kecil).
    let mut order: Vec<&ticket_drafts::Model> = Vec::new();
    let mut remaining = chosen.clone();
    while !remaining.is_empty() {
        let before = remaining.len();
        remaining.retain(|d| {
            let ready = d
                .parent_id
                .is_none_or(|p| !ids.contains(&p) || order.iter().any(|o| o.id == p));
            if ready {
                order.push(d);
            }
            !ready
        });
        if remaining.len() == before {
            order.append(&mut remaining);
        }
    }
    let mut created: Vec<(Uuid, tickets::Model)> = Vec::new();
    for d in order {
        let type_id = types
            .iter()
            .find(|t| t.key == d.type_key)
            .map(|t| t.id.to_string())
            .unwrap_or_default();
        let parent = d
            .parent_id
            .and_then(|p| created.iter().find(|(id, _)| *id == p))
            .map(|(_, t)| t.code.clone())
            .unwrap_or_default();
        let form = TicketForm {
            input: TicketInput {
                title: d.title.clone(),
                description: d.description.clone(),
                story_points: d.story_points.map(|p| p.to_string()).unwrap_or_default(),
                status: "backlog".into(),
                ..TicketInput::default()
            },
            type_id,
            parent,
            ..TicketForm::default()
        };
        let ticket = create::run_in(&txn, acting, project_id, &form).await?;
        if let Some(e) = d.estimate() {
            crate::services::planning::estimates::record_in(
                &txn,
                acting.workspace_id(),
                &ticket,
                crate::models::ticket_estimates::EstimateSource::Ai,
                &crate::services::planning::estimates::EstimateInput {
                    points: d.story_points,
                    stages: e.stages,
                    basis: e.basis,
                },
                Some(acting.user.id),
                &e.persona,
            )
            .await?;
        }
        created.push((d.id, ticket));
    }
    // Draft yang tidak dibuat kehilangan induknya yang sudah jadi tiket (bukan ikut terhapus).
    for d in all.iter().filter(|d| !ids.contains(&d.id)) {
        if d.parent_id.is_some_and(|p| ids.contains(&p)) {
            let mut m = d.clone().into_active_model();
            m.parent_id = Set(None);
            m.update(&txn).await?;
        }
    }
    for d in &chosen {
        ticket_drafts::Entity::delete_by_id(d.id).exec(&txn).await?;
    }
    txn.commit().await?;
    Ok(created.into_iter().map(|(_, t)| t).collect())
}
