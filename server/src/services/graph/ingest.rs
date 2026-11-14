//! Masukan index: CLI mengirim manifest (path + sha), server menjawab file yang berubah, CLI
//! meng-upload isinya per batch, lalu `finish` membangun graph. Server sendiri meng-index
//! direktori hasil clone lewat [`index_dir`] dengan aturan yang sama.
//!
//! Aturan file (spec "Keamanan code graph"): file rahasia (`.env`, kunci privat, kredensial)
//! tidak pernah disimpan isinya walau dikirim; file di atas 1 MB, hasil generate, dan biner
//! dicatat sebagai dilewati beserta alasannya.
use std::{collections::HashMap, path::Path};

use agentgate_graph::{
    Lang,
    history::GitStats,
    protocol::{
        IndexSummary, ManifestRequest, ManifestResponse, ScipRequest, UploadFile, UploadRequest,
        UploadResponse,
    },
    walk::{self, MAX_BYTES, SkipReason},
};
use sea_orm::{
    ActiveEnum, ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, DbErr,
    EntityTrait, IntoActiveModel, QueryFilter, sea_query::OnConflict,
};
use uuid::Uuid;

use super::{build, ensure_row, now, repo_for, summary};
use crate::{
    models::{
        graph_files, graph_repos,
        graph_repos::{IndexMode, IndexSource, IndexStatus},
        repos,
        workspaces::WorkspaceId,
    },
    services::{tickets::TicketFailure, workspaces::Acting},
};

/// Batas file per manifest.
pub const MAX_FILES: usize = 100_000;
/// Batas file per batch upload.
pub const MAX_BATCH: usize = agentgate_graph::protocol::UPLOAD_BATCH * 2;

/// Path relatif yang aman (tanpa `..`, tidak absolut, tidak kosong).
fn valid_path(p: &str) -> bool {
    !p.is_empty()
        && p.len() <= 1024
        && !p.starts_with('/')
        && !p.contains('\\')
        && !p.contains('\0')
        && p.split('/').all(|s| !s.is_empty() && s != ".." && s != ".")
}

/// Tandai index berjalan.
async fn start<C: sea_orm::ConnectionTrait>(
    db: &C,
    row: graph_repos::Model,
    source: IndexSource,
    full: bool,
    commit: &str,
    branch: &str,
    by: Option<Uuid>,
) -> Result<graph_repos::Model, DbErr> {
    let mut am = row.into_active_model();
    am.status = Set(IndexStatus::Running.to_value());
    am.source = Set(source.to_value());
    am.mode = Set(if full {
        IndexMode::Full
    } else {
        IndexMode::Incremental
    }
    .to_value());
    am.commit_sha = Set(commit.chars().take(64).collect());
    am.branch = Set(branch.chars().take(200).collect());
    am.started_at = Set(Some(now()));
    am.error = Set(String::new());
    am.requested_by = Set(by);
    am.update(db).await
}

/// Hapus file yang tidak ada di `keep`; kembalikan file yang perlu dikirim dan jumlahnya.
async fn diff_manifest(
    db: &DatabaseConnection,
    wid: WorkspaceId,
    repo: Uuid,
    files: &[(String, String)],
    full: bool,
) -> Result<ManifestResponse, DbErr> {
    let existing: HashMap<String, (Uuid, String, bool)> =
        graph_files::Model::manifest(db, wid, repo)
            .await?
            .into_iter()
            .map(|(id, path, sha, current)| (path, (id, sha, current)))
            .collect();
    let wanted: HashMap<&str, &str> = files
        .iter()
        .map(|(p, s)| (p.as_str(), s.as_str()))
        .collect();
    let gone: Vec<Uuid> = existing
        .iter()
        .filter(|(p, _)| !wanted.contains_key(p.as_str()))
        .map(|(_, (id, ..))| *id)
        .collect();
    for chunk in gone.chunks(1000) {
        graph_files::Entity::delete_many()
            .filter(graph_files::Column::WorkspaceId.eq(wid.get()))
            .filter(graph_files::Column::Id.is_in(chunk.to_vec()))
            .exec(db)
            .await?;
    }
    let mut need = Vec::new();
    let mut unchanged = 0u32;
    for (path, sha) in files {
        match existing.get(path) {
            Some((_, old, true)) if !full && old == sha => unchanged += 1,
            _ => need.push(path.clone()),
        }
    }
    Ok(ManifestResponse {
        need,
        removed: u32::try_from(gone.len()).unwrap_or(u32::MAX),
        unchanged,
    })
}

/// Langkah 1 CLI: manifest semua file repo.
///
/// # Errors
///
/// `NotFound` (project/repo/izin `EditProject`), `Invalid` (path tidak valid, terlalu banyak
/// file), error database.
pub async fn manifest(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    req: &ManifestRequest,
) -> Result<ManifestResponse, TicketFailure> {
    let (_, repo) = repo_for(db, acting, &req.project, &req.repo).await?;
    if req.files.len() > MAX_FILES {
        return Err(TicketFailure::field("files", "too-many"));
    }
    if req.files.iter().any(|f| !valid_path(&f.path)) {
        return Err(TicketFailure::field("files", "invalid-path"));
    }
    let wid = acting.workspace_id();
    let row = ensure_row(db, wid, &repo).await?;
    start(
        db,
        row,
        IndexSource::Cli,
        req.full,
        &req.commit,
        &req.branch,
        Some(acting.user.id),
    )
    .await?;
    let files: Vec<(String, String)> = req
        .files
        .iter()
        .map(|f| (f.path.clone(), f.sha.clone()))
        .collect();
    Ok(diff_manifest(db, wid, repo.id, &files, req.full).await?)
}

/// File siap disimpan (hasil klasifikasi + parsing).
struct Prepared {
    path: String,
    lang: Lang,
    sha: String,
    bytes: u64,
    lines: u32,
    skip: Option<SkipReason>,
    content: String,
    facts: serde_json::Value,
}

/// Klasifikasi dan parse satu file (CPU, dipanggil di `spawn_blocking`).
fn prepare(f: UploadFile) -> Option<Prepared> {
    let lang = Lang::detect(&f.path)?;
    let mut p = Prepared {
        path: f.path,
        lang,
        sha: f.sha,
        bytes: f.bytes,
        lines: 0,
        skip: f.skip,
        content: String::new(),
        facts: serde_json::json!({}),
    };
    if walk::is_secret_path(&p.path) {
        // Isi rahasia tidak pernah disimpan walau CLI lama mengirimnya.
        p.skip = Some(SkipReason::Secret);
        return Some(p);
    }
    if p.skip.is_some() {
        return Some(p);
    }
    let Some(content) = f.content else {
        p.skip = Some(SkipReason::Binary);
        return Some(p);
    };
    p.bytes = content.len() as u64;
    p.sha = walk::sha256(content.as_bytes());
    if p.bytes > MAX_BYTES {
        p.skip = Some(SkipReason::TooLarge);
        return Some(p);
    }
    if walk::is_generated_path(&p.path) || walk::is_generated_source(&content) {
        p.skip = Some(SkipReason::Generated);
        return Some(p);
    }
    match agentgate_graph::extract(&p.path, &content) {
        Ok(facts) => {
            p.lines = facts.lines;
            p.facts = serde_json::to_value(&facts).unwrap_or_else(|_| serde_json::json!({}));
            p.content = content;
        }
        Err(_) => p.skip = Some(SkipReason::ParseFailed),
    }
    Some(p)
}

/// Simpan (upsert) file hasil upload/clone. Tanpa cek izin: pemanggil sudah memeriksa.
///
/// # Errors
///
/// Error database.
pub async fn store(
    db: &DatabaseConnection,
    wid: WorkspaceId,
    repo: Uuid,
    files: Vec<UploadFile>,
) -> Result<UploadResponse, DbErr> {
    let prepared: Vec<Prepared> =
        tokio::task::spawn_blocking(move || files.into_iter().filter_map(prepare).collect())
            .await
            .map_err(|e| DbErr::Custom(format!("parse task: {e}")))?;
    let mut out = UploadResponse {
        parsed: 0,
        skipped: 0,
    };
    let t = now();
    let rows: Vec<graph_files::ActiveModel> = prepared
        .into_iter()
        .map(|p| {
            if p.skip.is_some() {
                out.skipped += 1;
            } else {
                out.parsed += 1;
            }
            graph_files::ActiveModel {
                id: Set(Uuid::now_v7()),
                workspace_id: Set(wid.get()),
                repo_id: Set(repo),
                path: Set(p.path),
                lang: Set(p.lang.as_str().to_string()),
                sha: Set(p.sha),
                bytes: Set(i32::try_from(p.bytes).unwrap_or(i32::MAX)),
                lines: Set(i32::try_from(p.lines).unwrap_or(i32::MAX)),
                skip_reason: Set(p.skip.map_or("", SkipReason::as_str).to_string()),
                content: Set(p.content),
                facts: Set(p.facts),
                created_at: Set(t),
                updated_at: Set(t),
            }
        })
        .collect();
    for chunk in rows.chunks(200) {
        graph_files::Entity::insert_many(chunk.to_vec())
            .on_conflict(
                OnConflict::columns([graph_files::Column::RepoId, graph_files::Column::Path])
                    .update_columns([
                        graph_files::Column::Lang,
                        graph_files::Column::Sha,
                        graph_files::Column::Bytes,
                        graph_files::Column::Lines,
                        graph_files::Column::SkipReason,
                        graph_files::Column::Content,
                        graph_files::Column::Facts,
                    ])
                    // `updated_at` hanya maju bila isi berubah (deteksi dokumen knowledge basi).
                    .value(
                        graph_files::Column::UpdatedAt,
                        sea_orm::sea_query::Expr::cust(
                            "CASE WHEN graph_files.sha = excluded.sha \
                             THEN graph_files.updated_at ELSE excluded.updated_at END",
                        ),
                    )
                    .to_owned(),
            )
            .exec_without_returning(db)
            .await?;
    }
    Ok(out)
}

/// Langkah 2 CLI: satu batch isi file.
///
/// # Errors
///
/// `NotFound`, `Invalid` (batch terlalu besar, path tidak valid), error database.
pub async fn upload(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    req: UploadRequest,
) -> Result<UploadResponse, TicketFailure> {
    let (_, repo) = repo_for(db, acting, &req.project, &req.repo).await?;
    if req.files.len() > MAX_BATCH {
        return Err(TicketFailure::field("files", "too-many"));
    }
    if req.files.iter().any(|f| !valid_path(&f.path)) {
        return Err(TicketFailure::field("files", "invalid-path"));
    }
    Ok(store(db, acting.workspace_id(), repo.id, req.files).await?)
}

/// Simpan statistik git lalu build.
async fn finish_repo(
    db: &DatabaseConnection,
    wid: WorkspaceId,
    repo: &repos::Model,
    git: Option<GitStats>,
) -> Result<graph_repos::Model, DbErr> {
    let row = ensure_row(db, wid, repo).await?;
    if let Some(git) = git {
        let mut am = row.into_active_model();
        am.git_stats = Set(serde_json::to_value(&git).unwrap_or_else(|_| serde_json::json!({})));
        am.update(db).await?;
    }
    build::rebuild(db, wid, repo).await
}

/// Langkah 3 CLI: semua file terkirim → build graph dan relasi lintas repo.
///
/// # Errors
///
/// `NotFound`, error database.
pub async fn finish(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    req: agentgate_graph::protocol::FinishRequest,
) -> Result<(IndexSummary, repos::Model), TicketFailure> {
    let (_, repo) = repo_for(db, acting, &req.project, &req.repo).await?;
    let row = finish_repo(db, acting.workspace_id(), &repo, req.git).await?;
    Ok((summary(db, &repo, &row).await?, repo))
}

/// Batas edge SCIP yang disimpan.
pub const MAX_SCIP_EDGES: usize = 500_000;

/// `agentgate graph scip`: index SCIP dari indexer bahasa (lebih presisi dari tree-sitter untuk
/// file yang dicakupnya). Graph dibangun ulang bila repo sudah punya file ter-index.
///
/// # Errors
///
/// `NotFound`, `Invalid` (terlalu besar), error database.
pub async fn scip(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    req: ScipRequest,
) -> Result<IndexSummary, TicketFailure> {
    let (_, repo) = repo_for(db, acting, &req.project, &req.repo).await?;
    if req.scip.edges.len() > MAX_SCIP_EDGES {
        return Err(TicketFailure::field("scip", "too-large"));
    }
    let wid = acting.workspace_id();
    let row = ensure_row(db, wid, &repo).await?;
    let has_files = row.files_indexed > 0
        || !graph_files::Model::manifest(db, wid, repo.id)
            .await?
            .is_empty();
    let mut am = row.into_active_model();
    am.scip_indexer = Set(req.scip.indexer.chars().take(100).collect());
    am.scip = Set(Some(serde_json::to_value(&req.scip).unwrap_or_default()));
    am.scip_by = Set(acting.user.email.clone());
    am.scip_at = Set(Some(now()));
    let row = am.update(db).await?;
    let row = if has_files {
        build::rebuild(db, wid, &repo).await?
    } else {
        row
    };
    Ok(summary(db, &repo, &row).await?)
}

/// Index direktori hasil clone server.
#[derive(Debug, Clone)]
pub struct DirIndex<'a> {
    pub root: &'a Path,
    pub full: bool,
    pub commit: &'a str,
    pub git: Option<GitStats>,
    pub by: Option<Uuid>,
}

/// Index direktori repo (clone server): walk → manifest → simpan file berubah → build.
///
/// # Errors
///
/// Error database.
pub async fn index_dir(
    db: &DatabaseConnection,
    wid: WorkspaceId,
    repo: &repos::Model,
    job: DirIndex<'_>,
) -> Result<graph_repos::Model, DbErr> {
    let DirIndex {
        root,
        full,
        commit,
        git,
        by,
    } = job;
    let row = ensure_row(db, wid, repo).await?;
    start(
        db,
        row,
        IndexSource::Server,
        full,
        commit,
        &repo.default_branch,
        by,
    )
    .await?;
    let root_buf = root.to_path_buf();
    let entries = tokio::task::spawn_blocking(move || walk::walk(&root_buf))
        .await
        .map_err(|e| DbErr::Custom(format!("walk task: {e}")))?;
    let manifest: Vec<(String, String)> = entries
        .iter()
        .map(|e| (e.path.clone(), e.sha.clone()))
        .collect();
    let diff = diff_manifest(db, wid, repo.id, &manifest, full).await?;
    let need: std::collections::HashSet<String> = diff.need.into_iter().collect();
    let mut batch = Vec::new();
    for e in entries {
        if !need.contains(&e.path) {
            continue;
        }
        batch.push(UploadFile {
            path: e.path,
            sha: e.sha,
            bytes: e.bytes,
            content: e.content,
            skip: e.skip,
        });
        if batch.len() >= MAX_BATCH {
            store(db, wid, repo.id, std::mem::take(&mut batch)).await?;
        }
    }
    if !batch.is_empty() {
        store(db, wid, repo.id, batch).await?;
    }
    finish_repo(db, wid, repo, git).await
}

/// Tandai index gagal (pesan untuk UI).
///
/// # Errors
///
/// Error database.
pub async fn fail(
    db: &DatabaseConnection,
    wid: WorkspaceId,
    repo: &repos::Model,
    error: &str,
) -> Result<(), DbErr> {
    let row = ensure_row(db, wid, repo).await?;
    let mut am = row.into_active_model();
    am.status = Set(IndexStatus::Failed.to_value());
    am.error = Set(error.chars().take(500).collect());
    am.update(db).await?;
    Ok(())
}

/// Tandai index antre (tombol Index ulang, webhook merge).
///
/// # Errors
///
/// Error database.
pub async fn queue(
    db: &DatabaseConnection,
    wid: WorkspaceId,
    repo: &repos::Model,
    by: Option<Uuid>,
) -> Result<graph_repos::Model, DbErr> {
    let row = ensure_row(db, wid, repo).await?;
    let mut am = row.into_active_model();
    am.status = Set(IndexStatus::Queued.to_value());
    am.requested_by = Set(by);
    am.error = Set(String::new());
    am.update(db).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_must_be_relative_and_clean() {
        assert!(valid_path("src/a.ts"));
        for bad in ["", "/etc/passwd", "../x", "a/../b", "a//b", "a\\b", "./a"] {
            assert!(!valid_path(bad), "{bad}");
        }
    }

    #[test]
    fn secrets_never_keep_content() {
        let p = prepare(UploadFile {
            path: "config/.env".into(),
            sha: String::new(),
            bytes: 10,
            content: Some("SECRET=1".into()),
            skip: None,
        });
        // `.env` tidak dikenal sebagai bahasa → tidak disimpan sama sekali.
        assert!(p.is_none());
        let p = prepare(UploadFile {
            path: "src/credentials.ts".into(),
            sha: String::new(),
            bytes: 10,
            content: Some("export const key = 'x'".into()),
            skip: None,
        })
        .expect("ts dikenal");
        assert_eq!(p.skip, Some(SkipReason::Secret));
        assert!(p.content.is_empty());
    }

    #[test]
    fn parses_source_into_facts() {
        let p = prepare(UploadFile {
            path: "src/a.ts".into(),
            sha: String::new(),
            bytes: 0,
            content: Some("export function a() { b() }\nfunction b() {}\n".into()),
            skip: None,
        })
        .expect("ts");
        assert_eq!(p.skip, None);
        assert_eq!(p.lines, 2);
        assert_eq!(p.facts["symbols"].as_array().map(Vec::len), Some(2));
    }
}
