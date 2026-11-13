//! Katalog jenis dokumen untuk satu project: jenis bawaan di kelompok yang relevan dengan kode
//! (dari code graph) + jenis custom workspace yang berlaku untuk project itu.

use sea_orm::{ColumnTrait, ConnectionTrait, DbErr, EntityTrait, PaginatorTrait, QueryFilter};
use uuid::Uuid;

use crate::models::{
    graph_files, graph_repos, knowledge_catalog, knowledge_types, workspaces::WorkspaceId,
};

/// Satu jenis dokumen yang tersedia (bawaan atau custom).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TypeInfo {
    pub key: String,
    pub name: String,
    pub group: String,
    pub hint: String,
    /// Instruksi AI (jenis custom).
    pub instructions: String,
    /// Template isi awal (jenis custom).
    pub template: String,
    pub custom: bool,
    /// Ikut dibuat saat scan AI.
    pub ai_scan: bool,
}

impl From<&knowledge_catalog::DocType> for TypeInfo {
    fn from(t: &knowledge_catalog::DocType) -> Self {
        Self {
            key: t.key.into(),
            name: t.name.into(),
            group: t.group.into(),
            hint: t.hint.into(),
            instructions: String::new(),
            template: String::new(),
            custom: false,
            ai_scan: true,
        }
    }
}

impl From<knowledge_types::Model> for TypeInfo {
    fn from(t: knowledge_types::Model) -> Self {
        Self {
            key: t.key,
            name: t.name,
            group: t.group_key,
            hint: String::new(),
            instructions: t.instructions,
            template: t.template,
            custom: true,
            ai_scan: t.ai_scan,
        }
    }
}

/// Kelompok yang hanya relevan bila kode menunjukkannya.
const CONDITIONAL: &[&str] = &["frontend", "mobile", "data"];

/// Kelompok yang relevan untuk project dari bahasa/path di code graph. Tanpa graph: semua
/// kelompok dianggap relevan.
///
/// # Errors
///
/// Error database.
pub async fn relevant_groups<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project_id: Uuid,
) -> Result<Vec<&'static str>, DbErr> {
    let graphs = graph_repos::Model::of_project(db, wid, project_id).await?;
    if graphs.iter().all(|g| g.files_indexed == 0) {
        return Ok(knowledge_catalog::GROUPS.iter().map(|g| g.key).collect());
    }
    let langs: Vec<String> = graphs
        .iter()
        .flat_map(|g| g.languages().into_iter().map(|(l, _)| l))
        .collect();
    let repo_ids: Vec<Uuid> = graphs.iter().map(|g| g.repo_id).collect();
    let has_path = |patterns: &'static [&'static str]| {
        let repo_ids = repo_ids.clone();
        async move {
            let mut cond = sea_orm::Condition::any();
            for p in patterns {
                cond = cond.add(graph_files::Column::Path.like(*p));
            }
            graph_files::Entity::find()
                .filter(graph_files::Column::WorkspaceId.eq(wid.get()))
                .filter(graph_files::Column::RepoId.is_in(repo_ids))
                .filter(cond)
                .count(db)
                .await
                .map(|n| n > 0)
        }
    };
    let has_lang = |names: &[&str]| langs.iter().any(|l| names.contains(&l.as_str()));
    let frontend = has_lang(&["tsx"])
        || has_path(&["%.vue", "%.svelte", "%.jsx", "%.css", "%.scss", "%.html"]).await?;
    let mobile = has_lang(&["dart", "swift"])
        || (has_lang(&["kotlin"]) && has_path(&["%AndroidManifest.xml"]).await?);
    let data = has_path(&[
        "%.sql",
        "%.ipynb",
        "%dbt_project.yml",
        "%/etl/%",
        "%/pipelines/%",
    ])
    .await?;
    Ok(knowledge_catalog::GROUPS
        .iter()
        .map(|g| g.key)
        .filter(|k| {
            !CONDITIONAL.contains(k)
                || (*k == "frontend" && frontend)
                || (*k == "mobile" && mobile)
                || (*k == "data" && data)
        })
        .collect())
}

/// Semua jenis yang bisa dipakai di project (bawaan semua kelompok + custom yang berlaku).
///
/// # Errors
///
/// Error database.
pub async fn available_types<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project_id: Uuid,
) -> Result<Vec<TypeInfo>, DbErr> {
    let mut out: Vec<TypeInfo> = knowledge_catalog::TYPES
        .iter()
        .map(TypeInfo::from)
        .collect();
    out.extend(
        knowledge_types::Model::active_in(db, wid)
            .await?
            .into_iter()
            .filter(|t| t.applies_to(project_id))
            .map(TypeInfo::from),
    );
    Ok(out)
}

/// Jenis yang dibuat saat scan: kelompok relevan + custom dengan "ikut scan AI".
///
/// # Errors
///
/// Error database.
pub async fn relevant_types<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project_id: Uuid,
) -> Result<Vec<TypeInfo>, DbErr> {
    let groups = relevant_groups(db, wid, project_id).await?;
    Ok(available_types(db, wid, project_id)
        .await?
        .into_iter()
        .filter(|t| t.ai_scan && groups.contains(&t.group.as_str()))
        .collect())
}

/// Nama jenis dari kunci (bawaan atau custom), atau kunci itu sendiri.
#[must_use]
pub fn type_name(types: &[TypeInfo], key: &str) -> String {
    types
        .iter()
        .find(|t| t.key == key)
        .map_or_else(|| key.to_string(), |t| t.name.clone())
}
