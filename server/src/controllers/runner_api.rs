//! API CLI dan runner (token bearer dari `agentgate login`), selalu JSON:
//!
//! - `GET /api/cli/me`, `GET /api/cli/rules?project=KEY`, `GET /api/cli/jobs`,
//!   `POST /api/cli/jobs/{id}/claim|release|cancel`;
//! - `POST /api/runner/register`, `GET /api/runner/ws?runner=…` (WebSocket kontrol),
//!   `GET /api/runner/events?runner=…` (event yang sama tanpa WebSocket);
//! - `POST /api/runner/jobs/{id}/start|logs|steps|evidence|permissions|finish|terminal-failed`,
//!   `GET /api/runner/jobs/{id}/permissions/{pid}` (header `x-agentgate-runner`);
//! - `POST /api/runner/drift`.
//!
//! Logika di `services::jobs`; error domain → 404/422 `{"error": "<kode>"}`.
use std::time::Duration;

use agentgate_core::runner::{
    protocol::{
        ClientMsg, DriftReport, EvidenceUpload, FinishReport, LogBatch, PermissionAsk,
        RegisterRunner, RunnerInfo, ServerMsg, StepsUpdate,
    },
    rules,
};
use axum::{
    Json,
    extract::{
        Path, Query,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, StatusCode},
};
use loco_rs::prelude::*;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    common::settings::Settings,
    controllers::extractors::BearerSession,
    models::{runner_prefs, runners},
    services::{
        jobs::{self, evidence, runner as svc},
        mcp::Session,
        oauth::Principal,
        tickets::TicketFailure,
    },
};

/// Header id runner untuk endpoint job.
pub const RUNNER_HEADER: &str = "x-agentgate-runner";
/// Interval server menilai event per koneksi WebSocket.
const WS_TICK: Duration = Duration::from_secs(2);

fn json_error(status: StatusCode, code: &str) -> Response {
    (status, Json(serde_json::json!({ "error": code }))).into_response()
}

fn failure(e: TicketFailure) -> Result<Response> {
    match e {
        TicketFailure::NotFound => Ok(json_error(StatusCode::NOT_FOUND, "not_found")),
        TicketFailure::Invalid(errors) => {
            let code = errors
                .errors
                .values()
                .flat_map(|es| es.iter().map(|e| e.code.to_string()))
                .next()
                .unwrap_or_else(|| "invalid".into());
            Ok(json_error(StatusCode::UNPROCESSABLE_ENTITY, &code))
        }
        TicketFailure::Db(e) => Err(e.into()),
    }
}

macro_rules! tri {
    ($e:expr) => {
        match $e {
            Ok(v) => v,
            Err(err) => return failure(err),
        }
    };
}

#[debug_handler]
async fn me(State(ctx): State<AppContext>, auth: BearerSession) -> Result<Response> {
    let base = Settings::from_ctx(&ctx)?.public_base_url;
    let s = &auth.session;
    Ok(Json(serde_json::json!({
        "user": s.user.name,
        "email": s.user.email,
        "workspace": s.workspace.name,
        "slug": s.workspace.slug,
        "server": base,
        "version": agentgate_core::VERSION,
    }))
    .into_response())
}

#[derive(Debug, Deserialize)]
struct RulesQuery {
    #[serde(default)]
    project: String,
}

#[debug_handler]
async fn rules_files(
    State(ctx): State<AppContext>,
    auth: BearerSession,
    Query(q): Query<RulesQuery>,
) -> Result<Response> {
    let base = Settings::from_ctx(&ctx)?.public_base_url;
    let acting = auth.session.acting();
    let projects =
        crate::models::projects::Model::accessible_for(&ctx.db, &acting.accessor()).await?;
    let Some(project) = projects
        .iter()
        .find(|p| p.key.eq_ignore_ascii_case(q.project.trim()))
    else {
        return Ok(json_error(StatusCode::NOT_FOUND, "project_not_found"));
    };
    let files = svc::rule_files(&base, &project.key);
    Ok(Json(serde_json::json!({
        "project": project.key,
        "project_id": project.id,
        "project_name": project.name,
        "checksum": rules::checksum(&files),
        "files": files,
    }))
    .into_response())
}

#[debug_handler]
async fn list_jobs(State(ctx): State<AppContext>, auth: BearerSession) -> Result<Response> {
    Ok(Json(svc::list(&ctx.db, &auth.session).await?).into_response())
}

#[derive(Debug, Default, Deserialize)]
struct ClaimBody {
    #[serde(default)]
    runner_id: Option<Uuid>,
    #[serde(default)]
    note: String,
}

#[debug_handler]
async fn claim(
    State(ctx): State<AppContext>,
    auth: BearerSession,
    Path(id): Path<Uuid>,
    body: Option<Json<ClaimBody>>,
) -> Result<Response> {
    let body = body.map(|b| b.0).unwrap_or_default();
    let job = tri!(jobs::claim(&ctx.db, &auth.session.acting(), id, body.runner_id).await);
    Ok(Json(serde_json::json!({ "id": job.id, "status": job.status })).into_response())
}

#[debug_handler]
async fn release(
    State(ctx): State<AppContext>,
    auth: BearerSession,
    Path(id): Path<Uuid>,
    body: Option<Json<ClaimBody>>,
) -> Result<Response> {
    let body = body.map(|b| b.0).unwrap_or_default();
    let job = tri!(jobs::release(&ctx.db, &auth.session.acting(), id, &body.note).await);
    Ok(Json(serde_json::json!({ "id": job.id, "status": job.status })).into_response())
}

#[debug_handler]
async fn cancel(
    State(ctx): State<AppContext>,
    auth: BearerSession,
    Path(id): Path<Uuid>,
) -> Result<Response> {
    let job = tri!(jobs::cancel(&ctx.db, &auth.session.acting(), id).await);
    Ok(Json(serde_json::json!({ "id": job.id, "status": job.status })).into_response())
}

#[debug_handler]
async fn register(
    State(ctx): State<AppContext>,
    auth: BearerSession,
    Json(req): Json<RegisterRunner>,
) -> Result<Response> {
    let runner = tri!(
        svc::register(
            &ctx.db,
            &auth.session,
            Some(auth.principal.grant_id),
            &req
        )
        .await
    );
    let s = &auth.session;
    let prefs = runner_prefs::Model::of(&ctx.db, s.workspace.workspace_id(), s.user.id).await?;
    Ok(Json(RunnerInfo {
        id: runner.id,
        name: runner.name,
        workspace: s.workspace.name.clone(),
        user_email: s.user.email.clone(),
        auto_claim: prefs.auto_claim,
        max_parallel: prefs.max_parallel.and_then(|m| u32::try_from(m).ok()),
    })
    .into_response())
}

#[derive(Debug, Deserialize)]
struct RunnerQuery {
    runner: Uuid,
}

#[debug_handler]
async fn events(
    State(ctx): State<AppContext>,
    auth: BearerSession,
    Query(q): Query<RunnerQuery>,
) -> Result<Response> {
    let runner = tri!(svc::own(&ctx.db, &auth.session, q.runner).await);
    svc::heartbeat(&ctx.db, &runner, false).await?;
    let mut state = svc::EventState::default();
    let msgs = svc::events(&ctx.db, &auth.session, &runner, &mut state).await?;
    Ok(Json(msgs).into_response())
}

#[debug_handler]
async fn ws(
    State(ctx): State<AppContext>,
    auth: BearerSession,
    Query(q): Query<RunnerQuery>,
    upgrade: WebSocketUpgrade,
) -> Result<Response> {
    let runner = tri!(svc::own(&ctx.db, &auth.session, q.runner).await);
    let principal = auth.principal;
    Ok(upgrade.on_upgrade(move |socket| socket_loop(ctx, principal, runner, socket)))
}

async fn send(socket: &mut WebSocket, msg: &ServerMsg) -> bool {
    match serde_json::to_string(msg) {
        Ok(text) => socket.send(Message::Text(text.into())).await.is_ok(),
        Err(_) => false,
    }
}

/// Satu koneksi runner: heartbeat dari runner, event dari server tiap [`WS_TICK`]. Session
/// dimuat ulang setiap tick; token/grant dicabut atau runner dicabut → koneksi ditutup.
async fn socket_loop(ctx: AppContext, principal: Principal, runner: runners::Model, mut socket: WebSocket) {
    if svc::heartbeat(&ctx.db, &runner, true).await.is_err()
        || !send(&mut socket, &ServerMsg::Welcome { runner_id: runner.id }).await
    {
        return;
    }
    let mut state = svc::EventState::default();
    let mut tick = tokio::time::interval(WS_TICK);
    loop {
        tokio::select! {
            msg = socket.recv() => match msg {
                Some(Ok(Message::Text(text))) => {
                    if serde_json::from_str::<ClientMsg>(&text).is_ok()
                        && svc::heartbeat(&ctx.db, &runner, false).await.is_err()
                    {
                        break;
                    }
                }
                Some(Ok(Message::Close(_)) | Err(_)) | None => break,
                Some(Ok(_)) => {}
            },
            _ = tick.tick() => {
                let session = match Session::load(&ctx.db, &principal).await {
                    Ok(Some(s)) => s,
                    _ => {
                        let _ = send(&mut socket, &ServerMsg::Error { message: "access revoked".into() }).await;
                        break;
                    }
                };
                let Ok(runner) = svc::own(&ctx.db, &session, runner.id).await else {
                    let _ = send(&mut socket, &ServerMsg::Error { message: "runner revoked".into() }).await;
                    break;
                };
                let Ok(msgs) = svc::events(&ctx.db, &session, &runner, &mut state).await else {
                    continue;
                };
                for m in &msgs {
                    if !send(&mut socket, m).await {
                        return;
                    }
                }
            }
        }
    }
}

/// Runner dari header + job yang dipegangnya.
async fn held(
    ctx: &AppContext,
    session: &Session,
    headers: &HeaderMap,
    job_id: Uuid,
) -> std::result::Result<(runners::Model, crate::models::jobs::Model), TicketFailure> {
    let runner_id = headers
        .get(RUNNER_HEADER)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| Uuid::try_parse(v.trim()).ok())
        .ok_or(TicketFailure::NotFound)?;
    let runner = svc::own(&ctx.db, session, runner_id).await?;
    let job = svc::held(&ctx.db, session, &runner, job_id).await?;
    Ok((runner, job))
}

#[debug_handler]
async fn start(
    State(ctx): State<AppContext>,
    auth: BearerSession,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Response> {
    let base = Settings::from_ctx(&ctx)?.public_base_url;
    let runner_id = headers
        .get(RUNNER_HEADER)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| Uuid::try_parse(v.trim()).ok());
    let Some(runner_id) = runner_id else {
        return Ok(json_error(StatusCode::NOT_FOUND, "not_found"));
    };
    let runner = tri!(svc::own(&ctx.db, &auth.session, runner_id).await);
    let spec = tri!(svc::start(&ctx.db, &auth.session, &runner, id, &base).await);
    Ok(Json(spec).into_response())
}

#[debug_handler]
async fn logs(
    State(ctx): State<AppContext>,
    auth: BearerSession,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(batch): Json<LogBatch>,
) -> Result<Response> {
    let (_, job) = tri!(held(&ctx, &auth.session, &headers, id).await);
    tri!(svc::append_logs(&ctx.db, &auth.session, &job, &batch).await);
    Ok(StatusCode::NO_CONTENT.into_response())
}

#[debug_handler]
async fn steps(
    State(ctx): State<AppContext>,
    auth: BearerSession,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(update): Json<StepsUpdate>,
) -> Result<Response> {
    let (_, job) = tri!(held(&ctx, &auth.session, &headers, id).await);
    tri!(svc::set_steps(&ctx.db, &job, &update).await);
    Ok(StatusCode::NO_CONTENT.into_response())
}

#[debug_handler]
async fn upload_evidence(
    State(ctx): State<AppContext>,
    auth: BearerSession,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(up): Json<EvidenceUpload>,
) -> Result<Response> {
    let (runner, job) = tri!(held(&ctx, &auth.session, &headers, id).await);
    let row = tri!(evidence::upload(&ctx.db, &ctx.storage, &auth.session, &runner, &job, &up).await);
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({ "id": row.id, "kind": row.kind })),
    )
        .into_response())
}

#[debug_handler]
async fn ask_permission(
    State(ctx): State<AppContext>,
    auth: BearerSession,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(ask): Json<PermissionAsk>,
) -> Result<Response> {
    let (_, job) = tri!(held(&ctx, &auth.session, &headers, id).await);
    let info = tri!(svc::ask_permission(&ctx.db, &auth.session, &job, &ask).await);
    Ok(Json(info).into_response())
}

#[debug_handler]
async fn permission_status(
    State(ctx): State<AppContext>,
    auth: BearerSession,
    headers: HeaderMap,
    Path((id, pid)): Path<(Uuid, Uuid)>,
) -> Result<Response> {
    let (_, job) = tri!(held(&ctx, &auth.session, &headers, id).await);
    let info = tri!(svc::permission(&ctx.db, &job, pid).await);
    Ok(Json(info).into_response())
}

#[debug_handler]
async fn finish(
    State(ctx): State<AppContext>,
    auth: BearerSession,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(report): Json<FinishReport>,
) -> Result<Response> {
    let (_, job) = tri!(held(&ctx, &auth.session, &headers, id).await);
    let job = tri!(svc::finish(&ctx.db, &auth.session, &job, &report).await);
    Ok(Json(serde_json::json!({ "id": job.id, "status": job.status })).into_response())
}

#[debug_handler]
async fn terminal_failed(
    State(ctx): State<AppContext>,
    auth: BearerSession,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Response> {
    let (_, job) = tri!(held(&ctx, &auth.session, &headers, id).await);
    tri!(svc::terminal_failed(&ctx.db, &auth.session, &job).await);
    Ok(StatusCode::NO_CONTENT.into_response())
}

#[debug_handler]
async fn drift(
    State(ctx): State<AppContext>,
    auth: BearerSession,
    Json(report): Json<DriftReport>,
) -> Result<Response> {
    svc::report_drift(&ctx.db, &auth.session, &report).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

pub fn routes() -> Routes {
    Routes::new()
        .add("/api/cli/me", get(me))
        .add("/api/cli/rules", get(rules_files))
        .add("/api/cli/jobs", get(list_jobs))
        .add("/api/cli/jobs/{id}/claim", post(claim))
        .add("/api/cli/jobs/{id}/release", post(release))
        .add("/api/cli/jobs/{id}/cancel", post(cancel))
        .add("/api/runner/register", post(register))
        .add("/api/runner/events", get(events))
        .add("/api/runner/ws", get(ws))
        .add("/api/runner/jobs/{id}/start", post(start))
        .add("/api/runner/jobs/{id}/logs", post(logs))
        .add("/api/runner/jobs/{id}/steps", post(steps))
        .add("/api/runner/jobs/{id}/evidence", post(upload_evidence))
        .add("/api/runner/jobs/{id}/permissions", post(ask_permission))
        .add(
            "/api/runner/jobs/{id}/permissions/{pid}",
            get(permission_status),
        )
        .add("/api/runner/jobs/{id}/finish", post(finish))
        .add("/api/runner/jobs/{id}/terminal-failed", post(terminal_failed))
        .add("/api/runner/drift", post(drift))
}
