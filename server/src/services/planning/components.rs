//! Components project: bagian produk dengan pemilik dan modul code graph (`<repo>/<modul>`).
//! Kelola = `AdministerProject`. Aturan project (`projects.planning`): pemilik component jadi
//! penerima default tiket baru tanpa penerima.
use std::collections::HashMap;

use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr,
    EntityTrait, IntoActiveModel, PaginatorTrait, QueryFilter, SqlErr,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Failure, NAME_INVALID, NAME_TAKEN, OWNER_INVALID, TEXT_TOO_LONG};
use crate::{
    models::{
        components::{self, DESCRIPTION_MAX_LEN, MODULES_MAX, NAME_MAX_LEN},
        graph_repos,
        permission::Permission,
        projects, repos, tickets,
        workspaces::WorkspaceId,
    },
    services::{
        tickets::{access_in, create::has_access},
        workspaces::Acting,
    },
};

/// Aturan planning per project (`projects.planning`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanningRules {
    /// Pemilik component jadi penerima default tiket baru tanpa penerima.
    #[serde(default = "yes")]
    pub component_assignee: bool,
}

fn yes() -> bool {
    true
}

impl Default for PlanningRules {
    fn default() -> Self {
        Self {
            component_assignee: true,
        }
    }
}

impl PlanningRules {
    #[must_use]
    pub fn of(project: &projects::Model) -> Self {
        serde_json::from_value(project.planning.clone()).unwrap_or_default()
    }
}

/// Isian component mentah.
#[derive(Debug, Clone, Default)]
pub struct ComponentInput {
    pub name: String,
    pub description: String,
    /// Id user atau kosong.
    pub owner: String,
    pub modules: Vec<String>,
}

/// Penerima default dari component (pemilik) bila aturannya aktif dan pemilik masih punya akses.
///
/// # Errors
///
/// Error database.
pub async fn default_assignee<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project: &projects::Model,
    component_id: Uuid,
) -> Result<Option<Uuid>, DbErr> {
    if !PlanningRules::of(project).component_assignee {
        return Ok(None);
    }
    let Some(owner) = components::Model::find_in(db, wid, component_id)
        .await?
        .and_then(|c| c.owner_id)
    else {
        return Ok(None);
    };
    Ok(has_access(db, wid, project.id, owner)
        .await?
        .then_some(owner))
}

/// Modul code graph project (`<repo>/<modul>`), urut.
///
/// # Errors
///
/// Error database.
pub async fn graph_modules<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project_id: Uuid,
) -> Result<Vec<String>, DbErr> {
    let names: HashMap<Uuid, String> = repos::Entity::find()
        .filter(repos::Column::WorkspaceId.eq(wid.get()))
        .filter(repos::Column::ProjectId.eq(project_id))
        .all(db)
        .await?
        .into_iter()
        .map(|r| (r.id, r.name))
        .collect();
    let mut out = Vec::new();
    for g in graph_repos::Model::of_project(db, wid, project_id).await? {
        let repo = names.get(&g.repo_id).cloned().unwrap_or_default();
        for m in g.modules() {
            out.push(format!("{repo}/{}", m.name.trim_start_matches('/')));
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

/// Jumlah tiket aktif per component.
///
/// # Errors
///
/// Error database.
pub async fn ticket_counts<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    ids: &[Uuid],
) -> Result<HashMap<Uuid, u64>, DbErr> {
    let mut out = HashMap::new();
    for id in ids {
        let n = tickets::Entity::find()
            .filter(tickets::Column::WorkspaceId.eq(wid.get()))
            .filter(tickets::Column::ComponentId.eq(*id))
            .filter(tickets::Column::DeletedAt.is_null())
            .count(db)
            .await?;
        out.insert(*id, n);
    }
    Ok(out)
}

async fn validate<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project_id: Uuid,
    input: &ComponentInput,
) -> Result<(String, String, Option<Uuid>, Vec<String>), Failure> {
    let name = input.name.trim().to_string();
    if name.is_empty() || name.chars().count() > NAME_MAX_LEN {
        return Err(Failure::field("name", NAME_INVALID));
    }
    let description = input.description.trim().to_string();
    if description.chars().count() > DESCRIPTION_MAX_LEN {
        return Err(Failure::field("description", TEXT_TOO_LONG));
    }
    let owner = match input.owner.trim() {
        "" => None,
        raw => {
            let id =
                uuid::Uuid::parse_str(raw).map_err(|_| Failure::field("owner", OWNER_INVALID))?;
            if !has_access(db, wid, project_id, id).await? {
                return Err(Failure::field("owner", OWNER_INVALID));
            }
            Some(id)
        }
    };
    let mut modules: Vec<String> = input
        .modules
        .iter()
        .map(|m| m.trim().to_string())
        .filter(|m| !m.is_empty() && m.chars().count() <= 300)
        .collect();
    modules.sort();
    modules.dedup();
    modules.truncate(MODULES_MAX);
    Ok((name, description, owner, modules))
}

fn taken(e: sea_orm::DbErr) -> Failure {
    if matches!(e.sql_err(), Some(SqlErr::UniqueConstraintViolation(_))) {
        Failure::field("name", NAME_TAKEN)
    } else {
        Failure::Db(e)
    }
}

/// Buat component.
///
/// # Errors
///
/// Validasi, `NotFound`, error database.
pub async fn create(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    project_id: Uuid,
    input: &ComponentInput,
) -> Result<components::Model, Failure> {
    let wid = acting.workspace_id();
    access_in(db, acting, project_id, Permission::AdministerProject).await?;
    let (name, description, owner, modules) = validate(db, wid, project_id, input).await?;
    components::ActiveModel {
        workspace_id: Set(wid.get()),
        project_id: Set(project_id),
        name: Set(name),
        description: Set(description),
        owner_id: Set(owner),
        modules: Set(modules),
        ..Default::default()
    }
    .insert(db)
    .await
    .map_err(taken)
}

async fn editable(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
) -> Result<components::Model, Failure> {
    let c = components::Model::find_in(db, acting.workspace_id(), id)
        .await?
        .ok_or(Failure::NotFound)?;
    access_in(db, acting, c.project_id, Permission::AdministerProject).await?;
    Ok(c)
}

/// Ubah component.
///
/// # Errors
///
/// Validasi, `NotFound`, error database.
pub async fn update(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
    input: &ComponentInput,
) -> Result<components::Model, Failure> {
    let c = editable(db, acting, id).await?;
    let (name, description, owner, modules) =
        validate(db, acting.workspace_id(), c.project_id, input).await?;
    let mut m = c.into_active_model();
    m.name = Set(name);
    m.description = Set(description);
    m.owner_id = Set(owner);
    m.modules = Set(modules);
    m.update(db).await.map_err(taken)
}

/// Hapus component (tiketnya kehilangan component).
///
/// # Errors
///
/// `NotFound`, error database.
pub async fn delete(db: &DatabaseConnection, acting: &Acting<'_>, id: Uuid) -> Result<(), Failure> {
    let c = editable(db, acting, id).await?;
    components::Entity::delete_by_id(c.id).exec(db).await?;
    Ok(())
}

/// Simpan aturan planning project.
///
/// # Errors
///
/// `NotFound`, error database.
pub async fn save_rules(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    project_id: Uuid,
    rules: &PlanningRules,
) -> Result<(), Failure> {
    access_in(db, acting, project_id, Permission::AdministerProject).await?;
    let project = projects::Model::find_in(db, acting.workspace_id(), project_id)
        .await?
        .ok_or(Failure::NotFound)?;
    let mut m = project.into_active_model();
    m.planning = Set(serde_json::to_value(rules).unwrap_or_default());
    m.update(db).await?;
    Ok(())
}
