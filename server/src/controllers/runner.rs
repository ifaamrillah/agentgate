//! Runner & CLI di web (M5), terbuka untuk semua anggota workspace:
//!
//! - `GET /runner` → Runner & CLI workspace pertama (tautan dari halaman login perangkat);
//! - `GET /w/{slug}/runner[/{panel}]`: subpage varian E (Permintaan izin, Antrean job, Runner
//!   saya, Runner tim, Pengaturan runner, Aturan & drift, Instal CLI);
//! - `POST /w/{slug}/runner/prefs`, `/runner/runners/{id}/revoke`, `/runner/permissions/{id}`;
//! - `GET /w/{slug}/jobs/{id}` (Log job) + `/jobs/{id}/live` (bagian live, HTMX polling);
//! - `POST /w/{slug}/jobs/{id}/claim|release|cancel|quota`;
//! - `GET /w/{slug}/evidence/{id}` (berkas evidence; gambar inline, teks sebagai text/plain).
//!
//! Aksi POST memakai PRG: sukses/gagal → flash lalu kembali ke `next` (path workspace ini).
//! Job/izin/runner di luar akses pelaku → 404.
use std::collections::HashMap;

use agentgate_core::{
    persona::CLIENTS,
    runner::protocol::{JobStatus, PermissionStatus},
};
use axum::{
    body::Body,
    http::{HeaderValue, StatusCode, header},
};
use loco_rs::prelude::*;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    common::settings::Settings,
    controllers::{
        extractors::{CurrentUser, IdOr404, RequestId, Tenant},
        flash,
        shell::app_shell,
    },
    models::{
        job_evidence, job_permissions, jobs,
        personas::{self, PersonaLabels},
        project_access, projects, rule_checks,
        runner_prefs::{self, QuotaPolicy},
        runners, tickets, users, workspace_members,
    },
    services::{
        jobs::{self as svc, QuotaChoice, evidence},
        tickets::{TicketFailure, stages::Flow},
        workspaces::Acting,
    },
    views::{
        self,
        datatable::{ColumnSpec, FilterOption, FilterSpec, TableSpec, TableView},
        flash::Flash,
        nav::NavItem,
        runner::{
            EvidenceView, JobContext, JobView, LogLineView, PermissionContext, PermissionView,
            RunnerJob, RunnerView, StepsView,
        },
        ui::Stat,
    },
};

/// Baris log yang ditampilkan di halaman job.
const LOG_LINES: u64 = 500;

/// Panel Runner & CLI.
const PANELS: &[(&str, &str, &str, &str)] = &[
    (
        "perm",
        "runner-panel-perm",
        "hand-stop",
        "runner-panel-perm-desc",
    ),
    (
        "queue",
        "runner-panel-queue",
        "list-details",
        "runner-panel-queue-desc",
    ),
    (
        "mine",
        "runner-panel-mine",
        "device-laptop",
        "runner-panel-mine-desc",
    ),
    (
        "team",
        "runner-panel-team",
        "users",
        "runner-panel-team-desc",
    ),
    (
        "settings",
        "runner-panel-settings",
        "settings",
        "runner-panel-settings-desc",
    ),
    (
        "drift",
        "runner-panel-drift",
        "shield-check",
        "runner-panel-drift-desc",
    ),
    (
        "cli",
        "runner-panel-cli",
        "terminal-2",
        "runner-panel-cli-desc",
    ),
];

/// Tabel antrean job (filter/sort/halaman di memori: antrean berisi job terbuka saja).
pub static QUEUE_TABLE: TableSpec = TableSpec {
    id: "job-queue",
    columns: &[
        ColumnSpec::new("job-col-ticket").sort("priority"),
        ColumnSpec::new("job-col-stage").hide("sm"),
        ColumnSpec::new("job-col-status").sort("status"),
        ColumnSpec::new("job-col-age").sort("age").hide("sm"),
        ColumnSpec::new("job-col-actions").right(),
    ],
    filters: &[
        FilterSpec {
            key: "status",
            label_key: "job-col-status",
            icon: "circle-dot",
        },
        FilterSpec {
            key: "mine",
            label_key: "job-filter-for",
            icon: "user",
        },
    ],
    default_sort: "priority",
    default_desc: false,
    default_per: 25,
    search_key: Some("job-search"),
};

pub(crate) fn base(slug: &str) -> String {
    format!("/w/{slug}/runner")
}

fn acting(tenant: &Tenant, request_id: Option<String>) -> Acting<'_> {
    Acting {
        workspace: &tenant.workspace,
        user: &tenant.user,
        role: &tenant.role,
        request_id,
    }
}

/// `next` dari form: hanya path di workspace ini (tanpa `//`), selain itu `fallback`.
fn back(next: &str, slug: &str, fallback: &str) -> String {
    let prefix = format!("/w/{slug}/");
    if next.starts_with(&prefix) && !next.contains("//") && !next.contains('\\') {
        next.to_string()
    } else {
        fallback.to_string()
    }
}

/// PRG: flash sukses, atau flash error berkode Fluent untuk penolakan domain; 404 untuk sisanya.
fn done<T>(
    session: Session,
    r: std::result::Result<T, TicketFailure>,
    ok_key: &str,
    to: &str,
) -> Result<Response> {
    let flash_msg = match r {
        Ok(_) => Flash::success(ok_key),
        Err(TicketFailure::Invalid(errors)) => {
            let code = errors
                .errors
                .values()
                .flat_map(|es| es.iter().map(|e| e.code.to_string()))
                .next()
                .unwrap_or_else(|| "error-generic-title".into());
            Flash::error(&code)
        }
        Err(TicketFailure::NotFound) => return Err(Error::NotFound),
        Err(TicketFailure::Db(e)) => return Err(e.into()),
    };
    let session = flash::set(session, &flash_msg)?;
    Ok((session, format::redirect(to)?).into_response())
}

/// Nama stage tiket menurut workflow tiket.
async fn stage_name(
    db: &DatabaseConnection,
    tenant: &Tenant,
    t: Option<&tickets::Model>,
    key: &str,
) -> String {
    let Some(t) = t else {
        return "Knowledge".into();
    };
    match Flow::load(db, tenant.workspace_id(), t).await {
        Ok(flow) => flow
            .rule(key)
            .or_else(|| flow.current.stage(key))
            .map_or_else(|| key.to_string(), |s| s.name.clone()),
        Err(_) => key.to_string(),
    }
}

async fn users_map(db: &DatabaseConnection, ids: Vec<Uuid>) -> Result<HashMap<Uuid, users::Model>> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    Ok(users::Entity::find()
        .filter(users::Column::Id.is_in(ids))
        .all(db)
        .await?
        .into_iter()
        .map(|u| (u.id, u))
        .collect())
}

async fn runners_map(
    db: &DatabaseConnection,
    tenant: &Tenant,
    ids: Vec<Uuid>,
) -> Result<HashMap<Uuid, runners::Model>> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    Ok(runners::Entity::find()
        .filter(runners::Column::WorkspaceId.eq(tenant.workspace_id().get()))
        .filter(runners::Column::Id.is_in(ids))
        .all(db)
        .await?
        .into_iter()
        .map(|r| (r.id, r))
        .collect())
}

/// [`JobView`] untuk pasangan job + tiket.
pub(crate) async fn job_views(
    db: &DatabaseConnection,
    tenant: &Tenant,
    rows: &[(jobs::Model, Option<tickets::Model>)],
) -> Result<Vec<JobView>> {
    let acting = acting(tenant, None);
    let labels: PersonaLabels = personas::Model::labels(db, tenant.workspace_id()).await?;
    let users = users_map(db, rows.iter().filter_map(|(j, _)| j.claimed_by).collect()).await?;
    let runner_map = runners_map(
        db,
        tenant,
        rows.iter().filter_map(|(j, _)| j.runner_id).collect(),
    )
    .await?;
    let now = chrono::Utc::now();
    let mut out = Vec::with_capacity(rows.len());
    for (job, ticket) in rows {
        let name = stage_name(db, tenant, ticket.as_ref(), &job.stage_key).await;
        let can_claim = svc::can_claim(db, &acting, job).await?;
        let is_admin = svc::is_project_admin(db, &acting, job).await;
        out.push(JobView::of(
            job,
            &JobContext {
                slug: &tenant.workspace.slug,
                ticket_code: ticket.as_ref().map_or("KB", |t| t.code.as_str()),
                ticket_title: ticket
                    .as_ref()
                    .map_or(job.prompt_note.as_str(), |t| t.title.as_str()),
                stage_name: name,
                labels: &labels,
                priority: ticket
                    .as_ref()
                    .and_then(|t| t.priority().ok())
                    .map_or(3, tickets::TicketPriority::rank),
                holder: job.claimed_by.and_then(|u| users.get(&u)),
                runner: job.runner_id.and_then(|r| runner_map.get(&r)),
                viewer: tenant.user.id,
                can_claim,
                is_admin,
                now,
            },
        ));
    }
    Ok(out)
}

async fn permission_views(
    db: &DatabaseConnection,
    tenant: &Tenant,
    rows: &[(job_permissions::Model, jobs::Model, Option<tickets::Model>)],
) -> Result<Vec<PermissionView>> {
    let labels = personas::Model::labels(db, tenant.workspace_id()).await?;
    let runner_map = runners_map(
        db,
        tenant,
        rows.iter().filter_map(|(_, j, _)| j.runner_id).collect(),
    )
    .await?;
    let deciders = users_map(
        db,
        rows.iter().filter_map(|(p, _, _)| p.decided_by).collect(),
    )
    .await?;
    let now = chrono::Utc::now();
    Ok(rows
        .iter()
        .map(|(p, j, t)| {
            PermissionView::of(
                p,
                j,
                t.as_ref().map_or(("KB", j.prompt_note.as_str()), |t| {
                    (t.code.as_str(), t.title.as_str())
                }),
                &PermissionContext {
                    slug: &tenant.workspace.slug,
                    labels: &labels,
                    runner: j.runner_id.and_then(|r| runner_map.get(&r)),
                    decided_by: p.decided_by.and_then(|u| deciders.get(&u)),
                    now,
                },
            )
        })
        .collect())
}

async fn runner_views(
    db: &DatabaseConnection,
    tenant: &Tenant,
    list: &[runners::Model],
) -> Result<Vec<RunnerView>> {
    let wid = tenant.workspace_id();
    let owners = users_map(db, list.iter().map(|r| r.user_id).collect()).await?;
    let ids: Vec<Uuid> = list.iter().map(|r| r.id).collect();
    let running = svc::running_on(db, wid, &ids).await?;
    let admin = project_access::is_admin_everywhere(&tenant.role);
    let mut out = Vec::new();
    for r in list {
        let Some(owner) = owners.get(&r.user_id) else {
            continue;
        };
        let job = match running.iter().find(|j| j.runner_id == Some(r.id)) {
            Some(j) => match j.ticket_id {
                Some(tid) => tickets::Model::find_in(db, wid, tid).await?.map(|t| t.code),
                None => Some("KB".into()),
            }
            .map(|code| RunnerJob {
                ticket: code,
                href: format!("/w/{}/jobs/{}", tenant.workspace.slug, j.id),
            }),
            None => None,
        };
        out.push(RunnerView::of(
            r,
            owner,
            job,
            &tenant.workspace.slug,
            tenant.user.id,
            r.user_id == tenant.user.id || admin,
        ));
    }
    Ok(out)
}

/// Runner milik pelaku di workspace ini (pilihan dialog Ambil job).
pub(crate) async fn my_runners(
    db: &DatabaseConnection,
    tenant: &Tenant,
) -> Result<Vec<RunnerView>> {
    let list = svc::runners_of(db, tenant.workspace_id(), Some(tenant.user.id)).await?;
    runner_views(db, tenant, &list).await
}

#[derive(Debug, serde::Serialize)]
struct ClientOption {
    key: &'static str,
    name: &'static str,
}

#[derive(Debug, serde::Serialize)]
struct TerminalRow {
    os: &'static str,
    label_key: &'static str,
    value: String,
    options: &'static [&'static str],
}

/// Pilihan program terminal per OS (stage interaktif).
const TERMINALS: &[(&str, &str, &[&str])] = &[
    (
        "macos",
        "runner-os-macos",
        &["Terminal", "iTerm2", "Ghostty", "WezTerm"],
    ),
    (
        "linux",
        "runner-os-linux",
        &[
            "x-terminal-emulator",
            "gnome-terminal",
            "konsole",
            "kitty",
            "Ghostty",
        ],
    ),
    (
        "windows",
        "runner-os-windows",
        &["Windows Terminal", "PowerShell", "Git Bash"],
    ),
];

/// Panel bawaan: Permintaan izin bila ada yang menunggu, selain itu Runner saya.
#[debug_handler]
async fn index(State(ctx): State<AppContext>, tenant: Tenant) -> Result<Response> {
    let pending = svc::pending_permissions(&ctx.db, &acting(&tenant, None)).await?;
    let panel = if pending.is_empty() { "mine" } else { "perm" };
    format::redirect(&format!("{}/{panel}", base(&tenant.workspace.slug)))
}

/// `/runner` (dari halaman login perangkat): workspace pertama user.
#[debug_handler]
async fn first(State(ctx): State<AppContext>, CurrentUser(user): CurrentUser) -> Result<Response> {
    let list = workspace_members::Model::workspaces_of(&ctx.db, user.id).await?;
    match list.first() {
        Some((w, _)) => format::redirect(&base(&w.slug)),
        None => format::redirect("/"),
    }
}

#[debug_handler]
async fn page(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    tenant: Tenant,
    session: Session,
    Path((_, panel)): Path<(String, String)>,
    Query(query): Query<Vec<(String, String)>>,
) -> Result<Response> {
    if !PANELS.iter().any(|(k, ..)| *k == panel) {
        return Err(Error::NotFound);
    }
    let (session, flash) = flash::take(session);
    let db = &ctx.db;
    let wid = tenant.workspace_id();
    let slug = tenant.workspace.slug.clone();
    let act = acting(&tenant, None);
    let pending = svc::pending_permissions(db, &act).await?;
    let queue = svc::queue(db, &act).await?;
    let base = base(&slug);
    let subnav: Vec<NavItem> = PANELS
        .iter()
        .map(|(key, label, icon, desc)| {
            let mut item = NavItem::link(key, label, icon, &format!("{base}/{key}")).describe(desc);
            item.current = *key == panel;
            match *key {
                "perm" if !pending.is_empty() => item.counted(pending.len() as u64, true),
                "queue" if !queue.is_empty() => item.counted(queue.len() as u64, false),
                _ => item,
            }
        })
        .collect();
    let next = format!("{base}/{panel}");
    let mut data = serde_json::json!({
        "panel": panel,
        "subnav": subnav,
        "flash": flash,
        "next": next,
        "base": base,
    });
    match panel.as_str() {
        "perm" => {
            data["permissions"] =
                serde_json::to_value(permission_views(db, &tenant, &pending).await?)?;
        }
        "queue" => {
            let rows: Vec<(jobs::Model, Option<tickets::Model>)> = queue
                .iter()
                .map(|r| (r.job.clone(), r.ticket.clone()))
                .collect();
            let mut views = job_views(db, &tenant, &rows).await?;
            let state = QUEUE_TABLE.parse(&query);
            if let Some(q) = state.query() {
                let q = q.to_lowercase();
                views.retain(|j| {
                    j.ticket.to_lowercase().contains(&q)
                        || j.title.to_lowercase().contains(&q)
                        || j.stage_name.to_lowercase().contains(&q)
                });
            }
            let statuses = state.filter("status").to_vec();
            if !statuses.is_empty() {
                views.retain(|j| statuses.iter().any(|s| s == j.status));
            }
            if !state.filter("mine").is_empty() {
                views.retain(|j| j.mine || j.can_claim);
            }
            match state.sort {
                "status" => views.sort_by(|a, b| a.status.cmp(b.status)),
                "age" => views.sort_by_key(|j| std::cmp::Reverse(j.id)),
                _ => {}
            }
            if state.desc {
                views.reverse();
            }
            let total = views.len() as u64;
            let page: Vec<JobView> = views
                .into_iter()
                .skip(usize::try_from(state.offset()).unwrap_or(0))
                .take(usize::try_from(state.per).unwrap_or(25))
                .collect();
            let table = TableView::new(&QUEUE_TABLE, &state, &format!("{base}/queue"), total)
                .with_filter_options(
                    "status",
                    JobStatus::ALL
                        .iter()
                        .filter(|s| s.is_open())
                        .map(|s| FilterOption::key(s.as_str(), views::runner::status_badge(*s).0))
                        .collect(),
                )
                .with_filter_options("mine", vec![FilterOption::key("1", "job-filter-for-you")]);
            data["table"] = serde_json::to_value(table)?;
            data["jobs"] = serde_json::to_value(page)?;
            data["my_runners"] = serde_json::to_value(my_runners(db, &tenant).await?)?;
        }
        "mine" | "team" => {
            let list =
                svc::runners_of(db, wid, (panel == "mine").then_some(tenant.user.id)).await?;
            let views = runner_views(db, &tenant, &list).await?;
            let held: Vec<(jobs::Model, Option<tickets::Model>)> = queue
                .iter()
                .filter(|r| r.job.claimed_by == Some(tenant.user.id))
                .map(|r| (r.job.clone(), r.ticket.clone()))
                .collect();
            let held = job_views(db, &tenant, &held).await?;
            let online = views.iter().filter(|r| r.state != "offline").count();
            let stats = vec![
                Stat::new("runner-stat-online", online, "device-laptop").tone("accent"),
                Stat::new("runner-stat-held", held.len(), "player-play"),
                Stat::new("runner-stat-pending", pending.len(), "hand-stop").tone("warn"),
            ];
            data["runners"] = serde_json::to_value(views)?;
            data["held"] = serde_json::to_value(held)?;
            data["stats"] = serde_json::to_value(stats)?;
        }
        "settings" => {
            let prefs = runner_prefs::Model::of(db, wid, tenant.user.id).await?;
            let terminals: Vec<TerminalRow> = TERMINALS
                .iter()
                .map(|(os, label, options)| TerminalRow {
                    os,
                    label_key: label,
                    value: prefs.terminals[*os]
                        .as_str()
                        .unwrap_or(options[0])
                        .to_string(),
                    options,
                })
                .collect();
            data["prefs"] = serde_json::json!({
                "auto_claim": prefs.auto_claim,
                "max_parallel": prefs.max_parallel.map(|n| n.to_string()).unwrap_or_default(),
                "default_client": prefs.default_client,
                "quota_policy": sea_orm::ActiveEnum::to_value(&prefs.quota_policy),
            });
            data["terminals"] = serde_json::to_value(terminals)?;
            data["clients"] = serde_json::to_value(
                CLIENTS
                    .iter()
                    .map(|c| ClientOption {
                        key: c.key,
                        name: c.name,
                    })
                    .collect::<Vec<_>>(),
            )?;
            data["action"] = serde_json::Value::from(format!("{base}/prefs"));
        }
        "drift" => {
            let checks = rule_checks::Entity::find()
                .filter(rule_checks::Column::WorkspaceId.eq(wid.get()))
                .all(db)
                .await?;
            let checks: Vec<serde_json::Value> = checks
                .iter()
                .map(|c| {
                    serde_json::json!({
                        "repo": c.repo,
                        "ok": c.changed.is_empty(),
                        "changed": c.changed,
                        "at": views::time::wib_datetime(&c.updated_at),
                    })
                })
                .collect();
            data["checks"] = serde_json::Value::from(checks);
            // Backup aturan lama dari `agentgate init` di project yang boleh dibuka.
            let projects =
                crate::models::projects::Model::accessible_for(db, &tenant.accessor()).await?;
            let backups = crate::models::rule_backups::Entity::find()
                .filter(crate::models::rule_backups::Column::WorkspaceId.eq(wid.get()))
                .filter(
                    crate::models::rule_backups::Column::ProjectId
                        .is_in(projects.iter().map(|p| p.id).collect::<Vec<_>>()),
                )
                .all(db)
                .await?;
            let mut backups: Vec<serde_json::Value> = backups
                .iter()
                .rev()
                .take(30)
                .map(|b| {
                    serde_json::json!({
                        "archive": b.archive,
                        "repo": b.repo,
                        "project": projects.iter().find(|p| p.id == b.project_id).map(|p| p.name.clone()),
                        "files": b.files,
                        "kb": (b.size + 1023) / 1024,
                        "at": views::time::wib_datetime(&b.created_at),
                        "href": format!("/w/{}/backups/{}", tenant.workspace.slug, b.id),
                    })
                })
                .collect();
            backups.sort_by(|a, b| b["at"].as_str().cmp(&a["at"].as_str()));
            data["backups"] = serde_json::Value::from(backups);
        }
        _ => {
            let base_url = Settings::from_ctx(&ctx)?.public_base_url;
            data["server"] = serde_json::Value::from(base_url);
            data["version"] = serde_json::Value::from(agentgate_core::VERSION);
        }
    }
    let shell = app_shell(&ctx, &tenant.user, Some(&tenant), None)
        .await?
        .at("runner");
    let res = views::tickets::render(&v, &shell, "runner/page.html", StatusCode::OK, data)?;
    Ok((session, res).into_response())
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct PrefsForm {
    #[serde(default)]
    auto_claim: Option<String>,
    #[serde(default)]
    max_parallel: String,
    #[serde(default)]
    default_client: String,
    #[serde(default)]
    quota_policy: String,
    #[serde(default)]
    terminal_macos: String,
    #[serde(default)]
    terminal_linux: String,
    #[serde(default)]
    terminal_windows: String,
}

#[debug_handler]
async fn save_prefs(
    State(ctx): State<AppContext>,
    tenant: Tenant,
    session: Session,
    Form(form): Form<PrefsForm>,
) -> Result<Response> {
    let pick = |os: &str, value: &str| -> String {
        TERMINALS
            .iter()
            .find(|(k, ..)| *k == os)
            .and_then(|(_, _, options)| options.iter().find(|o| **o == value.trim()))
            .map_or_else(String::new, |o| (*o).to_string())
    };
    let prefs = runner_prefs::Prefs {
        auto_claim: form.auto_claim.is_some(),
        max_parallel: form
            .max_parallel
            .trim()
            .parse::<i32>()
            .ok()
            .filter(|n| (1..=20).contains(n)),
        default_client: CLIENTS
            .iter()
            .find(|c| c.key == form.default_client.trim())
            .map_or("claude", |c| c.key)
            .to_string(),
        quota_policy: if form.quota_policy == "release" {
            QuotaPolicy::Release
        } else {
            QuotaPolicy::Wait
        },
        terminals: serde_json::json!({
            "macos": pick("macos", &form.terminal_macos),
            "linux": pick("linux", &form.terminal_linux),
            "windows": pick("windows", &form.terminal_windows),
        }),
    };
    svc::save_prefs(&ctx.db, &acting(&tenant, None), &prefs).await?;
    let session = flash::set(session, &Flash::success(views::runner::FLASH_PREFS))?;
    let to = format!("{}/settings", base(&tenant.workspace.slug));
    Ok((session, format::redirect(&to)?).into_response())
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct NextForm {
    #[serde(default)]
    next: String,
}

#[debug_handler]
async fn revoke_runner(
    State(ctx): State<AppContext>,
    tenant: Tenant,
    session: Session,
    RequestId(request_id): RequestId,
    id: IdOr404<runners::Entity>,
    Form(form): Form<NextForm>,
) -> Result<Response> {
    let r = svc::revoke_runner(&ctx.db, &acting(&tenant, request_id), id.id).await;
    let slug = &tenant.workspace.slug;
    let to = back(&form.next, slug, &format!("{}/mine", base(slug)));
    done(session, r, views::runner::FLASH_REVOKED, &to)
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct DecisionForm {
    #[serde(default)]
    decision: String,
    #[serde(default)]
    next: String,
}

#[debug_handler]
async fn decide_permission(
    State(ctx): State<AppContext>,
    tenant: Tenant,
    session: Session,
    RequestId(request_id): RequestId,
    id: IdOr404<job_permissions::Entity>,
    Form(form): Form<DecisionForm>,
) -> Result<Response> {
    let status = match form.decision.as_str() {
        "allow" => PermissionStatus::Allowed,
        "allow_ticket" => PermissionStatus::AllowedTicket,
        "deny" => PermissionStatus::Denied,
        _ => return Err(Error::BadRequest("decision".into())),
    };
    let r = svc::decide_permission(&ctx.db, &acting(&tenant, request_id), id.id, status).await;
    let slug = &tenant.workspace.slug;
    let to = back(&form.next, slug, &format!("{}/perm", base(slug)));
    done(session, r, views::runner::FLASH_PERMISSION, &to)
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct JobForm {
    #[serde(default)]
    next: String,
    #[serde(default)]
    note: String,
    #[serde(default)]
    runner: String,
    #[serde(default)]
    choice: String,
}

fn job_back(tenant: &Tenant, form: &JobForm, id: Uuid) -> String {
    let slug = &tenant.workspace.slug;
    back(&form.next, slug, &format!("/w/{slug}/jobs/{id}"))
}

#[debug_handler]
async fn claim(
    State(ctx): State<AppContext>,
    tenant: Tenant,
    session: Session,
    RequestId(request_id): RequestId,
    id: IdOr404<jobs::Entity>,
    Form(form): Form<JobForm>,
) -> Result<Response> {
    let runner = Uuid::try_parse(form.runner.trim()).ok();
    let r = svc::claim(&ctx.db, &acting(&tenant, request_id), id.id, runner).await;
    let to = job_back(&tenant, &form, id.id);
    done(session, r, views::runner::FLASH_CLAIMED, &to)
}

#[debug_handler]
async fn release(
    State(ctx): State<AppContext>,
    tenant: Tenant,
    session: Session,
    RequestId(request_id): RequestId,
    id: IdOr404<jobs::Entity>,
    Form(form): Form<JobForm>,
) -> Result<Response> {
    let r = svc::release(&ctx.db, &acting(&tenant, request_id), id.id, &form.note).await;
    let to = job_back(&tenant, &form, id.id);
    done(session, r, views::runner::FLASH_RELEASED, &to)
}

#[debug_handler]
async fn cancel(
    State(ctx): State<AppContext>,
    tenant: Tenant,
    session: Session,
    RequestId(request_id): RequestId,
    id: IdOr404<jobs::Entity>,
    Form(form): Form<JobForm>,
) -> Result<Response> {
    let r = svc::cancel(&ctx.db, &acting(&tenant, request_id), id.id).await;
    let to = job_back(&tenant, &form, id.id);
    done(session, r, views::runner::FLASH_CANCELLED, &to)
}

#[debug_handler]
async fn quota(
    State(ctx): State<AppContext>,
    tenant: Tenant,
    session: Session,
    RequestId(request_id): RequestId,
    id: IdOr404<jobs::Entity>,
    Form(form): Form<JobForm>,
) -> Result<Response> {
    let (choice, key) = if form.choice == "release" {
        (QuotaChoice::Release, views::runner::FLASH_RELEASED)
    } else {
        (QuotaChoice::Wait, views::runner::FLASH_QUOTA_WAIT)
    };
    let r = svc::decide_quota(&ctx.db, &acting(&tenant, request_id), id.id, choice).await;
    let to = job_back(&tenant, &form, id.id);
    done(session, r, key, &to)
}

/// Data bagian live halaman job (status, langkah, log, evidence, izin).
async fn live_data(
    ctx: &AppContext,
    tenant: &Tenant,
    job: &jobs::Model,
    ticket: &Option<tickets::Model>,
) -> Result<serde_json::Value> {
    let db = &ctx.db;
    let wid = tenant.workspace_id();
    let view = job_views(db, tenant, &[(job.clone(), ticket.clone())])
        .await?
        .pop()
        .ok_or(Error::NotFound)?;
    let logs: Vec<LogLineView> = svc::logs_of(db, wid, job.id, LOG_LINES)
        .await?
        .iter()
        .map(LogLineView::of)
        .collect();
    let evidence: Vec<EvidenceView> = evidence::of_job(db, wid, job.id)
        .await?
        .iter()
        .map(|e| EvidenceView::of(e, &tenant.workspace.slug))
        .collect();
    let perms = svc::permissions_of_job(db, wid, job.id).await?;
    let rows: Vec<_> = perms
        .into_iter()
        .map(|p| (p, job.clone(), ticket.clone()))
        .collect();
    let permissions = permission_views(db, tenant, &rows).await?;
    Ok(serde_json::json!({
        "job": view,
        "steps": StepsView::of(&job.steps()),
        "logs": logs,
        "evidence": evidence,
        "permissions": permissions,
        "live_href": format!("/w/{}/jobs/{}/live", tenant.workspace.slug, job.id),
    }))
}

#[debug_handler]
async fn show(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    tenant: Tenant,
    session: Session,
    id: IdOr404<jobs::Entity>,
) -> Result<Response> {
    let (session, flash) = flash::take(session);
    let (job, ticket) = match svc::visible(&ctx.db, &acting(&tenant, None), id.id).await {
        Ok(v) => v,
        Err(TicketFailure::Db(e)) => return Err(e.into()),
        Err(_) => return Err(Error::NotFound),
    };
    let project = projects::Model::find_in(&ctx.db, tenant.workspace_id(), job.project_id)
        .await?
        .ok_or(Error::NotFound)?;
    let mut data = live_data(&ctx, &tenant, &job, &ticket).await?;
    data["flash"] = serde_json::to_value(flash)?;
    data["project"] = serde_json::json!({
        "name": project.name,
        "key": project.key,
        "href": format!("/w/{}/p/{}", tenant.workspace.slug, project.key),
    });
    data["next"] = serde_json::Value::from(format!("/w/{}/jobs/{}", tenant.workspace.slug, job.id));
    data["my_runners"] = serde_json::to_value(my_runners(&ctx.db, &tenant).await?)?;
    let shell = app_shell(&ctx, &tenant.user, Some(&tenant), None).await?;
    let res = views::tickets::render(&v, &shell, "jobs/show.html", StatusCode::OK, data)?;
    Ok((session, res).into_response())
}

/// Bagian live (HTMX polling tiap 2 dtk selama job terbuka).
#[debug_handler]
async fn live(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    tenant: Tenant,
    id: IdOr404<jobs::Entity>,
) -> Result<Response> {
    let (job, ticket) = match svc::visible(&ctx.db, &acting(&tenant, None), id.id).await {
        Ok(v) => v,
        Err(TicketFailure::Db(e)) => return Err(e.into()),
        Err(_) => return Err(Error::NotFound),
    };
    let mut data = live_data(&ctx, &tenant, &job, &ticket).await?;
    data["next"] = serde_json::Value::from(format!("/w/{}/jobs/{}", tenant.workspace.slug, job.id));
    format::render().view(&v, "jobs/_live.html", data)
}

#[debug_handler]
async fn download_backup(
    State(ctx): State<AppContext>,
    tenant: Tenant,
    id: IdOr404<crate::models::rule_backups::Entity>,
) -> Result<Response> {
    use crate::models::rule_backups;
    let wid = tenant.workspace_id();
    let row = rule_backups::Entity::find_by_id(id.id)
        .filter(rule_backups::Column::WorkspaceId.eq(wid.get()))
        .one(&ctx.db)
        .await?
        .ok_or(Error::NotFound)?;
    let projects =
        crate::models::projects::Model::accessible_for(&ctx.db, &tenant.accessor()).await?;
    if !projects.iter().any(|p| p.id == row.project_id) {
        return Err(Error::NotFound);
    }
    let data: Vec<u8> = ctx
        .storage
        .download(std::path::Path::new(&row.storage_key))
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "backup tidak ada di storage");
            Error::NotFound
        })?;
    let mut res = Response::new(Body::from(data));
    let headers = res.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/gzip"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{}\"", row.archive))
            .unwrap_or(HeaderValue::from_static("attachment")),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    Ok(res)
}

#[debug_handler]
async fn download_evidence(
    State(ctx): State<AppContext>,
    tenant: Tenant,
    id: IdOr404<job_evidence::Entity>,
) -> Result<Response> {
    let wid = tenant.workspace_id();
    let row = job_evidence::Entity::find_by_id(id.id)
        .filter(job_evidence::Column::WorkspaceId.eq(wid.get()))
        .one(&ctx.db)
        .await?
        .ok_or(Error::NotFound)?;
    // Akses mengikuti job (tiket di project yang bisa dibuka pelaku).
    match svc::visible(&ctx.db, &acting(&tenant, None), row.job_id).await {
        Ok(_) => {}
        Err(TicketFailure::Db(e)) => return Err(e.into()),
        Err(_) => return Err(Error::NotFound),
    }
    let data: Vec<u8> = ctx
        .storage
        .download(std::path::Path::new(&row.storage_key))
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "evidence tidak ada di storage");
            Error::NotFound
        })?;
    let image = row.content_type.starts_with("image/");
    let content_type = if image {
        row.content_type.clone()
    } else {
        "text/plain; charset=utf-8".to_string()
    };
    let safe: String = row
        .name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let mut res = Response::new(Body::from(data));
    let headers = res.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&content_type)
            .unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("inline; filename=\"{safe}\""))
            .unwrap_or(HeaderValue::from_static("inline")),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=300"),
    );
    Ok(res)
}

pub fn routes() -> Routes {
    Routes::new()
        .add("/runner", get(first))
        .add("/w/{slug}/runner", get(index))
        .add("/w/{slug}/runner/prefs", post(save_prefs))
        .add("/w/{slug}/runner/runners/{id}/revoke", post(revoke_runner))
        .add("/w/{slug}/runner/permissions/{id}", post(decide_permission))
        .add("/w/{slug}/runner/{panel}", get(page))
        .add("/w/{slug}/jobs/{id}", get(show))
        .add("/w/{slug}/jobs/{id}/live", get(live))
        .add("/w/{slug}/jobs/{id}/claim", post(claim))
        .add("/w/{slug}/jobs/{id}/release", post(release))
        .add("/w/{slug}/jobs/{id}/cancel", post(cancel))
        .add("/w/{slug}/jobs/{id}/quota", post(quota))
        .add("/w/{slug}/evidence/{id}", get(download_evidence))
        .add("/w/{slug}/backups/{id}", get(download_backup))
}
