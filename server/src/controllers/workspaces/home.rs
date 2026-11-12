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
    models::{teams, workspace_members, workspaces::WorkspaceError},
    services::workspaces::create::{self, NewWorkspace},
    views::{
        self,
        time::wib_long_date,
        ui::Stat,
        workspaces::{HomePage, WorkspaceFormView},
    },
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
    let shell = app_shell(&ctx, &tenant.user, Some(&tenant), None)
        .await?
        .at("home");
    let create = CreateProjects::allows(&tenant.role)
        .then(|| crate::controllers::projects::new_project_path(&tenant.workspace.slug));
    let archived = shell.projects.iter().filter(|p| p.archived).count();
    let links = shell.settings.clone().unwrap_or_default();
    let members = workspace_members::Model::count_in(&ctx.db, tenant.workspace_id()).await?;
    let teams = teams::Model::count_in(&ctx.db, tenant.workspace_id()).await?;
    let link = |stat: Stat, href: Option<&String>| match href {
        Some(href) => stat.link(href.clone()),
        None => stat,
    };
    let stats = vec![
        Stat::new(
            "workspace-stat-projects",
            shell.projects.len() - archived,
            "folder",
        )
        .tone("accent"),
        Stat::new("workspace-stat-archived", archived, "archive"),
        link(
            Stat::new("workspace-stat-members", members, "users"),
            links.members.as_ref(),
        ),
        link(
            Stat::new("workspace-stat-teams", teams, "users-group"),
            links.teams.as_ref(),
        ),
    ];
    let first_name = tenant
        .user
        .name
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_string();
    views::workspaces::home(
        &v,
        &shell,
        &HomePage {
            workspace_name: &tenant.workspace.name,
            first_name,
            today: wib_long_date(&chrono::Utc::now()),
            create_href: create.as_deref(),
            stats,
            audit_href: links.audit.as_deref(),
        },
    )
}
