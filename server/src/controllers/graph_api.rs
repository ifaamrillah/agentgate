//! API index code graph untuk CLI (`agentgate graph index|scip|status`, token bearer):
//!
//! - `POST /api/cli/graph/manifest` → `{need, removed, unchanged}`;
//! - `POST /api/cli/graph/upload` (batch isi file, maks. 100 file / 24 MB);
//! - `POST /api/cli/graph/finish` → ringkasan index sesudah build;
//! - `POST /api/cli/graph/scip` (index SCIP hasil konversi CLI, maks. 64 MB);
//! - `GET /api/cli/graph/status?project=KEY` → ringkasan semua repo project.
//!
//! Logika di `services::graph`; error domain → 404/422 `{"error": "<kode>"}`.
use agentgate_graph::protocol::{
    FinishRequest, ManifestRequest, ScipRequest, StatusRequest, UploadRequest,
};
use axum::{
    Json,
    extract::{DefaultBodyLimit, Query},
};
use loco_rs::prelude::*;

use crate::{
    controllers::{extractors::BearerSession, runner_api::failure},
    services::graph::{self, ingest},
};

/// Batas body upload (100 file × rata-rata jauh di bawah 1 MB; CLI membatasi per 8 MB).
pub const UPLOAD_LIMIT: usize = 24 * 1024 * 1024;
/// Batas body SCIP (JSON edge).
pub const SCIP_LIMIT: usize = 64 * 1024 * 1024;
/// Batas body manifest (100 ribu path + sha).
pub const MANIFEST_LIMIT: usize = 16 * 1024 * 1024;

macro_rules! tri {
    ($e:expr) => {
        match $e {
            Ok(v) => v,
            Err(err) => return failure(err),
        }
    };
}

#[debug_handler]
async fn manifest(
    State(ctx): State<AppContext>,
    auth: BearerSession,
    Json(req): Json<ManifestRequest>,
) -> Result<Response> {
    let out = tri!(ingest::manifest(&ctx.db, &auth.session.acting(), &req).await);
    Ok(Json(out).into_response())
}

#[debug_handler]
async fn upload(
    State(ctx): State<AppContext>,
    auth: BearerSession,
    Json(req): Json<UploadRequest>,
) -> Result<Response> {
    let out = tri!(ingest::upload(&ctx.db, &auth.session.acting(), req).await);
    Ok(Json(out).into_response())
}

#[debug_handler]
async fn finish(
    State(ctx): State<AppContext>,
    auth: BearerSession,
    Json(req): Json<FinishRequest>,
) -> Result<Response> {
    let out = tri!(ingest::finish(&ctx.db, &auth.session.acting(), req).await);
    Ok(Json(out).into_response())
}

#[debug_handler]
async fn scip(
    State(ctx): State<AppContext>,
    auth: BearerSession,
    Json(req): Json<ScipRequest>,
) -> Result<Response> {
    let out = tri!(ingest::scip(&ctx.db, &auth.session.acting(), req).await);
    Ok(Json(out).into_response())
}

#[debug_handler]
async fn status(
    State(ctx): State<AppContext>,
    auth: BearerSession,
    Query(q): Query<StatusRequest>,
) -> Result<Response> {
    let out = tri!(graph::status(&ctx.db, &auth.session.acting(), &q.project).await);
    Ok(Json(out).into_response())
}

pub fn routes() -> Routes {
    Routes::new()
        .add(
            "/api/cli/graph/manifest",
            post(manifest).layer(DefaultBodyLimit::max(MANIFEST_LIMIT)),
        )
        .add(
            "/api/cli/graph/upload",
            post(upload).layer(DefaultBodyLimit::max(UPLOAD_LIMIT)),
        )
        .add("/api/cli/graph/finish", post(finish))
        .add(
            "/api/cli/graph/scip",
            post(scip).layer(DefaultBodyLimit::max(SCIP_LIMIT)),
        )
        .add("/api/cli/graph/status", get(status))
}
