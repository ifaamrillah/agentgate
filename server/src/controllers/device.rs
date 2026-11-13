//! Login perangkat (device code flow, RFC 8628) untuk `agentgate login`:
//! `POST /oauth/device_authorization` (API JSON, tanpa sesi) dan halaman solo `/device` (login
//! wajib): masukkan kode → setujui (pilih workspace) / tolak → selesai. Logika di
//! `services::device`; token di `POST /oauth/token` (`controllers::oauth`).
use axum::{Json, extract::Query, http::StatusCode};
use loco_rs::prelude::*;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    common::settings::Settings,
    controllers::extractors::{CurrentUser, RequestId},
    models::users,
    services::device::{self as service, format_user_code},
    views::device::DevicePage,
};

pub const CODE_INVALID: &str = "device-code-invalid";
pub const WORKSPACE_REQUIRED: &str = "oauth-workspace-required";

#[derive(Debug, Deserialize)]
struct StartForm {
    #[serde(default)]
    client_id: String,
    #[serde(default)]
    device_name: String,
}

#[debug_handler]
async fn authorize(State(ctx): State<AppContext>, Form(f): Form<StartForm>) -> Result<Response> {
    let base = Settings::from_ctx(&ctx)?.public_base_url;
    Ok(
        match service::start(&ctx.db, &f.client_id, &f.device_name, &base).await {
            Ok(start) => super::oauth::no_store(Json(start).into_response()),
            Err(e) => super::oauth::json_error(StatusCode::BAD_REQUEST, &e),
        },
    )
}

fn render(v: &TeraView, page: &DevicePage, status: StatusCode) -> Result<Response> {
    let mut res = format::render().view(v, "device/page.html", data!({ "page": page }))?;
    *res.status_mut() = status;
    Ok(res)
}

async fn confirm_page(
    ctx: &AppContext,
    user: &users::Model,
    code: &str,
    error: &'static str,
) -> Result<Option<DevicePage>> {
    let Some(row) = service::find_pending(&ctx.db, code).await? else {
        return Ok(None);
    };
    let choices = service::choices(&ctx.db, user).await?;
    Ok(Some(DevicePage::confirm(user, &row, &choices, error)))
}

#[derive(Debug, Deserialize)]
struct CodeQuery {
    #[serde(default)]
    code: String,
    #[serde(default)]
    done: String,
}

#[debug_handler]
async fn show(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    CurrentUser(user): CurrentUser,
    Query(q): Query<CodeQuery>,
) -> Result<Response> {
    match q.done.as_str() {
        "approved" => return render(&v, &DevicePage::done(&user, true), StatusCode::OK),
        "denied" => return render(&v, &DevicePage::done(&user, false), StatusCode::OK),
        _ => {}
    }
    if !q.code.is_empty()
        && let Some(page) = confirm_page(&ctx, &user, &q.code, "").await?
    {
        return render(&v, &page, StatusCode::OK);
    }
    render(&v, &DevicePage::enter(&q.code, ""), StatusCode::OK)
}

#[derive(Debug, Deserialize)]
struct EnterForm {
    #[serde(default)]
    user_code: String,
}

#[debug_handler]
async fn enter(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    CurrentUser(user): CurrentUser,
    Form(f): Form<EnterForm>,
) -> Result<Response> {
    match confirm_page(&ctx, &user, &f.user_code, "").await? {
        Some(page) => render(&v, &page, StatusCode::OK),
        None => render(
            &v,
            &DevicePage::enter(&f.user_code, CODE_INVALID),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
    }
}

#[derive(Debug, Deserialize)]
struct DecideForm {
    #[serde(default)]
    user_code: String,
    #[serde(default)]
    workspace: String,
    #[serde(default)]
    decision: String,
}

#[debug_handler]
async fn decide(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    CurrentUser(user): CurrentUser,
    RequestId(request_id): RequestId,
    Form(f): Form<DecideForm>,
) -> Result<Response> {
    if f.decision != "allow" {
        if service::deny(&ctx.db, &user, &f.user_code).await? {
            return format::redirect("/device?done=denied");
        }
        return render(
            &v,
            &DevicePage::enter(&f.user_code, CODE_INVALID),
            StatusCode::UNPROCESSABLE_ENTITY,
        );
    }
    let workspace = Uuid::try_parse(f.workspace.trim()).ok();
    if let Some(w) = workspace
        && service::approve(&ctx.db, &user, w, &f.user_code, request_id).await?
    {
        return format::redirect("/device?done=approved");
    }
    match confirm_page(&ctx, &user, &f.user_code, WORKSPACE_REQUIRED).await? {
        Some(page) => render(&v, &page, StatusCode::UNPROCESSABLE_ENTITY),
        None => render(
            &v,
            &DevicePage::enter(&format_user_code(&f.user_code), CODE_INVALID),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
    }
}

pub fn routes() -> Routes {
    Routes::new()
        .add("/oauth/device_authorization", post(authorize))
        .add("/device", get(show).post(enter))
        .add("/device/decide", post(decide))
}
