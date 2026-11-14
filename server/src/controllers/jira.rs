//! Pengaturan → **Jira** (M11, `ManageIntegrations`):
//!
//! - `GET /w/{slug}/settings/jira[/{panel}]`: Ringkasan, Koneksi, Mode sinkron, Status ↔ stage,
//!   Field & user, Log sinkron;
//! - `POST …/jira/conn` (simpan koneksi), `…/conn/test` (uji, dirender ulang tanpa menyimpan);
//! - `POST …/jira/{mode|status|fields}` (simpan lalu sinkron), `…/sync` (sinkron sekarang),
//!   `…/resync` (sinkron ulang semua), `…/pause`, `…/resume`, `…/disconnect`;
//! - `GET …/jira/log/download`: log sinkron sebagai JSONL.
//!
//! Tanpa izin → 404. Isian yang ditolak dirender ulang (422); yang lain PRG dengan flash.
use axum::{
    extract::Form,
    http::{StatusCode, header},
};
use chrono::Utc;
use loco_rs::prelude::*;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use crate::{
    common::{i18n, settings::Settings},
    controllers::{
        extractors::{RequestId, Tenant},
        flash,
        shell::app_shell,
    },
    models::{
        custom_fields,
        jira_connections::{self, FieldTarget, SyncMode},
        jira_links,
        jira_sync_logs::{self, LogQuery},
        permission::Permission,
        projects, workspace_members,
    },
    services::{
        jira::{
            connections::{self as conn_svc, ConnInput},
            mapping,
            sync::RunKind,
        },
        tickets::TicketFailure,
        workspaces::Acting,
    },
    views::{
        self,
        datatable::{ColumnSpec, FilterOption, FilterSpec, TableSpec, TableView},
        flash::Flash,
        jira::{self as jv, LogLine, LogRow},
        nav::NavItem,
        runner::age,
        ui::{ComboOption, Stat},
    },
    workers::jira_sync,
};

pub(crate) fn base(slug: &str) -> String {
    format!("/w/{slug}/settings/jira")
}

fn acting(tenant: &Tenant, request_id: Option<String>) -> Acting<'_> {
    Acting {
        workspace: &tenant.workspace,
        user: &tenant.user,
        role: &tenant.role,
        request_id,
    }
}

fn require(tenant: &Tenant) -> Result<()> {
    if tenant.can(Permission::ManageIntegrations) {
        Ok(())
    } else {
        Err(Error::NotFound)
    }
}

/// Panel: (kunci, label, ikon, deskripsi).
const PANELS: &[(&str, &str, &str, &str)] = &[
    (
        "overview",
        "jira-panel-overview",
        "layout-dashboard",
        "jira-panel-overview-desc",
    ),
    ("conn", "jira-panel-conn", "plug", "jira-panel-conn-desc"),
    (
        "mode",
        "jira-panel-mode",
        "arrows-exchange",
        "jira-panel-mode-desc",
    ),
    (
        "status",
        "jira-panel-status",
        "route",
        "jira-panel-status-desc",
    ),
    (
        "fields",
        "jira-panel-fields",
        "list-details",
        "jira-panel-fields-desc",
    ),
    ("log", "jira-panel-log", "history", "jira-panel-log-desc"),
];

pub static LOG_TABLE: TableSpec = TableSpec {
    id: "jira-log",
    columns: &[
        ColumnSpec::new("jira-col-time"),
        ColumnSpec::new("jira-col-result"),
        ColumnSpec::new("jira-col-ticket"),
        ColumnSpec::new("jira-col-message").hide("sm"),
    ],
    filters: &[
        FilterSpec {
            key: "result",
            label_key: "jira-filter-result",
            icon: "circle-check",
        },
        FilterSpec {
            key: "direction",
            label_key: "jira-filter-direction",
            icon: "arrows-exchange",
        },
    ],
    default_sort: "time",
    default_desc: true,
    default_per: 10,
    search_key: Some("jira-log-search"),
};

/// Baris unduhan JSONL terbanyak.
const DOWNLOAD_LIMIT: u64 = 10_000;

/// Keadaan form Koneksi yang dirender ulang setelah POST (gagal atau uji koneksi).
#[derive(Debug, Default, serde::Serialize)]
struct Extra {
    /// Isian koneksi + `field`/`code` error (kosong = tanpa error).
    conn_form: Option<serde_json::Value>,
    /// Hasil uji koneksi: `{account, projects}`.
    tested: Option<serde_json::Value>,
    /// Project Jira hasil uji (opsi combo).
    tested_projects: Vec<ComboOption>,
    /// Error panel lain: `{panel, code}`.
    failed: Option<serde_json::Value>,
}

impl Extra {
    fn is_failure(&self) -> bool {
        self.failed.is_some()
            || self
                .conn_form
                .as_ref()
                .is_some_and(|f| f["code"].as_str().is_some_and(|c| !c.is_empty()))
    }
}

fn first_code(e: &TicketFailure) -> (String, String) {
    match e {
        TicketFailure::Invalid(errors) => errors
            .errors
            .iter()
            .find_map(|(f, es)| es.first().map(|e| (f.to_string(), e.code.to_string())))
            .unwrap_or_default(),
        _ => (String::new(), String::new()),
    }
}

fn mode_key(mode: SyncMode) -> String {
    format!("jira-mode-{}", mode.as_str())
}

#[allow(clippy::too_many_lines)]
async fn render_page(
    ctx: &AppContext,
    v: &TeraView,
    tenant: &Tenant,
    panel: &str,
    query: &[(String, String)],
    flash: Option<Flash>,
    extra: Extra,
) -> Result<Response> {
    let db = &ctx.db;
    let status = if extra.is_failure() {
        StatusCode::UNPROCESSABLE_ENTITY
    } else {
        StatusCode::OK
    };
    let wid = tenant.workspace_id();
    let slug = &tenant.workspace.slug;
    let base = base(slug);
    let now = Utc::now();
    let conn = jira_connections::Model::of_workspace(db, wid).await?;
    let mut project_rows: Vec<projects::Model> = projects::Entity::find()
        .filter(projects::Column::WorkspaceId.eq(wid.get()))
        .filter(projects::Column::ArchivedAt.is_null())
        .all(db)
        .await?;
    project_rows.sort_by(|a, b| a.name.cmp(&b.name));
    let project = conn
        .as_ref()
        .and_then(|c| c.project_id)
        .and_then(|id| project_rows.iter().find(|p| p.id == id).cloned());
    let statuses = conn
        .as_ref()
        .map(jira_connections::Model::status_map)
        .unwrap_or_default();
    let fields = conn
        .as_ref()
        .map(jira_connections::Model::field_map)
        .unwrap_or_default();
    let users = conn
        .as_ref()
        .map(jira_connections::Model::user_map)
        .unwrap_or_default();
    let unmapped: Vec<String> = statuses.unmapped().iter().map(|s| s.name.clone()).collect();
    let log_total = match &conn {
        Some(c) => jira_sync_logs::Model::count_of(db, c.id).await?,
        None => 0,
    };
    let subnav: Vec<NavItem> = PANELS
        .iter()
        .map(|(k, label, icon, desc)| {
            let mut item = NavItem::link(k, label, icon, &format!("{base}/{k}")).describe(desc);
            item.current = *k == panel;
            match *k {
                "status" if !unmapped.is_empty() => item.counted(unmapped.len() as u64, true),
                "log" if log_total > 0 => item.counted(log_total, false),
                _ => item,
            }
        })
        .collect();
    let conn_view = conn.as_ref().map(|c| {
        serde_json::json!({
            "env": c.env,
            "env_key": format!("jira-env-{}", c.env),
            "base_url": c.base_url,
            "host": c.host(),
            "email": c.email,
            "account": c.account_name,
            "jira_project": c.jira_project_key,
            "jira_project_name": c.jira_project_name,
            "project": project.as_ref().map(|p| p.key.clone()).unwrap_or_default(),
            "project_name": project.as_ref().map(|p| p.name.clone()).unwrap_or_default(),
            "mode": c.mode,
            "mode_key": mode_key(c.mode()),
            "ready": c.is_ready(),
            "paused": c.is_paused(),
            "running": c.sync_started_at.is_some(),
            "last_sync": c.last_sync_at.as_ref().map(|t| age(t, now)),
            "last_error": c.last_error,
            "imported": c.imported_at.is_some(),
            "full_requested": c.full_sync_requested_at.is_some(),
        })
    });
    let catalog_options: Vec<ComboOption> = if extra.tested.is_some() {
        extra.tested_projects.clone()
    } else {
        conn.as_ref()
            .map(|c| {
                c.catalog()
                    .projects
                    .iter()
                    .map(|p| ComboOption::new(&p.key, format!("{} · {}", p.key, p.name)))
                    .collect()
            })
            .unwrap_or_default()
    };
    let project_options: Vec<ComboOption> = project_rows
        .iter()
        .map(|p| ComboOption::new(&p.key, &p.name).sub(&p.key))
        .collect();
    let conn_form = extra.conn_form.clone().unwrap_or_else(|| match &conn {
        Some(c) => serde_json::json!({
            "env": c.env, "base_url": c.base_url, "email": c.email,
            "jira_project": c.jira_project_key,
            "project": project.as_ref().map(|p| p.key.clone()).unwrap_or_default(),
            "field": "", "code": "",
        }),
        None => serde_json::json!({
            "env": "cloud", "base_url": "", "email": tenant.user.email,
            "jira_project": "", "project": project_rows.first().map(|p| p.key.clone()).unwrap_or_default(),
            "field": "", "code": "",
        }),
    });
    let mut data = serde_json::json!({
        "panel": panel,
        "subnav": subnav,
        "flash": flash,
        "base": base,
        "extra": extra,
        "conn": conn_view,
        "conn_form": conn_form,
        "has_token": conn.is_some(),
        "catalog_options": catalog_options,
        "project_options": project_options,
        "unmapped": unmapped,
        "members_href": format!("/w/{slug}/settings/members"),
        "modes": [
            { "key": "once", "icon": "file-import", "label_key": "jira-mode-once", "hint_key": "jira-mode-once-hint" },
            { "key": "mirror", "icon": "arrow-right", "label_key": "jira-mode-mirror", "hint_key": "jira-mode-mirror-hint" },
            { "key": "two_way", "icon": "arrows-exchange", "label_key": "jira-mode-two_way", "hint_key": "jira-mode-two_way-hint" },
        ],
        "envs": [
            { "key": "cloud", "icon": "cloud", "label_key": "jira-env-cloud", "hint_key": "jira-env-cloud-hint" },
            { "key": "data_center", "icon": "server", "label_key": "jira-env-data_center", "hint_key": "jira-env-data_center-hint" },
        ],
    });
    let Some(c) = conn.as_ref() else {
        let shell = app_shell(ctx, &tenant.user, Some(tenant), None)
            .await?
            .at("jira");
        return views::tickets::render(v, &shell, "integrations/jira.html", status, data);
    };
    match panel {
        "overview" => {
            let links = jira_links::Model::count_of(db, c.id).await?;
            let day = jira_sync_logs::Model::day_counts(db, c.id, now).await?;
            let stats = vec![
                Stat::new("jira-stat-tickets", links, "arrows-exchange")
                    .tone("accent")
                    .hint(
                        "jira-stat-tickets-hint",
                        format!(
                            "{} ↔ {}",
                            c.jira_project_key,
                            project.as_ref().map(|p| p.key.as_str()).unwrap_or("–")
                        ),
                    ),
                Stat::new("jira-stat-changes", day.incoming + day.outgoing, "activity")
                    .hint("jira-stat-changes-hint", day.incoming),
                Stat::new("jira-stat-gate", day.gate, "hand-stop")
                    .tone(if day.gate > 0 { "warn" } else { "" })
                    .hint("jira-stat-gate-hint", "")
                    .link(format!("{base}/log?result=gate")),
            ];
            data["stats"] = serde_json::to_value(stats)?;
            data["summary"] = serde_json::json!({
                "statuses": statuses.statuses.len(),
                "statuses_mapped": statuses.statuses.len() - unmapped.len(),
                "fields": fields.fields.iter().filter(|f| f.target != FieldTarget::Skip).count(),
                "users": users.users.len(),
                "users_mapped": users.mapped(),
                "data": c.stats(),
            });
        }
        "status" => {
            let stages = match &project {
                Some(p) => mapping::project_stages(db, wid, p).await?,
                None => Vec::new(),
            };
            let mut options = vec![ComboOption::new("", i18n::text("jira-unmapped", &[]))];
            options.extend(
                stages
                    .iter()
                    .map(|s| ComboOption::new(&s.key, &s.name).sub(&s.key)),
            );
            data["stage_options"] = serde_json::to_value(options)?;
            data["statuses"] = serde_json::to_value(
                statuses
                    .statuses
                    .iter()
                    .map(|s| {
                        serde_json::json!({
                            "id": s.id, "name": s.name, "category": s.category,
                            "stage": s.stage.clone().unwrap_or_default(),
                        })
                    })
                    .collect::<Vec<_>>(),
            )?;
        }
        "fields" => {
            let custom = custom_fields::Model::all_in(db, wid).await?;
            let mut options: Vec<ComboOption> = FieldTarget::BUILTIN
                .iter()
                .map(|t| {
                    ComboOption::new(
                        t.as_str(),
                        i18n::text(&format!("jira-target-{}", t.as_str()), &[]),
                    )
                })
                .collect();
            options.extend(custom.iter().map(|f| {
                ComboOption::new(FieldTarget::Custom(f.id).as_str(), &f.name)
                    .group(i18n::text("jira-target-custom-group", &[]))
            }));
            options.push(ComboOption::new(
                "none",
                i18n::text("jira-target-none", &[]),
            ));
            data["target_options"] = serde_json::to_value(options)?;
            data["fields"] = serde_json::to_value(&fields.fields)?;
            let members = workspace_members::Model::list_in(db, wid).await?;
            let mut people = vec![ComboOption::new("", i18n::text("jira-user-unmapped", &[]))];
            let mut sorted: Vec<_> = members.iter().map(|(_, u, _)| u).collect();
            sorted.sort_by(|a, b| a.name.cmp(&b.name));
            people.extend(
                sorted
                    .iter()
                    .map(|u| ComboOption::person(u.id, &u.name, &u.email)),
            );
            data["people_options"] = serde_json::to_value(people)?;
            data["users"] = serde_json::to_value(
                users
                    .users
                    .iter()
                    .map(|u| {
                        serde_json::json!({
                            "account": u.account,
                            "name": u.name,
                            "email": u.email,
                            "user_id": u.user_id.map(|id| id.to_string()).unwrap_or_default(),
                        })
                    })
                    .collect::<Vec<_>>(),
            )?;
            data["unmapped_users"] = serde_json::to_value(
                users
                    .users
                    .iter()
                    .filter(|u| u.user_id.is_none())
                    .map(|u| u.name.clone())
                    .collect::<Vec<_>>(),
            )?;
        }
        "log" => {
            let state = LOG_TABLE.parse(query);
            let lq = LogQuery {
                results: state.filter("result").to_vec(),
                directions: state.filter("direction").to_vec(),
                q: state.query().map(str::to_string),
                offset: state.offset(),
                limit: state.per,
            };
            let (rows, total) = jira_sync_logs::Model::page(db, c.id, &lq).await?;
            let rows: Vec<LogRow> = rows.iter().map(|l| LogRow::of(l, slug)).collect();
            let table = TableView::new(&LOG_TABLE, &state, &format!("{base}/log"), total)
                .with_filter_options(
                    "result",
                    vec![
                        FilterOption::key("ok", "jira-result-ok"),
                        FilterOption::key("gate", "jira-result-gate"),
                        FilterOption::key("fail", "jira-result-fail"),
                    ],
                )
                .with_filter_options(
                    "direction",
                    vec![
                        FilterOption::key("in", "jira-dir-in"),
                        FilterOption::key("out", "jira-dir-out"),
                    ],
                );
            data["table"] = serde_json::to_value(table)?;
            data["rows"] = serde_json::to_value(rows)?;
        }
        _ => {}
    }
    let shell = app_shell(ctx, &tenant.user, Some(tenant), None)
        .await?
        .at("jira");
    views::tickets::render(v, &shell, "integrations/jira.html", status, data)
}

#[debug_handler]
async fn index(State(_ctx): State<AppContext>, tenant: Tenant) -> Result<Response> {
    require(&tenant)?;
    format::redirect(&format!("{}/overview", base(&tenant.workspace.slug)))
}

async fn show(
    ctx: &AppContext,
    v: &TeraView,
    tenant: &Tenant,
    session: Session,
    panel: &str,
    query: &[(String, String)],
) -> Result<Response> {
    require(tenant)?;
    if !PANELS.iter().any(|(k, ..)| *k == panel) {
        return Err(Error::NotFound);
    }
    let (session, flash) = flash::take(session);
    let res = render_page(ctx, v, tenant, panel, query, flash, Extra::default()).await?;
    Ok((session, res).into_response())
}

/// `GET …/settings/jira/{panel}`.
#[debug_handler]
async fn page(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    tenant: Tenant,
    session: Session,
    Path((_, panel)): Path<(String, String)>,
    Query(query): Query<Vec<(String, String)>>,
) -> Result<Response> {
    show(&ctx, &v, &tenant, session, &panel, &query).await
}

macro_rules! panel_page {
    ($name:ident, $panel:literal) => {
        #[debug_handler]
        async fn $name(
            State(ctx): State<AppContext>,
            ViewEngine(v): ViewEngine<TeraView>,
            tenant: Tenant,
            session: Session,
            Query(query): Query<Vec<(String, String)>>,
        ) -> Result<Response> {
            show(&ctx, &v, &tenant, session, $panel, &query).await
        }
    };
}

panel_page!(conn_page, "conn");
panel_page!(mode_page, "mode");
panel_page!(status_page, "status");
panel_page!(fields_page, "fields");

fn value(form: &[(String, String)], key: &str) -> String {
    form.iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.trim().to_string())
        .unwrap_or_default()
}

/// Pasangan `prefix:<id>` → nilai.
fn prefixed(form: &[(String, String)], prefix: &str) -> Vec<(String, String)> {
    form.iter()
        .filter_map(|(k, v)| {
            k.strip_prefix(prefix)
                .map(|id| (id.to_string(), v.trim().to_string()))
        })
        .collect()
}

fn redirect_with(session: Session, flash: &Flash, to: &str) -> Result<Response> {
    let session = flash::set(session, flash)?;
    Ok((session, format::redirect(to)?).into_response())
}

fn conn_input(form: &[(String, String)]) -> ConnInput {
    ConnInput {
        env: value(form, "env"),
        base_url: value(form, "base_url"),
        email: value(form, "email"),
        token: value(form, "token"),
        jira_project: value(form, "jira_project"),
        project: value(form, "project"),
    }
}

fn conn_form_json(input: &ConnInput, field: &str, code: &str) -> serde_json::Value {
    serde_json::json!({
        "env": input.env, "base_url": input.base_url, "email": input.email,
        "jira_project": input.jira_project, "project": input.project,
        "field": field, "code": code,
    })
}

/// Antrekan putaran sinkron bila koneksi siap dan tidak dijeda.
async fn start_sync(ctx: &AppContext, conn: &jira_connections::Model, kind: RunKind) {
    if conn.is_ready() && !conn.is_paused() {
        jira_sync::enqueue(ctx, conn.id, kind).await;
    }
}

/// `POST …/jira/conn`: simpan koneksi (tanpa memulai sinkron; sinkron dimulai dari panel Mode,
/// Status, Field, atau tombol Sinkron sekarang setelah pemetaan diperiksa).
#[debug_handler]
async fn save_conn(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    tenant: Tenant,
    session: Session,
    RequestId(request_id): RequestId,
    Form(form): Form<Vec<(String, String)>>,
) -> Result<Response> {
    require(&tenant)?;
    let settings = Settings::from_ctx(&ctx)?;
    let input = conn_input(&form);
    match conn_svc::save(&ctx.db, &acting(&tenant, request_id), &settings, &input).await {
        Ok(_) => redirect_with(
            session,
            &Flash::success(jv::FLASH_SAVED),
            &format!("{}/conn", base(&tenant.workspace.slug)),
        ),
        Err(e @ TicketFailure::Invalid(_)) => {
            let (field, code) = first_code(&e);
            let extra = Extra {
                conn_form: Some(conn_form_json(&input, &field, &code)),
                ..Extra::default()
            };
            render_page(&ctx, &v, &tenant, "conn", &[], None, extra).await
        }
        Err(e) => Err(e.into()),
    }
}

/// `POST …/jira/conn/test`: uji kredensial isian tanpa menyimpan.
#[debug_handler]
async fn test_conn(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    tenant: Tenant,
    RequestId(request_id): RequestId,
    Form(form): Form<Vec<(String, String)>>,
) -> Result<Response> {
    require(&tenant)?;
    let settings = Settings::from_ctx(&ctx)?;
    let input = conn_input(&form);
    let extra = match conn_svc::test(&ctx.db, &acting(&tenant, request_id), &settings, &input).await
    {
        Ok(tested) => Extra {
            conn_form: Some(conn_form_json(&input, "", "")),
            tested: Some(serde_json::json!({
                "account": tested.account,
                "projects": tested.projects.len(),
            })),
            tested_projects: tested
                .projects
                .iter()
                .map(|p| ComboOption::new(&p.key, format!("{} · {}", p.key, p.name)))
                .collect(),
            failed: None,
        },
        Err(e @ TicketFailure::Invalid(_)) => {
            let (field, code) = first_code(&e);
            Extra {
                conn_form: Some(conn_form_json(&input, &field, &code)),
                ..Extra::default()
            }
        }
        Err(e) => return Err(e.into()),
    };
    render_page(&ctx, &v, &tenant, "conn", &[], None, extra).await
}

/// Hasil simpan panel: sukses → sinkron + PRG; ditolak → 422 dengan pesan di panel.
async fn after_save(
    ctx: &AppContext,
    v: &TeraView,
    tenant: &Tenant,
    session: Session,
    panel: &str,
    result: std::result::Result<jira_connections::Model, TicketFailure>,
) -> Result<Response> {
    match result {
        Ok(conn) => {
            start_sync(ctx, &conn, RunKind::Cycle).await;
            redirect_with(
                session,
                &Flash::success(jv::FLASH_SYNCING),
                &format!("{}/{panel}", base(&tenant.workspace.slug)),
            )
        }
        Err(e @ TicketFailure::Invalid(_)) => {
            let (_, code) = first_code(&e);
            let extra = Extra {
                failed: Some(serde_json::json!({ "panel": panel, "code": code })),
                ..Extra::default()
            };
            render_page(ctx, v, tenant, panel, &[], None, extra).await
        }
        Err(e) => Err(e.into()),
    }
}

#[debug_handler]
async fn save_mode(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    tenant: Tenant,
    session: Session,
    RequestId(request_id): RequestId,
    Form(form): Form<Vec<(String, String)>>,
) -> Result<Response> {
    require(&tenant)?;
    let result =
        conn_svc::save_mode(&ctx.db, &acting(&tenant, request_id), &value(&form, "mode")).await;
    after_save(&ctx, &v, &tenant, session, "mode", result).await
}

#[debug_handler]
async fn save_status(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    tenant: Tenant,
    session: Session,
    RequestId(request_id): RequestId,
    Form(form): Form<Vec<(String, String)>>,
) -> Result<Response> {
    require(&tenant)?;
    let choices = prefixed(&form, "st:");
    let result = conn_svc::save_statuses(&ctx.db, &acting(&tenant, request_id), &choices).await;
    after_save(&ctx, &v, &tenant, session, "status", result).await
}

#[debug_handler]
async fn save_fields(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    tenant: Tenant,
    session: Session,
    RequestId(request_id): RequestId,
    Form(form): Form<Vec<(String, String)>>,
) -> Result<Response> {
    require(&tenant)?;
    let fields = prefixed(&form, "f:");
    let users = prefixed(&form, "u:");
    let result =
        conn_svc::save_fields(&ctx.db, &acting(&tenant, request_id), &fields, &users).await;
    after_save(&ctx, &v, &tenant, session, "fields", result).await
}

async fn connection(tenant: &Tenant, ctx: &AppContext) -> Result<jira_connections::Model> {
    jira_connections::Model::of_workspace(&ctx.db, tenant.workspace_id())
        .await?
        .ok_or(Error::NotFound)
}

/// `POST …/jira/sync`: Sinkron sekarang.
#[debug_handler]
async fn sync_now(
    State(ctx): State<AppContext>,
    tenant: Tenant,
    session: Session,
) -> Result<Response> {
    require(&tenant)?;
    let conn = connection(&tenant, &ctx).await?;
    let to = format!("{}/overview", base(&tenant.workspace.slug));
    if !conn.is_ready() {
        return redirect_with(
            session,
            &Flash::error(crate::services::jira::NOT_CONNECTED),
            &to,
        );
    }
    start_sync(&ctx, &conn, RunKind::Cycle).await;
    redirect_with(session, &Flash::success(jv::FLASH_SYNCING), &to)
}

/// `POST …/jira/resync`: Sinkron ulang semua (rekonsiliasi penuh, menimpa Jira).
#[debug_handler]
async fn resync(
    State(ctx): State<AppContext>,
    tenant: Tenant,
    session: Session,
    RequestId(request_id): RequestId,
) -> Result<Response> {
    require(&tenant)?;
    let to = format!("{}/overview", base(&tenant.workspace.slug));
    match conn_svc::request_full(&ctx.db, &acting(&tenant, request_id)).await {
        Ok(conn) => {
            start_sync(&ctx, &conn, RunKind::Full).await;
            redirect_with(session, &Flash::success(jv::FLASH_FULL), &to)
        }
        Err(e @ TicketFailure::Invalid(_)) => {
            let (_, code) = first_code(&e);
            redirect_with(session, &Flash::error(&code), &to)
        }
        Err(e) => Err(e.into()),
    }
}

async fn set_paused(
    ctx: &AppContext,
    tenant: &Tenant,
    session: Session,
    request_id: Option<String>,
    paused: bool,
) -> Result<Response> {
    require(tenant)?;
    let conn = conn_svc::set_paused(&ctx.db, &acting(tenant, request_id), paused).await?;
    if !paused {
        start_sync(ctx, &conn, RunKind::Cycle).await;
    }
    redirect_with(
        session,
        &Flash::success(if paused {
            jv::FLASH_PAUSED
        } else {
            jv::FLASH_RESUMED
        }),
        &format!("{}/overview", base(&tenant.workspace.slug)),
    )
}

#[debug_handler]
async fn pause(
    State(ctx): State<AppContext>,
    tenant: Tenant,
    session: Session,
    RequestId(request_id): RequestId,
) -> Result<Response> {
    set_paused(&ctx, &tenant, session, request_id, true).await
}

#[debug_handler]
async fn resume(
    State(ctx): State<AppContext>,
    tenant: Tenant,
    session: Session,
    RequestId(request_id): RequestId,
) -> Result<Response> {
    set_paused(&ctx, &tenant, session, request_id, false).await
}

/// `POST …/jira/disconnect`: putuskan koneksi; tiket yang sudah diimpor/disinkron tetap ada.
#[debug_handler]
async fn disconnect(
    State(ctx): State<AppContext>,
    tenant: Tenant,
    session: Session,
    RequestId(request_id): RequestId,
) -> Result<Response> {
    require(&tenant)?;
    conn_svc::disconnect(&ctx.db, &acting(&tenant, request_id)).await?;
    redirect_with(
        session,
        &Flash::success(jv::FLASH_DISCONNECTED),
        &format!("{}/overview", base(&tenant.workspace.slug)),
    )
}

/// `GET …/jira/log/download`: log sinkron (terbaru dulu) sebagai JSON Lines.
#[debug_handler]
async fn download_log(State(ctx): State<AppContext>, tenant: Tenant) -> Result<Response> {
    require(&tenant)?;
    let conn = connection(&tenant, &ctx).await?;
    let rows = jira_sync_logs::Model::all_of(&ctx.db, conn.id, DOWNLOAD_LIMIT).await?;
    let mut body = String::new();
    for l in &rows {
        body.push_str(&serde_json::to_string(&LogLine::of(l))?);
        body.push('\n');
    }
    Ok((
        [
            (header::CONTENT_TYPE, "application/x-ndjson; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"jira-sync-log.jsonl\"",
            ),
        ],
        body,
    )
        .into_response())
}

pub fn routes() -> Routes {
    Routes::new()
        .add("/w/{slug}/settings/jira", get(index))
        .add(
            "/w/{slug}/settings/jira/conn",
            get(conn_page).post(save_conn),
        )
        .add("/w/{slug}/settings/jira/conn/test", post(test_conn))
        .add(
            "/w/{slug}/settings/jira/mode",
            get(mode_page).post(save_mode),
        )
        .add(
            "/w/{slug}/settings/jira/status",
            get(status_page).post(save_status),
        )
        .add(
            "/w/{slug}/settings/jira/fields",
            get(fields_page).post(save_fields),
        )
        .add("/w/{slug}/settings/jira/sync", post(sync_now))
        .add("/w/{slug}/settings/jira/resync", post(resync))
        .add("/w/{slug}/settings/jira/pause", post(pause))
        .add("/w/{slug}/settings/jira/resume", post(resume))
        .add("/w/{slug}/settings/jira/disconnect", post(disconnect))
        .add("/w/{slug}/settings/jira/log/download", get(download_log))
        .add("/w/{slug}/settings/jira/{panel}", get(page))
}
