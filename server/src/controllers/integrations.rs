//! Pengaturan → **API & webhook** (M11, `ManageIntegrations`):
//!
//! - `GET /w/{slug}/settings/api[/{panel}]`: Token API, Webhook, Log pengiriman, Dokumentasi;
//! - `POST …/settings/api/tokens` (buat; token mentah dirender sekali di respons, bukan PRG),
//!   `POST …/tokens/{id}/revoke`;
//! - `POST …/settings/api/hooks` (buat; secret dirender sekali), `POST …/hooks/{id}` (ubah),
//!   `…/hooks/{id}/toggle`, `…/ping`, `…/rotate` (secret baru dirender sekali), `…/delete`;
//! - `POST …/settings/api/log/{id}/resend`.
//!
//! Tanpa izin → 404. Isian yang ditolak dirender ulang (422) dengan dialognya terbuka.
use std::collections::HashMap;

use axum::{extract::Form, http::StatusCode};
use loco_rs::prelude::*;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::{
    common::settings::Settings,
    controllers::{
        extractors::{IdOr404, RequestId, Tenant},
        flash,
        shell::app_shell,
    },
    models::{
        api_tokens::{self, TokenExpiry},
        outbox_events::WebhookEvent,
        permission::Permission,
        projects, users,
        webhook_deliveries::{self, LogQuery},
        webhooks,
    },
    services::{
        api_tokens::{self as token_svc, TokenInput},
        tickets::TicketFailure,
        webhooks::{self as hook_svc, WebhookInput},
        workspaces::Acting,
    },
    views::{
        self,
        datatable::{ColumnSpec, FilterOption, FilterSpec, TableSpec, TableView},
        flash::Flash,
        integrations::{self as iv, DeliveryRow, HookRow, TokenRow},
        nav::NavItem,
        ui::ComboOption,
    },
};

pub(crate) fn base(slug: &str) -> String {
    format!("/w/{slug}/settings/api")
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
    ("tokens", "api-panel-tokens", "key", "api-panel-tokens-desc"),
    ("hooks", "api-panel-hooks", "webhook", "api-panel-hooks-desc"),
    ("log", "api-panel-log", "history", "api-panel-log-desc"),
    ("docs", "api-panel-docs", "book", "api-panel-docs-desc"),
];

pub static TOKENS_TABLE: TableSpec = TableSpec {
    id: "api-tokens",
    columns: &[
        ColumnSpec::new("api-col-name").sort("name"),
        ColumnSpec::new("api-col-scopes").hide("sm"),
        ColumnSpec::new("api-col-used").sort("used").hide("sm"),
        ColumnSpec::new("api-col-expires").sort("expires"),
        ColumnSpec::new("api-col-actions").right(),
    ],
    filters: &[FilterSpec {
        key: "creator",
        label_key: "api-filter-creator",
        icon: "user",
    }],
    default_sort: "name",
    default_desc: false,
    default_per: 10,
    search_key: Some("api-tokens-search"),
};

pub static HOOKS_TABLE: TableSpec = TableSpec {
    id: "api-hooks",
    columns: &[
        ColumnSpec::new("api-col-target").sort("url"),
        ColumnSpec::new("api-col-last").sort("state").hide("sm"),
        ColumnSpec::new("api-col-active"),
        ColumnSpec::new("api-col-actions").right(),
    ],
    filters: &[],
    default_sort: "url",
    default_desc: false,
    default_per: 10,
    search_key: None,
};

pub static LOG_TABLE: TableSpec = TableSpec {
    id: "api-log",
    columns: &[
        ColumnSpec::new("api-col-time"),
        ColumnSpec::new("api-col-event"),
        ColumnSpec::new("api-col-target").hide("sm"),
        ColumnSpec::new("api-col-status"),
        ColumnSpec::new("api-col-duration").hide("sm").right(),
        ColumnSpec::new("api-col-actions").right(),
    ],
    filters: &[
        FilterSpec {
            key: "event",
            label_key: "api-col-event",
            icon: "webhook",
        },
        FilterSpec {
            key: "result",
            label_key: "api-filter-result",
            icon: "circle-check",
        },
        FilterSpec {
            key: "hook",
            label_key: "api-col-target",
            icon: "world",
        },
    ],
    default_sort: "time",
    default_desc: true,
    default_per: 10,
    search_key: Some("api-log-search"),
};

/// Keadaan dialog yang dirender ulang setelah POST.
#[derive(Debug, Default, serde::Serialize)]
struct Extra {
    /// Isian Token baru yang ditolak.
    token_failed: Option<serde_json::Value>,
    /// Token baru (mentah, sekali tampil).
    token_created: Option<serde_json::Value>,
    /// Isian webhook yang ditolak (`id` kosong = Webhook baru).
    hook_failed: Option<serde_json::Value>,
    /// Secret webhook baru (sekali tampil).
    secret: Option<serde_json::Value>,
}

impl Extra {
    fn is_failure(&self) -> bool {
        self.token_failed.is_some() || self.hook_failed.is_some()
    }
}

/// Kode Fluent pertama dari error validasi.
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
    let base = base(&tenant.workspace.slug);
    let settings = Settings::from_ctx(ctx)?;
    let tokens = api_tokens::Model::active_of_workspace(db, wid).await?;
    let hooks = webhooks::Model::of_workspace(db, wid).await?;
    let project_rows = projects::Entity::find()
        .filter(projects::Column::WorkspaceId.eq(wid.get()))
        .all(db)
        .await?;
    let project_names: HashMap<Uuid, (String, String)> = project_rows
        .iter()
        .map(|p| (p.id, (p.key.clone(), p.name.clone())))
        .collect();
    let failing = hooks.iter().filter(|h| h.is_failing() || (h.is_active() && h.consecutive_failures > 0)).count();
    let subnav: Vec<NavItem> = PANELS
        .iter()
        .map(|(k, label, icon, desc)| {
            let mut item = NavItem::link(k, label, icon, &format!("{base}/{k}")).describe(desc);
            item.current = *k == panel;
            match *k {
                "tokens" => item.counted(tokens.len() as u64, false),
                "hooks" => item.counted(hooks.len() as u64, failing > 0),
                _ => item,
            }
        })
        .collect();
    let mut project_options = vec![ComboOption::new("all", "Semua project")];
    let mut sorted_projects = project_rows.clone();
    sorted_projects.sort_by(|a, b| a.name.cmp(&b.name));
    project_options.extend(
        sorted_projects
            .iter()
            .map(|p| ComboOption::new(&p.key, &p.name).sub(&p.key)),
    );
    let mut data = serde_json::json!({
        "panel": panel,
        "subnav": subnav,
        "flash": flash,
        "base": base,
        "extra": extra,
        "project_options": project_options,
        "event_options": iv::event_options(),
        "api_base": format!("{}/api/v1", settings.public_base_url),
        "rate_limit": settings.api.rate_limit_per_minute,
        "expiry_options": [
            { "id": "30", "name": "api-expiry-30" },
            { "id": "90", "name": "api-expiry-90" },
            { "id": "365", "name": "api-expiry-365" },
            { "id": "never", "name": "api-expiry-never" },
        ],
    });
    data["token_form"] = data["extra"]["token_failed"].clone();
    if data["token_form"].is_null() {
        data["token_form"] = serde_json::json!({
            "name": "", "scopes": ["tiket:baca"], "expiry": TokenExpiry::Days90.key(),
            "project": "all", "field": "", "code": "",
        });
    }
    data["hook_form"] = data["extra"]["hook_failed"].clone();
    if data["hook_form"].is_null() {
        data["hook_form"] = serde_json::json!({
            "id": null, "url": "", "events": [], "projects": ["all"], "field": "", "code": "",
        });
    } else if data["hook_form"]["projects"].as_array().is_some_and(Vec::is_empty) {
        data["hook_form"]["projects"] = serde_json::json!(["all"]);
    }
    let token_scopes: Vec<String> = data["extra"]["token_failed"]["scopes"]
        .as_array()
        .map(|a| a.iter().filter_map(|s| s.as_str().map(str::to_string)).collect())
        .unwrap_or_else(|| vec!["tiket:baca".to_string()]);
    data["scope_options"] = serde_json::to_value(iv::scope_options(&token_scopes))?;
    match panel {
        "tokens" => {
            let ids: Vec<Uuid> = tokens.iter().map(|t| t.user_id).collect();
            let names = users::Model::names_by_id(db, &ids).await?;
            let mut rows: Vec<TokenRow> = tokens
                .iter()
                .map(|t| {
                    let creator = names.get(&t.user_id).cloned().unwrap_or_default();
                    let project = t
                        .project_id
                        .and_then(|p| project_names.get(&p).map(|(_, n)| n.clone()))
                        .unwrap_or_default();
                    TokenRow::of(t, &creator, &project, &base)
                })
                .collect();
            let state = TOKENS_TABLE.parse(query);
            if let Some(q) = state.query() {
                let q = q.to_lowercase();
                rows.retain(|r| {
                    r.name.to_lowercase().contains(&q) || r.prefix.to_lowercase().contains(&q)
                });
            }
            let creators = state.filter("creator").to_vec();
            if !creators.is_empty() {
                rows.retain(|r| creators.contains(&r.creator));
            }
            match state.sort {
                "used" => rows.sort_by(|a, b| a.used.as_ref().map(|u| (u.key, u.n)).cmp(&b.used.as_ref().map(|u| (u.key, u.n)))),
                "expires" => rows.sort_by(|a, b| a.expires.cmp(&b.expires)),
                _ => rows.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
            }
            if state.desc {
                rows.reverse();
            }
            let total = rows.len() as u64;
            let page: Vec<TokenRow> = rows
                .into_iter()
                .skip(usize::try_from(state.offset()).unwrap_or(0))
                .take(usize::try_from(state.per).unwrap_or(10))
                .collect();
            let mut creator_opts: Vec<FilterOption> = names
                .values()
                .map(|n| FilterOption::raw(n, n))
                .collect();
            creator_opts.sort_by(|a, b| a.label.cmp(&b.label));
            creator_opts.dedup_by(|a, b| a.label == b.label);
            let table = TableView::new(&TOKENS_TABLE, &state, &format!("{base}/tokens"), total)
                .with_filter_options("creator", creator_opts);
            data["table"] = serde_json::to_value(table)?;
            data["rows"] = serde_json::to_value(page)?;
        }
        "hooks" => {
            let state = HOOKS_TABLE.parse(query);
            let mut rows: Vec<HookRow> = hooks
                .iter()
                .map(|h| HookRow::of(h, &project_names, &base))
                .collect();
            match state.sort {
                "state" => rows.sort_by(|a, b| a.state.cmp(b.state)),
                _ => rows.sort_by(|a, b| a.url.cmp(&b.url)),
            }
            if state.desc {
                rows.reverse();
            }
            let total = rows.len() as u64;
            let page: Vec<HookRow> = rows
                .into_iter()
                .skip(usize::try_from(state.offset()).unwrap_or(0))
                .take(usize::try_from(state.per).unwrap_or(10))
                .collect();
            let table = TableView::new(&HOOKS_TABLE, &state, &format!("{base}/hooks"), total);
            data["table"] = serde_json::to_value(table)?;
            data["rows"] = serde_json::to_value(page)?;
        }
        "log" => {
            let state = LOG_TABLE.parse(query);
            let hook_ids: Vec<Uuid> = state
                .filter("hook")
                .iter()
                .filter_map(|h| Uuid::try_parse(h).ok())
                .collect();
            let lq = LogQuery {
                webhook_ids: hook_ids,
                events: state.filter("event").to_vec(),
                results: state.filter("result").to_vec(),
                q: state.query().map(str::to_string),
                offset: state.offset(),
                limit: state.per,
            };
            let (rows, total) = webhook_deliveries::Model::log_page(db, wid, &lq).await?;
            let page: Vec<DeliveryRow> = rows
                .iter()
                .map(|d| DeliveryRow::of(d, hooks.iter().find(|h| h.id == d.webhook_id), &base))
                .collect();
            let table = TableView::new(&LOG_TABLE, &state, &format!("{base}/log"), total)
                .with_filter_options(
                    "event",
                    WebhookEvent::subscribable()
                        .into_iter()
                        .chain([WebhookEvent::Ping])
                        .map(|e| {
                            let v = e.as_str();
                            FilterOption::raw(&v, &v)
                        })
                        .collect(),
                )
                .with_filter_options(
                    "result",
                    vec![
                        FilterOption::key("ok", "api-result-ok"),
                        FilterOption::key("fail", "api-result-fail"),
                        FilterOption::key("pending", "api-result-pending"),
                    ],
                )
                .with_filter_options(
                    "hook",
                    hooks
                        .iter()
                        .map(|h| FilterOption::raw(&h.id.to_string(), &h.host()))
                        .collect(),
                );
            data["table"] = serde_json::to_value(table)?;
            data["rows"] = serde_json::to_value(page)?;
        }
        _ => {}
    }
    let shell = app_shell(ctx, &tenant.user, Some(tenant), None)
        .await?
        .at("api");
    views::tickets::render(v, &shell, "integrations/api.html", status, data)
}

#[debug_handler]
async fn index(State(_ctx): State<AppContext>, tenant: Tenant) -> Result<Response> {
    require(&tenant)?;
    format::redirect(&format!("{}/tokens", base(&tenant.workspace.slug)))
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

/// `GET …/settings/api/{log|docs}`.
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

/// `GET …/settings/api/tokens` (path yang sama dengan POST buat token).
#[debug_handler]
async fn tokens_page(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    tenant: Tenant,
    session: Session,
    Query(query): Query<Vec<(String, String)>>,
) -> Result<Response> {
    show(&ctx, &v, &tenant, session, "tokens", &query).await
}

/// `GET …/settings/api/hooks` (path yang sama dengan POST buat webhook).
#[debug_handler]
async fn hooks_page(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    tenant: Tenant,
    session: Session,
    Query(query): Query<Vec<(String, String)>>,
) -> Result<Response> {
    show(&ctx, &v, &tenant, session, "hooks", &query).await
}

fn values(form: &[(String, String)], key: &str) -> Vec<String> {
    form.iter()
        .filter(|(k, _)| k == key)
        .map(|(_, v)| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .collect()
}

fn value(form: &[(String, String)], key: &str) -> String {
    values(form, key).into_iter().next().unwrap_or_default()
}

fn redirect_with(session: Session, flash: &Flash, to: &str) -> Result<Response> {
    let session = flash::set(session, flash)?;
    Ok((session, format::redirect(to)?).into_response())
}

/// `ManageIntegrations` sebelum body dibaca.
#[debug_handler]
async fn create_token(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    tenant: Tenant,
    RequestId(request_id): RequestId,
    Form(form): Form<Vec<(String, String)>>,
) -> Result<Response> {
    require(&tenant)?;
    let input = TokenInput {
        name: value(&form, "name"),
        scopes: values(&form, "scopes"),
        expiry: value(&form, "expiry"),
        project: value(&form, "project"),
    };
    match token_svc::create(&ctx.db, &acting(&tenant, request_id), &input).await {
        Ok((token, raw)) => {
            let extra = Extra {
                token_created: Some(serde_json::json!({
                    "name": token.name,
                    "preview": format!("{}…", raw.chars().take(14).collect::<String>()),
                    "raw": raw,
                })),
                ..Extra::default()
            };
            render_page(&ctx, &v, &tenant, "tokens", &[], None, extra).await
        }
        Err(e @ TicketFailure::Invalid(_)) => {
            let (field, code) = first_code(&e);
            let extra = Extra {
                token_failed: Some(serde_json::json!({
                    "name": input.name, "scopes": input.scopes, "expiry": input.expiry,
                    "project": input.project, "field": field, "code": code,
                })),
                ..Extra::default()
            };
            render_page(&ctx, &v, &tenant, "tokens", &[], None, extra).await
        }
        Err(e) => Err(e.into()),
    }
}

#[debug_handler]
async fn revoke_token(
    State(ctx): State<AppContext>,
    tenant: Tenant,
    session: Session,
    RequestId(request_id): RequestId,
    id: IdOr404<api_tokens::Entity>,
) -> Result<Response> {
    require(&tenant)?;
    token_svc::revoke(&ctx.db, &acting(&tenant, request_id), id.id).await?;
    redirect_with(
        session,
        &Flash::success(iv::FLASH_TOKEN_REVOKED),
        &format!("{}/tokens", base(&tenant.workspace.slug)),
    )
}

fn hook_input(form: &[(String, String)]) -> WebhookInput {
    WebhookInput {
        url: value(form, "url"),
        events: values(form, "events"),
        projects: values(form, "projects"),
    }
}

fn hook_failed(input: &WebhookInput, id: Option<Uuid>, e: &TicketFailure) -> Extra {
    let (field, code) = first_code(e);
    Extra {
        hook_failed: Some(serde_json::json!({
            "id": id, "url": input.url, "events": input.events, "projects": input.projects,
            "field": field, "code": code,
        })),
        ..Extra::default()
    }
}

#[debug_handler]
async fn create_hook(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    tenant: Tenant,
    RequestId(request_id): RequestId,
    Form(form): Form<Vec<(String, String)>>,
) -> Result<Response> {
    require(&tenant)?;
    let settings = Settings::from_ctx(&ctx)?;
    let input = hook_input(&form);
    match hook_svc::create(&ctx.db, &acting(&tenant, request_id), &settings, &input).await {
        Ok((hook, secret)) => {
            let extra = Extra {
                secret: Some(serde_json::json!({ "url": hook.url, "secret": secret, "created": true })),
                ..Extra::default()
            };
            render_page(&ctx, &v, &tenant, "hooks", &[], None, extra).await
        }
        Err(e @ TicketFailure::Invalid(_)) => {
            render_page(&ctx, &v, &tenant, "hooks", &[], None, hook_failed(&input, None, &e)).await
        }
        Err(e) => Err(e.into()),
    }
}

#[debug_handler]
async fn update_hook(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    tenant: Tenant,
    session: Session,
    RequestId(request_id): RequestId,
    id: IdOr404<webhooks::Entity>,
    Form(form): Form<Vec<(String, String)>>,
) -> Result<Response> {
    require(&tenant)?;
    let settings = Settings::from_ctx(&ctx)?;
    let input = hook_input(&form);
    match hook_svc::update(&ctx.db, &acting(&tenant, request_id), &settings, id.id, &input).await
    {
        Ok(_) => redirect_with(
            session,
            &Flash::success(iv::FLASH_HOOK_UPDATED),
            &format!("{}/hooks", base(&tenant.workspace.slug)),
        ),
        Err(e @ TicketFailure::Invalid(_)) => {
            render_page(&ctx, &v, &tenant, "hooks", &[], None, hook_failed(&input, Some(id.id), &e))
                .await
        }
        Err(e) => Err(e.into()),
    }
}

#[debug_handler]
async fn toggle_hook(
    State(ctx): State<AppContext>,
    tenant: Tenant,
    session: Session,
    RequestId(request_id): RequestId,
    id: IdOr404<webhooks::Entity>,
    Form(form): Form<Vec<(String, String)>>,
) -> Result<Response> {
    require(&tenant)?;
    let active = value(&form, "active") == "1";
    hook_svc::set_active(&ctx.db, &acting(&tenant, request_id), id.id, active).await?;
    redirect_with(
        session,
        &Flash::success(if active { iv::FLASH_HOOK_ON } else { iv::FLASH_HOOK_OFF }),
        &format!("{}/hooks", base(&tenant.workspace.slug)),
    )
}

#[debug_handler]
async fn ping_hook(
    State(ctx): State<AppContext>,
    tenant: Tenant,
    session: Session,
    RequestId(request_id): RequestId,
    id: IdOr404<webhooks::Entity>,
) -> Result<Response> {
    require(&tenant)?;
    let to = format!("{}/log?hook={}", base(&tenant.workspace.slug), id.id);
    match hook_svc::ping(&ctx.db, &acting(&tenant, request_id), id.id).await {
        Ok(_) => redirect_with(session, &Flash::success(iv::FLASH_PING), &to),
        Err(TicketFailure::Invalid(_)) => redirect_with(
            session,
            &Flash::error(hook_svc::INACTIVE),
            &format!("{}/hooks", base(&tenant.workspace.slug)),
        ),
        Err(e) => Err(e.into()),
    }
}

#[debug_handler]
async fn rotate_hook(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    tenant: Tenant,
    RequestId(request_id): RequestId,
    id: IdOr404<webhooks::Entity>,
) -> Result<Response> {
    require(&tenant)?;
    let settings = Settings::from_ctx(&ctx)?;
    let (hook, secret) =
        hook_svc::rotate_secret(&ctx.db, &acting(&tenant, request_id), &settings, id.id).await?;
    let extra = Extra {
        secret: Some(serde_json::json!({ "url": hook.url, "secret": secret, "created": false })),
        ..Extra::default()
    };
    render_page(&ctx, &v, &tenant, "hooks", &[], None, extra).await
}

#[debug_handler]
async fn delete_hook(
    State(ctx): State<AppContext>,
    tenant: Tenant,
    session: Session,
    RequestId(request_id): RequestId,
    id: IdOr404<webhooks::Entity>,
) -> Result<Response> {
    require(&tenant)?;
    hook_svc::delete(&ctx.db, &acting(&tenant, request_id), id.id).await?;
    redirect_with(
        session,
        &Flash::success(iv::FLASH_HOOK_DELETED),
        &format!("{}/hooks", base(&tenant.workspace.slug)),
    )
}

#[debug_handler]
async fn resend(
    State(ctx): State<AppContext>,
    tenant: Tenant,
    session: Session,
    RequestId(request_id): RequestId,
    id: IdOr404<webhook_deliveries::Entity>,
) -> Result<Response> {
    require(&tenant)?;
    let to = format!("{}/log", base(&tenant.workspace.slug));
    match hook_svc::resend(&ctx.db, &acting(&tenant, request_id), id.id).await {
        Ok(_) => redirect_with(session, &Flash::success(iv::FLASH_RESENT), &to),
        Err(TicketFailure::Invalid(_)) => {
            redirect_with(session, &Flash::error(hook_svc::INACTIVE), &to)
        }
        Err(e) => Err(e.into()),
    }
}

pub fn routes() -> Routes {
    Routes::new()
        .add("/w/{slug}/settings/api", get(index))
        .add(
            "/w/{slug}/settings/api/tokens",
            get(tokens_page).post(create_token),
        )
        .add("/w/{slug}/settings/api/tokens/{id}/revoke", post(revoke_token))
        .add("/w/{slug}/settings/api/hooks", get(hooks_page).post(create_hook))
        .add("/w/{slug}/settings/api/hooks/{id}", post(update_hook))
        .add("/w/{slug}/settings/api/hooks/{id}/toggle", post(toggle_hook))
        .add("/w/{slug}/settings/api/hooks/{id}/ping", post(ping_hook))
        .add("/w/{slug}/settings/api/hooks/{id}/rotate", post(rotate_hook))
        .add("/w/{slug}/settings/api/hooks/{id}/delete", post(delete_hook))
        .add("/w/{slug}/settings/api/log/{id}/resend", post(resend))
        .add("/w/{slug}/settings/api/{panel}", get(page))
}
