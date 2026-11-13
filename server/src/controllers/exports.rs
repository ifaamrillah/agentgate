//! Pengaturan → **Ekspor data** (M11, Owner/Admin: `Require<ExportData>`):
//!
//! - `GET /w/{slug}/settings/export[/new]`: form Ekspor baru (isi, cakupan project, penerima);
//! - `POST /w/{slug}/settings/export/new`: minta ekspor → 303 ke Riwayat (422 bila isian salah);
//! - `GET /w/{slug}/settings/export/hist`: riwayat (status, progres, unduh, ulangi);
//! - `POST /w/{slug}/settings/export/{id}/download`: unduh ZIP (`attachment`, `no-store`).
//!   POST karena unduhan diaudit (GET tidak menulis, rules §6); form-nya `hx-boost="false"`;
//! - `POST /w/{slug}/settings/export/{id}/retry`: ulangi ekspor yang gagal.
//!
//! Tanpa izin atau ekspor workspace lain → 404.
use axum::{
    body::Body,
    http::{HeaderValue, StatusCode, header},
};
use loco_rs::prelude::*;
use sea_orm::Iterable;
use serde::Serialize;

use crate::{
    controllers::{
        extractors::{IdOr404, RequestId, Require, Tenant, require::ExportData},
        flash,
        shell::app_shell,
    },
    models::{
        data_exports::{self, ExportPart, ExportStatus},
        projects,
    },
    services::{
        exports::{self as svc, ExportForm},
        tickets::TicketFailure,
        workspaces::Acting,
    },
    views::{
        self,
        datatable::{ColumnSpec, FilterOption, FilterSpec, TableSpec, TableView},
        flash::Flash,
        nav::NavItem,
        ui::ComboOption,
    },
};

pub const FLASH_STARTED: &str = "export-flash-started";
pub const FLASH_RETRIED: &str = "export-flash-retried";

pub(crate) fn base(slug: &str) -> String {
    format!("/w/{slug}/settings/export")
}

fn acting(tenant: &Tenant, request_id: Option<String>) -> Acting<'_> {
    Acting {
        workspace: &tenant.workspace,
        user: &tenant.user,
        role: &tenant.role,
        request_id,
    }
}

pub static HISTORY_TABLE: TableSpec = TableSpec {
    id: "export-hist",
    columns: &[
        ColumnSpec::new("export-col-created"),
        ColumnSpec::new("export-col-by").hide("sm"),
        ColumnSpec::new("export-col-status"),
        ColumnSpec::new("export-col-size").hide("sm").right(),
        ColumnSpec::new("export-col-actions").right(),
    ],
    filters: &[FilterSpec {
        key: "status",
        label_key: "export-col-status",
        icon: "filter",
    }],
    default_sort: "created",
    default_desc: true,
    default_per: 10,
    search_key: None,
};

/// Satu baris riwayat.
#[derive(Debug, Serialize)]
struct ExportRow {
    id: uuid::Uuid,
    created: String,
    by: Option<views::tickets::PersonView>,
    status: &'static str,
    variant: &'static str,
    progress: i32,
    phase: Option<String>,
    size: String,
    expires: String,
    error: String,
    download_action: Option<String>,
    retry_action: Option<String>,
}

/// Opsi isi ekspor di form.
#[derive(Debug, Serialize)]
struct PartView {
    key: String,
    checked: bool,
}

fn status_badge(s: ExportStatus) -> (&'static str, &'static str) {
    match s {
        ExportStatus::Queued => ("queued", "neutral"),
        ExportStatus::Running => ("running", "info"),
        ExportStatus::Ready => ("ready", "success"),
        ExportStatus::Expired => ("expired", "neutral"),
        ExportStatus::Failed => ("failed", "destructive"),
    }
}

/// Ukuran berkas ringkas (KB/MB/GB, desimal koma).
fn human_size(bytes: Option<i64>) -> String {
    let Some(b) = bytes else {
        return "—".into();
    };
    #[allow(clippy::cast_precision_loss)]
    let b = b as f64;
    let (v, unit) = if b >= 1e9 {
        (b / 1e9, "GB")
    } else if b >= 1e6 {
        (b / 1e6, "MB")
    } else {
        (b / 1e3, "KB")
    };
    format!("{v:.1} {unit}").replace('.', ",")
}

async fn subnav(ctx: &AppContext, tenant: &Tenant, current: &str) -> Result<Vec<NavItem>> {
    let b = base(&tenant.workspace.slug);
    let count = data_exports::Model::count_in(&ctx.db, tenant.workspace_id()).await?;
    let mut items = vec![
        NavItem::link("new", "export-nav-new", "file-export", &format!("{b}/new")),
        NavItem::link("hist", "export-nav-hist", "history", &format!("{b}/hist"))
            .counted(count, false),
    ];
    for i in &mut items {
        i.current = i.key == current;
    }
    Ok(items)
}

/// Isian form yang ditolak (dirender ulang dengan 422).
#[derive(Debug, Default, Serialize)]
struct Rejected {
    parts: Vec<String>,
    projects: Vec<String>,
    recipient: String,
    field: String,
    code: String,
}

async fn render_new(
    ctx: &AppContext,
    v: &TeraView,
    tenant: &Tenant,
    flash: Option<Flash>,
    rejected: Option<Rejected>,
) -> Result<Response> {
    let wid = tenant.workspace_id();
    let project_rows = projects::Model::accessible_for(&ctx.db, &tenant.accessor()).await?;
    let mut project_options =
        vec![ComboOption::new("all", "").group_key("export-projects-group-all")];
    project_options[0].label = String::new();
    project_options.extend(project_rows.iter().map(|p| {
        ComboOption::new(p.id, &p.name).sub(&p.key).color(
            p.color
                .clone()
                .unwrap_or_else(|| views::palette::color_for(&p.key).to_string()),
        )
    }));
    let recipients: Vec<ComboOption> = svc::recipients(&ctx.db, wid)
        .await?
        .iter()
        .map(|u| ComboOption::person(u.id, &u.name, &u.email))
        .collect();
    let parts: Vec<PartView> = ExportPart::iter()
        .map(|p| PartView {
            key: p.key(),
            checked: rejected
                .as_ref()
                .map_or_else(|| p.default_on(), |r| r.parts.contains(&p.key())),
        })
        .collect();
    let status = if rejected.is_some() {
        StatusCode::UNPROCESSABLE_ENTITY
    } else {
        StatusCode::OK
    };
    let rejected = rejected.unwrap_or_default();
    let projects_value = if rejected.projects.is_empty() {
        vec!["all".to_string()]
    } else {
        rejected.projects.clone()
    };
    let recipient = if rejected.recipient.is_empty() {
        tenant.user.id.to_string()
    } else {
        rejected.recipient.clone()
    };
    let shell = app_shell(ctx, &tenant.user, Some(tenant), None)
        .await?
        .at("export");
    views::tickets::render(
        v,
        &shell,
        "exports/page.html",
        status,
        serde_json::json!({
            "panel": "new",
            "subnav": subnav(ctx, tenant, "new").await?,
            "flash": flash,
            "action": format!("{}/new", base(&tenant.workspace.slug)),
            "audit_href": format!("/w/{}/settings/audit", tenant.workspace.slug),
            "parts": parts,
            "project_options": project_options,
            "projects_value": projects_value,
            "recipient_options": recipients,
            "recipient": recipient,
            "error_field": rejected.field,
            "error_code": rejected.code,
            "workspace_name": tenant.workspace.name,
        }),
    )
}

#[debug_handler(state = AppContext)]
async fn index(access: Require<ExportData>) -> Result<Response> {
    format::redirect(&format!("{}/new", base(&access.tenant.workspace.slug)))
}

#[debug_handler]
async fn new(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ExportData>,
    session: Session,
) -> Result<Response> {
    let (session, flash) = flash::take(session);
    let res = render_new(&ctx, &v, &access.tenant, flash, None).await?;
    Ok((session, res).into_response())
}

/// `Require` sebelum body: tanpa izin → 404 sebelum isian dibaca.
#[debug_handler]
async fn create(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ExportData>,
    session: Session,
    RequestId(request_id): RequestId,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let values = |name: &str| -> Vec<String> {
        pairs
            .iter()
            .filter(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
            .collect()
    };
    let form = ExportForm {
        parts: values("parts"),
        projects: values("projects"),
        recipient: values("recipient").into_iter().next().unwrap_or_default(),
    };
    match svc::request(&ctx, &acting(tenant, request_id), &form).await {
        Ok(_) => {
            let session = flash::set(session, &Flash::success(FLASH_STARTED))?;
            Ok((
                session,
                format::redirect(&format!("{}/hist", base(&tenant.workspace.slug)))?,
            )
                .into_response())
        }
        Err(TicketFailure::Invalid(errors)) => {
            let (field, code) = errors
                .errors
                .iter()
                .find_map(|(f, es)| es.first().map(|e| (f.to_string(), e.code.to_string())))
                .unwrap_or_default();
            let rejected = Rejected {
                parts: form.parts,
                projects: form.projects,
                recipient: form.recipient,
                field,
                code,
            };
            render_new(&ctx, &v, tenant, None, Some(rejected)).await
        }
        Err(e) => Err(e.into()),
    }
}

#[debug_handler]
async fn history(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ExportData>,
    session: Session,
    Query(query): Query<Vec<(String, String)>>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let (session, flash) = flash::take(session);
    let b = base(&tenant.workspace.slug);
    let state = HISTORY_TABLE.parse(&query);
    let statuses: Vec<ExportStatus> = state
        .filter("status")
        .iter()
        .filter_map(|s| sea_orm::ActiveEnum::try_from_value(s).ok())
        .collect();
    let (rows, total) = data_exports::Model::newest_first(
        &ctx.db,
        tenant.workspace_id(),
        &statuses,
        state.offset(),
        state.per,
    )
    .await?;
    let people = views::transfer::people_by_id(
        &ctx.db,
        rows.iter().filter_map(|r| r.requested_by).collect(),
    )
    .await?;
    let now = chrono::Utc::now();
    let items: Vec<ExportRow> = rows
        .iter()
        .map(|r| {
            // `ready` yang lewat 7 hari tetapi belum disapu cleanup tampil sebagai kedaluwarsa.
            let st = if r.status() == ExportStatus::Ready && !r.downloadable(now) {
                ExportStatus::Expired
            } else {
                r.status()
            };
            let (status, variant) = status_badge(st);
            ExportRow {
                id: r.id,
                created: views::time::wib_datetime(&r.created_at),
                by: r.requested_by.and_then(|id| people.get(&id).cloned()),
                status,
                variant,
                progress: r.progress,
                phase: r.phase().map(|p| p.key()),
                size: human_size(r.size_bytes),
                expires: r
                    .expires_at
                    .map(|e| views::time::wib_datetime(&e))
                    .unwrap_or_default(),
                error: r.error.clone(),
                download_action: (st == ExportStatus::Ready)
                    .then(|| format!("{b}/{}/download", r.id)),
                retry_action: (st == ExportStatus::Failed).then(|| format!("{b}/{}/retry", r.id)),
            }
        })
        .collect();
    let pending = rows
        .iter()
        .any(|r| matches!(r.status(), ExportStatus::Queued | ExportStatus::Running));
    let table = TableView::new(&HISTORY_TABLE, &state, &format!("{b}/hist"), total)
        .with_filter_options(
            "status",
            ExportStatus::iter()
                .map(|s| {
                    let (key, _) = status_badge(s);
                    FilterOption::key(
                        &sea_orm::ActiveEnum::to_value(&s),
                        &format!("export-status-{key}"),
                    )
                })
                .collect(),
        );
    let shell = app_shell(&ctx, &tenant.user, Some(tenant), None)
        .await?
        .at("export");
    let res = views::tickets::render(
        &v,
        &shell,
        "exports/page.html",
        StatusCode::OK,
        serde_json::json!({
            "panel": "hist",
            "subnav": subnav(&ctx, tenant, "hist").await?,
            "flash": flash,
            "table": table,
            "rows": items,
            "pending": pending,
            "workspace_name": tenant.workspace.name,
            "new_href": format!("{b}/new"),
        }),
    )?;
    Ok((session, res).into_response())
}

/// Unduh ZIP (diaudit). Selalu `attachment` + `no-store` + `nosniff`.
#[debug_handler]
async fn download(
    State(ctx): State<AppContext>,
    access: Require<ExportData>,
    RequestId(request_id): RequestId,
    id: IdOr404<data_exports::Entity>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let (filename, data) = svc::download(&ctx, &acting(tenant, request_id), id.id).await?;
    let mut res = Response::new(Body::from(data));
    let headers = res.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/zip"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))
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

#[debug_handler]
async fn retry(
    State(ctx): State<AppContext>,
    access: Require<ExportData>,
    session: Session,
    RequestId(request_id): RequestId,
    id: IdOr404<data_exports::Entity>,
) -> Result<Response> {
    let tenant = &access.tenant;
    svc::retry(&ctx, &acting(tenant, request_id), id.id).await?;
    let session = flash::set(session, &Flash::success(FLASH_RETRIED))?;
    Ok((
        session,
        format::redirect(&format!("{}/hist", base(&tenant.workspace.slug)))?,
    )
        .into_response())
}

pub fn routes() -> Routes {
    Routes::new()
        .add("/w/{slug}/settings/export", get(index))
        .add("/w/{slug}/settings/export/new", get(new).post(create))
        .add("/w/{slug}/settings/export/hist", get(history))
        .add("/w/{slug}/settings/export/{id}/download", post(download))
        .add("/w/{slug}/settings/export/{id}/retry", post(retry))
}
