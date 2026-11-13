//! API knowledge untuk CLI (token bearer):
//!
//! - `GET /api/cli/knowledge/export?project=KEY` → dokumen project (`agentgate knowledge pull`);
//! - `POST /api/cli/knowledge/import` → perubahan file lokal (`agentgate knowledge sync`);
//! - `POST /api/cli/backups` → backup aturan lama dari `agentgate init`;
//! - `GET /api/cli/backups?project=KEY` → daftar backup.
//!
//! Logika di `services::knowledge::sync`; error domain → 404/422 `{"error": "<kode>"}`.
use axum::{
    Json,
    extract::{DefaultBodyLimit, Query},
};
use loco_rs::prelude::*;
use serde::Deserialize;

use crate::{
    controllers::{extractors::BearerSession, runner_api::failure},
    services::knowledge::sync,
};

/// Body backup: arsip base64 (maks. 5 MB) + isi teks aturan.
const BACKUP_LIMIT: usize = 12 * 1024 * 1024;
/// Body impor (dokumen maks. 200 ribu karakter × beberapa file).
const IMPORT_LIMIT: usize = 16 * 1024 * 1024;

macro_rules! tri {
    ($e:expr) => {
        match $e {
            Ok(v) => v,
            Err(err) => return failure(err),
        }
    };
}

#[derive(Debug, Deserialize)]
struct ProjectQuery {
    #[serde(default)]
    project: String,
}

#[debug_handler]
async fn export(
    State(ctx): State<AppContext>,
    auth: BearerSession,
    Query(q): Query<ProjectQuery>,
) -> Result<Response> {
    let out = tri!(sync::export(&ctx.db, &auth.session.acting(), &q.project).await);
    Ok(Json(out).into_response())
}

#[debug_handler]
async fn import(
    State(ctx): State<AppContext>,
    auth: BearerSession,
    Json(req): Json<sync::ImportRequest>,
) -> Result<Response> {
    let out = tri!(sync::import(&ctx.db, &auth.session.acting(), &req).await);
    Ok(Json(serde_json::json!({ "results": out })).into_response())
}

#[debug_handler]
async fn backup(
    State(ctx): State<AppContext>,
    auth: BearerSession,
    Json(req): Json<sync::BackupRequest>,
) -> Result<Response> {
    let out = tri!(sync::backup(&ctx.db, &ctx.storage, &auth.session.acting(), &req).await);
    Ok(Json(out).into_response())
}

#[debug_handler]
async fn backups(
    State(ctx): State<AppContext>,
    auth: BearerSession,
    Query(q): Query<ProjectQuery>,
) -> Result<Response> {
    let list = tri!(sync::backups(&ctx.db, &auth.session.acting(), &q.project).await);
    let out: Vec<serde_json::Value> = list
        .iter()
        .map(|b| {
            serde_json::json!({
                "id": b.id, "repo": b.repo, "archive": b.archive, "size": b.size,
                "files": b.files, "created_at": b.created_at,
            })
        })
        .collect();
    Ok(Json(serde_json::json!({ "backups": out })).into_response())
}

pub fn routes() -> Routes {
    Routes::new()
        .add("/api/cli/knowledge/export", get(export))
        .add(
            "/api/cli/knowledge/import",
            post(import).layer(DefaultBodyLimit::max(IMPORT_LIMIT)),
        )
        .add(
            "/api/cli/backups",
            get(backups)
                .post(backup)
                .layer(DefaultBodyLimit::max(BACKUP_LIMIT)),
        )
}
