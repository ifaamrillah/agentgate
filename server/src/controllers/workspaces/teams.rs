//! Pengaturan → Tim (`Require<ManageTeams>`): daftar dengan form tambah, halaman satu tim
//! (ganti nama, tambah/keluarkan anggota, hapus). Anggota tim dipilih dari anggota workspace;
//! tim workspace lain, orang di luar workspace, atau id tidak valid → 404.
use axum::http::StatusCode;
use loco_rs::prelude::*;
use serde::Deserialize;
use uuid::Uuid;

use super::{members::form_id, teams_path};
use crate::{
    controllers::{
        extractors::{
            IdOr404, PermissionMarker, RequestId, Require, Tenant, ValidForm, require::ManageTeams,
        },
        flash,
        shell::app_shell,
    },
    models::{team_members, teams, workspace_members},
    services::workspaces::{
        Actor,
        teams::{self as service, TeamFailure},
    },
    views::{
        self,
        flash::Flash,
        ui::ComboOption,
        teams::{TeamFormView, TeamLink, TeamMemberRow, TeamPage},
    },
};

/// Isian form tambah dan ganti nama. Field yang tidak dikirim dianggap kosong (→ 422).
#[derive(Debug, Deserialize)]
pub struct TeamForm {
    #[serde(default)]
    name: String,
}

/// Isian tambah/keluarkan anggota.
#[derive(Debug, Deserialize)]
pub struct MemberForm {
    #[serde(default)]
    user_id: String,
}

fn actor(tenant: &Tenant, request_id: Option<String>) -> Actor {
    Actor {
        workspace_id: tenant.workspace_id(),
        user_id: tenant.user.id,
        request_id,
    }
}

/// Tim yang hilang di tengah jalan, orang di luar workspace, atau pelaku yang kehilangan
/// `ManageTeams` → 404; sisanya error biasa.
fn failure(err: TeamFailure) -> Error {
    err.into()
}

async fn find_team(ctx: &AppContext, tenant: &Tenant, id: Uuid) -> Result<teams::Model> {
    teams::Model::find_in(&ctx.db, tenant.workspace_id(), id)
        .await?
        .ok_or(Error::NotFound)
}

async fn render_index(
    ctx: &AppContext,
    v: &TeraView,
    tenant: &Tenant,
    create: &TeamFormView,
    flash: Option<Flash>,
    status: StatusCode,
) -> Result<Response> {
    let base = teams_path(&tenant.workspace.slug);
    let memberships = team_members::Model::all_in(&ctx.db, tenant.workspace_id()).await?;
    let teams: Vec<TeamLink> = teams::Model::in_workspace(&ctx.db, tenant.workspace_id())
        .await?
        .iter()
        .map(|team| {
            let count = memberships.iter().filter(|m| m.team_id == team.id).count();
            TeamLink::new(team, &base, u64::try_from(count).unwrap_or(u64::MAX))
        })
        .collect();
    let shell = app_shell(ctx, &tenant.user, Some(tenant), None)
        .await?
        .at("teams");
    views::teams::index(v, &shell, &teams, create, &base, flash, status)
}

/// Halaman satu tim dengan isian nama `form`.
async fn render_team(
    ctx: &AppContext,
    v: &TeraView,
    tenant: &Tenant,
    team: &teams::Model,
    form: TeamFormView,
    flash: Option<Flash>,
    status: StatusCode,
) -> Result<Response> {
    let wid = tenant.workspace_id();
    let members = team_members::Model::of_team(&ctx.db, wid, team.id).await?;
    let candidates = workspace_members::Model::list_in(&ctx.db, wid)
        .await?
        .iter()
        .filter(|(_, user, _)| !members.iter().any(|(m, _)| m.user_id == user.id))
        .map(|(_, user, _)| ComboOption::person(user.id, &user.name, &user.email))
        .collect();
    let people_src = format!("/w/{}/people?not_in_team={}", tenant.workspace.slug, team.id);
    let page = TeamPage::new(
        team,
        &teams_path(&tenant.workspace.slug),
        form,
        members
            .iter()
            .map(|(_, user)| TeamMemberRow::new(user))
            .collect(),
        candidates,
        people_src,
        flash,
    );
    let shell = app_shell(ctx, &tenant.user, Some(tenant), None)
        .await?
        .at("teams");
    views::teams::show(v, &shell, &page, status)
}

async fn redirect_with(session: Session, path: &str, key: &str) -> Result<Response> {
    let session = flash::set(session, &Flash::success(key))?;
    Ok((session, format::redirect(path)?).into_response())
}

#[debug_handler]
pub(super) async fn index(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageTeams>,
    session: Session,
) -> Result<Response> {
    let (session, flash) = flash::take(session);
    let create = TeamFormView::new("", None);
    let res = render_index(&ctx, &v, &access.tenant, &create, flash, StatusCode::OK).await?;
    Ok((session, res).into_response())
}

/// `Require` sebelum `ValidForm`: tanpa izin → 404 sebelum body dibaca (rules §6). Sukses →
/// halaman tim baru, untuk langsung menambah anggota.
#[debug_handler]
pub(super) async fn create(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageTeams>,
    session: Session,
    RequestId(request_id): RequestId,
    ValidForm(form): ValidForm<TeamForm>,
) -> Result<Response> {
    let tenant = &access.tenant;
    match service::create(&ctx.db, &actor(tenant, request_id), &form.name).await {
        Ok(team) => {
            let path = format!("{}/{}", teams_path(&tenant.workspace.slug), team.id);
            redirect_with(session, &path, views::teams::FLASH_CREATED).await
        }
        Err(TeamFailure::Invalid(errors)) => {
            let create = TeamFormView::new(form.name.trim(), Some(&errors));
            render_index(
                &ctx,
                &v,
                tenant,
                &create,
                None,
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => Err(failure(e)),
    }
}

#[debug_handler]
pub(super) async fn show(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageTeams>,
    path: IdOr404<teams::Entity>,
    session: Session,
) -> Result<Response> {
    let tenant = &access.tenant;
    let team = find_team(&ctx, tenant, path.id).await?;
    let (session, flash) = flash::take(session);
    let form = TeamFormView::new(&team.name, None);
    let res = render_team(&ctx, &v, tenant, &team, form, flash, StatusCode::OK).await?;
    Ok((session, res).into_response())
}

#[debug_handler]
pub(super) async fn rename(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageTeams>,
    path: IdOr404<teams::Entity>,
    session: Session,
    RequestId(request_id): RequestId,
    ValidForm(form): ValidForm<TeamForm>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let team = find_team(&ctx, tenant, path.id).await?;
    match service::rename(&ctx.db, &actor(tenant, request_id), team.id, &form.name).await {
        Ok(team) => {
            let path = format!("{}/{}", teams_path(&tenant.workspace.slug), team.id);
            redirect_with(session, &path, views::teams::FLASH_UPDATED).await
        }
        Err(TeamFailure::Invalid(errors)) => {
            let form = TeamFormView::new(form.name.trim(), Some(&errors));
            render_team(
                &ctx,
                &v,
                tenant,
                &team,
                form,
                None,
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => Err(failure(e)),
    }
}

#[debug_handler]
pub(super) async fn delete(
    State(ctx): State<AppContext>,
    access: Require<ManageTeams>,
    path: IdOr404<teams::Entity>,
    session: Session,
    RequestId(request_id): RequestId,
) -> Result<Response> {
    let tenant = &access.tenant;
    find_team(&ctx, tenant, path.id).await?;
    service::delete(&ctx.db, &actor(tenant, request_id), path.id)
        .await
        .map_err(failure)?;
    redirect_with(
        session,
        &teams_path(&tenant.workspace.slug),
        views::teams::FLASH_DELETED,
    )
    .await
}

/// Tim dengan grant di atas akses pelaku → 422 halaman tim dengan pesannya
/// (`team-member-not-assignable`).
#[debug_handler]
pub(super) async fn add_member(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageTeams>,
    path: IdOr404<teams::Entity>,
    session: Session,
    RequestId(request_id): RequestId,
    ValidForm(form): ValidForm<MemberForm>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let user_id = form_id(&form.user_id)?;
    match service::add_member(&ctx.db, &actor(tenant, request_id), path.id, user_id).await {
        Ok(()) => {
            let back = format!("{}/{}", teams_path(&tenant.workspace.slug), path.id);
            redirect_with(session, &back, views::teams::FLASH_MEMBER_ADDED).await
        }
        Err(e) => {
            let Some(code) = e.code() else {
                return Err(failure(e));
            };
            let team = find_team(&ctx, tenant, path.id).await?;
            let form = TeamFormView::new(&team.name, None);
            render_team(
                &ctx,
                &v,
                tenant,
                &team,
                form,
                Some(Flash::error(code)),
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
    }
}

#[debug_handler]
pub(super) async fn remove_member(
    State(ctx): State<AppContext>,
    access: Require<ManageTeams>,
    path: IdOr404<teams::Entity>,
    session: Session,
    RequestId(request_id): RequestId,
    ValidForm(form): ValidForm<MemberForm>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let user_id = form_id(&form.user_id)?;
    service::remove_member(&ctx.db, &actor(tenant, request_id), path.id, user_id)
        .await
        .map_err(failure)?;
    let back = format!("{}/{}", teams_path(&tenant.workspace.slug), path.id);
    redirect_with(session, &back, views::teams::FLASH_MEMBER_REMOVED).await
}

/// Tautan indeks Pengaturan ke halaman ini, bila role membukanya.
pub(super) fn settings_link(tenant: &Tenant) -> Option<String> {
    ManageTeams::allows(&tenant.role).then(|| teams_path(&tenant.workspace.slug))
}
