//! Akun → Hubungkan client AI: `GET /account/connect?section=…` (langkah pemasangan, client
//! terhubung, tool MCP, slash command) dan `POST /account/connect/{id}/revoke` (cabut grant milik
//! sendiri). Data per user, bukan per workspace: satu grant = satu client di satu workspace.
use axum::extract::{Path, Query};
use loco_rs::prelude::*;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    common::settings::Settings,
    controllers::{
        extractors::{CurrentUser, RequestId},
        flash,
        shell::app_shell,
    },
    services::{mcp::prompts, oauth},
    views::{
        self,
        flash::Flash,
        oauth::{ClientRow, FLASH_REVOKED, SECTIONS},
    },
};

#[derive(Debug, Deserialize)]
struct SectionQuery {
    #[serde(default)]
    section: String,
}

#[debug_handler]
async fn show(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    CurrentUser(user): CurrentUser,
    Query(q): Query<SectionQuery>,
    session: Session,
) -> Result<Response> {
    let (session, flash) = flash::take(session);
    let section = SECTIONS
        .iter()
        .find(|s| **s == q.section && **s != "files")
        .copied()
        .unwrap_or("steps");
    let base = Settings::from_ctx(&ctx)?.public_base_url;
    let clients: Vec<ClientRow> = oauth::grants_of(&ctx.db, user.id)
        .await?
        .iter()
        .map(|(g, c, w)| ClientRow::of(g, c, w))
        .collect();
    let workspaces = oauth::consent_workspaces(&ctx.db, &user).await?;
    let slash = prompts::catalog(&ctx.db, &workspaces).await?;
    let slash_count = slash.iter().map(prompts::Group::len).sum::<usize>();
    let shell = app_shell(&ctx, &user, None, None).await?.at("connect");
    let res = views::tickets::render(
        &v,
        &shell,
        "account/connect.html",
        axum::http::StatusCode::OK,
        data!({
            "section": section,
            "subnav": views::oauth::subnav(section, clients.len(), slash_count),
            "stats": views::oauth::stats(clients.len(), workspaces.len()),
            "mcp_url": format!("{base}/mcp"),
            "clients": clients,
            "tools": views::oauth::tools(),
            "slash": slash,
            "utilities": prompts::UTILITIES
                .iter()
                .map(|(name, key, ready)| serde_json::json!({ "name": name, "key": key, "ready": ready }))
                .collect::<Vec<_>>(),
            "flash": flash,
        }),
    )?;
    Ok((session, res).into_response())
}

#[debug_handler]
async fn revoke(
    State(ctx): State<AppContext>,
    CurrentUser(user): CurrentUser,
    RequestId(request_id): RequestId,
    Path(id): Path<Uuid>,
    session: Session,
) -> Result<Response> {
    if !oauth::revoke_own(&ctx.db, user.id, id, request_id).await? {
        return Err(Error::NotFound);
    }
    let session = flash::set(session, &Flash::success(FLASH_REVOKED))?;
    Ok((
        session,
        format::redirect("/account/connect?section=clients")?,
    )
        .into_response())
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/account/connect")
        .add("/", get(show))
        .add("/{id}/revoke", post(revoke))
}
