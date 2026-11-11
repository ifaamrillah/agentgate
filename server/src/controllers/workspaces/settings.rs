//! `GET|POST /w/{slug}/settings`: nama dan slug workspace (`ManageWorkspace`). Pengaturan lain
//! (2FA wajib, hapus workspace) datang di Task 13.
use axum::http::StatusCode;
use loco_rs::prelude::*;

use super::WorkspaceForm;
use crate::{
    controllers::{
        extractors::{RequestId, Require, ValidForm, require::ManageWorkspace},
        flash,
        shell::app_shell,
    },
    models::workspaces::WorkspaceError,
    services::workspaces::update::{self, WorkspaceDetails},
    views::{self, flash::Flash, workspaces::WorkspaceFormView},
};

fn settings_path(slug: &str) -> String {
    format!("/w/{slug}/settings")
}

#[debug_handler]
pub(super) async fn show(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageWorkspace>,
    session: Session,
) -> Result<Response> {
    let tenant = &access.tenant;
    let (session, flash) = flash::take(session);
    let shell = app_shell(&ctx, &tenant.user, Some(tenant)).await?;
    let form = WorkspaceFormView::new(&tenant.workspace.name, &tenant.workspace.slug, None);
    let res = views::workspaces::settings(
        &v,
        &shell,
        &settings_path(&tenant.workspace.slug),
        &form,
        flash,
        StatusCode::OK,
    )?;
    Ok((session, res).into_response())
}

/// `Require` sebelum `ValidForm`: tanpa izin → 404 sebelum body dibaca (rules §6).
#[debug_handler]
pub(super) async fn update(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageWorkspace>,
    session: Session,
    RequestId(request_id): RequestId,
    ValidForm(form): ValidForm<WorkspaceForm>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let details = WorkspaceDetails {
        name: form.name.clone(),
        slug: form.slug.clone(),
    };
    match update::run(
        &ctx,
        tenant.workspace_id(),
        tenant.user.id,
        details,
        request_id,
    )
    .await
    {
        Ok(workspace) => {
            let session = flash::set(
                session,
                &Flash::success(views::workspaces::FLASH_SETTINGS_SAVED),
            )?;
            Ok((session, format::redirect(&settings_path(&workspace.slug))?).into_response())
        }
        Err(WorkspaceError::Invalid(errors)) => {
            let shell = app_shell(&ctx, &tenant.user, Some(tenant)).await?;
            let form = WorkspaceFormView::new(
                form.name.trim(),
                &form.slug.trim().to_lowercase(),
                Some(&errors),
            );
            views::workspaces::settings(
                &v,
                &shell,
                &settings_path(&tenant.workspace.slug),
                &form,
                None,
                StatusCode::UNPROCESSABLE_ENTITY,
            )
        }
        Err(e) => Err(e.into()),
    }
}
