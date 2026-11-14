//! Git & PR di web (M6):
//!
//! - Pengaturan → **Git provider** (`ManageIntegrations`): `GET /w/{slug}/settings/git[/{panel}]`
//!   (Cloud, Versi perusahaan, Repo terhubung), `POST …/settings/git/connect`,
//!   `POST …/settings/git/{id}/disconnect`, `POST …/settings/git/repos/{id}/relink`;
//! - Pengaturan project → **Konvensi git** (`ProjectAdmin`):
//!   `GET /w/{slug}/p/{key}/settings/git[/{panel}]`, `POST /w/{slug}/p/{key}/settings/git`;
//! - Project → **Pull request** (`ViewProject`): `GET /w/{slug}/p/{key}/prs`.
//!
//! Tanpa izin → 404. Aksi POST memakai PRG; isian Hubungkan yang ditolak dirender ulang (422)
//! dengan dialog terbuka.
use std::collections::HashMap;

use agentgate_core::git::{
    CiState, CodeOutput, CommitStyle, GitSettings, Language, MergeMethod, MergeMode, PrState,
};
use axum::http::StatusCode;
use loco_rs::prelude::*;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    common::settings::Settings,
    controllers::{
        extractors::{IdOr404, ProjectAccess, ProjectAdmin, RequestId, Tenant},
        flash,
        shell::app_shell,
    },
    models::{
        git_connections::{self, GitProviderKind},
        permission::Permission,
        projects, pull_requests, repos, tickets,
    },
    services::{
        git::{self as svc, connections::ConnectInput},
        tickets::TicketFailure,
        workspaces::Acting,
    },
    views::{
        self,
        datatable::{ColumnSpec, FilterOption, FilterSpec, TableSpec, TableView},
        flash::Flash,
        git::{ConnectionView, PrView, RepoLinkView, TicketRef},
        nav::{self, NavItem},
        projects::{ProjectPaths, ProjectSummary},
        ui::Stat,
    },
};

pub(crate) fn base(slug: &str) -> String {
    format!("/w/{slug}/settings/git")
}

fn acting(tenant: &Tenant, request_id: Option<String>) -> Acting<'_> {
    Acting {
        workspace: &tenant.workspace,
        user: &tenant.user,
        role: &tenant.role,
        request_id,
    }
}

fn require_integrations(tenant: &Tenant) -> Result<()> {
    if tenant.can(Permission::ManageIntegrations) {
        Ok(())
    } else {
        Err(Error::NotFound)
    }
}

/// Kode Fluent pertama dari error validasi.
fn first_code(errors: &loco_rs::validation::ModelValidationErrors) -> Option<(String, String)> {
    errors
        .errors
        .iter()
        .find_map(|(field, es)| es.first().map(|e| (field.to_string(), e.code.to_string())))
}

// ---------------------------------------------------------------------------------------------
// Pengaturan → Git provider
// ---------------------------------------------------------------------------------------------

/// Panel Git provider: (kunci, label, ikon, deskripsi).
const PANELS: &[(&str, &str, &str, &str)] = &[
    ("cloud", "git-panel-cloud", "world", "git-panel-cloud-desc"),
    (
        "enterprise",
        "git-panel-enterprise",
        "server",
        "git-panel-enterprise-desc",
    ),
    ("repos", "git-panel-repos", "folder", "git-panel-repos-desc"),
];

/// Tabel repo terhubung (filter/sort/halaman di memori).
pub static REPOS_TABLE: TableSpec = TableSpec {
    id: "git-repos",
    columns: &[
        ColumnSpec::new("git-col-repo").sort("repo"),
        ColumnSpec::new("git-col-project").sort("project"),
        ColumnSpec::new("git-col-provider").hide("sm"),
        ColumnSpec::new("git-col-webhook").sort("webhook"),
        ColumnSpec::new("git-col-last-event").hide("sm"),
        ColumnSpec::new("git-col-actions").right(),
    ],
    filters: &[
        FilterSpec {
            key: "project",
            label_key: "git-col-project",
            icon: "folder",
        },
        FilterSpec {
            key: "webhook",
            label_key: "git-col-webhook",
            icon: "plug-connected",
        },
    ],
    default_sort: "repo",
    default_desc: false,
    default_per: 25,
    search_key: Some("git-repos-search"),
};

/// Provider yang dikenal per panel: (kunci form, ikon, label, tersedia).
fn providers(panel: &str) -> Vec<(&'static str, &'static str, &'static str, bool)> {
    match panel {
        "cloud" => vec![
            ("github", "brand-github", "GitHub", true),
            ("gitlab", "brand-gitlab", "GitLab", false),
            ("bitbucket", "brand-bitbucket", "Bitbucket", false),
        ],
        _ => vec![
            (
                "github_enterprise",
                "brand-github",
                "GitHub Enterprise Server",
                true,
            ),
            ("gitlab_sm", "brand-gitlab", "GitLab Self-Managed", false),
            (
                "bitbucket_dc",
                "brand-bitbucket",
                "Bitbucket Data Center",
                false,
            ),
        ],
    }
}

/// Isian dialog Hubungkan yang ditolak.
#[derive(Debug, Default, serde::Serialize)]
struct ConnectError {
    kind: String,
    url: String,
    field: String,
    code: String,
}

async fn render_provider_page(
    ctx: &AppContext,
    v: &TeraView,
    tenant: &Tenant,
    panel: &str,
    query: &[(String, String)],
    flash: Option<Flash>,
    failed: Option<ConnectError>,
) -> Result<Response> {
    let db = &ctx.db;
    let wid = tenant.workspace_id();
    let settings = Settings::from_ctx(ctx)?;
    let base = base(&tenant.workspace.slug);
    let conns = git_connections::Model::of_workspace(db, wid).await?;
    let repo_rows = svc::connections::linked_repos(db, wid).await?;
    let project_rows = projects::Entity::find()
        .filter(projects::Column::WorkspaceId.eq(wid.get()))
        .all(db)
        .await?;
    let key = svc::key(&settings).ok();
    let conn_views: Vec<ConnectionView> = conns
        .iter()
        .map(|c| {
            let secret = key
                .as_ref()
                .and_then(|k| c.webhook_secret(k).ok())
                .unwrap_or_default();
            let n = repo_rows
                .iter()
                .filter(|r| r.connection_id == Some(c.id))
                .count();
            ConnectionView::of(
                c,
                svc::connections::webhook_url(&settings, c),
                secret,
                n,
                &base,
            )
        })
        .collect();
    let mut repo_views: Vec<RepoLinkView> = repo_rows
        .iter()
        .map(|r| {
            let p = project_rows.iter().find(|p| p.id == r.project_id);
            let color = p
                .and_then(|p| p.color.clone())
                .unwrap_or_else(|| views::palette::color_for(&r.name).to_string());
            RepoLinkView::of(
                r,
                (
                    p.map_or("", |p| p.key.as_str()),
                    p.map_or("", |p| p.name.as_str()),
                    &color,
                ),
                conns.iter().find(|c| Some(c.id) == r.connection_id),
                &base,
            )
        })
        .collect();
    let active = repo_views
        .iter()
        .filter(|r| matches!(r.webhook, "active" | "app"))
        .count();
    let broken = repo_views.iter().filter(|r| r.webhook == "error").count();
    let enterprise = conns
        .iter()
        .filter(|c| c.kind() == GitProviderKind::GithubEnterprise)
        .count();
    let subnav: Vec<NavItem> = PANELS
        .iter()
        .map(|(k, label, icon, desc)| {
            let mut item = NavItem::link(k, label, icon, &format!("{base}/{k}")).describe(desc);
            item.current = *k == panel;
            match *k {
                "cloud" => item.counted((conns.len() - enterprise) as u64, false),
                "enterprise" => item.counted(enterprise as u64, false),
                "repos" => item.counted(repo_views.len() as u64, broken > 0),
                _ => item,
            }
        })
        .collect();
    let stats = vec![
        Stat::new("git-stat-providers", conns.len(), "plug-connected").tone("success"),
        Stat::new("git-stat-repos", repo_views.len(), "folder"),
        Stat::new("git-stat-webhooks", active, "api").tone("accent"),
        Stat::new("git-stat-broken", broken, "alert-triangle").tone(if broken > 0 {
            "danger"
        } else {
            ""
        }),
    ];
    let mut data = serde_json::json!({
        "panel": panel,
        "subnav": subnav,
        "stats": stats,
        "flash": flash,
        "base": base,
        "connect_action": format!("{base}/connect"),
        "app_install_action": format!("{base}/app/install"),
        "app_enabled": settings.git.app.enabled() && svc::app::key_ok(&settings.git.app),
        "failed": failed,
    });
    if panel == "repos" {
        let state = REPOS_TABLE.parse(query);
        if let Some(q) = state.query() {
            let q = q.to_lowercase();
            repo_views.retain(|r| {
                r.full_name.to_lowercase().contains(&q)
                    || r.project_name.to_lowercase().contains(&q)
            });
        }
        let projects_filter = state.filter("project").to_vec();
        if !projects_filter.is_empty() {
            repo_views.retain(|r| projects_filter.contains(&r.project_key));
        }
        let webhook_filter = state.filter("webhook").to_vec();
        if !webhook_filter.is_empty() {
            repo_views.retain(|r| webhook_filter.iter().any(|w| w == r.webhook));
        }
        match state.sort {
            "project" => repo_views.sort_by(|a, b| a.project_name.cmp(&b.project_name)),
            "webhook" => repo_views.sort_by(|a, b| a.webhook.cmp(b.webhook)),
            _ => repo_views.sort_by(|a, b| a.full_name.cmp(&b.full_name)),
        }
        if state.desc {
            repo_views.reverse();
        }
        let total = repo_views.len() as u64;
        let page: Vec<RepoLinkView> = repo_views
            .into_iter()
            .skip(usize::try_from(state.offset()).unwrap_or(0))
            .take(usize::try_from(state.per).unwrap_or(25))
            .collect();
        let mut project_options: Vec<FilterOption> = project_rows
            .iter()
            .filter(|p| repo_rows.iter().any(|r| r.project_id == p.id))
            .map(|p| FilterOption::raw(&p.key, &p.name))
            .collect();
        project_options.sort_by(|a, b| a.label.cmp(&b.label));
        let table = TableView::new(&REPOS_TABLE, &state, &format!("{base}/repos"), total)
            .with_filter_options("project", project_options)
            .with_filter_options(
                "webhook",
                vec![
                    FilterOption::key("active", "git-webhook-active"),
                    FilterOption::key("error", "git-webhook-error"),
                    FilterOption::key("none", "git-webhook-none"),
                ],
            );
        data["table"] = serde_json::to_value(table)?;
        data["repos"] = serde_json::to_value(page)?;
    } else {
        let kinds: Vec<serde_json::Value> = providers(panel)
            .into_iter()
            .map(|(k, icon, label, available)| {
                let conn = conn_views.iter().find(|c| c.provider == k);
                serde_json::json!({
                    "key": k,
                    "icon": icon,
                    "label": label,
                    "available": available,
                    "connection": conn,
                })
            })
            .collect();
        data["providers"] = serde_json::Value::from(kinds);
    }
    let shell = app_shell(ctx, &tenant.user, Some(tenant), None)
        .await?
        .at("git");
    let status = if data["failed"].is_null() {
        StatusCode::OK
    } else {
        StatusCode::UNPROCESSABLE_ENTITY
    };
    views::tickets::render(v, &shell, "git/providers.html", status, data)
}

#[debug_handler]
async fn providers_index(State(_ctx): State<AppContext>, tenant: Tenant) -> Result<Response> {
    require_integrations(&tenant)?;
    format::redirect(&format!("{}/cloud", base(&tenant.workspace.slug)))
}

#[debug_handler]
async fn providers_page(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    tenant: Tenant,
    session: Session,
    Path((_, panel)): Path<(String, String)>,
    Query(query): Query<Vec<(String, String)>>,
) -> Result<Response> {
    require_integrations(&tenant)?;
    if !PANELS.iter().any(|(k, ..)| *k == panel) {
        return Err(Error::NotFound);
    }
    let (session, flash) = flash::take(session);
    let res = render_provider_page(&ctx, &v, &tenant, &panel, &query, flash, None).await?;
    Ok((session, res).into_response())
}

#[derive(Deserialize)]
struct ConnectForm {
    #[serde(default)]
    kind: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    token: String,
}

impl std::fmt::Debug for ConnectForm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectForm")
            .field("kind", &self.kind)
            .field("url", &self.url)
            .field("token", &"[redacted]")
            .finish()
    }
}

/// `ManageIntegrations` sebelum body dibaca.
#[debug_handler]
async fn connect(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    tenant: Tenant,
    session: Session,
    RequestId(request_id): RequestId,
    Form(form): Form<ConnectForm>,
) -> Result<Response> {
    require_integrations(&tenant)?;
    let kind = match form.kind.as_str() {
        "github" => GitProviderKind::Github,
        "github_enterprise" => GitProviderKind::GithubEnterprise,
        _ => return Err(Error::NotFound),
    };
    let panel = if kind == GitProviderKind::Github {
        "cloud"
    } else {
        "enterprise"
    };
    let settings = Settings::from_ctx(&ctx)?;
    let input = ConnectInput {
        kind,
        url: form.url.clone(),
        token: form.token.clone(),
    };
    match svc::connections::connect(&ctx.db, &acting(&tenant, request_id), &settings, &input).await
    {
        Ok(_) => {
            let session = flash::set(session, &Flash::success(views::git::FLASH_CONNECTED))?;
            let to = format!("{}/{panel}", base(&tenant.workspace.slug));
            Ok((session, format::redirect(&to)?).into_response())
        }
        Err(TicketFailure::Invalid(errors)) => {
            let (field, code) =
                first_code(&errors).unwrap_or_else(|| ("token".into(), svc::TOKEN_INVALID.into()));
            let failed = ConnectError {
                kind: form.kind.clone(),
                url: form.url.trim().to_string(),
                field,
                code,
            };
            render_provider_page(&ctx, &v, &tenant, panel, &[], None, Some(failed)).await
        }
        Err(TicketFailure::NotFound) => Err(Error::NotFound),
        Err(TicketFailure::Db(e)) => Err(e.into()),
    }
}

#[debug_handler]
async fn disconnect(
    State(ctx): State<AppContext>,
    tenant: Tenant,
    session: Session,
    RequestId(request_id): RequestId,
    id: IdOr404<git_connections::Entity>,
) -> Result<Response> {
    require_integrations(&tenant)?;
    let kind = git_connections::Model::find_in(&ctx.db, tenant.workspace_id(), id.id)
        .await?
        .ok_or(Error::NotFound)?
        .kind();
    match svc::connections::disconnect(&ctx.db, &acting(&tenant, request_id), id.id).await {
        Ok(()) => {
            let session = flash::set(session, &Flash::success(views::git::FLASH_DISCONNECTED))?;
            let panel = if kind == GitProviderKind::Github {
                "cloud"
            } else {
                "enterprise"
            };
            let to = format!("{}/{panel}", base(&tenant.workspace.slug));
            Ok((session, format::redirect(&to)?).into_response())
        }
        Err(TicketFailure::Db(e)) => Err(e.into()),
        Err(_) => Err(Error::NotFound),
    }
}

#[debug_handler]
async fn relink(
    State(ctx): State<AppContext>,
    tenant: Tenant,
    session: Session,
    RequestId(request_id): RequestId,
    id: IdOr404<repos::Entity>,
) -> Result<Response> {
    require_integrations(&tenant)?;
    let settings = Settings::from_ctx(&ctx)?;
    let to = format!("{}/repos", base(&tenant.workspace.slug));
    match svc::connections::relink(&ctx.db, &acting(&tenant, request_id), &settings, id.id).await {
        Ok(repo) => {
            let msg = if repo.webhook_id.is_some() {
                Flash::success(views::git::FLASH_RELINKED)
            } else if repo.connection_id.is_none() {
                Flash::error(svc::NOT_CONNECTED)
            } else {
                Flash::error(if repo.webhook_error.is_empty() {
                    svc::PROVIDER_REJECTED
                } else {
                    &repo.webhook_error
                })
            };
            let session = flash::set(session, &msg)?;
            Ok((session, format::redirect(&to)?).into_response())
        }
        Err(TicketFailure::Db(e)) => Err(e.into()),
        Err(_) => Err(Error::NotFound),
    }
}

// ---------------------------------------------------------------------------------------------
// GitHub App (R2)
// ---------------------------------------------------------------------------------------------

/// Mulai instalasi App: redirect ke GitHub dengan state bertanda tangan (workspace + user).
#[debug_handler]
async fn app_install(
    State(ctx): State<AppContext>,
    tenant: Tenant,
    session: Session,
) -> Result<Response> {
    require_integrations(&tenant)?;
    let settings = Settings::from_ctx(&ctx)?;
    if !settings.git.app.enabled() || !svc::app::key_ok(&settings.git.app) {
        let session = flash::set(session, &Flash::error(svc::APP_NOT_CONFIGURED))?;
        let to = format!("{}/cloud", base(&tenant.workspace.slug));
        return Ok((session, format::redirect(&to)?).into_response());
    }
    let key = svc::key(&settings).map_err(|_| Error::InternalServerError)?;
    let state = svc::app::sign_state(
        &key,
        tenant.workspace.id,
        tenant.user.id,
        chrono::Utc::now().timestamp(),
    );
    format::redirect(&svc::app::install_url(
        &settings.git.github_web_base,
        &settings.git.app,
        &state,
    ))
}

#[derive(Debug, Deserialize)]
struct AppCallbackQuery {
    #[serde(default)]
    installation_id: String,
    #[serde(default)]
    setup_action: String,
    #[serde(default)]
    code: String,
    #[serde(default)]
    state: String,
}

/// Setup URL App (global, satu per App): state menunjuk workspace tujuan, lalu diteruskan ke
/// callback di bawah workspace itu (keanggotaan dan izin diperiksa extractor `Tenant`).
#[debug_handler]
async fn app_callback_global(
    State(ctx): State<AppContext>,
    Query(q): Query<AppCallbackQuery>,
) -> Result<Response> {
    let settings = Settings::from_ctx(&ctx)?;
    let key = svc::key(&settings).map_err(|_| Error::InternalServerError)?;
    let Some(state) = svc::app::verify_state(&key, &q.state, chrono::Utc::now().timestamp()) else {
        return Err(Error::NotFound);
    };
    let Some(ws) = crate::models::workspaces::Model::find_by_id(&ctx.db, state.w).await? else {
        return Err(Error::NotFound);
    };
    let query = serde_urlencoded::to_string([
        ("installation_id", q.installation_id.as_str()),
        ("setup_action", q.setup_action.as_str()),
        ("code", q.code.as_str()),
        ("state", q.state.as_str()),
    ])
    .map_err(|_| Error::NotFound)?;
    format::redirect(&format!("{}/app/callback?{query}", base(&ws.slug)))
}

/// Callback instalasi App di workspace: state harus milik workspace + user ini.
#[debug_handler]
async fn app_callback(
    State(ctx): State<AppContext>,
    tenant: Tenant,
    session: Session,
    RequestId(request_id): RequestId,
    Query(q): Query<AppCallbackQuery>,
) -> Result<Response> {
    require_integrations(&tenant)?;
    let settings = Settings::from_ctx(&ctx)?;
    let key = svc::key(&settings).map_err(|_| Error::InternalServerError)?;
    let to = format!("{}/cloud", base(&tenant.workspace.slug));
    let state = svc::app::verify_state(&key, &q.state, chrono::Utc::now().timestamp())
        .filter(|s| s.w == tenant.workspace.id && s.u == tenant.user.id);
    let installation = q.installation_id.trim().parse::<i64>().ok();
    let (Some(_), Some(installation)) = (state, installation) else {
        let session = flash::set(session, &Flash::error(svc::APP_INSTALL_INVALID))?;
        return Ok((session, format::redirect(&to)?).into_response());
    };
    let result = svc::connections::connect_app(
        &ctx.db,
        &acting(&tenant, request_id),
        &settings,
        installation,
        &q.code,
    )
    .await;
    let msg = match result {
        Ok(_) => Flash::success(views::git::FLASH_APP_CONNECTED),
        Err(TicketFailure::Invalid(errors)) => {
            let code = first_code(&errors).map_or_else(|| svc::APP_INSTALL_INVALID.into(), |c| c.1);
            Flash::error(&code)
        }
        Err(TicketFailure::NotFound) => return Err(Error::NotFound),
        Err(TicketFailure::Db(e)) => return Err(e.into()),
    };
    let session = flash::set(session, &msg)?;
    Ok((session, format::redirect(&to)?).into_response())
}

/// Segarkan izin dan daftar repo instalasi App.
#[debug_handler]
async fn app_refresh(
    State(ctx): State<AppContext>,
    tenant: Tenant,
    session: Session,
    RequestId(request_id): RequestId,
    id: IdOr404<git_connections::Entity>,
) -> Result<Response> {
    require_integrations(&tenant)?;
    let settings = Settings::from_ctx(&ctx)?;
    let to = format!("{}/cloud", base(&tenant.workspace.slug));
    let msg =
        match svc::connections::refresh(&ctx.db, &acting(&tenant, request_id), &settings, id.id)
            .await
        {
            Ok(_) => Flash::success(views::git::FLASH_APP_REFRESHED),
            Err(TicketFailure::Invalid(errors)) => {
                let code = first_code(&errors).map_or_else(|| svc::UNREACHABLE.into(), |c| c.1);
                Flash::error(&code)
            }
            Err(TicketFailure::NotFound) => return Err(Error::NotFound),
            Err(TicketFailure::Db(e)) => return Err(e.into()),
        };
    let session = flash::set(session, &msg)?;
    Ok((session, format::redirect(&to)?).into_response())
}

// ---------------------------------------------------------------------------------------------
// Pengaturan project → Konvensi git
// ---------------------------------------------------------------------------------------------

/// Panel Konvensi git: (kunci, label, ikon, deskripsi, tersedia).
const PROJECT_PANELS: &[(&str, &str, &str, &str, bool)] = &[
    (
        "git",
        "project-git-panel-git",
        "git-branch",
        "project-git-panel-git-desc",
        true,
    ),
    (
        "out",
        "project-git-panel-out",
        "git-pull-request",
        "project-git-panel-out-desc",
        true,
    ),
    (
        "ci",
        "project-git-panel-ci",
        "circle-check",
        "project-git-panel-ci-desc",
        true,
    ),
    (
        "env",
        "project-git-panel-env",
        "test-pipe",
        "project-git-panel-env-desc",
        false,
    ),
    (
        "deploy",
        "project-git-panel-deploy",
        "rocket",
        "project-git-panel-deploy-desc",
        false,
    ),
    (
        "path",
        "project-git-panel-path",
        "cloud-upload",
        "project-git-panel-path-desc",
        false,
    ),
];

fn project_paths(access: &ProjectAccess) -> ProjectPaths {
    ProjectPaths::new(&access.tenant.workspace.slug, &access.project.key)
}

fn project_summary(access: &ProjectAccess) -> ProjectSummary {
    ProjectSummary::new(&access.project, access.role.label(), &project_paths(access))
}

/// Opsi pilihan dengan kunci Fluent.
fn options<T: Copy>(
    all: &[T],
    value: impl Fn(T) -> &'static str,
    prefix: &str,
) -> serde_json::Value {
    serde_json::Value::from(
        all.iter()
            .map(|x| {
                let v = value(*x);
                serde_json::json!({ "value": v, "label_key": format!("{prefix}-{}", v.replace('_', "-")) })
            })
            .collect::<Vec<_>>(),
    )
}

async fn render_project_git(
    ctx: &AppContext,
    v: &TeraView,
    access: &ProjectAccess,
    panel: &str,
    form: &GitSettings,
    error: Option<(String, String)>,
    flash: Option<Flash>,
) -> Result<Response> {
    let paths = project_paths(access);
    let subnav: Vec<NavItem> = PROJECT_PANELS
        .iter()
        .map(|(k, label, icon, desc, available)| {
            let mut item = if *available {
                NavItem::link(k, label, icon, &format!("{}/{k}", paths.git))
            } else {
                NavItem::soon(k, label, icon)
            }
            .describe(desc);
            item.current = *k == panel;
            item
        })
        .collect();
    let repo_count =
        repos::Model::of_project(&ctx.db, access.tenant.workspace_id(), access.project.id)
            .await?
            .len();
    let example = agentgate_core::git::render_branch(
        &form.branch_pattern,
        &format!("{}-123", access.project.key),
        "Varian produk",
        "feature",
    );
    let shell = app_shell(ctx, &access.tenant.user, Some(&access.tenant), Some(access))
        .await?
        .at("project_git");
    let status = if error.is_some() {
        StatusCode::UNPROCESSABLE_ENTITY
    } else {
        StatusCode::OK
    };
    views::tickets::render(
        v,
        &shell,
        "git/project.html",
        status,
        serde_json::json!({
            "project": project_summary(access),
            "paths": paths,
            "panel": panel,
            "subnav": subnav,
            "settings_nav": nav::project_settings_nav(&paths, "git"),
            "form": form,
            "required_checks": form.required_checks.join(", "),
            "example": example,
            "error": error.map(|(field, code)| serde_json::json!({ "field": field, "code": code })),
            "flash": flash,
            "action": paths.git,
            "repo_count": repo_count,
            "styles": options(CommitStyle::ALL, CommitStyle::as_str, "git-commit-style"),
            "languages": options(Language::ALL, Language::as_str, "git-language"),
            "outputs": options(CodeOutput::ALL, CodeOutput::as_str, "git-code-output"),
            "merges": options(MergeMode::ALL, MergeMode::as_str, "git-merge-mode"),
            "merge_methods": options(MergeMethod::ALL, MergeMethod::as_str, "git-merge-method"),
        }),
    )
}

#[debug_handler]
async fn project_index(
    State(_ctx): State<AppContext>,
    ProjectAdmin(access): ProjectAdmin,
) -> Result<Response> {
    format::redirect(&format!("{}/git", project_paths(&access).git))
}

#[debug_handler]
async fn project_page(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    ProjectAdmin(access): ProjectAdmin,
    session: Session,
    Path((_, _, panel)): Path<(String, String, String)>,
) -> Result<Response> {
    if !PROJECT_PANELS
        .iter()
        .any(|(k, .., available)| *k == panel && *available)
    {
        return Err(Error::NotFound);
    }
    let (session, flash) = flash::take(session);
    let form = access.project.git_settings();
    let res = render_project_git(&ctx, &v, &access, &panel, &form, None, flash).await?;
    Ok((session, res).into_response())
}

fn pick(pairs: &[(String, String)], key: &str) -> Option<String> {
    pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
}

/// Isian satu panel di atas konvensi tersimpan (field panel lain tidak berubah).
fn merge_form(current: &GitSettings, panel: &str, pairs: &[(String, String)]) -> GitSettings {
    let mut s = current.clone();
    match panel {
        "git" => {
            if let Some(v) = pick(pairs, "branch_pattern") {
                s.branch_pattern = v;
            }
            if let Some(v) = pick(pairs, "commit_style").and_then(|v| CommitStyle::parse(&v)) {
                s.commit_style = v;
            }
            for (key, slot) in [
                ("commit_language", &mut s.commit_language),
                ("pr_language", &mut s.pr_language),
                ("code_language", &mut s.code_language),
            ] {
                if let Some(v) = pick(pairs, key).and_then(|v| Language::parse(&v)) {
                    *slot = v;
                }
            }
        }
        "out" => {
            if let Some(v) = pick(pairs, "code_output").and_then(|v| CodeOutput::parse(&v)) {
                s.code_output = v;
            }
            if let Some(v) = pick(pairs, "merge_mode").and_then(|v| MergeMode::parse(&v)) {
                s.merge_mode = v;
            }
            if let Some(v) = pick(pairs, "merge_method").and_then(|v| MergeMethod::parse(&v)) {
                s.merge_method = v;
            }
        }
        "ci" => {
            s.require_ci = pick(pairs, "require_ci").is_some();
            s.required_checks = pick(pairs, "required_checks").into_iter().collect();
        }
        _ => {}
    }
    s
}

/// `ProjectAdmin` sebelum body dibaca.
#[debug_handler]
async fn project_save(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    ProjectAdmin(access): ProjectAdmin,
    session: Session,
    RequestId(request_id): RequestId,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Result<Response> {
    let panel = pick(&pairs, "panel").unwrap_or_default();
    if !PROJECT_PANELS
        .iter()
        .any(|(k, .., available)| *k == panel && *available)
    {
        return Err(Error::NotFound);
    }
    let form = merge_form(&access.project.git_settings(), &panel, &pairs);
    let act = Acting {
        workspace: &access.tenant.workspace,
        user: &access.tenant.user,
        role: &access.tenant.role,
        request_id,
    };
    match svc::settings::update(&ctx.db, &act, access.project.id, form.clone()).await {
        Ok(_) => {
            let session = flash::set(session, &Flash::success(views::git::FLASH_SAVED))?;
            let to = format!("{}/{panel}", project_paths(&access).git);
            Ok((session, format::redirect(&to)?).into_response())
        }
        Err(TicketFailure::Invalid(errors)) => {
            render_project_git(&ctx, &v, &access, &panel, &form, first_code(&errors), None).await
        }
        Err(TicketFailure::NotFound) => Err(Error::NotFound),
        Err(TicketFailure::Db(e)) => Err(e.into()),
    }
}

// ---------------------------------------------------------------------------------------------
// Project → Pull request
// ---------------------------------------------------------------------------------------------

/// Tabel PR project (filter/sort/halaman di memori).
pub static PRS_TABLE: TableSpec = TableSpec {
    id: "project-prs",
    columns: &[
        ColumnSpec::new("pr-col-pr").sort("number"),
        ColumnSpec::new("pr-col-ticket"),
        ColumnSpec::new("pr-col-branch").hide("md"),
        ColumnSpec::new("pr-col-ci").sort("ci"),
        ColumnSpec::new("pr-col-state").sort("state"),
        ColumnSpec::new("pr-col-updated").sort("updated").hide("sm"),
    ],
    filters: &[
        FilterSpec {
            key: "repo",
            label_key: "pr-filter-repo",
            icon: "folder",
        },
        FilterSpec {
            key: "ci",
            label_key: "pr-col-ci",
            icon: "circle-check",
        },
    ],
    default_sort: "updated",
    default_desc: true,
    default_per: 25,
    search_key: Some("pr-search"),
};

#[derive(Debug, Deserialize)]
struct PrsQuery {
    #[serde(default)]
    tab: String,
}

#[debug_handler]
async fn project_prs(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: ProjectAccess,
    Query(tab): Query<PrsQuery>,
    Query(query): Query<Vec<(String, String)>>,
) -> Result<Response> {
    access.require(Permission::ViewProject)?;
    let db = &ctx.db;
    let wid = access.tenant.workspace_id();
    let slug = access.tenant.workspace.slug.clone();
    let paths = project_paths(&access);
    let prs = pull_requests::Model::of_project(db, wid, access.project.id).await?;
    let repo_rows = repos::Model::of_project(db, wid, access.project.id).await?;
    let ticket_ids: Vec<Uuid> = prs.iter().filter_map(|p| p.ticket_id).collect();
    let ticket_rows: HashMap<Uuid, tickets::Model> = if ticket_ids.is_empty() {
        HashMap::new()
    } else {
        tickets::Entity::find()
            .filter(tickets::Column::WorkspaceId.eq(wid.get()))
            .filter(tickets::Column::Id.is_in(ticket_ids))
            .all(db)
            .await?
            .into_iter()
            .map(|t| (t.id, t))
            .collect()
    };
    let open_n = prs
        .iter()
        .filter(|p| matches!(p.state(), PrState::Open | PrState::Draft))
        .count();
    let merged_n = prs.iter().filter(|p| p.state() == PrState::Merged).count();
    let failing = prs
        .iter()
        .filter(|p| {
            matches!(p.state(), PrState::Open | PrState::Draft) && p.ci() == CiState::Failure
        })
        .count();
    let tab = match tab.tab.as_str() {
        "open" | "merged" => tab.tab.clone(),
        _ => "all".to_string(),
    };
    let mut views_list: Vec<PrView> = prs
        .iter()
        .filter(|p| match tab.as_str() {
            "open" => matches!(p.state(), PrState::Open | PrState::Draft),
            "merged" => p.state() == PrState::Merged,
            _ => true,
        })
        .map(|p| {
            let ticket = p
                .ticket_id
                .and_then(|id| ticket_rows.get(&id))
                .map(|t| TicketRef {
                    code: t.code.clone(),
                    title: t.title.clone(),
                    href: views::tickets::ticket_href(&slug, &t.code),
                });
            PrView::of(
                &slug,
                p,
                repo_rows.iter().find(|r| r.id == p.repo_id),
                ticket,
            )
        })
        .collect();
    let state = PRS_TABLE.parse(&query);
    if let Some(q) = state.query() {
        let q = q.to_lowercase();
        views_list.retain(|p| {
            p.title.to_lowercase().contains(&q)
                || p.head_branch.to_lowercase().contains(&q)
                || p.repo.to_lowercase().contains(&q)
                || format!("#{}", p.number).contains(&q)
                || p.ticket
                    .as_ref()
                    .is_some_and(|t| t.code.to_lowercase().contains(&q))
        });
    }
    let repo_filter = state.filter("repo").to_vec();
    if !repo_filter.is_empty() {
        views_list.retain(|p| repo_filter.contains(&p.repo));
    }
    let ci_filter = state.filter("ci").to_vec();
    if !ci_filter.is_empty() {
        views_list.retain(|p| ci_filter.iter().any(|c| c == p.ci.value));
    }
    match state.sort {
        "number" => views_list.sort_by_key(|p| p.number),
        "ci" => views_list.sort_by(|a, b| a.ci.value.cmp(b.ci.value)),
        "state" => views_list.sort_by(|a, b| a.state.value.cmp(b.state.value)),
        _ => {}
    }
    if state.desc && state.sort != "updated" || !state.desc && state.sort == "updated" {
        views_list.reverse();
    }
    let total = views_list.len() as u64;
    let page: Vec<PrView> = views_list
        .into_iter()
        .skip(usize::try_from(state.offset()).unwrap_or(0))
        .take(usize::try_from(state.per).unwrap_or(25))
        .collect();
    let mut table_base = paths.prs.clone();
    if tab != "all" {
        table_base = format!("{table_base}?tab={tab}");
    }
    let mut repo_names: Vec<String> = repo_rows
        .iter()
        .map(|r| {
            if r.full_name.is_empty() {
                r.name.clone()
            } else {
                r.full_name.clone()
            }
        })
        .collect();
    repo_names.sort();
    repo_names.dedup();
    let table = TableView::new(&PRS_TABLE, &state, &table_base, total)
        .with_filter_options(
            "repo",
            repo_names.iter().map(|r| FilterOption::raw(r, r)).collect(),
        )
        .with_filter_options(
            "ci",
            CiState::ALL
                .iter()
                .map(|c| FilterOption::key(c.as_str(), views::git::ci_badge(*c).0))
                .collect(),
        );
    let tabs = vec![
        tab_item("all", "pr-tab-all", &paths.prs, prs.len(), &tab),
        tab_item(
            "open",
            "pr-tab-open",
            &format!("{}?tab=open", paths.prs),
            open_n,
            &tab,
        ),
        tab_item(
            "merged",
            "pr-tab-merged",
            &format!("{}?tab=merged", paths.prs),
            merged_n,
            &tab,
        ),
    ];
    let stats = vec![
        Stat::new("pr-stat-open", open_n, "git-pull-request").tone("accent"),
        Stat::new("pr-stat-failing", failing, "circle-x").tone(if failing > 0 {
            "danger"
        } else {
            ""
        }),
        Stat::new("pr-stat-merged", merged_n, "git-merge").tone("success"),
        Stat::new("pr-stat-repos", repo_rows.len(), "folder"),
    ];
    let home = paths.home.clone();
    let shell = app_shell(
        &ctx,
        &access.tenant.user,
        Some(&access.tenant),
        Some(&access),
    )
    .await?;
    views::tickets::render(
        &v,
        &shell,
        "git/prs.html",
        StatusCode::OK,
        serde_json::json!({
            "project": project_summary(&access),
            "views": nav::project_views(&home, &access.tenant.user.project_views, "prs"),
            "views_action": views::projects::PROJECT_VIEWS_ACTION,
            "return_to": paths.prs,
            "paths": paths,
            "tabs": tabs,
            "stats": stats,
            "table": table,
            "pulls": page,
            "can_admin": access.role.has(Permission::AdministerProject),
            "has_repos": !repo_rows.is_empty(),
        }),
    )
}

fn tab_item(
    key: &'static str,
    label: &'static str,
    href: &str,
    n: usize,
    current: &str,
) -> NavItem {
    let mut item = NavItem::link(key, label, "git-pull-request", href).counted(n as u64, false);
    item.current = key == current;
    item
}

pub fn routes() -> Routes {
    Routes::new()
        .add("/w/{slug}/settings/git", get(providers_index))
        .add("/w/{slug}/settings/git/connect", post(connect))
        .add("/w/{slug}/settings/git/app/install", post(app_install))
        .add("/w/{slug}/settings/git/app/callback", get(app_callback))
        .add("/w/{slug}/settings/git/{id}/refresh", post(app_refresh))
        .add("/git/github-app/callback", get(app_callback_global))
        .add("/w/{slug}/settings/git/{id}/disconnect", post(disconnect))
        .add("/w/{slug}/settings/git/repos/{id}/relink", post(relink))
        .add("/w/{slug}/settings/git/{panel}", get(providers_page))
        .add(
            "/w/{slug}/p/{key}/settings/git",
            get(project_index).post(project_save),
        )
        .add("/w/{slug}/p/{key}/settings/git/{panel}", get(project_page))
        .add("/w/{slug}/p/{key}/prs", get(project_prs))
}
