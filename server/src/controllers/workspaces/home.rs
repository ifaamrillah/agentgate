//! `/` memilih workspace, `/w/new` membuatnya, `/w/{slug}` beranda workspace (daftar project
//! yang bisa dibuka dan tombol buat project untuk pemegang `CreateProjects`).
use axum::http::StatusCode;
use loco_rs::prelude::*;

use super::WorkspaceForm;
use crate::{
    controllers::{
        extractors::{
            CurrentUser, PermissionMarker, RequestId, Tenant, ValidForm, require::CreateProjects,
        },
        shell::app_shell,
    },
    models::workspaces::WorkspaceError,
    services::workspaces::create::{self, NewWorkspace},
    views::{self, workspaces::WorkspaceFormView},
};

const NEW: &str = "/w/new";

/// Tanpa workspace → buat workspace; tepat satu → langsung ke sana; lebih → halaman pilih.
#[debug_handler]
pub(super) async fn choose(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    CurrentUser(user): CurrentUser,
) -> Result<Response> {
    let shell = app_shell(&ctx, &user, None, None).await?;
    match shell.workspaces.as_slice() {
        [] => format::redirect(NEW),
        [only] => format::redirect(&only.href),
        several => views::workspaces::choose(&v, &shell, several),
    }
}

#[debug_handler]
pub(super) async fn new(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    CurrentUser(user): CurrentUser,
) -> Result<Response> {
    let shell = app_shell(&ctx, &user, None, None).await?;
    views::workspaces::new(
        &v,
        &shell,
        &WorkspaceFormView::new("", "", None),
        StatusCode::OK,
    )
}

#[debug_handler]
pub(super) async fn create(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    CurrentUser(user): CurrentUser,
    RequestId(request_id): RequestId,
    ValidForm(form): ValidForm<WorkspaceForm>,
) -> Result<Response> {
    let new = NewWorkspace {
        name: form.name.clone(),
        slug: form.slug.clone(),
    };
    match create::run(&ctx, &user, new, request_id).await {
        Ok(workspace) => format::redirect(&format!("/w/{}", workspace.slug)),
        Err(WorkspaceError::Invalid(errors)) => {
            let shell = app_shell(&ctx, &user, None, None).await?;
            let form = WorkspaceFormView::new(
                form.name.trim(),
                &form.slug.trim().to_lowercase(),
                Some(&errors),
            );
            views::workspaces::new(&v, &shell, &form, StatusCode::UNPROCESSABLE_ENTITY)
        }
        Err(e) => Err(e.into()),
    }
}

#[debug_handler]
pub(super) async fn show(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    tenant: Tenant,
) -> Result<Response> {
    let shell = app_shell(&ctx, &tenant.user, Some(&tenant), None).await?;
    let create = CreateProjects::allows(&tenant.role)
        .then(|| crate::controllers::projects::new_project_path(&tenant.workspace.slug));
    views::workspaces::home(&v, &shell, &tenant.workspace.name, create.as_deref())
}
