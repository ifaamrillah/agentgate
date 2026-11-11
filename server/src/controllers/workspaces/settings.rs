//! `GET /w/{slug}/settings`: indeks Pengaturan, terbuka untuk role yang memegang salah satu izin
//! Pengaturan (`Require<AnySettings>`, sama dengan tautan di sidebar). Setiap bagian tampil
//! sesuai izinnya sendiri: nama/slug (`ManageWorkspace`), tautan Role (`ManageRoles`) dan
//! Jabatan (`ManageMembers`); anggota dan tim (Task 11–12) menambah bagiannya di sini, 2FA wajib
//! dan hapus workspace di Task 13.
//! `POST /w/{slug}/settings` mengubah nama/slug dan butuh `ManageWorkspace`.
use axum::http::StatusCode;
use loco_rs::prelude::*;

use super::{WorkspaceForm, job_titles, roles, settings_path};
use crate::{
    controllers::{
        extractors::{
            PermissionMarker, RequestId, Require, Tenant, ValidForm,
            require::{AnySettings, ManageWorkspace},
        },
        flash,
        shell::app_shell,
    },
    models::workspaces::WorkspaceError,
    services::workspaces::update::{self, WorkspaceDetails},
    views::{
        self,
        flash::Flash,
        workspaces::{SettingsLinks, WorkspaceFormView},
    },
};

#[debug_handler]
pub(super) async fn show(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<AnySettings>,
    session: Session,
) -> Result<Response> {
    let tenant = &access.tenant;
    let (session, flash) = flash::take(session);
    let shell = app_shell(&ctx, &tenant.user, Some(tenant)).await?;
    let form = general_form(tenant)
        .then(|| WorkspaceFormView::new(&tenant.workspace.name, &tenant.workspace.slug, None));
    let res = views::workspaces::settings(
        &v,
        &shell,
        &settings_path(&tenant.workspace.slug),
        form.as_ref(),
        &links(tenant),
        flash,
        StatusCode::OK,
    )?;
    Ok((session, res).into_response())
}

/// Tautan ke halaman Pengaturan lain, masing-masing menurut guard halaman itu sendiri.
fn links(tenant: &Tenant) -> SettingsLinks {
    SettingsLinks {
        roles: roles::settings_link(tenant),
        job_titles: job_titles::settings_link(tenant),
    }
}

/// Bagian Umum (nama, slug) tampil hanya untuk `ManageWorkspace`: aturan yang sama dengan guard
/// `POST`.
fn general_form(tenant: &Tenant) -> bool {
    ManageWorkspace::allows(&tenant.role)
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
                Some(&form),
                &links(tenant),
                None,
                StatusCode::UNPROCESSABLE_ENTITY,
            )
        }
        Err(e) => Err(e.into()),
    }
}
