//! Code graph (M7): index repo dari CLI atau clone server, build graph dari fakta tree-sitter
//! (plus SCIP bila ada), relasi lintas repo per project, dan query untuk tool MCP dan UI.
//!
//! - [`ingest`]: manifest → upload → finish (CLI), SCIP, dan index dari direktori (server).
//! - [`build`]: bangun ulang simbol/edge satu repo dari `graph_files`, lalu hitung ulang relasi
//!   lintas repo project.
//! - [`query`]: cari simbol, trace, snippet, dampak, kode mati, query Cypher.
pub mod build;
pub mod clone;
pub mod fetch;
pub mod ingest;
pub mod query;
pub mod viewer;

use agentgate_graph::protocol::IndexSummary;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DbErr, EntityTrait,
    PaginatorTrait, QueryFilter,
};
use uuid::Uuid;

use crate::{
    models::{
        graph_links, graph_repos, permission::Permission, projects, repos, workspaces::WorkspaceId,
    },
    services::{
        tickets::{TicketFailure, access_in},
        workspaces::Acting,
    },
};

pub(crate) fn now() -> chrono::DateTime<chrono::FixedOffset> {
    chrono::Utc::now().into()
}

/// Project `key` yang boleh dibuka pelaku dengan izin `permission`.
///
/// # Errors
///
/// `NotFound` bila project tidak ada atau izin kurang; error database.
pub async fn project_for<C: ConnectionTrait>(
    db: &C,
    acting: &Acting<'_>,
    key: &str,
    permission: Permission,
) -> Result<projects::Model, TicketFailure> {
    let project = projects::Model::accessible_for(db, &acting.accessor())
        .await?
        .into_iter()
        .find(|p| p.key.eq_ignore_ascii_case(key.trim()))
        .ok_or(TicketFailure::NotFound)?;
    access_in(db, acting, project.id, permission).await?;
    Ok(project)
}

/// Repo `name` di project `key` (izin `EditProject`, sama dengan `agentgate rules sync`).
///
/// # Errors
///
/// `NotFound`; error database.
pub async fn repo_for<C: ConnectionTrait>(
    db: &C,
    acting: &Acting<'_>,
    key: &str,
    name: &str,
) -> Result<(projects::Model, repos::Model), TicketFailure> {
    let project = project_for(db, acting, key, Permission::EditProject).await?;
    let repo = repos::Model::of_project(db, acting.workspace_id(), project.id)
        .await?
        .into_iter()
        .find(|r| r.name.eq_ignore_ascii_case(name.trim()))
        .ok_or(TicketFailure::NotFound)?;
    Ok((project, repo))
}

/// Baris `graph_repos` repo (dibuat bila belum ada).
///
/// # Errors
///
/// Error database.
pub async fn ensure_row<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    repo: &repos::Model,
) -> Result<graph_repos::Model, DbErr> {
    if let Some(row) = graph_repos::Model::of_repo(db, wid, repo.id).await? {
        return Ok(row);
    }
    graph_repos::ActiveModel {
        repo_id: Set(repo.id),
        workspace_id: Set(repo.workspace_id),
        project_id: Set(repo.project_id),
        ..Default::default()
    }
    .insert(db)
    .await
}

/// Jumlah relasi lintas repo yang berawal di repo.
///
/// # Errors
///
/// Error database.
pub async fn links_from<C: ConnectionTrait>(db: &C, repo: Uuid) -> Result<u64, DbErr> {
    graph_links::Entity::find()
        .filter(graph_links::Column::FromRepoId.eq(repo))
        .count(db)
        .await
}

/// Ringkasan index untuk CLI.
///
/// # Errors
///
/// Error database.
pub async fn summary<C: ConnectionTrait>(
    db: &C,
    repo: &repos::Model,
    row: &graph_repos::Model,
) -> Result<IndexSummary, DbErr> {
    let links = links_from(db, repo.id).await?;
    Ok(IndexSummary {
        repo: repo.name.clone(),
        status: row.status.clone(),
        commit: row.commit_sha.clone(),
        files_indexed: row.files_indexed,
        files_skipped: row.files_skipped,
        symbols: row.symbols,
        edges: row.edges,
        edges_certain: row.edges_certain,
        links: i32::try_from(links).unwrap_or(i32::MAX),
        scip_indexer: row.scip_indexer.clone(),
        indexed_at: row.indexed_at.map(|t| t.to_rfc3339()),
        error: row.error.clone(),
    })
}

/// Status index semua repo project `key` (izin `ViewProject`).
///
/// # Errors
///
/// `NotFound`; error database.
pub async fn status(
    db: &sea_orm::DatabaseConnection,
    acting: &Acting<'_>,
    key: &str,
) -> Result<Vec<IndexSummary>, TicketFailure> {
    let project = project_for(db, acting, key, Permission::ViewProject).await?;
    let wid = acting.workspace_id();
    let mut out = Vec::new();
    for repo in repos::Model::of_project(db, wid, project.id).await? {
        let row = match graph_repos::Model::of_repo(db, wid, repo.id).await? {
            Some(r) => r,
            None => graph_repos::Model {
                repo_id: repo.id,
                workspace_id: repo.workspace_id,
                project_id: repo.project_id,
                status: "none".into(),
                ..empty_row()
            },
        };
        out.push(summary(db, &repo, &row).await?);
    }
    Ok(out)
}

/// Baris kosong untuk repo yang belum pernah di-index (tidak disimpan).
#[must_use]
pub fn empty_row() -> graph_repos::Model {
    graph_repos::Model::empty()
}

/// Antrekan index server untuk repo (status `queued`, lalu `GraphIndexWorker`).
///
/// # Errors
///
/// Error database atau antrean.
pub async fn enqueue(
    ctx: &loco_rs::app::AppContext,
    repo: &repos::Model,
    by: Option<Uuid>,
    full: bool,
) -> loco_rs::Result<()> {
    use loco_rs::bgworker::BackgroundWorker;
    crate::workers::graph_index::GraphIndexWorker::perform_later(
        ctx,
        crate::workers::graph_index::GraphIndexArgs {
            workspace_id: repo.workspace_id,
            repo_id: repo.id,
            requested_by: by,
            full,
        },
    )
    .await
    .map(|_| ())
}
