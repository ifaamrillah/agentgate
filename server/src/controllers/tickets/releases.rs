//! Project → Rilis (Fix version): daftar versi (datatable: cari, filter status, sort), detail
//! rilis (angka, release notes, tiket, rilis sekarang, edit/arsip/hapus), dan editor release
//! notes berversi (simpan = versi baru, pulihkan versi lama, terbitkan, susun dengan AI lewat job
//! persona `writer`). Melihat butuh `ViewProject`, mengubah `EditProject`.
use axum::http::StatusCode;
use loco_rs::prelude::*;
use serde::Serialize;
use uuid::Uuid;

use super::{acting, lookup, project_paths, summary};
use crate::{
    controllers::{
        extractors::{ProjectAccess, RequestId},
        flash,
        shell::app_shell,
    },
    models::{
        permission::Permission,
        release_note_versions::{self, VersionSource},
        releases::{self as model, ReleaseStatus},
        tickets,
    },
    services::{
        planning::{
            self, Failure, estimates,
            releases::{self as service, ReleaseInput},
        },
        tickets::TicketFailure,
    },
    views::{
        self,
        datatable::{ColumnSpec, FilterOption, FilterSpec, TableSpec, TableView},
        flash::Flash,
        form::{FieldErrors, field_errors},
        nav,
        planning as pv,
        projects::PROJECT_VIEWS_ACTION,
        tickets::{Lookup, TicketRow},
        ui::Stat,
    },
};

const FIELDS: &[&str] = &["name", "description", "starts_on", "target_on"];

pub const FLASH_CREATED: &str = "release-flash-created";
pub const FLASH_SAVED: &str = "release-flash-saved";
pub const FLASH_DELETED: &str = "release-flash-deleted";
pub const FLASH_RELEASED: &str = "release-flash-released";
pub const FLASH_ARCHIVED: &str = "release-flash-archived";
pub const FLASH_REOPENED: &str = "release-flash-reopened";
pub const FLASH_NOTES_SAVED: &str = "release-flash-notes-saved";
pub const FLASH_NOTES_RESTORED: &str = "release-flash-notes-restored";
pub const FLASH_NOTES_PUBLISHED: &str = "release-flash-notes-published";
pub const FLASH_NOTES_UNPUBLISHED: &str = "release-flash-notes-unpublished";
pub const FLASH_NOTES_REQUESTED: &str = "release-flash-notes-requested";
/// Job release notes AI masih berjalan.
pub const FLASH_NOTES_BUSY: &str = "release-flash-notes-busy";

/// Status rilis: (nilai, kunci Fluent, varian badge).
const STATUSES: &[(&str, &str, &str)] = &[
    ("planned", "release-status-planned", "neutral"),
    ("unreleased", "release-status-unreleased", "info"),
    ("released", "release-status-released", "success"),
    ("archived", "release-status-archived", "neutral"),
];

/// Kunci Fluent yang dibangun di kode (dites kontrak i18n).
#[must_use]
pub fn label_keys() -> Vec<String> {
    [
        FLASH_CREATED,
        FLASH_SAVED,
        FLASH_DELETED,
        FLASH_RELEASED,
        FLASH_ARCHIVED,
        FLASH_REOPENED,
        FLASH_NOTES_SAVED,
        FLASH_NOTES_RESTORED,
        FLASH_NOTES_PUBLISHED,
        FLASH_NOTES_UNPUBLISHED,
        FLASH_NOTES_REQUESTED,
        FLASH_NOTES_BUSY,
        "release-stat-tickets",
        "release-stat-tickets-hint",
        "release-stat-done",
        "release-stat-done-hint",
        "release-stat-points",
        "release-stat-points-hint",
        "release-stat-forecast",
        "release-stat-forecast-hint",
        "release-stat-forecast-late",
        "release-stat-forecast-none",
        "release-stat-forecast-ready",
        "release-stat-released",
        "release-notes-none",
        "release-notes-draft",
        "release-notes-published",
        "release-version-ai",
        "release-version-manual",
    ]
    .iter()
    .chain(STATUSES.iter().map(|(_, k, _)| k))
    .map(|k| (*k).to_string())
    .collect()
}

static RELEASES_TABLE: TableSpec = TableSpec {
    id: "releases",
    columns: &[
        ColumnSpec::new("release-col-version").sort("name"),
        ColumnSpec::new("release-col-status").sort("status"),
        ColumnSpec::new("release-col-progress").sort("progress"),
        ColumnSpec::new("release-col-target").sort("target").hide("sm"),
        ColumnSpec::new("release-col-notes").hide("sm"),
    ],
    filters: &[FilterSpec {
        key: "status",
        label_key: "release-col-status",
        icon: "rocket",
    }],
    default_sort: "target",
    default_desc: true,
    default_per: 25,
    search_key: Some("release-search"),
};

fn base(access: &ProjectAccess) -> String {
    format!("{}/releases", project_paths(access).home)
}

fn value(fields: &[(String, String)], name: &str) -> String {
    fields
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.clone())
        .unwrap_or_default()
}

fn input(fields: &[(String, String)]) -> ReleaseInput {
    ReleaseInput {
        name: value(fields, "name"),
        description: value(fields, "description"),
        starts_on: value(fields, "starts_on"),
        target_on: value(fields, "target_on"),
    }
}

fn status_of(status: ReleaseStatus) -> (&'static str, &'static str, &'static str) {
    let key = match status {
        ReleaseStatus::Planned => "planned",
        ReleaseStatus::Unreleased => "unreleased",
        ReleaseStatus::Released => "released",
        ReleaseStatus::Archived => "archived",
    };
    STATUSES
        .iter()
        .copied()
        .find(|(k, ..)| *k == key)
        .unwrap_or(STATUSES[0])
}

/// Keadaan release notes: `none`, `draft`, `published`.
fn notes_state(r: &model::Model) -> &'static str {
    if r.notes.trim().is_empty() {
        "none"
    } else if r.notes_published {
        "published"
    } else {
        "draft"
    }
}

/// Isian form rilis untuk dialog.
#[derive(Debug, Serialize, Default)]
struct FormValues {
    name: String,
    description: String,
    starts_on: String,
    target_on: String,
}

impl From<&ReleaseInput> for FormValues {
    fn from(i: &ReleaseInput) -> Self {
        Self {
            name: i.name.clone(),
            description: i.description.clone(),
            starts_on: i.starts_on.clone(),
            target_on: i.target_on.clone(),
        }
    }
}

impl From<&model::Model> for FormValues {
    fn from(r: &model::Model) -> Self {
        Self {
            name: r.name.clone(),
            description: r.description.clone(),
            starts_on: r.starts_on.map(|d| d.to_string()).unwrap_or_default(),
            target_on: r.target_on.map(|d| d.to_string()).unwrap_or_default(),
        }
    }
}

/// Dialog yang gagal divalidasi.
struct Failed {
    errors: FieldErrors,
    input: ReleaseInput,
}

/// Ringkasan rilis (baris daftar dan kepala detail).
#[derive(Debug, Serialize)]
struct ReleaseView {
    id: Uuid,
    name: String,
    description: String,
    status: &'static str,
    status_key: &'static str,
    variant: &'static str,
    total: usize,
    done: usize,
    pct: usize,
    target: String,
    #[serde(skip)]
    target_sort: Option<chrono::NaiveDate>,
    notes_state: &'static str,
    href: String,
}

fn release_view(base: &str, r: &model::Model, tickets: &[tickets::Model]) -> ReleaseView {
    let status = service::effective_status(r, tickets);
    let (status, status_key, variant) = status_of(status);
    let total = tickets.len();
    let done = tickets.iter().filter(|t| t.status == "done").count();
    ReleaseView {
        id: r.id,
        name: r.name.clone(),
        description: r.description.clone(),
        status,
        status_key,
        variant,
        total,
        done,
        pct: if total == 0 { 0 } else { done * 100 / total },
        target: r.target_on.map(pv::long_day).unwrap_or_default(),
        target_sort: r.target_on,
        notes_state: notes_state(r),
        href: format!("{base}/{}", r.id),
    }
}

/// Rilis project yang dibuka di path; 404 bila bukan milik project ini.
async fn find(
    ctx: &AppContext,
    access: &ProjectAccess,
    id: Uuid,
) -> Result<model::Model> {
    model::Model::find_in(&ctx.db, access.tenant.workspace_id(), id)
        .await?
        .filter(|r| r.project_id == access.project.id)
        .ok_or(Error::NotFound)
}

fn shell_data(access: &ProjectAccess, current: &str) -> serde_json::Value {
    let home = project_paths(access).home;
    serde_json::json!({
        "project": summary(access),
        "views": nav::project_views(&home, &access.tenant.user.project_views, "releases"),
        "views_action": PROJECT_VIEWS_ACTION,
        "return_to": current,
        "can_edit": access.role.has(Permission::EditProject),
        "base": base(access),
    })
}

fn merge(mut a: serde_json::Value, b: serde_json::Value) -> serde_json::Value {
    if let (Some(a), serde_json::Value::Object(b)) = (a.as_object_mut(), b) {
        a.extend(b);
    }
    a
}

// ---------------------------------------------------------------------------------------------
// Daftar

async fn render_list(
    ctx: &AppContext,
    v: &TeraView,
    access: &ProjectAccess,
    pairs: &[(String, String)],
    flash: Option<Flash>,
    failed: Option<Failed>,
) -> Result<Response> {
    let db = &ctx.db;
    let wid = access.tenant.workspace_id();
    let base = base(access);
    let mut rows = Vec::new();
    for r in model::Model::of_project(db, wid, access.project.id).await? {
        let tickets = service::tickets_of(db, wid, r.id).await?;
        rows.push(release_view(&base, &r, &tickets));
    }
    let all = rows.len();
    let state = RELEASES_TABLE.parse(pairs);
    if let Some(q) = state.query() {
        let q = q.to_lowercase();
        rows.retain(|r| {
            r.name.to_lowercase().contains(&q) || r.description.to_lowercase().contains(&q)
        });
    }
    let statuses = state.filter("status").to_vec();
    if !statuses.is_empty() {
        rows.retain(|r| statuses.iter().any(|s| s == r.status));
    }
    match state.sort {
        "name" => rows.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
        "status" => rows.sort_by_key(|r| STATUSES.iter().position(|(k, ..)| *k == r.status)),
        "progress" => rows.sort_by_key(|r| r.pct),
        _ => rows.sort_by_key(|r| r.target_sort),
    }
    if state.desc {
        rows.reverse();
    }
    let total = rows.len() as u64;
    let page: Vec<ReleaseView> = rows
        .into_iter()
        .skip(usize::try_from(state.offset()).unwrap_or(0))
        .take(usize::try_from(state.per).unwrap_or(25))
        .collect();
    let table = TableView::new(&RELEASES_TABLE, &state, &base, total).with_filter_options(
        "status",
        STATUSES
            .iter()
            .map(|(k, label, _)| FilterOption::key(k, label))
            .collect(),
    );
    let status = if failed.is_some() {
        StatusCode::UNPROCESSABLE_ENTITY
    } else {
        StatusCode::OK
    };
    let (errors, form) = match failed {
        Some(f) => (f.errors, Some(FormValues::from(&f.input))),
        None => (field_errors(FIELDS, None), None),
    };
    let shell = app_shell(ctx, &access.tenant.user, Some(&access.tenant), Some(access))
        .await?
        .at("project_releases");
    let data = merge(
        shell_data(access, &base),
        serde_json::json!({
            "table": table,
            "releases": page,
            "all": all,
            "new_open": form.is_some(),
            "new_input": form.unwrap_or_default(),
            "errors": errors,
            "flash": flash,
        }),
    );
    views::tickets::render(v, &shell, "tickets/releases.html", status, data)
}

#[debug_handler]
pub(super) async fn index(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: ProjectAccess,
    session: Session,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Result<Response> {
    access.require(Permission::ViewProject)?;
    let (session, flash) = flash::take(session);
    let res = render_list(&ctx, &v, &access, &pairs, flash, None).await?;
    Ok((session, res).into_response())
}

#[debug_handler]
pub(super) async fn create(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: ProjectAccess,
    session: Session,
    RequestId(request_id): RequestId,
    Form(fields): Form<Vec<(String, String)>>,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    let input = input(&fields);
    match service::create(
        &ctx.db,
        &acting(&access.tenant, request_id),
        access.project.id,
        &input,
    )
    .await
    {
        Ok(r) => {
            let session = flash::set(session, &Flash::success(FLASH_CREATED))?;
            Ok((session, format::redirect(&format!("{}/{}", base(&access), r.id))?).into_response())
        }
        Err(TicketFailure::Invalid(errors)) => {
            let failed = Failed {
                errors: field_errors(FIELDS, Some(&errors)),
                input,
            };
            let res = render_list(&ctx, &v, &access, &[], None, Some(failed)).await?;
            Ok((session, res).into_response())
        }
        Err(e) => Err(e.into()),
    }
}

// ---------------------------------------------------------------------------------------------
// Detail

#[derive(Debug, Serialize)]
struct TicketLine {
    #[serde(flatten)]
    row: TicketRow,
    stage: String,
    done: bool,
}

#[derive(Debug, Serialize)]
struct Target {
    id: Uuid,
    name: String,
}

fn ticket_lines(
    lookup: &Lookup,
    list: &[tickets::Model],
    stages: &std::collections::HashMap<String, String>,
) -> Vec<TicketLine> {
    list.iter()
        .map(|t| TicketLine {
            row: lookup.row(t),
            stage: stages
                .get(&t.stage_key)
                .cloned()
                .unwrap_or_else(|| t.stage_key.clone()),
            done: t.status == "done",
        })
        .collect()
}

async fn stage_names(
    ctx: &AppContext,
    access: &ProjectAccess,
    list: &[tickets::Model],
) -> Result<std::collections::HashMap<String, String>> {
    Ok(super::board::project_definitions(ctx, access, list)
        .await?
        .iter()
        .flat_map(|d| d.stages.iter().map(|s| (s.key.clone(), s.name.clone())))
        .collect())
}

/// Hal ihwal satu rilis untuk halaman detail dan editor notes.
struct Loaded {
    release: model::Model,
    tickets: Vec<tickets::Model>,
    view: ReleaseView,
    ai_running: bool,
    paths: serde_json::Value,
}

async fn load(ctx: &AppContext, access: &ProjectAccess, id: Uuid) -> Result<Loaded> {
    let r = find(ctx, access, id).await?;
    let wid = access.tenant.workspace_id();
    let tickets = service::tickets_of(&ctx.db, wid, r.id).await?;
    let base = base(access);
    let view = release_view(&base, &r, &tickets);
    let ai_running = service::open_notes_job(&ctx.db, wid, &access.project.key, &r)
        .await?
        .is_some();
    let href = view.href.clone();
    let paths = serde_json::json!({
        "show": href,
        "update": href,
        "delete": format!("{href}/delete"),
        "status": format!("{href}/status"),
        "release": format!("{href}/release"),
        "notes": format!("{href}/notes"),
        "notes_restore": format!("{href}/notes/restore"),
        "notes_publish": format!("{href}/notes/publish"),
        "notes_ai": format!("{href}/notes/ai"),
        "reports": format!("{}/reports", project_paths(access).home),
    });
    Ok(Loaded {
        release: r,
        tickets,
        view,
        ai_running,
        paths,
    })
}

#[allow(clippy::too_many_lines)]
async fn render_show(
    ctx: &AppContext,
    v: &TeraView,
    access: &ProjectAccess,
    id: Uuid,
    flash: Option<Flash>,
    failed: Option<Failed>,
) -> Result<Response> {
    let db = &ctx.db;
    let wid = access.tenant.workspace_id();
    let l = load(ctx, access, id).await?;
    let r = &l.release;
    let lookup = lookup(db, &access.tenant, &l.tickets, &[]).await?;
    let stages = stage_names(ctx, access, &l.tickets).await?;
    let lines = ticket_lines(&lookup, &l.tickets, &stages);

    // Angka.
    let points: i32 = l.tickets.iter().filter_map(|t| t.story_points).sum();
    let mut stats = vec![
        Stat::new("release-stat-tickets", l.view.total, "ticket")
            .hint("release-stat-tickets-hint", ""),
        Stat::new("release-stat-done", l.view.done, "circle-check")
            .tone("success")
            .hint("release-stat-done-hint", format!("{}%", l.view.pct)),
        Stat::new("release-stat-points", points, "chart-bar").hint("release-stat-points-hint", ""),
    ];
    let released = matches!(r.status(), ReleaseStatus::Released | ReleaseStatus::Archived);
    if released {
        stats.push(
            Stat::new("release-stat-forecast", "✓", "rocket")
                .tone("success")
                .hint(
                    "release-stat-released",
                    r.released_at
                        .map(|at| pv::long_day(planning::wib_date(&at)))
                        .unwrap_or_default(),
                ),
        );
    } else {
        let open: Vec<&tickets::Model> = l.tickets.iter().filter(|t| t.status != "done").collect();
        let ids: Vec<Uuid> = open.iter().map(|t| t.id).collect();
        let latest = estimates::latest(db, wid, &ids).await?;
        let calib = estimates::calibration(db, wid, access.project.id).await?;
        let hours: f64 = open
            .iter()
            .filter_map(|t| {
                estimates::remaining_hours(
                    t,
                    latest.get(&t.id).and_then(|e| e.current.as_ref()),
                    &calib,
                )
            })
            .sum();
        let stat = if open.is_empty() && !l.tickets.is_empty() {
            Stat::new("release-stat-forecast", "✓", "circle-check")
                .tone("success")
                .hint("release-stat-forecast-ready", "")
        } else if hours <= 0.0 {
            Stat::new("release-stat-forecast", "–", "calendar").hint("release-stat-forecast-none", "")
        } else {
            let finish = estimates::finish_date(planning::today(), hours);
            let late = r.target_on.is_some_and(|t| finish > t);
            Stat::new("release-stat-forecast", pv::day_month(finish), if late { "alert-triangle" } else { "calendar" })
                .tone(if late { "warn" } else { "" })
                .hint(
                    if late { "release-stat-forecast-late" } else { "release-stat-forecast-hint" },
                    r.target_on.map(pv::long_day).unwrap_or_default(),
                )
        };
        stats.push(stat);
    }

    // Tujuan pindah untuk "Rilis sekarang".
    let targets: Vec<Target> = model::Model::of_project(db, wid, access.project.id)
        .await?
        .into_iter()
        .filter(|x| {
            x.id != r.id && !matches!(x.status(), ReleaseStatus::Released | ReleaseStatus::Archived)
        })
        .map(|x| Target {
            id: x.id,
            name: x.name,
        })
        .collect();
    let left = l.tickets.iter().filter(|t| t.status != "done").count();
    let status = if failed.is_some() {
        StatusCode::UNPROCESSABLE_ENTITY
    } else {
        StatusCode::OK
    };
    let (errors, form, edit_open) = match failed {
        Some(f) => (f.errors, FormValues::from(&f.input), true),
        None => (field_errors(FIELDS, None), FormValues::from(r), false),
    };
    let shell = app_shell(ctx, &access.tenant.user, Some(&access.tenant), Some(access))
        .await?
        .at("project_releases");
    let data = merge(
        shell_data(access, &l.view.href),
        serde_json::json!({
            "release": l.view,
            "released": released,
            "archived": r.status() == ReleaseStatus::Archived,
            "notes": r.notes,
            "notes_version": r.notes_version,
            "notes_ai": r.notes_source() == model::NotesSource::Ai,
            "notes_updated": r.notes_updated_at.as_ref().map(views::time::wib_datetime).unwrap_or_default(),
            "notes_published": r.notes_published,
            "ai_running": l.ai_running,
            "stats": stats,
            "tickets": lines,
            "left": left,
            "targets": targets,
            "paths": l.paths,
            "edit_open": edit_open,
            "edit_input": form,
            "errors": errors,
            "flash": flash,
        }),
    );
    views::tickets::render(v, &shell, "tickets/release.html", status, data)
}

#[debug_handler]
pub(super) async fn show(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: ProjectAccess,
    session: Session,
    Path((_slug, _key, id)): Path<(String, String, Uuid)>,
) -> Result<Response> {
    access.require(Permission::ViewProject)?;
    let (session, flash) = flash::take(session);
    let res = render_show(&ctx, &v, &access, id, flash, None).await?;
    Ok((session, res).into_response())
}

/// Hasil aksi: berhasil → flash sukses; error validasi → flash error kodenya; lain → error.
fn done<T>(
    session: Session,
    result: Result<T, Failure>,
    ok: &str,
    back: &str,
) -> Result<Response> {
    let flash = match result {
        Ok(_) => Flash::success(ok),
        Err(TicketFailure::Invalid(errors)) => {
            let code = errors
                .errors
                .values()
                .flat_map(|l| l.iter())
                .map(|e| e.code.to_string())
                .next()
                .unwrap_or_else(|| planning::TARGET_INVALID.to_string());
            Flash::error(&code)
        }
        Err(e) => return Err(e.into()),
    };
    let session = flash::set(session, &flash)?;
    Ok((session, format::redirect(back)?).into_response())
}

#[debug_handler]
pub(super) async fn update(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: ProjectAccess,
    session: Session,
    RequestId(request_id): RequestId,
    Path((_slug, _key, id)): Path<(String, String, Uuid)>,
    Form(fields): Form<Vec<(String, String)>>,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    let r = find(&ctx, &access, id).await?;
    let input = input(&fields);
    match service::update(&ctx.db, &acting(&access.tenant, request_id), r.id, &input).await {
        Err(TicketFailure::Invalid(errors)) => {
            let failed = Failed {
                errors: field_errors(FIELDS, Some(&errors)),
                input,
            };
            let res = render_show(&ctx, &v, &access, id, None, Some(failed)).await?;
            Ok((session, res).into_response())
        }
        other => done(session, other, FLASH_SAVED, &format!("{}/{id}", base(&access))),
    }
}

#[debug_handler]
pub(super) async fn delete(
    State(ctx): State<AppContext>,
    access: ProjectAccess,
    session: Session,
    RequestId(request_id): RequestId,
    Path((_slug, _key, id)): Path<(String, String, Uuid)>,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    let r = find(&ctx, &access, id).await?;
    let result = service::delete(&ctx.db, &acting(&access.tenant, request_id), r.id).await;
    done(session, result, FLASH_DELETED, &base(&access))
}

/// Rilis sekarang: `target` = id rilis tujuan tiket yang belum selesai atau `none` (dilepas);
/// `publish` = terbitkan release notes sekaligus.
#[debug_handler]
pub(super) async fn release_now(
    State(ctx): State<AppContext>,
    access: ProjectAccess,
    session: Session,
    RequestId(request_id): RequestId,
    Path((_slug, _key, id)): Path<(String, String, Uuid)>,
    Form(fields): Form<Vec<(String, String)>>,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    let r = find(&ctx, &access, id).await?;
    let acting = acting(&access.tenant, request_id);
    let target = match value(&fields, "target").trim() {
        "" | "none" => None,
        raw => Some(Uuid::parse_str(raw).map_err(|_| Error::BadRequest("target".into()))?),
    };
    let back = format!("{}/{id}", base(&access));
    let result = service::release_now(&ctx.db, &acting, r.id, target).await;
    if result.is_ok() && !value(&fields, "publish").is_empty() && !r.notes.trim().is_empty() {
        service::publish_notes(&ctx.db, &acting, r.id, true).await?;
    }
    done(session, result, FLASH_RELEASED, &back)
}

/// Ubah keadaan: `archived` atau `unreleased` (buka arsip / batal rilis).
#[debug_handler]
pub(super) async fn set_status(
    State(ctx): State<AppContext>,
    access: ProjectAccess,
    session: Session,
    RequestId(request_id): RequestId,
    Path((_slug, _key, id)): Path<(String, String, Uuid)>,
    Form(fields): Form<Vec<(String, String)>>,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    let r = find(&ctx, &access, id).await?;
    let (status, ok) = match value(&fields, "status").as_str() {
        "archived" => (ReleaseStatus::Archived, FLASH_ARCHIVED),
        "unreleased" => (ReleaseStatus::Unreleased, FLASH_REOPENED),
        _ => return Err(Error::BadRequest("status".into())),
    };
    let result = service::set_status(&ctx.db, &acting(&access.tenant, request_id), r.id, status).await;
    done(session, result, ok, &format!("{}/{id}", base(&access)))
}

// ---------------------------------------------------------------------------------------------
// Release notes

#[derive(Debug, Serialize)]
struct VersionView {
    number: i32,
    ai: bool,
    who: String,
    at: String,
    current: bool,
}

async fn render_notes(
    ctx: &AppContext,
    v: &TeraView,
    access: &ProjectAccess,
    id: Uuid,
    flash: Option<Flash>,
    body: Option<String>,
) -> Result<Response> {
    let db = &ctx.db;
    let wid = access.tenant.workspace_id();
    let l = load(ctx, access, id).await?;
    let r = &l.release;
    let versions = release_note_versions::Model::of_release(db, wid, r.id, 30).await?;
    let people: Vec<Uuid> = versions.iter().filter_map(|x| x.author_id).collect();
    let lookup = lookup(db, &access.tenant, &l.tickets, &people).await?;
    let stages = stage_names(ctx, access, &l.tickets).await?;
    let version_views: Vec<VersionView> = versions
        .iter()
        .map(|x| VersionView {
            number: x.number,
            ai: x.source() == VersionSource::Ai,
            who: if x.source() == VersionSource::Ai {
                x.persona.clone()
            } else {
                lookup
                    .person(x.author_id)
                    .map(|p| p.name)
                    .unwrap_or_default()
            },
            at: views::time::wib_datetime(&x.created_at),
            current: x.number == r.notes_version,
        })
        .collect();
    let status = if body.is_some() {
        StatusCode::UNPROCESSABLE_ENTITY
    } else {
        StatusCode::OK
    };
    let shell = app_shell(ctx, &access.tenant.user, Some(&access.tenant), Some(access))
        .await?
        .at("project_releases");
    let data = merge(
        shell_data(access, &format!("{}/notes", l.view.href)),
        serde_json::json!({
            "release": l.view,
            "body": body.unwrap_or_else(|| r.notes.clone()),
            "notes_published": r.notes_published,
            "ai_running": l.ai_running,
            "versions": version_views,
            "tickets": ticket_lines(&lookup, &l.tickets, &stages),
            "paths": l.paths,
            "flash": flash,
        }),
    );
    views::tickets::render(v, &shell, "tickets/release_notes.html", status, data)
}

#[debug_handler]
pub(super) async fn notes(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: ProjectAccess,
    session: Session,
    Path((_slug, _key, id)): Path<(String, String, Uuid)>,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    let (session, flash) = flash::take(session);
    let res = render_notes(&ctx, &v, &access, id, flash, None).await?;
    Ok((session, res).into_response())
}

/// Simpan release notes (`body`) sebagai versi baru.
#[debug_handler]
pub(super) async fn save_notes(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: ProjectAccess,
    session: Session,
    RequestId(request_id): RequestId,
    Path((_slug, _key, id)): Path<(String, String, Uuid)>,
    Form(fields): Form<Vec<(String, String)>>,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    let r = find(&ctx, &access, id).await?;
    let body = value(&fields, "body");
    let result = service::save_notes(
        &ctx.db,
        &acting(&access.tenant, request_id),
        r.id,
        &body,
        VersionSource::Manual,
        "",
    )
    .await;
    match result {
        Err(TicketFailure::Invalid(errors))
            if errors.errors.values().flatten().any(|e| e.code == planning::NOTES_TOO_LONG) =>
        {
            let res = render_notes(
                &ctx,
                &v,
                &access,
                id,
                Some(Flash::error(planning::NOTES_TOO_LONG)),
                Some(body),
            )
            .await?;
            Ok((session, res).into_response())
        }
        other => done(session, other, FLASH_NOTES_SAVED, &format!("{}/{id}/notes", base(&access))),
    }
}

/// Pulihkan versi `number` sebagai versi baru.
#[debug_handler]
pub(super) async fn restore_notes(
    State(ctx): State<AppContext>,
    access: ProjectAccess,
    session: Session,
    RequestId(request_id): RequestId,
    Path((_slug, _key, id)): Path<(String, String, Uuid)>,
    Form(fields): Form<Vec<(String, String)>>,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    let r = find(&ctx, &access, id).await?;
    let number: i32 = value(&fields, "number")
        .trim()
        .parse()
        .map_err(|_| Error::BadRequest("number".into()))?;
    let result =
        service::restore_notes(&ctx.db, &acting(&access.tenant, request_id), r.id, number).await;
    if matches!(result, Err(TicketFailure::NotFound)) {
        return Err(Error::NotFound);
    }
    done(session, result, FLASH_NOTES_RESTORED, &format!("{}/{id}/notes", base(&access)))
}

/// Terbitkan (`published=1`) atau tarik terbitan release notes.
#[debug_handler]
pub(super) async fn publish_notes(
    State(ctx): State<AppContext>,
    access: ProjectAccess,
    session: Session,
    RequestId(request_id): RequestId,
    Path((_slug, _key, id)): Path<(String, String, Uuid)>,
    Form(fields): Form<Vec<(String, String)>>,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    let r = find(&ctx, &access, id).await?;
    let on = value(&fields, "published") == "1";
    if on && r.notes.trim().is_empty() {
        return Err(Error::BadRequest("notes".into()));
    }
    let result = service::publish_notes(&ctx.db, &acting(&access.tenant, request_id), r.id, on).await;
    let ok = if on { FLASH_NOTES_PUBLISHED } else { FLASH_NOTES_UNPUBLISHED };
    let back = match value(&fields, "back").as_str() {
        "notes" => format!("{}/{id}/notes", base(&access)),
        _ => format!("{}/{id}", base(&access)),
    };
    done(session, result, ok, &back)
}

/// Susun release notes dengan AI (job persona `writer`).
#[debug_handler]
pub(super) async fn request_ai(
    State(ctx): State<AppContext>,
    access: ProjectAccess,
    session: Session,
    RequestId(request_id): RequestId,
    Path((_slug, _key, id)): Path<(String, String, Uuid)>,
    Form(fields): Form<Vec<(String, String)>>,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    let r = find(&ctx, &access, id).await?;
    let result = service::request_ai_notes(
        &ctx.db,
        &acting(&access.tenant, request_id),
        r.id,
        &access.project.key,
    )
    .await;
    let back = match value(&fields, "back").as_str() {
        "notes" => format!("{}/{id}/notes", base(&access)),
        _ => format!("{}/{id}", base(&access)),
    };
    let session = match result {
        Ok(_) => flash::set(session, &Flash::success(FLASH_NOTES_REQUESTED))?,
        Err(TicketFailure::Invalid(_)) => flash::set(session, &Flash::error(FLASH_NOTES_BUSY))?,
        Err(e) => return Err(e.into()),
    };
    Ok((session, format::redirect(&back)?).into_response())
}
