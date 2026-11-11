//! Buat project, beranda project, dan Pengaturan project → Umum (nama, deskripsi, arsip).
use axum::http::StatusCode;
use loco_rs::prelude::*;
use serde::Deserialize;

use super::{acting, new_project_path, paths, summary};
use crate::{
    controllers::{
        extractors::{
            ProjectAccess, ProjectAdmin, RequestId, Require, ValidForm, require::CreateProjects,
        },
        flash,
        shell::app_shell,
    },
    models::{permission::Permission, projects, repos},
    services::{
        projects::{self as service, ProjectFailure, ProjectInput},
        workspaces::Acting,
    },
    views::{
        self,
        flash::Flash,
        projects::{ProjectFormView, ProjectPaths, RepoView},
    },
};

/// Isian form project. Field yang tidak dikirim dianggap kosong (→ 422). `key` hanya dipakai
/// saat membuat.
#[derive(Debug, Deserialize)]
pub struct ProjectForm {
    #[serde(default)]
    key: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
}

#[debug_handler]
pub(super) async fn new(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<CreateProjects>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let shell = app_shell(&ctx, &tenant.user, Some(tenant), None).await?;
    views::projects::new(
        &v,
        &shell,
        &ProjectFormView::new("", "", "", None),
        &new_project_path(&tenant.workspace.slug),
        StatusCode::OK,
    )
}

/// `Require` sebelum `ValidForm`: tanpa izin → 404 sebelum body dibaca (rules §6).
#[debug_handler]
pub(super) async fn create(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<CreateProjects>,
    RequestId(request_id): RequestId,
    ValidForm(form): ValidForm<ProjectForm>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let actor = Acting {
        workspace: &tenant.workspace,
        user: &tenant.user,
        role: &tenant.role,
        request_id,
    };
    let input = ProjectInput {
        key: form.key.clone(),
        name: form.name.clone(),
        description: form.description.clone(),
    };
    match service::create(&ctx.db, &actor, input).await {
        Ok(project) => {
            format::redirect(&ProjectPaths::new(&tenant.workspace.slug, &project.key).home)
        }
        Err(ProjectFailure::Invalid(errors)) => {
            let shell = app_shell(&ctx, &tenant.user, Some(tenant), None).await?;
            let form = ProjectFormView::new(
                &projects::normalize_key(&form.key),
                form.name.trim(),
                form.description.trim(),
                Some(&errors),
            );
            views::projects::new(
                &v,
                &shell,
                &form,
                &new_project_path(&tenant.workspace.slug),
                StatusCode::UNPROCESSABLE_ENTITY,
            )
        }
        Err(e) => Err(e.into()),
    }
}

/// Beranda project: ringkasan, repo, dan board (M2). Semua yang punya akses project.
#[debug_handler]
pub(super) async fn show(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: ProjectAccess,
) -> Result<Response> {
    access.require(Permission::ViewProject)?;
    let repos: Vec<RepoView> =
        repos::Model::of_project(&ctx.db, access.tenant.workspace_id(), access.project.id)
            .await?
            .iter()
            .map(RepoView::new)
            .collect();
    let admin_paths = access
        .role
        .has(Permission::AdministerProject)
        .then(|| paths(&access));
    let shell = app_shell(
        &ctx,
        &access.tenant.user,
        Some(&access.tenant),
        Some(&access),
    )
    .await?;
    views::projects::show(&v, &shell, &summary(&access), &repos, admin_paths.as_ref())
}

async fn render_settings(
    ctx: &AppContext,
    v: &TeraView,
    access: &ProjectAccess,
    form: &ProjectFormView,
    flash: Option<Flash>,
    status: StatusCode,
) -> Result<Response> {
    let shell = app_shell(ctx, &access.tenant.user, Some(&access.tenant), Some(access)).await?;
    views::projects::settings(
        v,
        &shell,
        &summary(access),
        &paths(access),
        form,
        flash,
        status,
    )
}

#[debug_handler]
pub(super) async fn settings(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    ProjectAdmin(access): ProjectAdmin,
    session: Session,
) -> Result<Response> {
    let (session, flash) = flash::take(session);
    let form = ProjectFormView::of(&access.project);
    let res = render_settings(&ctx, &v, &access, &form, flash, StatusCode::OK).await?;
    Ok((session, res).into_response())
}

/// `ProjectAdmin` sebelum `ValidForm`: tanpa izin → 404 sebelum body dibaca (rules §6).
#[debug_handler]
pub(super) async fn update(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    ProjectAdmin(access): ProjectAdmin,
    session: Session,
    RequestId(request_id): RequestId,
    ValidForm(form): ValidForm<ProjectForm>,
) -> Result<Response> {
    match service::update(
        &ctx.db,
        &acting(&access, request_id),
        access.project.id,
        &form.name,
        &form.description,
    )
    .await
    {
        Ok(_) => {
            let session = flash::set(session, &Flash::success(views::projects::FLASH_SAVED))?;
            Ok((session, format::redirect(&paths(&access).settings)?).into_response())
        }
        Err(ProjectFailure::Invalid(errors)) => {
            let form = ProjectFormView::new(
                &access.project.key,
                form.name.trim(),
                form.description.trim(),
                Some(&errors),
            );
            render_settings(
                &ctx,
                &v,
                &access,
                &form,
                None,
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => Err(e.into()),
    }
}

#[debug_handler]
pub(super) async fn archive(
    State(ctx): State<AppContext>,
    ProjectAdmin(access): ProjectAdmin,
    session: Session,
    RequestId(request_id): RequestId,
) -> Result<Response> {
    service::archive(&ctx.db, &acting(&access, request_id), access.project.id).await?;
    let session = flash::set(session, &Flash::success(views::projects::FLASH_ARCHIVED))?;
    Ok((session, format::redirect(&paths(&access).settings)?).into_response())
}
