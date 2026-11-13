//! Tiket → panel Planning & estimasi (sidebar Ringkasan): sprint, Fix version, component,
//! usulan AI (poin + jam AI/manusia per stage + dasar), estimasi berlaku, perkiraan selesai
//! (estimasi × faktor kalibrasi, 8 jam/hari kerja). "Ubah estimasi" menyimpan estimasi manusia;
//! "Minta estimasi AI" menulis komentar `@agent-analyst` (job komentar M5).
use loco_rs::prelude::*;
use serde::Serialize;

use super::{acting, project_paths};
use crate::{
    controllers::{
        extractors::{RequestId, TicketAccess},
        flash,
    },
    models::{
        components, permission::Permission, releases, sprints, ticket_estimates::StageHours,
    },
    services::{
        planning::{self, estimates},
        tickets::{TicketFailure, comments, stages::Flow},
    },
    views::{flash::Flash, planning as pv, tickets::ticket_href},
};

pub const FLASH_SAVED: &str = "estimate-flash-saved";
pub const FLASH_REQUESTED: &str = "estimate-flash-requested";
pub const FLASH_INVALID: &str = "planning-estimate-invalid";

#[derive(Debug, Serialize)]
pub struct EstimateView {
    pub points: Option<i32>,
    pub ai: String,
    pub human: String,
    pub basis: String,
    pub author: String,
    pub stages: Vec<StageRow>,
}

#[derive(Debug, Serialize)]
pub struct StageRow {
    pub key: String,
    pub name: String,
    pub ai: String,
    pub human: String,
}

#[derive(Debug, Serialize)]
pub struct Named {
    pub name: String,
    pub href: String,
    pub hint: String,
}

/// Data panel planning tiket.
#[derive(Debug, Serialize)]
pub struct Panel {
    pub sprint: Option<Named>,
    pub release: Option<Named>,
    pub component: Option<Named>,
    pub ai: Option<EstimateView>,
    pub current: Option<EstimateView>,
    /// AI diusulkan lalu diubah manusia.
    pub changed: bool,
    pub forecast: String,
    pub forecast_late: bool,
    /// Isian dialog ubah: semua stage workflow tiket dengan jam yang berlaku.
    pub form: Vec<StageRow>,
    pub points: String,
    pub save_action: String,
    pub request_action: String,
}

fn view(
    e: &crate::models::ticket_estimates::Model,
    names: &dyn Fn(&str) -> String,
) -> EstimateView {
    let (a, h) = e.totals();
    EstimateView {
        points: e.points,
        ai: pv::decimal(a),
        human: pv::decimal(h),
        basis: e.basis.clone(),
        author: e.persona.clone(),
        stages: e
            .stages()
            .iter()
            .map(|s| StageRow {
                key: s.stage.clone(),
                name: names(&s.stage),
                ai: pv::decimal(s.ai_hours),
                human: pv::decimal(s.human_hours),
            })
            .collect(),
    }
}

/// Bangun panel planning tiket.
///
/// # Errors
///
/// Error database.
pub async fn panel(ctx: &AppContext, access: &TicketAccess) -> Result<Panel> {
    let db = &ctx.db;
    let wid = access.access.tenant.workspace_id();
    let t = &access.ticket;
    let home = project_paths(&access.access).home;
    let sprint = match t.sprint_id {
        Some(id) => sprints::Model::find_in(db, wid, id).await?.map(|s| Named {
            href: format!("{home}/backlog"),
            hint: pv::range(s.starts_on, s.ends_on),
            name: s.name,
        }),
        None => None,
    };
    let release = match t.release_id {
        Some(id) => releases::Model::find_in(db, wid, id).await?.map(|r| Named {
            href: format!("{home}/releases/{}", r.id),
            hint: r.target_on.map(pv::long_day).unwrap_or_default(),
            name: r.name,
        }),
        None => None,
    };
    let component = match t.component_id {
        Some(id) => components::Model::find_in(db, wid, id).await?.map(|c| Named {
            href: format!("{home}/settings/components"),
            hint: String::new(),
            name: c.name,
        }),
        None => None,
    };
    let flow = Flow::load(db, wid, t).await?;
    let stages: Vec<(String, String)> = flow
        .current
        .stages
        .iter()
        .map(|s| (s.key.clone(), s.name.clone()))
        .collect();
    let names = |k: &str| {
        stages
            .iter()
            .find(|(key, _)| key == k)
            .map_or_else(|| k.to_string(), |(_, n)| n.clone())
    };
    let latest = estimates::latest(db, wid, &[t.id])
        .await?
        .remove(&t.id)
        .unwrap_or_default();
    let calib = estimates::calibration(db, wid, t.project_id).await?;
    let current_stages: Vec<StageHours> = latest
        .current
        .as_ref()
        .map(crate::models::ticket_estimates::Model::stages)
        .unwrap_or_default();
    let (forecast, forecast_late) = match estimates::remaining_hours(t, latest.current.as_ref(), &calib) {
        Some(h) if t.status != "done" && h > 0.0 => {
            let d = estimates::finish_date(planning::today(), h);
            (pv::long_day(d), t.due_on.is_some_and(|due| d > due))
        }
        _ => (String::new(), false),
    };
    let ticket_paths = format!("{}", ticket_href(&access.access.tenant.workspace.slug, &t.code));
    Ok(Panel {
        sprint,
        release,
        component,
        changed: latest
            .ai
            .as_ref()
            .is_some_and(|a| a.points.is_some() && a.points != t.story_points),
        ai: latest.ai.as_ref().map(|e| view(e, &names)),
        current: latest.current.as_ref().map(|e| view(e, &names)),
        forecast,
        forecast_late,
        form: stages
            .iter()
            .map(|(k, n)| {
                let cur = current_stages.iter().find(|s| &s.stage == k);
                StageRow {
                    key: k.clone(),
                    name: n.clone(),
                    ai: cur.map(|s| pv::decimal(s.ai_hours)).unwrap_or_default(),
                    human: cur.map(|s| pv::decimal(s.human_hours)).unwrap_or_default(),
                }
            })
            .collect(),
        points: t.story_points.map(|p| p.to_string()).unwrap_or_default(),
        save_action: format!("{ticket_paths}/estimate"),
        request_action: format!("{ticket_paths}/estimate/ai"),
    })
}

fn hours(raw: &str) -> Option<f64> {
    let v = raw.trim().replace(',', ".");
    if v.is_empty() {
        Some(0.0)
    } else {
        v.parse::<f64>().ok()
    }
}

/// Simpan estimasi manusia: `points`, `ai_<stage>`, `human_<stage>`, `basis`.
#[debug_handler]
pub(super) async fn save(
    State(ctx): State<AppContext>,
    access: TicketAccess,
    session: Session,
    RequestId(request_id): RequestId,
    Form(fields): Form<Vec<(String, String)>>,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    let get = |k: &str| {
        fields
            .iter()
            .find(|(n, _)| n == k)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };
    let back = format!(
        "{}#estimate",
        ticket_href(&access.access.tenant.workspace.slug, &access.ticket.code)
    );
    let mut stages: Vec<StageHours> = Vec::new();
    let mut bad = false;
    for (k, v) in &fields {
        if let Some(stage) = k.strip_prefix("ai_") {
            let human = get(&format!("human_{stage}"));
            match (hours(v), hours(&human)) {
                (Some(a), Some(h)) if a > 0.0 || h > 0.0 => stages.push(StageHours {
                    stage: stage.to_string(),
                    ai_hours: a,
                    human_hours: h,
                }),
                (Some(_), Some(_)) => {}
                _ => bad = true,
            }
        }
    }
    let points = match get("points").trim() {
        "" => None,
        p => match p.parse::<i32>() {
            Ok(n) => Some(n),
            Err(_) => {
                bad = true;
                None
            }
        },
    };
    if bad {
        let session = flash::set(session, &Flash::error(FLASH_INVALID))?;
        return Ok((session, format::redirect(&back)?).into_response());
    }
    let input = estimates::EstimateInput {
        points,
        stages,
        basis: get("basis"),
    };
    match estimates::save_human(
        &ctx.db,
        &acting(&access.access.tenant, request_id),
        access.ticket.id,
        &input,
    )
    .await
    {
        Ok(_) => {
            let session = flash::set(session, &Flash::success(FLASH_SAVED))?;
            Ok((session, format::redirect(&back)?).into_response())
        }
        Err(TicketFailure::Invalid(_)) => {
            let session = flash::set(session, &Flash::error(FLASH_INVALID))?;
            Ok((session, format::redirect(&back)?).into_response())
        }
        Err(e) => Err(e.into()),
    }
}

/// Minta estimasi AI: komentar `@agent-analyst` (job komentar di runner).
#[debug_handler]
pub(super) async fn request_ai(
    State(ctx): State<AppContext>,
    access: TicketAccess,
    session: Session,
    RequestId(request_id): RequestId,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    let body = format!(
        "@agent-analyst Mohon estimasi tiket {code}: story point dan jam kerja AI + manusia per stage \
         workflow, beserta dasar estimasinya (tiket mirip). Simpan dengan ag_planning action=estimate.",
        code = access.ticket.code
    );
    comments::post(
        &ctx.db,
        &acting(&access.access.tenant, request_id),
        access.ticket.id,
        &body,
    )
    .await?;
    let session = flash::set(session, &Flash::success(FLASH_REQUESTED))?;
    let back = format!(
        "{}#estimate",
        ticket_href(&access.access.tenant.workspace.slug, &access.ticket.code)
    );
    Ok((session, format::redirect(&back)?).into_response())
}
