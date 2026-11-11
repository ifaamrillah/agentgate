//! `/` memilih workspace, `/w/new` membuatnya, `/w/{slug}` beranda workspace.
use axum::http::StatusCode;
use loco_rs::prelude::*;

use super::WorkspaceForm;
use crate::{
    controllers::{
        extractors::{CurrentUser, RequestId, Tenant, ValidForm},
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
    let shell = app_shell(&ctx, &user, None).await?;
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
    let shell = app_shell(&ctx, &user, None).await?;
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
            let shell = app_shell(&ctx, &user, None).await?;
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
    let shell = app_shell(&ctx, &tenant.user, Some(&tenant)).await?;
    views::workspaces::home(&v, &shell, &tenant.workspace.name)
}
