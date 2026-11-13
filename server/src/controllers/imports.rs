//! Pengaturan → **Impor CSV** (M11, `Require<ImportTickets>` = `ManageIntegrations`; project
//! tujuan butuh `EditProject`). Wizard tanpa JS, setiap langkah form POST + PRG, state di baris
//! `ticket_imports`:
//!
//! - `GET  …/settings/import-csv[/new]`: langkah 1 Unggah; `POST …/new` (multipart, ≤ 20 MB)
//!   → 303 ke langkah 2 (422 bila berkas ditolak);
//! - `GET|POST …/import-csv/{id}/mapping`: langkah 2 Pemetaan kolom + project tujuan;
//! - `GET …/{id}/preview`: langkah 3 Pratinjau (filter hasil, cari judul);
//!   `POST …/{id}/run`: mulai impor (worker) → 303 ke `…/{id}`;
//! - `GET …/{id}`: langkah 4 (progres, lalu hasil); draf diarahkan ke langkahnya;
//! - `POST …/{id}/cancel`: batalkan draf;
//! - `GET …/{id}/report.csv`: laporan baris dilewati (30 hari); `GET …/template.csv`;
//! - `GET …/import-csv/hist`: riwayat impor.
//!
//! Impor workspace lain atau tanpa izin → 404.
use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart},
    http::{HeaderValue, StatusCode, header},
};
use loco_rs::prelude::*;
use sea_orm::Iterable;
use serde::Serialize;
use uuid::Uuid;

use crate::{
    common::i18n,
    controllers::{
        extractors::{IdOr404, RequestId, Require, Tenant, require::ImportTickets},
        flash,
        shell::app_shell,
    },
    models::{
        projects,
        ticket_imports::{self, ImportField, ImportStatus, MAX_BYTES, SkippedRow},
    },
    services::{
        imports::{self as svc, csv_file, plan},
        tickets::TicketFailure,
        workspaces::Acting,
    },
    views::{
        self,
        datatable::{ColumnSpec, FilterOption, FilterSpec, TableSpec, TableView},
        flash::Flash,
        nav::NavItem,
        ui::{ComboOption, Stat, WizardStep},
    },
};

pub const FLASH_CANCELLED: &str = "import-flash-cancelled";
/// Batas body unggah: berkas + overhead multipart.
const BODY_LIMIT: usize = MAX_BYTES + 512 * 1024;

pub(crate) fn base(slug: &str) -> String {
    format!("/w/{slug}/settings/import-csv")
}

fn acting(tenant: &Tenant, request_id: Option<String>) -> Acting<'_> {
    Acting {
        workspace: &tenant.workspace,
        user: &tenant.user,
        role: &tenant.role,
        request_id,
    }
}

pub static PREVIEW_TABLE: TableSpec = TableSpec {
    id: "import-preview",
    columns: &[
        ColumnSpec::new("import-col-row"),
        ColumnSpec::new("import-col-title"),
        ColumnSpec::new("import-col-type"),
        ColumnSpec::new("import-col-stage").hide("sm"),
        ColumnSpec::new("import-col-note"),
    ],
    filters: &[FilterSpec {
        key: "res",
        label_key: "import-res",
        icon: "filter",
    }],
    default_sort: "row",
    default_desc: false,
    default_per: 25,
    search_key: Some("import-search"),
};

pub static HISTORY_TABLE: TableSpec = TableSpec {
    id: "import-hist",
    columns: &[
        ColumnSpec::new("import-col-file"),
        ColumnSpec::new("import-col-project").hide("sm"),
        ColumnSpec::new("import-col-result"),
        ColumnSpec::new("import-col-actions").right(),
    ],
    filters: &[],
    default_sort: "created",
    default_desc: true,
    default_per: 10,
    search_key: Some("import-hist-search"),
};

fn step_labels() -> Vec<String> {
    [
        "import-step-upload",
        "import-step-mapping",
        "import-step-preview",
        "import-step-import",
    ]
    .iter()
    .map(|k| i18n::text(k, &[]))
    .collect()
}

/// Langkah wizard; langkah yang sudah lewat bisa diklik selama impor masih draf.
fn steps(slug: &str, current: usize, import: Option<&ticket_imports::Model>) -> Vec<WizardStep> {
    let b = base(slug);
    let hrefs: Vec<String> = match import {
        Some(i) if i.status().is_draft() => vec![
            format!("{b}/new"),
            format!("{b}/{}/mapping", i.id),
            format!("{b}/{}/preview", i.id),
        ],
        _ => Vec::new(),
    };
    WizardStep::sequence(&step_labels(), current, &hrefs)
}

async fn subnav(ctx: &AppContext, tenant: &Tenant, current: &str) -> Result<Vec<NavItem>> {
    let b = base(&tenant.workspace.slug);
    let count = ticket_imports::Model::history_count(&ctx.db, tenant.workspace_id()).await?;
    let mut items = vec![
        NavItem::link("new", "import-nav-new", "file-import", &format!("{b}/new")),
        NavItem::link("hist", "import-nav-hist", "history", &format!("{b}/hist"))
            .counted(count, false),
    ];
    for i in &mut items {
        i.current = i.key == current;
    }
    Ok(items)
}

async fn page(
    ctx: &AppContext,
    v: &TeraView,
    tenant: &Tenant,
    nav: &str,
    status: StatusCode,
    mut data: serde_json::Value,
) -> Result<Response> {
    data["subnav"] = serde_json::to_value(subnav(ctx, tenant, nav).await?)?;
    data["base"] = serde_json::Value::String(base(&tenant.workspace.slug));
    if data.get("flash").is_none() {
        data["flash"] = serde_json::Value::Null;
    }
    let shell = app_shell(ctx, &tenant.user, Some(tenant), None)
        .await?
        .at("import_csv");
    views::tickets::render(v, &shell, "imports/page.html", status, data)
}

/// Kode error pertama dari kegagalan validasi.
fn first_error(errors: &loco_rs::validation::ModelValidationErrors) -> (String, String) {
    errors
        .errors
        .iter()
        .find_map(|(f, es)| es.first().map(|e| (f.to_string(), e.code.to_string())))
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------------------------
// Langkah 1: Unggah

async fn render_upload(
    ctx: &AppContext,
    v: &TeraView,
    tenant: &Tenant,
    flash: Option<Flash>,
    error: &str,
) -> Result<Response> {
    let status = if error.is_empty() {
        StatusCode::OK
    } else {
        StatusCode::UNPROCESSABLE_ENTITY
    };
    page(
        ctx,
        v,
        tenant,
        "new",
        status,
        serde_json::json!({
            "panel": "upload",
            "step": 1,
            "steps": steps(&tenant.workspace.slug, 0, None),
            "flash": flash,
            "error": error,
        }),
    )
    .await
}

#[debug_handler(state = AppContext)]
async fn index(access: Require<ImportTickets>) -> Result<Response> {
    format::redirect(&format!("{}/new", base(&access.tenant.workspace.slug)))
}

#[debug_handler]
async fn new(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ImportTickets>,
    session: Session,
) -> Result<Response> {
    let (session, flash) = flash::take(session);
    let res = render_upload(&ctx, &v, &access.tenant, flash, "").await?;
    Ok((session, res).into_response())
}

async fn file_field(mut multipart: Multipart) -> std::result::Result<(String, Vec<u8>), ()> {
    while let Some(field) = multipart.next_field().await.map_err(|_| ())? {
        if field.name() == Some("file") {
            let name = field.file_name().unwrap_or_default().to_string();
            let data = field.bytes().await.map_err(|_| ())?;
            return Ok((name, data.to_vec()));
        }
    }
    Ok((String::new(), Vec::new()))
}

/// `Require` sebelum `Multipart`: tanpa izin → 404 sebelum body dibaca.
#[debug_handler]
async fn upload(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ImportTickets>,
    RequestId(request_id): RequestId,
    multipart: Multipart,
) -> Result<Response> {
    let tenant = &access.tenant;
    let Ok((name, data)) = file_field(multipart).await else {
        // Body terpotong karena melebihi batas.
        return render_upload(&ctx, &v, tenant, None, csv_file::CSV_TOO_LARGE).await;
    };
    match svc::upload(&ctx, &acting(tenant, request_id), &name, &data).await {
        Ok(row) => format::redirect(&format!(
            "{}/{}/mapping",
            base(&tenant.workspace.slug),
            row.id
        )),
        Err(TicketFailure::Invalid(errors)) => {
            let (_, code) = first_error(&errors);
            render_upload(&ctx, &v, tenant, None, &code).await
        }
        Err(e) => Err(e.into()),
    }
}

#[debug_handler(state = AppContext)]
async fn template(_access: Require<ImportTickets>) -> Result<Response> {
    let mut res = Response::new(Body::from(svc::TEMPLATE));
    let headers = res.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/csv; charset=utf-8"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"template-impor-agentgate.csv\""),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    Ok(res)
}

// ---------------------------------------------------------------------------------------------
// Langkah 2: Pemetaan kolom

/// Satu baris tabel pemetaan.
#[derive(Debug, Serialize)]
struct MapRow {
    index: usize,
    header: String,
    example: String,
    field: &'static str,
}

fn field_options() -> Vec<ComboOption> {
    let mut out: Vec<ComboOption> = ImportField::iter()
        .filter(|f| *f != ImportField::Skip)
        .map(|f| {
            ComboOption::new(
                f.key(),
                i18n::text(&format!("import-field-{}", f.key()), &[]),
            )
        })
        .collect();
    out.push(
        ComboOption::new(
            ImportField::Skip.key(),
            i18n::text("import-field-skip", &[]),
        )
        .group_key("import-field-group-other"),
    );
    out
}

async fn project_options(ctx: &AppContext, tenant: &Tenant) -> Result<Vec<ComboOption>> {
    Ok(projects::Model::accessible_for(&ctx.db, &tenant.accessor())
        .await?
        .iter()
        .filter(|p| p.archived_at.is_none())
        .map(|p| {
            ComboOption::new(p.id, &p.name).sub(&p.key).color(
                p.color
                    .clone()
                    .unwrap_or_else(|| views::palette::color_for(&p.key).to_string()),
            )
        })
        .collect())
}

/// Contoh nilai per kolom: nilai tidak kosong pertama dari beberapa baris awal.
async fn examples(ctx: &AppContext, row: &ticket_imports::Model) -> Vec<String> {
    let headers = row.headers();
    let Some(key) = &row.source_key else {
        return vec![String::new(); headers.len()];
    };
    let Ok(bytes) = ctx
        .storage
        .download::<Vec<u8>>(std::path::Path::new(key))
        .await
    else {
        return vec![String::new(); headers.len()];
    };
    let Ok(file) = csv_file::parse(&bytes) else {
        return vec![String::new(); headers.len()];
    };
    (0..headers.len())
        .map(|i| {
            file.rows
                .iter()
                .take(20)
                .filter_map(|r| r.get(i))
                .find(|v| !v.trim().is_empty())
                .map(|v| v.chars().take(80).collect())
                .unwrap_or_default()
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
async fn render_mapping(
    ctx: &AppContext,
    v: &TeraView,
    tenant: &Tenant,
    row: &ticket_imports::Model,
    mapping: &[ImportField],
    project: Option<Uuid>,
    error: (String, String),
    status: StatusCode,
) -> Result<Response> {
    let ex = examples(ctx, row).await;
    let rows: Vec<MapRow> = row
        .headers()
        .into_iter()
        .enumerate()
        .map(|(i, header)| MapRow {
            index: i,
            header,
            example: ex.get(i).cloned().unwrap_or_default(),
            field: mapping.get(i).copied().unwrap_or_default().key(),
        })
        .collect();
    let auto = mapping.iter().filter(|f| **f != ImportField::Skip).count();
    let delimiter = sea_orm::ActiveEnum::to_value(&row.delimiter());
    page(
        ctx,
        v,
        tenant,
        "new",
        status,
        serde_json::json!({
            "panel": "mapping",
            "step": 2,
            "steps": steps(&tenant.workspace.slug, 1, Some(row)),
            "import": { "id": row.id, "filename": row.filename, "rows": row.row_count, "cols": rows.len(), "delimiter": delimiter },
            "map_rows": rows,
            "auto_mapped": auto,
            "field_options": field_options(),
            "project_options": project_options(ctx, tenant).await?,
            "project": project.map(|p| p.to_string()).unwrap_or_default(),
            "action": format!("{}/{}/mapping", base(&tenant.workspace.slug), row.id),
            "cancel_action": format!("{}/{}/cancel", base(&tenant.workspace.slug), row.id),
            "error_field": error.0,
            "error_code": error.1,
        }),
    )
    .await
}

async fn draft(
    ctx: &AppContext,
    tenant: &Tenant,
    id: Uuid,
) -> Result<std::result::Result<ticket_imports::Model, Response>> {
    let row = svc::find(&ctx.db, tenant.workspace_id(), id).await?;
    if row.status().is_draft() {
        Ok(Ok(row))
    } else {
        Ok(Err(format::redirect(&format!(
            "{}/{}",
            base(&tenant.workspace.slug),
            row.id
        ))?))
    }
}

#[debug_handler]
async fn mapping(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ImportTickets>,
    id: IdOr404<ticket_imports::Entity>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let row = match draft(&ctx, tenant, id.id).await? {
        Ok(r) => r,
        Err(res) => return Ok(res),
    };
    let m = row.mapping();
    render_mapping(
        &ctx,
        &v,
        tenant,
        &row,
        &m,
        row.project_id,
        Default::default(),
        StatusCode::OK,
    )
    .await
}

#[debug_handler]
async fn save_mapping(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ImportTickets>,
    RequestId(request_id): RequestId,
    id: IdOr404<ticket_imports::Entity>,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let row = match draft(&ctx, tenant, id.id).await? {
        Ok(r) => r,
        Err(res) => return Ok(res),
    };
    let n = row.headers().len();
    let mapping: Vec<ImportField> = (0..n)
        .map(|i| {
            pairs
                .iter()
                .find(|(k, _)| *k == format!("col_{i}"))
                .and_then(|(_, v)| ImportField::parse(v))
                .unwrap_or_default()
        })
        .collect();
    let project = pairs
        .iter()
        .find(|(k, _)| k == "project")
        .and_then(|(_, v)| Uuid::parse_str(v.trim()).ok());
    match svc::save_mapping(
        &ctx.db,
        &acting(tenant, request_id),
        row.id,
        project,
        mapping.clone(),
    )
    .await
    {
        Ok(saved) => format::redirect(&format!(
            "{}/{}/preview",
            base(&tenant.workspace.slug),
            saved.id
        )),
        Err(TicketFailure::Invalid(errors)) => {
            render_mapping(
                &ctx,
                &v,
                tenant,
                &row,
                &mapping,
                project,
                first_error(&errors),
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => Err(e.into()),
    }
}

// ---------------------------------------------------------------------------------------------
// Langkah 3: Pratinjau

/// Satu baris pratinjau.
#[derive(Debug, Serialize)]
struct PreviewRow {
    row: u32,
    title: String,
    type_key: String,
    stage: String,
    outcome: plan::Outcome,
    notes: Vec<String>,
}

fn note_text(code: &str, arg: &str) -> String {
    let (a, b) = arg.split_once('|').unwrap_or((arg, ""));
    i18n::text(code, &[("a", a), ("b", b)])
}

#[debug_handler]
async fn preview(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ImportTickets>,
    RequestId(request_id): RequestId,
    id: IdOr404<ticket_imports::Entity>,
    Query(query): Query<Vec<(String, String)>>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let row = match draft(&ctx, tenant, id.id).await? {
        Ok(r) => r,
        Err(res) => return Ok(res),
    };
    let b = base(&tenant.workspace.slug);
    let p = match svc::preview(&ctx, &acting(tenant, request_id), &row).await {
        Ok(p) => p,
        // Pemetaan belum lengkap: kembali ke langkah 2.
        Err(TicketFailure::Invalid(_) | TicketFailure::NotFound) => {
            return format::redirect(&format!("{b}/{}/mapping", row.id));
        }
        Err(e) => return Err(e.into()),
    };
    let state = PREVIEW_TABLE.parse(&query);
    let res_filter = state.filter("res").to_vec();
    let q = state.query().map(str::to_lowercase);
    let mut rows: Vec<&plan::RowPlan> = p
        .rows
        .iter()
        .filter(|r| {
            res_filter.is_empty()
                || res_filter.iter().any(|f| match r.outcome {
                    plan::Outcome::Ready => f == "ready",
                    plan::Outcome::Warning => f == "warning",
                    plan::Outcome::Skipped => f == "skipped",
                })
        })
        .filter(|r| {
            q.as_ref()
                .is_none_or(|q| r.title.to_lowercase().contains(q))
        })
        .collect();
    if state.desc {
        rows.reverse();
    }
    let total = rows.len() as u64;
    let page_rows: Vec<PreviewRow> = rows
        .into_iter()
        .skip(usize::try_from(state.offset()).unwrap_or(0))
        .take(usize::try_from(state.per).unwrap_or(25))
        .map(|r| PreviewRow {
            row: r.row,
            title: r.title.clone(),
            type_key: r.type_key.clone(),
            stage: r.stage_name.clone(),
            outcome: r.outcome,
            notes: r.notes.iter().map(|n| note_text(n.code, &n.arg)).collect(),
        })
        .collect();
    let table = TableView::new(
        &PREVIEW_TABLE,
        &state,
        &format!("{b}/{}/preview", row.id),
        total,
    )
    .with_filter_options(
        "res",
        vec![
            FilterOption::key("ready", "import-res-ready"),
            FilterOption::key("warning", "import-res-warning"),
            FilterOption::key("skipped", "import-res-skipped"),
        ],
    );
    let all = p.rows.len();
    let stats = vec![
        Stat::new("import-stat-ready", p.ready + p.warned, "circle-check")
            .tone("success")
            .hint("import-stat-ready-hint", all),
        Stat::new("import-stat-warn", p.warned, "alert-triangle")
            .tone(if p.warned > 0 { "warn" } else { "" })
            .hint("import-stat-warn-hint", ""),
        Stat::new("import-stat-skip", p.skipped, "circle-x")
            .tone(if p.skipped > 0 { "danger" } else { "" })
            .hint("import-stat-skip-hint", ""),
    ];
    let types = crate::models::ticket_types::Model::all_in(&ctx.db, tenant.workspace_id()).await?;
    let type_views: Vec<serde_json::Value> = types
        .iter()
        .map(|t| serde_json::json!({ "key": t.key, "name": t.name, "icon": t.icon, "color": t.color }))
        .collect();
    page(
        &ctx,
        &v,
        tenant,
        "new",
        StatusCode::OK,
        serde_json::json!({
            "panel": "preview",
            "step": 3,
            "steps": steps(&tenant.workspace.slug, 2, Some(&row)),
            "import": { "id": row.id, "filename": row.filename },
            "project_name": p.project.name,
            "stats": stats,
            "table": table,
            "rows": page_rows,
            "types": type_views,
            "importable": p.ready + p.warned,
            "back_href": format!("{b}/{}/mapping", row.id),
            "run_action": format!("{b}/{}/run", row.id),
            "cancel_action": format!("{b}/{}/cancel", row.id),
        }),
    )
    .await
}

#[debug_handler]
async fn run(
    State(ctx): State<AppContext>,
    access: Require<ImportTickets>,
    RequestId(request_id): RequestId,
    id: IdOr404<ticket_imports::Entity>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let b = base(&tenant.workspace.slug);
    match svc::start(&ctx, &acting(tenant, request_id), id.id).await {
        Ok(row) => format::redirect(&format!("{b}/{}", row.id)),
        // Belum lengkap / sudah diproses: kembali ke halaman impor (yang mengarahkan ke langkahnya).
        Err(TicketFailure::Invalid(_)) => format::redirect(&format!("{b}/{}", id.id)),
        Err(e) => Err(e.into()),
    }
}

#[debug_handler]
async fn cancel(
    State(ctx): State<AppContext>,
    access: Require<ImportTickets>,
    session: Session,
    RequestId(request_id): RequestId,
    id: IdOr404<ticket_imports::Entity>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let b = base(&tenant.workspace.slug);
    match svc::cancel(&ctx, &acting(tenant, request_id), id.id).await {
        Ok(()) => {
            let session = flash::set(session, &Flash::success(FLASH_CANCELLED))?;
            Ok((session, format::redirect(&format!("{b}/new"))?).into_response())
        }
        Err(TicketFailure::Invalid(_)) => format::redirect(&format!("{b}/{}", id.id)),
        Err(e) => Err(e.into()),
    }
}

// ---------------------------------------------------------------------------------------------
// Langkah 4: Impor (progres + hasil)

#[debug_handler]
async fn show(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ImportTickets>,
    id: IdOr404<ticket_imports::Entity>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let b = base(&tenant.workspace.slug);
    let row = svc::find(&ctx.db, tenant.workspace_id(), id.id).await?;
    if row.status().is_draft() {
        let to = if row.project_id.is_some() && svc::check_mapping(&row.mapping()).is_ok() {
            "preview"
        } else {
            "mapping"
        };
        return format::redirect(&format!("{b}/{}/{to}", row.id));
    }
    let project = match row.project_id {
        Some(pid) => projects::Model::find_in(&ctx.db, tenant.workspace_id(), pid).await?,
        None => None,
    };
    let pct = if row.row_count > 0 {
        row.processed * 100 / row.row_count
    } else {
        0
    };
    let now = chrono::Utc::now();
    page(
        &ctx,
        &v,
        tenant,
        "new",
        StatusCode::OK,
        serde_json::json!({
            "panel": "result",
            "step": 4,
            "steps": steps(&tenant.workspace.slug, if row.status() == ImportStatus::Done { 4 } else { 3 }, None),
            "import": {
                "id": row.id,
                "filename": row.filename,
                "status": sea_orm::ActiveEnum::to_value(&row.status()),
                "rows": row.row_count,
                "processed": row.processed,
                "imported": row.imported,
                "skipped": row.skipped,
                "warned": row.warned,
                "error": row.error,
                "pct": pct,
            },
            "pending": row.status().is_pending(),
            "project_name": project.as_ref().map(|p| p.name.clone()).unwrap_or_default(),
            "board_href": project.as_ref().map(|p| format!("/w/{}/p/{}/board", tenant.workspace.slug, p.key)),
            "report_href": row.report_available(now).then(|| format!("{b}/{}/report.csv", row.id)),
            "self_href": format!("{b}/{}", row.id),
            "new_href": format!("{b}/new"),
        }),
    )
    .await
}

fn reason_text(s: &SkippedRow) -> String {
    note_text(&s.reason, &s.arg)
}

#[debug_handler]
async fn report(
    State(ctx): State<AppContext>,
    access: Require<ImportTickets>,
    id: IdOr404<ticket_imports::Entity>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let row = svc::find(&ctx.db, tenant.workspace_id(), id.id).await?;
    if !row.report_available(chrono::Utc::now()) {
        return Err(Error::NotFound);
    }
    let body = svc::report_csv(&row, reason_text);
    let stem: String = row
        .filename
        .trim_end_matches(".csv")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let mut res = Response::new(Body::from(format!("\u{feff}{body}")));
    let headers = res.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/csv; charset=utf-8"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"laporan-{stem}.csv\""))
            .unwrap_or(HeaderValue::from_static("attachment")),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    Ok(res)
}

// ---------------------------------------------------------------------------------------------
// Riwayat

#[derive(Debug, Serialize)]
struct HistRow {
    id: Uuid,
    filename: String,
    by: String,
    at: String,
    project: Option<serde_json::Value>,
    status: String,
    imported: i32,
    skipped: i32,
    href: String,
    report_href: Option<String>,
}

#[debug_handler]
async fn history(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ImportTickets>,
    session: Session,
    Query(query): Query<Vec<(String, String)>>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let wid = tenant.workspace_id();
    let (session, flash) = flash::take(session);
    let b = base(&tenant.workspace.slug);
    let state = HISTORY_TABLE.parse(&query);
    let (rows, total) =
        ticket_imports::Model::history(&ctx.db, wid, state.query(), state.offset(), state.per)
            .await?;
    let people =
        views::transfer::people_by_id(&ctx.db, rows.iter().filter_map(|r| r.created_by).collect())
            .await?;
    let project_rows = {
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
        projects::Entity::find()
            .filter(projects::Column::WorkspaceId.eq(wid.get()))
            .filter(projects::Column::Id.is_in(rows.iter().filter_map(|r| r.project_id)))
            .all(&ctx.db)
            .await?
    };
    let now = chrono::Utc::now();
    let items: Vec<HistRow> = rows
        .iter()
        .map(|r| HistRow {
            id: r.id,
            filename: r.filename.clone(),
            by: r.created_by.and_then(|id| people.get(&id)).map(|p| p.name.clone()).unwrap_or_default(),
            at: views::time::wib_datetime(&r.created_at),
            project: r.project_id.and_then(|pid| project_rows.iter().find(|p| p.id == pid)).map(|p| {
                serde_json::json!({
                    "name": p.name,
                    "color": p.color.clone().unwrap_or_else(|| views::palette::color_for(&p.key).to_string()),
                })
            }),
            status: sea_orm::ActiveEnum::to_value(&r.status()),
            imported: r.imported,
            skipped: r.skipped,
            href: format!("{b}/{}", r.id),
            report_href: r.report_available(now).then(|| format!("{b}/{}/report.csv", r.id)),
        })
        .collect();
    let table = TableView::new(&HISTORY_TABLE, &state, &format!("{b}/hist"), total);
    let res = page(
        &ctx,
        &v,
        tenant,
        "hist",
        StatusCode::OK,
        serde_json::json!({ "panel": "hist", "flash": flash, "table": table, "rows": items }),
    )
    .await?;
    Ok((session, res).into_response())
}

pub fn routes() -> Routes {
    Routes::new()
        .add("/w/{slug}/settings/import-csv", get(index))
        .add(
            "/w/{slug}/settings/import-csv/new",
            get(new)
                .post(upload)
                .layer(DefaultBodyLimit::max(BODY_LIMIT)),
        )
        .add("/w/{slug}/settings/import-csv/template.csv", get(template))
        .add("/w/{slug}/settings/import-csv/hist", get(history))
        .add("/w/{slug}/settings/import-csv/{id}", get(show))
        .add(
            "/w/{slug}/settings/import-csv/{id}/mapping",
            get(mapping).post(save_mapping),
        )
        .add("/w/{slug}/settings/import-csv/{id}/preview", get(preview))
        .add("/w/{slug}/settings/import-csv/{id}/run", post(run))
        .add("/w/{slug}/settings/import-csv/{id}/cancel", post(cancel))
        .add("/w/{slug}/settings/import-csv/{id}/report.csv", get(report))
}
