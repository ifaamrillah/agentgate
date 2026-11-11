//! Pengaturan project → Akses (`ProjectAdmin`): daftar grant (tim atau orang + role), form beri
//! akses, ganti role, cabut. Penolakan aturan (`member-role-not-assignable`,
//! `project-grant-exists`) → 422 halaman akses dengan pesan; grant, tim, orang, atau role di
//! luar workspace/project ini dan id tidak valid → 404.
use axum::http::StatusCode;
use loco_rs::prelude::*;
use serde::Deserialize;

use super::{acting, paths, summary};
use crate::{
    controllers::{
        extractors::{IdOr404, ProjectAccess, ProjectAdmin, RequestId, ValidForm, form_id},
        flash,
        shell::app_shell,
    },
    models::{
        project_access,
        project_grants::{self, GrantSubject},
        roles, teams, workspace_members,
    },
    services::projects::{ProjectFailure, access as service},
    views::{
        self,
        flash::Flash,
        members::OptionView,
        projects::{AccessPage, GrantRow, parse_subject, subject_value},
    },
};

#[derive(Debug, Deserialize)]
pub struct GrantForm {
    #[serde(default)]
    subject: String,
    #[serde(default)]
    role_id: String,
}

#[derive(Debug, Deserialize)]
pub struct RoleForm {
    #[serde(default)]
    role_id: String,
}

async fn render(
    ctx: &AppContext,
    v: &TeraView,
    access: &ProjectAccess,
    flash: Option<Flash>,
    status: StatusCode,
) -> Result<Response> {
    let wid = access.tenant.workspace_id();
    let actor_role = &access.tenant.role;
    let base = paths(access).access;
    let grants = project_grants::Model::of_project(&ctx.db, wid, access.project.id).await?;
    let all_roles = roles::Model::in_workspace(&ctx.db, wid).await?;
    let all_teams = teams::Model::in_workspace(&ctx.db, wid).await?;
    let members = workspace_members::Model::list_in(&ctx.db, wid).await?;
    let granted = |subject: GrantSubject| grants.iter().any(|g| g.subject().ok() == Some(subject));
    let rows = grants
        .iter()
        .filter_map(|grant| {
            let role = all_roles.iter().find(|r| r.id == grant.role_id)?;
            let subject = match grant.subject().ok()? {
                GrantSubject::Team(id) => {
                    let team = all_teams.iter().find(|t| t.id == id)?;
                    (team.name.as_str(), "")
                }
                GrantSubject::User(id) => {
                    let (_, user, _) = members.iter().find(|(_, u, _)| u.id == id)?;
                    (user.name.as_str(), user.email.as_str())
                }
            };
            let manageable = project_access::can_grant(actor_role, &access.role, role);
            Some(GrantRow::new(grant, subject, role, manageable, &base))
        })
        .collect();
    let page = AccessPage {
        grants: rows,
        teams: all_teams
            .iter()
            .filter(|t| !granted(GrantSubject::Team(t.id)))
            .map(|t| OptionView::new(subject_value(GrantSubject::Team(t.id)), &t.name))
            .collect(),
        people: members
            .iter()
            .filter(|(_, u, _)| !granted(GrantSubject::User(u.id)))
            .map(|(_, u, _)| {
                OptionView::new(
                    subject_value(GrantSubject::User(u.id)),
                    &format!("{} ({})", u.name, u.email),
                )
            })
            .collect(),
        roles: all_roles
            .iter()
            .filter(|r| project_access::can_grant(actor_role, &access.role, r))
            .filter(|r| !project_access::project_permissions_of(r).is_empty())
            .map(OptionView::role)
            .collect(),
        action: base,
        flash,
    };
    let shell = app_shell(ctx, &access.tenant.user, Some(&access.tenant), Some(access)).await?;
    views::projects::access(v, &shell, &summary(access), &paths(access), &page, status)
}

async fn done(session: Session, access: &ProjectAccess, key: &str) -> Result<Response> {
    let session = flash::set(session, &Flash::success(key))?;
    Ok((session, format::redirect(&paths(access).access)?).into_response())
}

/// Penolakan aturan → 422 halaman akses dengan pesannya; lainnya error biasa (404, 500).
async fn rejected(
    ctx: &AppContext,
    v: &TeraView,
    access: &ProjectAccess,
    err: ProjectFailure,
) -> Result<Response> {
    let Some(code) = err.code() else {
        return Err(err.into());
    };
    render(
        ctx,
        v,
        access,
        Some(Flash::error(code)),
        StatusCode::UNPROCESSABLE_ENTITY,
    )
    .await
}

#[debug_handler]
pub(super) async fn index(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    ProjectAdmin(access): ProjectAdmin,
    session: Session,
) -> Result<Response> {
    let (session, flash) = flash::take(session);
    let res = render(&ctx, &v, &access, flash, StatusCode::OK).await?;
    Ok((session, res).into_response())
}

/// `ProjectAdmin` sebelum `ValidForm`: tanpa izin → 404 sebelum body dibaca (rules §6).
#[debug_handler]
pub(super) async fn grant(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    ProjectAdmin(access): ProjectAdmin,
    session: Session,
    RequestId(request_id): RequestId,
    ValidForm(form): ValidForm<GrantForm>,
) -> Result<Response> {
    let subject = parse_subject(&form.subject).ok_or(Error::NotFound)?;
    let role_id = form_id(&form.role_id)?;
    match service::grant(
        &ctx.db,
        &acting(&access, request_id),
        access.project.id,
        subject,
        role_id,
    )
    .await
    {
        Ok(_) => done(session, &access, views::projects::FLASH_ACCESS_GRANTED).await,
        Err(e) => rejected(&ctx, &v, &access, e).await,
    }
}

#[debug_handler]
pub(super) async fn change_role(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    ProjectAdmin(access): ProjectAdmin,
    path: IdOr404<project_grants::Entity>,
    session: Session,
    RequestId(request_id): RequestId,
    ValidForm(form): ValidForm<RoleForm>,
) -> Result<Response> {
    let role_id = form_id(&form.role_id)?;
    match service::change_role(
        &ctx.db,
        &acting(&access, request_id),
        access.project.id,
        path.id,
        role_id,
    )
    .await
    {
        Ok(_) => done(session, &access, views::projects::FLASH_ACCESS_CHANGED).await,
        Err(e) => rejected(&ctx, &v, &access, e).await,
    }
}

#[debug_handler]
pub(super) async fn revoke(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    ProjectAdmin(access): ProjectAdmin,
    path: IdOr404<project_grants::Entity>,
    session: Session,
    RequestId(request_id): RequestId,
) -> Result<Response> {
    match service::revoke(
        &ctx.db,
        &acting(&access, request_id),
        access.project.id,
        path.id,
    )
    .await
    {
        Ok(()) => done(session, &access, views::projects::FLASH_ACCESS_REVOKED).await,
        Err(e) => rejected(&ctx, &v, &access, e).await,
    }
}
