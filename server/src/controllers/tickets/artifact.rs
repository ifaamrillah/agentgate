//! Artifact tiket (M3): navigator (daftar per stage + pratinjau versi), editor dokumen di halaman
//! sendiri (aturan UX: dokumen panjang punya halaman editor), dan perbandingan dua versi.
use std::collections::BTreeMap;

use axum::http::StatusCode;
use loco_rs::{prelude::*, validation::ModelValidationErrors};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{acting, lookup, stage, summary, ticket::tabs};
use crate::{
    controllers::{
        extractors::{IdOr404, RequestId, TicketAccess},
        flash,
        shell::app_shell,
    },
    models::{
        artifact_versions::{self, VersionStatus},
        permission::Permission,
        ticket_artifacts, ticket_events,
        tickets::StageState,
    },
    services::tickets::{TicketFailure, artifacts as service, stages::Flow},
    views::{
        self,
        flash::Flash,
        tickets::{BoardPaths, PersonView, TicketPaths},
    },
};

pub const FLASH_ARTIFACT_SAVED: &str = "flash-artifact-saved";
pub const FLASH_ARTIFACT_CREATED: &str = "flash-artifact-created";

/// Jumlah artifact dan jumlah yang versi terbarunya menunggu review (badge tab).
///
/// # Errors
///
/// Error database.
pub(super) async fn counts(ctx: &AppContext, access: &TicketAccess) -> Result<(u64, u64)> {
    let wid = access.access.tenant.workspace_id();
    let artifacts = ticket_artifacts::Model::of_ticket(&ctx.db, wid, access.ticket.id).await?;
    let ids: Vec<Uuid> = artifacts.iter().map(|a| a.id).collect();
    let versions = artifact_versions::Model::of_artifacts(&ctx.db, wid, &ids).await?;
    let pending = artifacts
        .iter()
        .filter(|a| {
            versions
                .iter()
                .find(|v| v.artifact_id == a.id)
                .is_some_and(|v| v.status().ok() == Some(VersionStatus::Pending))
        })
        .count();
    Ok((artifacts.len() as u64, pending as u64))
}

#[derive(Debug, Clone, Serialize)]
struct VersionView {
    number: i32,
    status: String,
    status_key: String,
    variant: &'static str,
    author: Option<PersonView>,
    at: String,
    note: String,
    href: String,
    current: bool,
}

#[derive(Debug, Clone, Serialize)]
struct ArtifactView {
    id: Uuid,
    name: String,
    stage: String,
    stage_key: String,
    href: String,
    edit_href: String,
    latest: Option<VersionView>,
    current: bool,
}

fn status_variant(s: VersionStatus) -> &'static str {
    match s {
        VersionStatus::Draft => "neutral",
        VersionStatus::Pending => "gate",
        VersionStatus::Approved => "success",
        VersionStatus::Rejected => "destructive",
    }
}

fn status_key(raw: &str) -> String {
    format!("artifact-status-{raw}")
}

struct Loaded {
    artifacts: Vec<ticket_artifacts::Model>,
    versions: Vec<artifact_versions::Model>,
    lookup: views::tickets::Lookup,
    flow: Flow,
}

async fn load(ctx: &AppContext, access: &TicketAccess) -> Result<Loaded> {
    let tenant = &access.access.tenant;
    let wid = tenant.workspace_id();
    let artifacts = ticket_artifacts::Model::of_ticket(&ctx.db, wid, access.ticket.id).await?;
    let ids: Vec<Uuid> = artifacts.iter().map(|a| a.id).collect();
    let versions = artifact_versions::Model::of_artifacts(&ctx.db, wid, &ids).await?;
    let people: Vec<Uuid> = versions.iter().filter_map(|v| v.author_id).collect();
    let lookup = lookup(
        &ctx.db,
        tenant,
        std::slice::from_ref(&access.ticket),
        &people,
    )
    .await?;
    let flow = Flow::load(&ctx.db, wid, &access.ticket).await?;
    Ok(Loaded {
        artifacts,
        versions,
        lookup,
        flow,
    })
}

fn version_view(
    v: &artifact_versions::Model,
    lookup: &views::tickets::Lookup,
    base: &str,
    current: bool,
) -> VersionView {
    let status = v.status().unwrap_or(VersionStatus::Draft);
    VersionView {
        number: v.number,
        status_key: status_key(&v.status),
        status: v.status.clone(),
        variant: status_variant(status),
        author: lookup.person(v.author_id),
        at: views::time::wib_datetime(&v.created_at),
        note: v.note.clone(),
        href: format!("{base}&v={}", v.number),
        current,
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct NavQuery {
    #[serde(default)]
    a: Option<Uuid>,
    #[serde(default)]
    v: Option<i32>,
}

/// Navigator artifact.
#[debug_handler]
pub(super) async fn index(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: TicketAccess,
    session: Session,
    Query(query): Query<NavQuery>,
) -> Result<Response> {
    let (session, flash) = flash::take(session);
    let res = render_index(&ctx, &v, &access, &query, "", "", flash, StatusCode::OK).await?;
    Ok((session, res).into_response())
}

#[allow(clippy::too_many_arguments)]
async fn render_index(
    ctx: &AppContext,
    v: &TeraView,
    access: &TicketAccess,
    query: &NavQuery,
    new_name: &str,
    error: &str,
    flash: Option<Flash>,
    status: StatusCode,
) -> Result<Response> {
    let tenant = &access.access.tenant;
    let t = &access.ticket;
    let paths = TicketPaths::new(&tenant.workspace.slug, &t.code);
    let loaded = load(ctx, access).await?;
    let selected = query
        .a
        .and_then(|id| loaded.artifacts.iter().find(|a| a.id == id))
        .or_else(|| loaded.artifacts.iter().find(|a| a.stage_key == t.stage_key))
        .or(loaded.artifacts.first());
    let base_of = |a: &ticket_artifacts::Model| format!("{}?a={}", paths.artifacts, a.id);
    let mut groups: BTreeMap<usize, (String, Vec<ArtifactView>)> = BTreeMap::new();
    for a in &loaded.artifacts {
        let stage_name = loaded
            .flow
            .rule(&a.stage_key)
            .map_or_else(|| a.stage_key.clone(), |s| s.name.clone());
        let latest = loaded
            .versions
            .iter()
            .find(|v| v.artifact_id == a.id)
            .map(|v| version_view(v, &loaded.lookup, &base_of(a), false));
        groups
            .entry(loaded.flow.position(&a.stage_key))
            .or_insert_with(|| (stage_name.clone(), Vec::new()))
            .1
            .push(ArtifactView {
                id: a.id,
                name: a.name.clone(),
                stage: stage_name,
                stage_key: a.stage_key.clone(),
                href: base_of(a),
                edit_href: format!("{}/{}/edit", paths.artifacts, a.id),
                latest,
                current: selected.is_some_and(|s| s.id == a.id),
            });
    }
    let groups: Vec<_> = groups
        .into_values()
        .map(|(stage, items)| serde_json::json!({ "stage": stage, "items": items }))
        .collect();
    let preview = selected.map(|a| {
        let versions: Vec<&artifact_versions::Model> = loaded
            .versions
            .iter()
            .filter(|v| v.artifact_id == a.id)
            .collect();
        let shown = query
            .v
            .and_then(|n| versions.iter().find(|v| v.number == n).copied())
            .or(versions.first().copied());
        let number = shown.map_or(0, |s| s.number);
        let previous = versions.iter().find(|v| v.number < number).map(|v| v.number);
        serde_json::json!({
            "id": a.id,
            "name": a.name,
            "stage": loaded.flow.rule(&a.stage_key).map_or_else(|| a.stage_key.clone(), |s| s.name.clone()),
            "in_review": a.stage_key == t.stage_key && t.stage_state().ok() == Some(StageState::Review),
            "versions": versions.iter().map(|v| version_view(v, &loaded.lookup, &base_of(a), v.number == number)).collect::<Vec<_>>(),
            "shown": shown.map(|v| version_view(v, &loaded.lookup, &base_of(a), true)),
            "body": shown.map(|v| v.body.clone()).unwrap_or_default(),
            "edit_href": format!("{}/{}/edit", paths.artifacts, a.id),
            "compare_href": previous.map(|p| format!("{}/{}/compare?from={p}&to={number}", paths.artifacts, a.id)),
        })
    });
    let events = ticket_events::Model::of_ticket(&ctx.db, tenant.workspace_id(), t.id).await?;
    let stage_panel = stage::panel(ctx, access).await?;
    let suggested = stage_panel.output.clone();
    let shell = app_shell(ctx, &tenant.user, Some(tenant), Some(&access.access))
        .await?
        .at("project_ticket");
    let res = views::tickets::render(
        v,
        &shell,
        "tickets/artifacts.html",
        status,
        data!({
            "project": summary(&access.access),
            "ticket": loaded.lookup.row(t),
            "paths": paths,
            "board": BoardPaths::new(&super::project_paths(&access.access).home),
            "tabs": tabs(&paths, "artifacts", events.len() as u64, counts(ctx, access).await?, super::pulls::counts(ctx, access).await?),
            "stage": stage_panel,
            "groups": groups,
            "preview": preview,
            "can_edit": access.can(Permission::EditProject),
            "new_name": if new_name.is_empty() { suggested } else { new_name.to_string() },
            "new_open": !error.is_empty(),
            "error": error,
            "errors": { "dialogs": stage::errors_view("", "") },
            "watching": crate::models::ticket_watchers::Model::user_ids(&ctx.db, tenant.workspace_id(), t.id)
                .await?
                .contains(&tenant.user.id),
            "flash": flash,
        }),
    )?;
    Ok(res)
}

#[derive(Debug, Deserialize)]
pub struct NewForm {
    #[serde(default)]
    name: String,
}

#[debug_handler]
pub(super) async fn create(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: TicketAccess,
    session: Session,
    RequestId(request_id): RequestId,
    Form(form): Form<NewForm>,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    let tenant = &access.access.tenant;
    match service::create(
        &ctx.db,
        &acting(tenant, request_id),
        access.ticket.id,
        &form.name,
    )
    .await
    {
        Ok(a) => {
            let paths = TicketPaths::new(&tenant.workspace.slug, &access.ticket.code);
            let session = flash::set(session, &Flash::success(FLASH_ARTIFACT_CREATED))?;
            let to = format!("{}/{}/edit", paths.artifacts, a.id);
            Ok((session, format::redirect(&to)?).into_response())
        }
        Err(TicketFailure::Invalid(errors)) => {
            render_index(
                &ctx,
                &v,
                &access,
                &NavQuery::default(),
                &form.name,
                &first(&errors),
                None,
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => Err(e.into()),
    }
}

fn first(errors: &ModelValidationErrors) -> String {
    errors
        .errors
        .values()
        .flat_map(|l| l.iter())
        .next()
        .map(|e| e.code.clone())
        .unwrap_or_default()
}

async fn find(
    ctx: &AppContext,
    access: &TicketAccess,
    id: Uuid,
) -> Result<ticket_artifacts::Model> {
    ticket_artifacts::Model::find_of(
        &ctx.db,
        access.access.tenant.workspace_id(),
        access.ticket.id,
        id,
    )
    .await?
    .ok_or(Error::NotFound)
}

/// Halaman editor artifact.
#[debug_handler]
pub(super) async fn edit(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: TicketAccess,
    session: Session,
    id: IdOr404<ticket_artifacts::Entity>,
) -> Result<Response> {
    let artifact = find(&ctx, &access, id.id).await?;
    let (session, flash) = flash::take(session);
    let res = render_edit(
        &ctx,
        &v,
        &access,
        &artifact,
        None,
        "",
        flash,
        StatusCode::OK,
    )
    .await?;
    Ok((session, res).into_response())
}

#[allow(clippy::too_many_arguments)]
async fn render_edit(
    ctx: &AppContext,
    v: &TeraView,
    access: &TicketAccess,
    artifact: &ticket_artifacts::Model,
    body: Option<&str>,
    error: &str,
    flash: Option<Flash>,
    status: StatusCode,
) -> Result<Response> {
    let tenant = &access.access.tenant;
    let t = &access.ticket;
    let paths = TicketPaths::new(&tenant.workspace.slug, &t.code);
    let loaded = load(ctx, access).await?;
    let base = format!("{}?a={}", paths.artifacts, artifact.id);
    let versions: Vec<&artifact_versions::Model> = loaded
        .versions
        .iter()
        .filter(|v| v.artifact_id == artifact.id)
        .collect();
    let latest = versions.first();
    let href = format!("{}/{}", paths.artifacts, artifact.id);
    let shell = app_shell(ctx, &tenant.user, Some(tenant), Some(&access.access))
        .await?
        .at("project_ticket");
    let res = views::tickets::render(
        v,
        &shell,
        "tickets/artifact_edit.html",
        status,
        data!({
            "project": summary(&access.access),
            "ticket": loaded.lookup.row(t),
            "paths": paths,
            "artifact": {
                "id": artifact.id,
                "name": artifact.name,
                "stage": loaded.flow.rule(&artifact.stage_key).map_or_else(|| artifact.stage_key.clone(), |s| s.name.clone()),
            },
            "body": body.map_or_else(|| latest.map(|v| v.body.clone()).unwrap_or_default(), str::to_string),
            "latest": latest.map(|v| version_view(v, &loaded.lookup, &base, true)),
            "versions": versions.iter().enumerate().map(|(i, v)| {
                let mut view = version_view(v, &loaded.lookup, &base, i == 0);
                if let Some(prev) = versions.get(i + 1) {
                    view.href = format!("{href}/compare?from={}&to={}", prev.number, v.number);
                } else {
                    view.href = format!("{base}&v={}", v.number);
                }
                view
            }).collect::<Vec<_>>(),
            "action": href,
            "back": base,
            "can_edit": access.can(Permission::EditProject),
            "error": error,
            "flash": flash,
        }),
    )?;
    Ok(res)
}

#[derive(Debug, Deserialize)]
pub struct SaveForm {
    #[serde(default)]
    body: String,
}

#[debug_handler]
pub(super) async fn save(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: TicketAccess,
    session: Session,
    RequestId(request_id): RequestId,
    id: IdOr404<ticket_artifacts::Entity>,
    Form(form): Form<SaveForm>,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    let artifact = find(&ctx, &access, id.id).await?;
    match service::save(
        &ctx.db,
        &acting(&access.access.tenant, request_id),
        access.ticket.id,
        artifact.id,
        &form.body,
    )
    .await
    {
        Ok(_) => {
            let paths = TicketPaths::new(&access.access.tenant.workspace.slug, &access.ticket.code);
            let session = flash::set(session, &Flash::success(FLASH_ARTIFACT_SAVED))?;
            let to = format!("{}/{}/edit", paths.artifacts, artifact.id);
            Ok((session, format::redirect(&to)?).into_response())
        }
        Err(TicketFailure::Invalid(errors)) => {
            render_edit(
                &ctx,
                &v,
                &access,
                &artifact,
                Some(&form.body),
                &first(&errors),
                None,
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => Err(e.into()),
    }
}

#[derive(Debug, Deserialize)]
pub struct CompareQuery {
    #[serde(default)]
    from: Option<i32>,
    #[serde(default)]
    to: Option<i32>,
}

/// Bandingkan dua versi (diff baris).
#[debug_handler]
pub(super) async fn compare(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: TicketAccess,
    id: IdOr404<ticket_artifacts::Entity>,
    Query(query): Query<CompareQuery>,
) -> Result<Response> {
    let artifact = find(&ctx, &access, id.id).await?;
    let tenant = &access.access.tenant;
    let t = &access.ticket;
    let paths = TicketPaths::new(&tenant.workspace.slug, &t.code);
    let loaded = load(&ctx, &access).await?;
    let versions: Vec<&artifact_versions::Model> = loaded
        .versions
        .iter()
        .filter(|v| v.artifact_id == artifact.id)
        .collect();
    let to = query
        .to
        .and_then(|n| versions.iter().find(|v| v.number == n).copied())
        .or(versions.first().copied())
        .ok_or(Error::NotFound)?;
    let from = query
        .from
        .and_then(|n| versions.iter().find(|v| v.number == n).copied())
        .or_else(|| versions.iter().find(|v| v.number < to.number).copied())
        .unwrap_or(to);
    let lines = service::diff(&from.body, &to.body);
    let base = format!("{}?a={}", paths.artifacts, artifact.id);
    let shell = app_shell(&ctx, &tenant.user, Some(tenant), Some(&access.access))
        .await?
        .at("project_ticket");
    views::tickets::render(
        &v,
        &shell,
        "tickets/artifact_compare.html",
        StatusCode::OK,
        data!({
            "project": summary(&access.access),
            "ticket": loaded.lookup.row(t),
            "paths": paths,
            "artifact": { "name": artifact.name, "id": artifact.id },
            "from": version_view(from, &loaded.lookup, &base, false),
            "to": version_view(to, &loaded.lookup, &base, true),
            "versions": versions.iter().map(|v| v.number).collect::<Vec<_>>(),
            "lines": lines,
            "added": lines.iter().filter(|l| l.kind == "add").count(),
            "removed": lines.iter().filter(|l| l.kind == "del").count(),
            "back": base,
            "action": format!("{}/{}/compare", paths.artifacts, artifact.id),
        }),
    )
}
