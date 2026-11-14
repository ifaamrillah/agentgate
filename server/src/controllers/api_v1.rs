//! API publik REST JSON (M11) di `/api/v1`, diautentikasi token API pribadi
//! (`Authorization: Bearer ag_pat_…`, `services::api_tokens`). Spesifikasi lengkap:
//! `GET /api/v1/openapi.yaml` (ditulis tangan, `assets/api/openapi.yaml`; tes memastikan setiap
//! route di sini ada di dokumen).
//!
//! - Token bertindak sebagai pembuatnya: setiap endpoint memanggil service domain yang sama
//!   dengan UI/MCP (izin role dan akses project dicek ulang; pindah stage lewat gate yang sama).
//!   Project di luar akses pembuat atau di luar batas project token → 404, bukan 403.
//! - Cakupan token yang kurang → 403 `{"error": "insufficient_scope"}`.
//! - Rate limit per token per menit: header `X-RateLimit-Remaining` di setiap respons
//!   terautentikasi; lewat batas → 429 `{"error": "rate_limited"}`.
//! - Error selalu `{"error": "<kode>"}` (validasi: kode Fluent field pertama + `field`).
//! - Hanya bearer (tanpa cookie), jadi path ini dikecualikan dari cek CSRF berbasis asal.

use std::sync::Arc;

use axum::{
    Extension, Json,
    extract::{Query, Request},
    http::{HeaderValue, StatusCode, header},
    middleware::Next,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use loco_rs::prelude::*;
use sea_orm::ActiveEnum;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    common::settings::Settings,
    controllers::extractors::bearer::bearer_token,
    models::{
        api_tokens::ApiScope,
        knowledge_docs,
        outbox_events::ticket_json,
        permission::Permission,
        projects, sprints, ticket_comments, ticket_types,
        tickets::{self, TicketInput, TicketQuery, TicketStatus},
        users, webhooks,
    },
    services::{
        api_tokens::{self, ApiCaller, AuthOutcome},
        knowledge::docs,
        tickets::{
            TicketFailure, access_in, comments,
            create::{self, TicketForm},
            stages::{self, Target},
            update,
        },
        webhooks::{self as hooks, WebhookInput},
    },
};

/// Dokumen OpenAPI 3 (ditulis tangan).
pub const OPENAPI: &str = include_str!("../../assets/api/openapi.yaml");
/// Batas `limit` daftar tiket.
const MAX_LIMIT: u64 = 100;

/// Error JSON `{"error": code}` (+ `field` untuk validasi).
#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: String,
    field: Option<String>,
    /// Kalimat untuk manusia (Fluent), mis. alasan batas WIP penuh.
    message: Option<String>,
}

impl ApiError {
    fn new(status: StatusCode, code: &str) -> Self {
        Self {
            status,
            code: code.to_string(),
            field: None,
            message: None,
        }
    }

    fn not_found() -> Self {
        Self::new(StatusCode::NOT_FOUND, "not_found")
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut body = json!({ "error": self.code });
        if let Some(f) = self.field {
            body["field"] = json!(f);
        }
        if let Some(m) = self.message {
            body["message"] = json!(m);
        }
        (self.status, Json(body)).into_response()
    }
}

impl From<TicketFailure> for ApiError {
    fn from(e: TicketFailure) -> Self {
        match e {
            TicketFailure::NotFound => Self::not_found(),
            TicketFailure::Invalid(ref errors) => {
                let message = e.message();
                let (field, code) = errors
                    .errors
                    .iter()
                    .find_map(|(f, es)| es.first().map(|e| (f.to_string(), e.code.to_string())))
                    .unwrap_or_else(|| (String::new(), "invalid_request".into()));
                Self {
                    status: StatusCode::UNPROCESSABLE_ENTITY,
                    code,
                    field: Some(field).filter(|f| !f.is_empty()),
                    message,
                }
            }
            TicketFailure::Db(e) => {
                tracing::error!(error = %e, "api v1 db error");
                Self::new(StatusCode::INTERNAL_SERVER_ERROR, "server_error")
            }
        }
    }
}

impl From<sea_orm::DbErr> for ApiError {
    fn from(e: sea_orm::DbErr) -> Self {
        TicketFailure::Db(e).into()
    }
}

type ApiResult = std::result::Result<Response, ApiError>;

fn ok(value: Value) -> ApiResult {
    Ok(Json(value).into_response())
}

fn created(value: Value) -> ApiResult {
    Ok((StatusCode::CREATED, Json(value)).into_response())
}

/// Autentikasi + rate limit semua route `/api/v1` kecuali dokumen OpenAPI.
pub async fn auth_layer(State(ctx): State<AppContext>, mut req: Request, next: Next) -> Response {
    if req.uri().path() == "/api/v1/openapi.yaml" {
        return next.run(req).await;
    }
    let (mut parts, body) = req.into_parts();
    let token = bearer_token(&parts);
    let limit = Settings::from_ctx(&ctx).map_or(600, |s| s.api.rate_limit_per_minute);
    let outcome = match token {
        Some(t) => api_tokens::authenticate(&ctx.db, &t, limit, chrono::Utc::now()).await,
        None => Ok(AuthOutcome::Invalid),
    };
    let caller = match outcome {
        Ok(AuthOutcome::Ok(c)) => c,
        Ok(AuthOutcome::Invalid) => {
            return ApiError::new(StatusCode::UNAUTHORIZED, "invalid_token").into_response();
        }
        Ok(AuthOutcome::Limited) => {
            let mut res =
                ApiError::new(StatusCode::TOO_MANY_REQUESTS, "rate_limited").into_response();
            res.headers_mut()
                .insert("x-ratelimit-remaining", HeaderValue::from_static("0"));
            res.headers_mut()
                .insert(header::RETRY_AFTER, HeaderValue::from_static("60"));
            return res;
        }
        Err(e) => return ApiError::from(e).into_response(),
    };
    let remaining = caller.remaining;
    parts.extensions.insert(Arc::new(*caller));
    req = Request::from_parts(parts, body);
    let mut res = next.run(req).await;
    if let Ok(v) = HeaderValue::from_str(&remaining.to_string()) {
        res.headers_mut().insert("x-ratelimit-remaining", v);
    }
    res
}

type Caller = Extension<Arc<ApiCaller>>;

fn need(caller: &ApiCaller, scope: ApiScope) -> std::result::Result<(), ApiError> {
    if caller.has(scope) {
        Ok(())
    } else {
        Err(ApiError::new(StatusCode::FORBIDDEN, "insufficient_scope"))
    }
}

/// Project `key` yang boleh dibuka pemanggil (akses pembuat + batas token), selain itu 404.
async fn project_for(
    ctx: &AppContext,
    caller: &ApiCaller,
    key: &str,
) -> std::result::Result<projects::Model, ApiError> {
    let wid = caller.workspace.workspace_id();
    let project = projects::Model::find_by_key(&ctx.db, wid, key.trim())
        .await?
        .filter(|p| caller.allows_project(p.id))
        .ok_or_else(ApiError::not_found)?;
    access_in(
        &ctx.db,
        &caller.acting(),
        project.id,
        Permission::ViewProject,
    )
    .await?;
    Ok(project)
}

/// Project yang boleh dibuka pemanggil.
async fn accessible(
    ctx: &AppContext,
    caller: &ApiCaller,
) -> std::result::Result<Vec<projects::Model>, ApiError> {
    Ok(
        projects::Model::accessible_for(&ctx.db, &caller.acting().accessor())
            .await?
            .into_iter()
            .filter(|p| caller.allows_project(p.id))
            .collect(),
    )
}

/// Tiket `code` (bukan di sampah) yang boleh dibuka pemanggil, beserta project-nya.
async fn ticket_for(
    ctx: &AppContext,
    caller: &ApiCaller,
    code: &str,
) -> std::result::Result<(tickets::Model, projects::Model), ApiError> {
    let wid = caller.workspace.workspace_id();
    let (ticket, _) = tickets::Model::find_by_code(&ctx.db, wid, code.trim())
        .await?
        .filter(|(t, _)| !t.is_trashed() && caller.allows_project(t.project_id))
        .ok_or_else(ApiError::not_found)?;
    let project = projects::Model::find_in(&ctx.db, wid, ticket.project_id)
        .await?
        .ok_or_else(ApiError::not_found)?;
    access_in(
        &ctx.db,
        &caller.acting(),
        project.id,
        Permission::ViewProject,
    )
    .await?;
    Ok((ticket, project))
}

// ---------------------------------------------------------------------------------------------
// Project & tiket
// ---------------------------------------------------------------------------------------------

async fn list_projects(State(ctx): State<AppContext>, Extension(c): Caller) -> ApiResult {
    let list: Vec<Value> = accessible(&ctx, &c)
        .await?
        .iter()
        .map(|p| json!({ "id": p.id, "key": p.key, "name": p.name }))
        .collect();
    ok(json!({ "projects": list }))
}

#[derive(Debug, Deserialize, Default)]
struct TicketListQuery {
    #[serde(default)]
    project: String,
    #[serde(default)]
    stage: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    q: String,
    #[serde(default)]
    limit: Option<u64>,
    #[serde(default)]
    cursor: String,
}

/// Kursor opaque = base64url dari offset.
fn decode_cursor(cursor: &str) -> std::result::Result<u64, ApiError> {
    if cursor.is_empty() {
        return Ok(0);
    }
    URL_SAFE_NO_PAD
        .decode(cursor)
        .ok()
        .and_then(|b| String::from_utf8(b).ok())
        .and_then(|s| s.strip_prefix("o:").and_then(|n| n.parse().ok()))
        .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "invalid_cursor"))
}

fn encode_cursor(offset: u64) -> String {
    URL_SAFE_NO_PAD.encode(format!("o:{offset}"))
}

async fn list_tickets(
    State(ctx): State<AppContext>,
    Extension(c): Caller,
    Query(q): Query<TicketListQuery>,
) -> ApiResult {
    need(&c, ApiScope::TicketsRead)?;
    let projects = if q.project.trim().is_empty() {
        accessible(&ctx, &c).await?
    } else {
        vec![project_for(&ctx, &c, &q.project).await?]
    };
    let limit = q.limit.unwrap_or(25).clamp(1, MAX_LIMIT);
    let offset = decode_cursor(q.cursor.trim())?;
    let statuses = q
        .status
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            TicketStatus::try_from_value(&s.to_string())
                .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid_status"))
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if projects.is_empty() {
        return ok(json!({ "tickets": [], "total": 0, "next_cursor": null }));
    }
    let query = TicketQuery {
        project_ids: projects.iter().map(|p| p.id).collect(),
        q: Some(q.q.trim().to_string()).filter(|s| !s.is_empty()),
        statuses,
        stage_keys: q
            .stage
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect(),
        offset,
        limit,
        ..TicketQuery::default()
    };
    let (rows, total) = tickets::Model::page(&ctx.db, c.workspace.workspace_id(), &query).await?;
    let key = |pid: Uuid| {
        projects
            .iter()
            .find(|p| p.id == pid)
            .map(|p| p.key.clone())
            .unwrap_or_default()
    };
    let next = offset + rows.len() as u64;
    let list: Vec<Value> = rows
        .iter()
        .map(|t| ticket_json(t, &key(t.project_id)))
        .collect();
    ok(json!({
        "tickets": list,
        "total": total,
        "next_cursor": (next < total).then(|| encode_cursor(next)),
    }))
}

async fn ticket_detail(
    ctx: &AppContext,
    caller: &ApiCaller,
    ticket: &tickets::Model,
    project: &projects::Model,
) -> std::result::Result<Value, ApiError> {
    let wid = caller.workspace.workspace_id();
    let ty = ticket_types::Model::find_in(&ctx.db, wid, ticket.type_id).await?;
    let parent = match ticket.parent_id {
        Some(id) => tickets::Model::find_in(&ctx.db, wid, id)
            .await?
            .map(|p| p.code),
        None => None,
    };
    let mut out = ticket_json(ticket, &project.key);
    out["type"] = json!(ty.map(|t| t.key));
    out["description"] = json!(ticket.description);
    out["parent"] = json!(parent);
    out["due_on"] = json!(ticket.due_on);
    out["stage_state"] = json!(ticket.stage_state);
    out["created_at"] = json!(ticket.created_at);
    Ok(out)
}

async fn get_ticket(
    State(ctx): State<AppContext>,
    Extension(c): Caller,
    Path(code): Path<String>,
) -> ApiResult {
    need(&c, ApiScope::TicketsRead)?;
    let (ticket, project) = ticket_for(&ctx, &c, &code).await?;
    ok(json!({ "ticket": ticket_detail(&ctx, &c, &ticket, &project).await? }))
}

#[derive(Debug, Deserialize, Default)]
struct TicketBody {
    #[serde(default)]
    project: String,
    title: Option<String>,
    description: Option<String>,
    status: Option<String>,
    priority: Option<String>,
    labels: Option<Vec<String>>,
    due_on: Option<String>,
    story_points: Option<i32>,
    #[serde(rename = "type")]
    type_key: Option<String>,
    parent: Option<String>,
    #[serde(default)]
    stage: String,
}

async fn type_id_of(
    ctx: &AppContext,
    caller: &ApiCaller,
    key: &str,
) -> std::result::Result<String, ApiError> {
    ticket_types::Model::all_in(&ctx.db, caller.workspace.workspace_id())
        .await?
        .into_iter()
        .find(|t| t.key == key.trim().to_lowercase())
        .map(|t| t.id.to_string())
        .ok_or(ApiError {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            code: "ticket-type-unknown".into(),
            field: Some("type".into()),
            message: None,
        })
}

async fn create_ticket(
    State(ctx): State<AppContext>,
    Extension(c): Caller,
    Json(body): Json<TicketBody>,
) -> ApiResult {
    need(&c, ApiScope::TicketsWrite)?;
    let project = project_for(&ctx, &c, &body.project).await?;
    let type_id = match body.type_key.as_deref() {
        Some(k) if !k.trim().is_empty() => type_id_of(&ctx, &c, k).await?,
        _ => String::new(),
    };
    let form = TicketForm {
        input: TicketInput {
            title: body.title.clone().unwrap_or_default(),
            description: body.description.clone().unwrap_or_default(),
            status: body.status.clone().unwrap_or_default(),
            priority: body.priority.clone().unwrap_or_default(),
            labels: body.labels.clone().unwrap_or_default().join(", "),
            due_on: body.due_on.clone().unwrap_or_default(),
            story_points: body.story_points.map(|p| p.to_string()).unwrap_or_default(),
        },
        type_id,
        parent: body.parent.clone().unwrap_or_default(),
        stage: body.stage.trim().to_string(),
        ..TicketForm::default()
    };
    let ticket = create::run(&ctx.db, &c.acting(), project.id, &form).await?;
    created(json!({ "ticket": ticket_detail(&ctx, &c, &ticket, &project).await? }))
}

async fn patch_ticket(
    State(ctx): State<AppContext>,
    Extension(c): Caller,
    Path(code): Path<String>,
    Json(body): Json<TicketBody>,
) -> ApiResult {
    need(&c, ApiScope::TicketsWrite)?;
    let (ticket, project) = ticket_for(&ctx, &c, &code).await?;
    let wid = c.workspace.workspace_id();
    let parent = match ticket.parent_id {
        Some(id) => tickets::Model::find_in(&ctx.db, wid, id)
            .await?
            .map(|p| p.code),
        None => None,
    };
    let mut form = crate::controllers::tickets::ticket::current_form(&ticket, parent.as_deref());
    if let Some(v) = body.title {
        form.input.title = v;
    }
    if let Some(v) = body.description {
        form.input.description = v;
    }
    if let Some(v) = body.status {
        form.input.status = v;
    }
    if let Some(v) = body.priority {
        form.input.priority = v;
    }
    if let Some(v) = body.labels {
        form.input.labels = v.join(", ");
    }
    if let Some(v) = body.due_on {
        form.input.due_on = v;
    }
    if let Some(v) = body.story_points {
        form.input.story_points = v.to_string();
    }
    if let Some(v) = body.parent {
        form.parent = v;
    }
    if let Some(k) = body.type_key.as_deref().filter(|k| !k.trim().is_empty()) {
        form.type_id = type_id_of(&ctx, &c, k).await?;
    }
    let ticket = update::run(&ctx.db, &c.acting(), ticket.id, &form).await?;
    ok(json!({ "ticket": ticket_detail(&ctx, &c, &ticket, &project).await? }))
}

#[derive(Debug, Deserialize)]
struct TransitionBody {
    #[serde(default)]
    to: String,
}

/// Pindah stage lewat gate yang sama dengan board/MCP (`stages::move_to`).
async fn transition(
    State(ctx): State<AppContext>,
    Extension(c): Caller,
    Path(code): Path<String>,
    Json(body): Json<TransitionBody>,
) -> ApiResult {
    need(&c, ApiScope::TicketsWrite)?;
    let (ticket, project) = ticket_for(&ctx, &c, &code).await?;
    if body.to.trim().is_empty() {
        return Err(ApiError {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            code: stages::STAGE_UNKNOWN.into(),
            field: Some("to".into()),
            message: None,
        });
    }
    let moved = stages::move_to(
        &ctx.db,
        &c.acting(),
        ticket.id,
        Target::Stage(body.to.trim().to_string()),
        None,
    )
    .await?;
    ok(json!({ "ticket": ticket_detail(&ctx, &c, &moved, &project).await? }))
}

async fn list_comments(
    State(ctx): State<AppContext>,
    Extension(c): Caller,
    Path(code): Path<String>,
) -> ApiResult {
    need(&c, ApiScope::TicketsRead)?;
    let (ticket, _) = ticket_for(&ctx, &c, &code).await?;
    let wid = c.workspace.workspace_id();
    let rows = ticket_comments::Model::of_ticket(&ctx.db, wid, ticket.id).await?;
    let ids: Vec<Uuid> = rows.iter().filter_map(|r| r.author_id).collect();
    let names = users::Model::names_by_id(&ctx.db, &ids).await?;
    let list: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.id,
                "author": r.author_id.and_then(|id| names.get(&id).cloned()),
                "body": r.body,
                "created_at": r.created_at,
            })
        })
        .collect();
    ok(json!({ "comments": list }))
}

#[derive(Debug, Deserialize)]
struct CommentBody {
    #[serde(default)]
    body: String,
}

async fn post_comment(
    State(ctx): State<AppContext>,
    Extension(c): Caller,
    Path(code): Path<String>,
    Json(body): Json<CommentBody>,
) -> ApiResult {
    need(&c, ApiScope::CommentsWrite)?;
    let (ticket, _) = ticket_for(&ctx, &c, &code).await?;
    let comment = comments::post(&ctx.db, &c.acting(), ticket.id, &body.body).await?;
    created(
        json!({ "comment": { "id": comment.id, "body": comment.body, "created_at": comment.created_at } }),
    )
}

// ---------------------------------------------------------------------------------------------
// Sprint & knowledge
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Deserialize, Default)]
struct ProjectQuery {
    #[serde(default)]
    project: String,
}

fn project_required(q: &ProjectQuery) -> std::result::Result<(), ApiError> {
    if q.project.trim().is_empty() {
        Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            code: "project_required".into(),
            field: Some("project".into()),
            message: None,
        })
    } else {
        Ok(())
    }
}

async fn list_sprints(
    State(ctx): State<AppContext>,
    Extension(c): Caller,
    Query(q): Query<ProjectQuery>,
) -> ApiResult {
    need(&c, ApiScope::ReportsRead)?;
    project_required(&q)?;
    let project = project_for(&ctx, &c, &q.project).await?;
    let rows = sprints::Model::of_project(&ctx.db, c.workspace.workspace_id(), project.id).await?;
    let list: Vec<Value> = rows
        .iter()
        .map(|s| {
            json!({
                "id": s.id, "name": s.name, "goal": s.goal, "state": s.state,
                "starts_on": s.starts_on, "ends_on": s.ends_on,
                "committed_points": s.committed_points, "completed_points": s.completed_points,
                "started_at": s.started_at, "completed_at": s.completed_at,
            })
        })
        .collect();
    ok(json!({ "sprints": list }))
}

fn doc_json(d: &knowledge_docs::Model, body: bool) -> Value {
    let mut v = json!({
        "id": d.id, "slug": d.slug, "title": d.title, "group": d.group_key, "type": d.type_key,
        "version": d.version, "stale": d.stale_at.is_some(), "updated_at": d.updated_at,
        "scope": if d.project_id.is_some() { "project" } else { "workspace" },
    });
    if body {
        v["body"] = json!(d.body);
    }
    v
}

async fn list_knowledge(
    State(ctx): State<AppContext>,
    Extension(c): Caller,
    Query(q): Query<ProjectQuery>,
) -> ApiResult {
    need(&c, ApiScope::KnowledgeRead)?;
    project_required(&q)?;
    let project = project_for(&ctx, &c, &q.project).await?;
    let rows = docs::readable(&ctx.db, c.workspace.workspace_id(), project.id, None).await?;
    let list: Vec<Value> = rows.iter().map(|d| doc_json(d, false)).collect();
    ok(json!({ "documents": list }))
}

async fn get_knowledge(
    State(ctx): State<AppContext>,
    Extension(c): Caller,
    Path(slug): Path<String>,
    Query(q): Query<ProjectQuery>,
) -> ApiResult {
    need(&c, ApiScope::KnowledgeRead)?;
    project_required(&q)?;
    let project = project_for(&ctx, &c, &q.project).await?;
    let doc = docs::readable(&ctx.db, c.workspace.workspace_id(), project.id, None)
        .await?
        .into_iter()
        .find(|d| d.slug == slug)
        .ok_or_else(ApiError::not_found)?;
    ok(json!({ "document": doc_json(&doc, true) }))
}

// ---------------------------------------------------------------------------------------------
// Webhook
// ---------------------------------------------------------------------------------------------

fn hook_json(h: &webhooks::Model) -> Value {
    json!({
        "id": h.id, "url": h.url, "events": h.events, "project_ids": h.project_ids,
        "active": h.is_active(), "consecutive_failures": h.consecutive_failures,
        "last_delivery_at": h.last_delivery_at, "last_status": h.last_status,
        "created_at": h.created_at,
    })
}

/// Pemanggil boleh mengelola webhook: cakupan `webhook:kelola` dan role pembuat
/// `ManageIntegrations` (tanpa izin → 404, sama dengan UI).
fn manage(c: &ApiCaller) -> std::result::Result<(), ApiError> {
    need(c, ApiScope::WebhooksManage)?;
    if c.role.has(Permission::ManageIntegrations) {
        Ok(())
    } else {
        Err(ApiError::not_found())
    }
}

async fn list_webhooks(State(ctx): State<AppContext>, Extension(c): Caller) -> ApiResult {
    manage(&c)?;
    let rows = webhooks::Model::of_workspace(&ctx.db, c.workspace.workspace_id()).await?;
    ok(json!({ "webhooks": rows.iter().map(hook_json).collect::<Vec<_>>() }))
}

#[derive(Debug, Deserialize)]
struct WebhookBody {
    #[serde(default)]
    url: String,
    #[serde(default)]
    events: Vec<String>,
    #[serde(default)]
    projects: Vec<String>,
}

async fn create_webhook(
    State(ctx): State<AppContext>,
    Extension(c): Caller,
    Json(body): Json<WebhookBody>,
) -> ApiResult {
    manage(&c)?;
    let settings = Settings::from_ctx(&ctx)
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "server_error"))?;
    let input = WebhookInput {
        url: body.url,
        events: body.events,
        projects: body.projects,
    };
    let (hook, secret) = hooks::create(&ctx.db, &c.acting(), &settings, &input).await?;
    let mut v = hook_json(&hook);
    v["secret"] = json!(secret);
    created(json!({ "webhook": v }))
}

async fn delete_webhook(
    State(ctx): State<AppContext>,
    Extension(c): Caller,
    Path(id): Path<String>,
) -> ApiResult {
    manage(&c)?;
    let id = Uuid::try_parse(&id).map_err(|_| ApiError::not_found())?;
    hooks::delete(&ctx.db, &c.acting(), id).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn openapi() -> Response {
    (
        [(header::CONTENT_TYPE, "application/yaml; charset=utf-8")],
        OPENAPI,
    )
        .into_response()
}

pub fn routes(ctx: &AppContext) -> Routes {
    Routes::new()
        .prefix("/api/v1")
        .add("/openapi.yaml", get(openapi))
        .add("/projects", get(list_projects))
        .add("/tickets", get(list_tickets).post(create_ticket))
        .add("/tickets/{code}", get(get_ticket).patch(patch_ticket))
        .add("/tickets/{code}/transition", post(transition))
        .add(
            "/tickets/{code}/comments",
            get(list_comments).post(post_comment),
        )
        .add("/sprints", get(list_sprints))
        .add("/knowledge", get(list_knowledge))
        .add("/knowledge/{slug}", get(get_knowledge))
        .add("/webhooks", get(list_webhooks).post(create_webhook))
        .add("/webhooks/{id}", delete(delete_webhook))
        .layer(axum::middleware::from_fn_with_state(
            ctx.clone(),
            auth_layer,
        ))
}
