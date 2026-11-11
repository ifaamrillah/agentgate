//! Pengaturan → Jabatan (`Require<ManageJobTitles>`, aturan `ManageMembers`): daftar dengan form
//! tambah, ganti nama per baris, dan hapus. Jabatan workspace lain atau id tidak valid → 404.
//! Hapus membuat anggota yang memakainya tanpa jabatan (FK `ON DELETE SET NULL`).
use axum::http::StatusCode;
use loco_rs::prelude::*;
use sea_orm::DbErr;
use serde::Deserialize;

use super::job_titles_path;
use crate::{
    controllers::{
        extractors::{
            IdOr404, PermissionMarker, RequestId, Require, Tenant, ValidForm,
            require::ManageJobTitles,
        },
        flash,
        shell::app_shell,
    },
    models::job_titles::{self, JobTitleError},
    services::workspaces::{Actor, job_titles as service},
    views::{
        self,
        flash::Flash,
        job_titles::{JobTitleFormView, JobTitleRow},
    },
};

/// Isian form tambah dan ganti nama. Field yang tidak dikirim dianggap kosong (→ 422).
#[derive(Debug, Deserialize)]
pub struct JobTitleForm {
    #[serde(default)]
    name: String,
}

fn actor(tenant: &Tenant, request_id: Option<String>) -> Actor {
    Actor {
        workspace_id: tenant.workspace_id(),
        user_id: tenant.user.id,
        request_id,
    }
}

/// Halaman daftar. `create` = isian form tambah; `failed` = (id, isian, error) baris yang gagal
/// diganti namanya.
async fn render_index(
    ctx: &AppContext,
    v: &TeraView,
    tenant: &Tenant,
    create: &JobTitleFormView,
    failed: Option<(
        uuid::Uuid,
        &str,
        &loco_rs::validation::ModelValidationErrors,
    )>,
    flash: Option<Flash>,
    status: StatusCode,
) -> Result<Response> {
    let base = job_titles_path(&tenant.workspace.slug);
    let rows: Vec<JobTitleRow> = job_titles::Model::in_workspace(&ctx.db, tenant.workspace_id())
        .await?
        .iter()
        .map(|title| {
            let row = JobTitleRow::new(title, &base);
            match failed {
                Some((id, name, errors)) if id == title.id => row.with_error(name, errors),
                _ => row,
            }
        })
        .collect();
    let shell = app_shell(ctx, &tenant.user, Some(tenant)).await?;
    views::job_titles::index(v, &shell, &rows, create, &base, flash, status)
}

/// Jabatan yang hilang di tengah jalan → 404; sisanya error biasa.
fn failure(err: JobTitleError) -> Error {
    match err {
        JobTitleError::Db(DbErr::RecordNotFound(_)) => Error::NotFound,
        other => other.into(),
    }
}

async fn redirect_with(session: Session, tenant: &Tenant, key: &str) -> Result<Response> {
    let session = flash::set(session, &Flash::success(key))?;
    Ok((
        session,
        format::redirect(&job_titles_path(&tenant.workspace.slug))?,
    )
        .into_response())
}

/// Jabatan `id` di workspace ini, atau 404.
async fn find_title(ctx: &AppContext, tenant: &Tenant, id: uuid::Uuid) -> Result<()> {
    job_titles::Model::find_in(&ctx.db, tenant.workspace_id(), id)
        .await?
        .map(|_| ())
        .ok_or(Error::NotFound)
}

#[debug_handler]
pub(super) async fn index(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageJobTitles>,
    session: Session,
) -> Result<Response> {
    let (session, flash) = flash::take(session);
    let create = JobTitleFormView::new("", None);
    let res = render_index(
        &ctx,
        &v,
        &access.tenant,
        &create,
        None,
        flash,
        StatusCode::OK,
    )
    .await?;
    Ok((session, res).into_response())
}

/// `Require` sebelum `ValidForm`: tanpa izin → 404 sebelum body dibaca (rules §6).
#[debug_handler]
pub(super) async fn create(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageJobTitles>,
    session: Session,
    RequestId(request_id): RequestId,
    ValidForm(form): ValidForm<JobTitleForm>,
) -> Result<Response> {
    let tenant = &access.tenant;
    match service::create(&ctx.db, &actor(tenant, request_id), &form.name).await {
        Ok(_) => redirect_with(session, tenant, views::job_titles::FLASH_CREATED).await,
        Err(JobTitleError::Invalid(errors)) => {
            let create = JobTitleFormView::new(form.name.trim(), Some(&errors));
            render_index(
                &ctx,
                &v,
                tenant,
                &create,
                None,
                None,
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => Err(failure(e)),
    }
}

#[debug_handler]
pub(super) async fn rename(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageJobTitles>,
    path: IdOr404<job_titles::Entity>,
    session: Session,
    RequestId(request_id): RequestId,
    ValidForm(form): ValidForm<JobTitleForm>,
) -> Result<Response> {
    let tenant = &access.tenant;
    find_title(&ctx, tenant, path.id).await?;
    match service::rename(&ctx.db, &actor(tenant, request_id), path.id, &form.name).await {
        Ok(_) => redirect_with(session, tenant, views::job_titles::FLASH_UPDATED).await,
        Err(JobTitleError::Invalid(errors)) => {
            let create = JobTitleFormView::new("", None);
            render_index(
                &ctx,
                &v,
                tenant,
                &create,
                Some((path.id, form.name.trim(), &errors)),
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
    access: Require<ManageJobTitles>,
    path: IdOr404<job_titles::Entity>,
    session: Session,
    RequestId(request_id): RequestId,
) -> Result<Response> {
    let tenant = &access.tenant;
    find_title(&ctx, tenant, path.id).await?;
    service::delete(&ctx.db, &actor(tenant, request_id), path.id)
        .await
        .map_err(failure)?;
    redirect_with(session, tenant, views::job_titles::FLASH_DELETED).await
}

/// Tautan indeks Pengaturan ke halaman ini, bila role membukanya.
pub(super) fn settings_link(tenant: &Tenant) -> Option<String> {
    ManageJobTitles::allows(&tenant.role).then(|| job_titles_path(&tenant.workspace.slug))
}
