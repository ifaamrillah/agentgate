//! Pemicu update knowledge otomatis (masing-masing bisa dimatikan per workspace di Pengaturan →
//! Knowledge workspace → Update otomatis):
//!
//! - **Tiket Done**: dokumen kandidat (sumbernya di repo yang punya PR tiket itu dan filenya
//!   berubah sejak dokumen diperbarui, atau menyebut kode tiket) → run `update`. Tanpa kandidat,
//!   agent memeriksa sendiri dokumen mana yang terdampak tiket.
//! - **Graph di-rebuild** (setelah merge/index): dokumen yang file sumbernya berubah ditandai basi
//!   → run `update`.
//! - **Audit mingguan** (task `knowledge_audit`): semua project ber-dokumen → run `audit`.

use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, FromQueryResult,
    IntoActiveModel, QueryFilter, Set, Statement,
};
use uuid::Uuid;

use super::{docs, runs};
use crate::models::{
    knowledge_docs,
    knowledge_runs::{RunKind, RunTrigger},
    knowledge_settings, knowledge_types, projects, pull_requests, repos, tickets,
    workspaces::{self, WorkspaceId},
};

#[derive(Debug, FromQueryResult)]
struct DocId {
    doc_id: Uuid,
}

/// Dokumen aktif (belum basi) yang file sumbernya di `repo` berubah sejak dokumen terakhir
/// diperbarui.
async fn changed_docs<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    repo: Uuid,
    only_fresh: bool,
) -> Result<Vec<Uuid>, DbErr> {
    let sql = format!(
        "SELECT DISTINCT s.doc_id FROM knowledge_sources s \
         JOIN knowledge_docs d ON d.id = s.doc_id \
         JOIN graph_files f ON f.repo_id = s.repo_id \
           AND (s.path = '' OR f.path = s.path OR f.path LIKE s.path || '/%') \
         WHERE s.workspace_id = $1 AND s.repo_id = $2 AND d.archived_at IS NULL \
           AND f.updated_at > d.updated_at{}",
        if only_fresh {
            " AND d.stale_reason = ''"
        } else {
            ""
        }
    );
    Ok(DocId::find_by_statement(Statement::from_sql_and_values(
        db.get_database_backend(),
        sql,
        [wid.get().into(), repo.into()],
    ))
    .all(db)
    .await?
    .into_iter()
    .map(|r| r.doc_id)
    .collect())
}

/// Graph repo baru di-rebuild: tandai basi dokumen yang sumbernya berubah dan minta run update.
///
/// # Errors
///
/// Error database.
pub async fn on_graph_rebuilt<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    repo: &repos::Model,
) -> Result<(), DbErr> {
    let settings = knowledge_settings::Model::of(db, wid).await?;
    if !settings.on_merge {
        return Ok(());
    }
    let ids = changed_docs(db, wid, repo.id, true).await?;
    if ids.is_empty() {
        return Ok(());
    }
    docs::mark_stale(
        db,
        wid,
        &ids,
        &format!(
            "Kode sumber di {} berubah sejak dokumen diperbarui",
            repo.name
        ),
    )
    .await?;
    let Some(project) = projects::Model::find_in(db, wid, repo.project_id).await? else {
        return Ok(());
    };
    runs::request_system(
        db,
        wid,
        &project,
        RunKind::Update,
        RunTrigger::Merge,
        ids,
        None,
        &format!("Graph {} di-rebuild", repo.name),
    )
    .await?;
    Ok(())
}

/// Tiket baru masuk kategori Done: minta run update dokumen terdampak.
///
/// # Errors
///
/// Error database.
pub async fn on_ticket_done<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    ticket: &tickets::Model,
) -> Result<(), DbErr> {
    let settings = knowledge_settings::Model::of(db, wid).await?;
    if !settings.on_done {
        return Ok(());
    }
    let Some(project) = projects::Model::find_in(db, wid, ticket.project_id).await? else {
        return Ok(());
    };
    let docs = knowledge_docs::Model::of_project(db, wid, project.id).await?;
    if docs.is_empty() {
        return Ok(());
    }
    let mut ids: Vec<Uuid> = Vec::new();
    let prs = pull_requests::Entity::find()
        .filter(pull_requests::Column::WorkspaceId.eq(wid.get()))
        .filter(pull_requests::Column::TicketId.eq(ticket.id))
        .all(db)
        .await?;
    let mut repo_ids: Vec<Uuid> = prs.iter().map(|p| p.repo_id).collect();
    repo_ids.dedup();
    for r in repo_ids {
        for id in changed_docs(db, wid, r, false).await? {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
    }
    let on_done_types: Vec<String> = knowledge_types::Model::active_in(db, wid)
        .await?
        .into_iter()
        .filter(|t| t.on_done && t.applies_to(project.id))
        .map(|t| t.key)
        .collect();
    for d in &docs {
        if (d.body.contains(&ticket.code) || on_done_types.contains(&d.type_key))
            && !ids.contains(&d.id)
        {
            ids.push(d.id);
        }
    }
    runs::request_system(
        db,
        wid,
        &project,
        RunKind::Update,
        RunTrigger::Done,
        ids,
        Some(ticket.id),
        &format!("Tiket {} selesai: {}", ticket.code, ticket.title),
    )
    .await?;
    Ok(())
}

/// Ringkasan audit satu workspace.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AuditReport {
    pub projects: usize,
    pub runs: usize,
}

/// Audit mingguan: untuk workspace yang mengaktifkan audit dan hari ini = hari audit (dan belum
/// diaudit 6 hari terakhir), minta run `audit` di setiap project yang punya dokumen.
/// `force` mengabaikan hari dan jeda (task manual).
///
/// # Errors
///
/// Error database.
pub async fn weekly_audit<C: ConnectionTrait>(
    db: &C,
    today: chrono::DateTime<chrono::Utc>,
    force: bool,
) -> Result<AuditReport, DbErr> {
    use chrono::Datelike;
    let mut report = AuditReport::default();
    let weekday = i16::try_from(today.weekday().num_days_from_sunday()).unwrap_or(0);
    for ws in workspaces::Entity::find()
        .filter(workspaces::Column::DeletionScheduledAt.is_null())
        .all(db)
        .await?
    {
        let wid = ws.workspace_id();
        let settings = knowledge_settings::Model::of(db, wid).await?;
        if !settings.audit && !force {
            continue;
        }
        let recent = settings
            .last_audit_at
            .is_some_and(|t| today.signed_duration_since(t) < chrono::Duration::days(6));
        if !force && (settings.audit_day != weekday || recent) {
            continue;
        }
        for project in projects::Entity::find()
            .filter(projects::Column::WorkspaceId.eq(wid.get()))
            .all(db)
            .await?
        {
            if knowledge_docs::Model::of_project(db, wid, project.id)
                .await?
                .is_empty()
            {
                continue;
            }
            report.projects += 1;
            if runs::request_system(
                db,
                wid,
                &project,
                RunKind::Audit,
                RunTrigger::Audit,
                Vec::new(),
                None,
                "Audit mingguan",
            )
            .await?
            .is_some()
            {
                report.runs += 1;
            }
        }
        let mut m = match knowledge_settings::Entity::find_by_id(wid.get())
            .one(db)
            .await?
        {
            Some(s) => s.into_active_model(),
            None => {
                let d = knowledge_settings::Model::defaults(wid);
                knowledge_settings::ActiveModel {
                    workspace_id: Set(d.workspace_id),
                    on_done: Set(d.on_done),
                    on_merge: Set(d.on_merge),
                    audit: Set(d.audit),
                    audit_day: Set(d.audit_day),
                    default_depth: Set(d.default_depth),
                    repo_sync: Set(d.repo_sync),
                    updated_at: Set(d.updated_at),
                    ..Default::default()
                }
                .insert(db)
                .await?
                .into_active_model()
            }
        };
        m.last_audit_at = Set(Some(today.into()));
        m.update(db).await?;
    }
    Ok(report)
}
