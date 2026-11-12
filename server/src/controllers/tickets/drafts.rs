//! Project → Draft tiket: draft hasil brainstorming agent (`ag_draft_tickets`) ditinjau, diubah,
//! dihapus, lalu yang dicentang dibuat menjadi tiket (`services::drafts`). Melihat butuh
//! `ViewProject`, mengubah `EditProject`.
use axum::http::StatusCode;
use loco_rs::prelude::*;
use uuid::Uuid;

use super::{acting, project_paths, summary};
use crate::{
    controllers::{
        extractors::{ProjectAccess, RequestId},
        flash,
        shell::app_shell,
    },
    models::{permission::Permission, ticket_drafts, ticket_types},
    services::drafts::{self as service, DraftEdit},
    views::{
        self,
        drafts::{FLASH_CREATED, FLASH_DELETED, FLASH_SAVED},
        flash::Flash,
        form::field_errors,
        nav,
        projects::PROJECT_VIEWS_ACTION,
    },
};

const EDIT_FIELDS: &[&str] = &["title", "description", "type", "story_points"];

fn base(access: &ProjectAccess) -> String {
    format!("{}/drafts", project_paths(access).home)
}

async fn render(
    ctx: &AppContext,
    v: &TeraView,
    access: &ProjectAccess,
    flash: Option<Flash>,
    failed: Option<(Uuid, &loco_rs::validation::ModelValidationErrors)>,
    status: StatusCode,
) -> Result<Response> {
    let wid = access.tenant.workspace_id();
    let drafts = ticket_drafts::Model::of_project(&ctx.db, wid, access.project.id).await?;
    let types = ticket_types::Model::all_in(&ctx.db, wid).await?;
    let rows = views::drafts::tree(&drafts, &types, &base(access));
    let home = project_paths(access).home;
    let shell = app_shell(ctx, &access.tenant.user, Some(&access.tenant), Some(access))
        .await?
        .at("project_board");
    views::tickets::render(
        v,
        &shell,
        "tickets/drafts.html",
        status,
        data!({
            "project": summary(access),
            "views": nav::project_views(&home, &access.tenant.user.project_views, "board"),
            "views_action": PROJECT_VIEWS_ACTION,
            "return_to": base(access),
            "stats": views::drafts::stats(&rows),
            "approved": rows.iter().filter(|r| r.approved).count(),
            "rows": rows,
            "types": types.iter().map(views::tickets::TypeView::of).collect::<Vec<_>>(),
            "create_action": format!("{}/create", base(access)),
            "board": format!("{home}/board"),
            "can_edit": access.role.has(Permission::EditProject),
            "failed_id": failed.map(|(id, _)| id.to_string()).unwrap_or_default(),
            "errors": field_errors(EDIT_FIELDS, failed.map(|(_, e)| e)),
            "blank": field_errors(EDIT_FIELDS, None),
            "flash": flash,
        }),
    )
}

#[debug_handler]
pub(super) async fn index(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: ProjectAccess,
    session: Session,
) -> Result<Response> {
    access.require(Permission::ViewProject)?;
    let (session, flash) = flash::take(session);
    let res = render(&ctx, &v, &access, flash, None, StatusCode::OK).await?;
    Ok((session, res).into_response())
}

fn values<'a>(fields: &'a [(String, String)], name: &'a str) -> impl Iterator<Item = &'a str> {
    fields
        .iter()
        .filter(move |(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

fn value(fields: &[(String, String)], name: &str) -> String {
    values(fields, name).next().unwrap_or_default().to_string()
}

#[debug_handler]
pub(super) async fn create(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: ProjectAccess,
    session: Session,
    RequestId(request_id): RequestId,
    Form(fields): Form<Vec<(String, String)>>,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    let ids: Vec<Uuid> = values(&fields, "draft")
        .filter_map(|v| Uuid::try_parse(v).ok())
        .collect();
    match service::create_tickets(
        &ctx.db,
        &acting(&access.tenant, request_id),
        access.project.id,
        &ids,
    )
    .await
    {
        Ok(_) => {
            let session = flash::set(session, &Flash::success(FLASH_CREATED))?;
            Ok((
                session,
                format::redirect(&format!("{}/list", project_paths(&access).home))?,
            )
                .into_response())
        }
        Err(crate::services::tickets::TicketFailure::Invalid(_)) => {
            render(
                &ctx,
                &v,
                &access,
                Some(Flash::error(service::NONE_SELECTED)),
                None,
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => Err(e.into()),
    }
}

#[debug_handler]
pub(super) async fn update(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: ProjectAccess,
    session: Session,
    RequestId(request_id): RequestId,
    Path((_slug, _key, id)): Path<(String, String, Uuid)>,
    Form(fields): Form<Vec<(String, String)>>,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    let edit = DraftEdit {
        title: value(&fields, "title"),
        description: value(&fields, "description"),
        type_key: value(&fields, "type"),
        story_points: value(&fields, "story_points"),
    };
    match service::update(
        &ctx.db,
        &acting(&access.tenant, request_id),
        access.project.id,
        id,
        &edit,
    )
    .await
    {
        Ok(_) => {
            let session = flash::set(session, &Flash::success(FLASH_SAVED))?;
            Ok((session, format::redirect(&base(&access))?).into_response())
        }
        Err(crate::services::tickets::TicketFailure::Invalid(errors)) => {
            render(
                &ctx,
                &v,
                &access,
                None,
                Some((id, &errors)),
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => Err(e.into()),
    }
}

#[debug_handler]
pub(super) async fn delete(
    State(ctx): State<AppContext>,
    access: ProjectAccess,
    session: Session,
    RequestId(request_id): RequestId,
    Path((_slug, _key, id)): Path<(String, String, Uuid)>,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    service::delete(
        &ctx.db,
        &acting(&access.tenant, request_id),
        access.project.id,
        id,
    )
    .await?;
    let session = flash::set(session, &Flash::success(FLASH_DELETED))?;
    Ok((session, format::redirect(&base(&access))?).into_response())
}
