//! Pengaturan project → Repo (`ProjectAdmin`): daftar repo dengan form ubah per baris, form
//! tambah, dan hapus. Repo project lain atau id tidak valid → 404.
use axum::http::StatusCode;
use loco_rs::{prelude::*, validation::ModelValidationErrors};
use serde::Deserialize;
use uuid::Uuid;

use super::{acting, paths, summary};
use crate::{
    controllers::{
        extractors::{IdOr404, ProjectAccess, ProjectAdmin, RequestId, ValidForm},
        flash,
        shell::app_shell,
    },
    models::repos::{self, RepoInput},
    services::projects::{ProjectFailure, repos as service},
    views::{
        self,
        flash::Flash,
        projects::{RepoFormView, ReposPage, provider_options},
    },
};

/// Isian form repo. Field yang tidak dikirim dianggap kosong (→ 422).
#[derive(Debug, Deserialize)]
pub struct RepoForm {
    #[serde(default)]
    name: String,
    #[serde(default)]
    provider: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    default_branch: String,
}

impl RepoForm {
    fn input(&self) -> RepoInput {
        RepoInput {
            name: self.name.clone(),
            provider: self.provider.clone(),
            url: self.url.clone(),
            default_branch: self.default_branch.clone(),
        }
    }
}

/// Isian yang gagal: form tambah (`None`) atau baris repo `Some(id)`, dengan error-nya.
type Failed<'a> = (Option<Uuid>, &'a RepoInput, &'a ModelValidationErrors);

async fn render(
    ctx: &AppContext,
    v: &TeraView,
    access: &ProjectAccess,
    failed: Option<Failed<'_>>,
    flash: Option<Flash>,
    status: StatusCode,
) -> Result<Response> {
    let base = paths(access).repos;
    let rows = repos::Model::of_project(&ctx.db, access.tenant.workspace_id(), access.project.id)
        .await?
        .iter()
        .map(|repo| match failed {
            Some((Some(id), input, errors)) if id == repo.id => RepoFormView::from_input(
                &format!("repo-{id}"),
                input,
                &format!("{base}/{id}"),
                &format!("{base}/{id}/delete"),
                Some(errors),
            ),
            _ => RepoFormView::of(repo, &base),
        })
        .collect();
    let create = match failed {
        Some((None, input, errors)) => {
            RepoFormView::from_input("repo", input, &base, "", Some(errors))
        }
        _ => RepoFormView::empty(&base),
    };
    let page = ReposPage {
        repos: rows,
        create,
        providers: provider_options(),
        flash,
    };
    let shell = app_shell(ctx, &access.tenant.user, Some(&access.tenant), Some(access)).await?;
    views::projects::repos(v, &shell, &summary(access), &paths(access), &page, status)
}

async fn done(session: Session, access: &ProjectAccess, key: &str) -> Result<Response> {
    let session = flash::set(session, &Flash::success(key))?;
    Ok((session, format::redirect(&paths(access).repos)?).into_response())
}

#[debug_handler]
pub(super) async fn index(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    ProjectAdmin(access): ProjectAdmin,
    session: Session,
) -> Result<Response> {
    let (session, flash) = flash::take(session);
    let res = render(&ctx, &v, &access, None, flash, StatusCode::OK).await?;
    Ok((session, res).into_response())
}

/// `ProjectAdmin` sebelum `ValidForm`: tanpa izin → 404 sebelum body dibaca (rules §6).
#[debug_handler]
pub(super) async fn create(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    ProjectAdmin(access): ProjectAdmin,
    session: Session,
    RequestId(request_id): RequestId,
    ValidForm(form): ValidForm<RepoForm>,
) -> Result<Response> {
    let input = form.input();
    match service::add(
        &ctx.db,
        &acting(&access, request_id),
        access.project.id,
        &input,
    )
    .await
    {
        Ok(_) => done(session, &access, views::projects::FLASH_REPO_ADDED).await,
        Err(ProjectFailure::Invalid(errors)) => {
            render(
                &ctx,
                &v,
                &access,
                Some((None, &input, &errors)),
                None,
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => Err(e.into()),
    }
}

#[debug_handler]
pub(super) async fn update(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    ProjectAdmin(access): ProjectAdmin,
    path: IdOr404<repos::Entity>,
    session: Session,
    RequestId(request_id): RequestId,
    ValidForm(form): ValidForm<RepoForm>,
) -> Result<Response> {
    let input = form.input();
    match service::update(
        &ctx.db,
        &acting(&access, request_id),
        access.project.id,
        path.id,
        &input,
    )
    .await
    {
        Ok(_) => done(session, &access, views::projects::FLASH_REPO_UPDATED).await,
        Err(ProjectFailure::Invalid(errors)) => {
            render(
                &ctx,
                &v,
                &access,
                Some((Some(path.id), &input, &errors)),
                None,
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => Err(e.into()),
    }
}

#[debug_handler]
pub(super) async fn delete(
    State(ctx): State<AppContext>,
    ProjectAdmin(access): ProjectAdmin,
    path: IdOr404<repos::Entity>,
    session: Session,
    RequestId(request_id): RequestId,
) -> Result<Response> {
    service::remove(
        &ctx.db,
        &acting(&access, request_id),
        access.project.id,
        path.id,
    )
    .await?;
    done(session, &access, views::projects::FLASH_REPO_REMOVED).await
}
