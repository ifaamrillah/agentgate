//! Pengaturan project → **Penghemat token** (M10, `ProjectAdmin`):
//! `GET /w/{slug}/p/{key}/settings/tokens[/{panel}]` (Ringkasan, Fitur penghemat, Per tiket,
//! Per job) dan `POST /w/{slug}/p/{key}/settings/tokens` (toggle fitur opsional, PRG).
//!
//! Pemakaian dan biaya berasal dari laporan client lewat runner (`jobs.tokens_*`); penghematan dari
//! catatan server (`token_savings`). Rentang ringkasan: 30 hari terakhir; tren: 8 minggu.
use axum::http::StatusCode;
use chrono::{Duration, Utc};
use loco_rs::prelude::*;

use crate::{
    controllers::{
        extractors::{ProjectAccess, ProjectAdmin, RequestId},
        flash,
        shell::app_shell,
    },
    models::personas,
    services::{
        tickets::TicketFailure,
        tokens::{self as svc, TokenSaver},
        workspaces::Acting,
    },
    views::{
        self,
        flash::Flash,
        nav::{self, NavItem},
        projects::{ProjectPaths, ProjectSummary},
        tokens as tv,
        ui::Stat,
    },
};

const RANGE_DAYS: i64 = 30;
const WEEKS: u32 = 8;
const TICKETS: u64 = 12;
const JOBS: u64 = 50;

fn paths(access: &ProjectAccess) -> ProjectPaths {
    ProjectPaths::new(&access.tenant.workspace.slug, &access.project.key)
}

async fn render(
    ctx: &AppContext,
    v: &TeraView,
    access: &ProjectAccess,
    panel: &str,
    flash: Option<Flash>,
) -> Result<Response> {
    let paths = paths(access);
    let wid = access.tenant.workspace_id();
    let pid = access.project.id;
    let slug = &access.tenant.workspace.slug;
    let since = (Utc::now() - Duration::days(RANGE_DAYS)).fixed_offset();
    let saver = TokenSaver::of(&access.project);
    let saved = svc::saved_since(&ctx.db, wid, pid, since).await?;
    let saved_total: i64 = saved.values().sum();

    let subnav: Vec<NavItem> = tv::PANELS
        .iter()
        .map(|(k, label, icon, desc)| {
            let mut item =
                NavItem::link(k, label, icon, &format!("{}/{k}", paths.tokens)).describe(desc);
            item.current = *k == panel;
            item
        })
        .collect();

    let mut data = serde_json::json!({
        "project": ProjectSummary::new(&access.project, access.role.label(), &paths),
        "paths": paths,
        "panel": panel,
        "subnav": subnav,
        "settings_nav": nav::project_settings_nav(&paths, "tokens"),
        "flash": flash,
        "action": paths.tokens,
        "personas_href": format!("/w/{slug}/settings/personas"),
    });
    match panel {
        "ringkasan" => {
            let usage = svc::usage_since(&ctx.db, wid, pid, since).await?;
            let weeks = svc::weeks(&ctx.db, wid, pid, Utc::now().date_naive(), WEEKS).await?;
            // Biaya dihemat = token dihemat × tarif rata-rata pemakaian (perkiraan).
            #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
            let saved_cost = if usage.total > 0 {
                (saved_total as f64 * usage.cost_micros as f64 / usage.total as f64) as i64
            } else {
                0
            };
            let stats = vec![
                Stat::new("tokens-stat-saved", tv::short(saved_total), "leaf")
                    .tone("success")
                    .hint(
                        "tokens-stat-saved-hint",
                        tv::percent(saved_total, saved_total + usage.total),
                    ),
                Stat::new("tokens-stat-saved-cost", tv::usd(saved_cost), "leaf")
                    .tone("success")
                    .hint("tokens-stat-saved-cost-hint", ""),
                Stat::new("tokens-stat-used", tv::short(usage.total), "chart-bar")
                    .hint("tokens-stat-used-hint", ""),
                Stat::new("tokens-stat-cost", tv::usd(usage.cost_micros), "coins").hint(
                    "tokens-stat-cost-hint",
                    format!("{} job · {} client", usage.jobs, usage.clients),
                ),
            ];
            let has_weeks = weeks.iter().any(|w| w.used + w.saved > 0);
            data["stats"] = serde_json::to_value(stats)?;
            data["chart"] = if has_weeks {
                tv::week_chart(&weeks).into()
            } else {
                serde_json::Value::Null
            };
            data["top"] = tv::top_feature(&saved)
                .map(|(key, value)| serde_json::json!({ "key": key, "value": value }))
                .into();
            data["saved_total"] = tv::short(saved_total).into();
        }
        "fitur" => {
            data["features"] = serde_json::to_value(tv::features(&saved, saver))?;
        }
        "tiket" => {
            let rows = svc::per_ticket(&ctx.db, wid, pid, since, TICKETS).await?;
            data["tickets"] = serde_json::to_value(tv::ticket_bars(slug, &rows))?;
        }
        _ => {
            let rows = svc::per_job(&ctx.db, wid, pid, JOBS).await?;
            let labels = personas::Model::labels(&ctx.db, wid).await?;
            data["jobs"] = serde_json::to_value(tv::job_rows(slug, &rows, &labels))?;
        }
    }
    let shell = app_shell(ctx, &access.tenant.user, Some(&access.tenant), Some(access))
        .await?
        .at("project_tokens");
    views::tickets::render(v, &shell, "tokens/page.html", StatusCode::OK, data)
}

#[debug_handler]
async fn index(
    State(_ctx): State<AppContext>,
    ProjectAdmin(access): ProjectAdmin,
) -> Result<Response> {
    format::redirect(&format!("{}/ringkasan", paths(&access).tokens))
}

#[debug_handler]
async fn page(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    ProjectAdmin(access): ProjectAdmin,
    session: Session,
    Path((_, _, panel)): Path<(String, String, String)>,
) -> Result<Response> {
    if !tv::PANELS.iter().any(|(k, ..)| *k == panel) {
        return Err(Error::NotFound);
    }
    let (session, flash) = flash::take(session);
    let res = render(&ctx, &v, &access, &panel, flash).await?;
    Ok((session, res).into_response())
}

/// Toggle fitur opsional (checkbox tidak terkirim = mati). `ProjectAdmin` sebelum body dibaca.
#[debug_handler]
async fn save(
    State(ctx): State<AppContext>,
    ProjectAdmin(access): ProjectAdmin,
    session: Session,
    RequestId(request_id): RequestId,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Result<Response> {
    let has = |k: &str| pairs.iter().any(|(key, _)| key == k);
    let input = TokenSaver {
        concise: has("concise"),
        persona_models: has("persona_models"),
    };
    let act = Acting {
        workspace: &access.tenant.workspace,
        user: &access.tenant.user,
        role: &access.tenant.role,
        request_id,
    };
    match svc::update_saver(&ctx.db, &act, access.project.id, input).await {
        Ok(_) => {
            let session = flash::set(session, &Flash::success(tv::FLASH_SAVED))?;
            let to = format!("{}/fitur", paths(&access).tokens);
            Ok((session, format::redirect(&to)?).into_response())
        }
        Err(TicketFailure::Db(e)) => Err(e.into()),
        Err(_) => Err(Error::NotFound),
    }
}

pub fn routes() -> Routes {
    Routes::new()
        .add("/w/{slug}/p/{key}/settings/tokens", get(index).post(save))
        .add("/w/{slug}/p/{key}/settings/tokens/{panel}", get(page))
}
