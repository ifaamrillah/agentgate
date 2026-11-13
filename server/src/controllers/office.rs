//! **AI Office** (M10), terbuka untuk semua anggota workspace:
//!
//! - `GET /w/{slug}/office[?c=<karakter>]`: kantor isometrik (kanvas `static/js/office.js` →
//!   `office-engine.mjs`) + panel samping server-side (ringkasan, atau detail karakter `c`);
//! - `GET /w/{slug}/office/state`: JSON keadaan kantor (dipolling kanvas tiap 3 dtk, GET murni);
//! - `GET /w/{slug}/office/panel?c=`: fragmen panel (HTMX saat karakter diklik, dan polling 2
//!   dtk bagian live selama karakter bekerja);
//! - `POST /w/{slug}/office/looks`: kustom karakter (PRG kembali ke kantor).
//!
//! Approve/Reject memakai endpoint stage tiket yang sudah ada dengan `return_to` ke kantor.
//! Karakter di project yang tidak boleh dibuka pelaku tidak pernah dikirim (404 di panel).
use std::collections::HashMap;

use axum::http::StatusCode;
use loco_rs::prelude::*;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    controllers::{
        extractors::{RequestId, Tenant},
        flash,
        shell::app_shell,
    },
    models::{
        jobs,
        office::{Char, CharStatus, Office, Who},
        office_looks::Look,
        personas::{self, PersonaLabels},
        project_access, projects, tickets, users,
    },
    services::{
        jobs as job_svc, office as svc,
        tickets::{TicketFailure, stages},
        workspaces::Acting,
    },
    views::{
        self,
        office::{self as ov, Ctx},
        runner::LogLineView,
        tickets::PersonView,
    },
};

/// Baris log di panel detail.
const LOG_LINES: u64 = 14;
/// Baris aktivitas terbaru di panel ringkasan.
const FEED: u64 = 6;

fn acting(tenant: &Tenant, request_id: Option<String>) -> Acting<'_> {
    Acting {
        workspace: &tenant.workspace,
        user: &tenant.user,
        role: &tenant.role,
        request_id,
    }
}

fn base(slug: &str) -> String {
    format!("/w/{slug}/office")
}

async fn users_of(db: &DatabaseConnection, office: &Office) -> Result<HashMap<Uuid, users::Model>> {
    let mut ids: Vec<Uuid> = office.runners.iter().map(|r| r.owner).collect();
    ids.extend(office.chars.iter().filter_map(|c| match c.who {
        Who::Human(u) => Some(u),
        Who::Persona(_) => None,
    }));
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

struct Loaded {
    office: Office,
    labels: PersonaLabels,
    users: HashMap<Uuid, users::Model>,
}

async fn load(ctx: &AppContext, tenant: &Tenant) -> Result<Loaded> {
    let office = svc::state(&ctx.db, &acting(tenant, None)).await?;
    let labels = personas::Model::labels(&ctx.db, tenant.workspace_id()).await?;
    let users = users_of(&ctx.db, &office).await?;
    Ok(Loaded {
        office,
        labels,
        users,
    })
}

#[derive(Debug, Deserialize, Default)]
struct PanelQuery {
    #[serde(default)]
    c: String,
}

/// Aktivitas terbaru: job yang terakhir berubah di project yang bisa dibuka pelaku.
async fn feed(ctx: &AppContext, tenant: &Tenant, l: &Loaded) -> Result<serde_json::Value> {
    let wid = tenant.workspace_id();
    let projects = projects::Model::accessible_for(&ctx.db, &tenant.accessor()).await?;
    if projects.is_empty() {
        return Ok(serde_json::json!([]));
    }
    let rows = jobs::Entity::find()
        .filter(jobs::Column::WorkspaceId.eq(wid.get()))
        .filter(jobs::Column::ProjectId.is_in(projects.iter().map(|p| p.id)))
        .order_by_desc(jobs::Column::UpdatedAt)
        .limit(FEED)
        .all(&ctx.db)
        .await?;
    let tids: Vec<Uuid> = rows.iter().filter_map(|j| j.ticket_id).collect();
    let codes: HashMap<Uuid, String> = if tids.is_empty() {
        HashMap::new()
    } else {
        tickets::Entity::find()
            .filter(tickets::Column::WorkspaceId.eq(wid.get()))
            .filter(tickets::Column::Id.is_in(tids))
            .all(&ctx.db)
            .await?
            .into_iter()
            .map(|t| (t.id, t.code))
            .collect()
    };
    let now = chrono::Utc::now();
    let slug = &tenant.workspace.slug;
    Ok(rows
        .iter()
        .map(|j| {
            let mark = views::runner::PersonaMark::of(&j.persona, &l.labels);
            let code = j
                .ticket_id
                .and_then(|t| codes.get(&t).cloned())
                .unwrap_or_else(|| "KB".into());
            serde_json::json!({
                "persona": mark,
                "status_key": format!("office-feed-{}", j.status),
                "code": code,
                "href": format!("/w/{slug}/jobs/{}", j.id),
                "age": views::runner::age(&j.updated_at, now),
            })
        })
        .collect())
}

/// Data panel: ringkasan (tanpa karakter) atau detail karakter.
async fn panel_data(
    ctx: &AppContext,
    tenant: &Tenant,
    l: &Loaded,
    selected: Option<&Char>,
) -> Result<serde_json::Value> {
    let slug = tenant.workspace.slug.clone();
    let cx = Ctx {
        slug: &slug,
        labels: &l.labels,
        users: &l.users,
    };
    let home = base(&slug);
    let Some(c) = selected else {
        let todo: Vec<ov::CharView> = l
            .office
            .chars
            .iter()
            .filter(|c| c.status.needs_action())
            .map(|c| ov::char_view(&l.office, c, &cx))
            .collect();
        let runners = ov::runner_rows(&l.office, &l.users);
        let online = runners.iter().filter(|r| r.state != "offline").count();
        return Ok(serde_json::json!({
            "mode": "overview",
            "todo": todo,
            "runners": runners,
            "runners_online": online,
            "feed": feed(ctx, tenant, l).await?,
            "inbox_href": format!("/w/{slug}/inbox"),
            "runner_href": format!("/w/{slug}/runner/team"),
            "home": home,
        }));
    };
    let view = ov::char_view(&l.office, c, &cx);
    let wid = tenant.workspace_id();
    let mut stage_name = String::new();
    let mut can_decide = false;
    let mut summary = String::new();
    if let Some(tid) = c.ticket_id {
        if let Some(t) = tickets::Model::find_in(&ctx.db, wid, tid).await? {
            if let Ok(flow) = stages::Flow::load(&ctx.db, wid, &t).await {
                stage_name = flow
                    .rule(&c.stage_key)
                    .or_else(|| flow.current.stage(&c.stage_key))
                    .map_or_else(|| c.stage_key.clone(), |s| s.name.clone());
            }
            if c.status == CharStatus::Review && c.permission_id.is_none() {
                if let Some(role) =
                    project_access::effective_role(&ctx.db, &tenant.accessor(), t.project_id)
                        .await?
                {
                    can_decide =
                        stages::can_decide(&ctx.db, wid, &t, tenant.user.id, &role).await?;
                }
            }
        }
    }
    if let Some(p) = c.persona_key() {
        summary = personas::Model::find_active(&ctx.db, wid, p)
            .await?
            .map(|p| p.summary)
            .unwrap_or_default();
    }
    let logs: Vec<LogLineView> = match c.job_id {
        Some(j) => {
            let mut rows = job_svc::logs_of(&ctx.db, wid, j, 200).await?;
            let skip = rows
                .len()
                .saturating_sub(usize::try_from(LOG_LINES).unwrap_or(14));
            rows.drain(..skip);
            rows.iter().map(LogLineView::of).collect()
        }
        None => Vec::new(),
    };
    let live = matches!(
        c.status,
        CharStatus::Working | CharStatus::Meeting | CharStatus::Paused
    ) || c.permission_id.is_some();
    let room = l
        .office
        .rooms
        .iter()
        .find(|r| r.key == c.room)
        .map(|r| r.name.clone())
        .unwrap_or_default();
    let ticket_base = format!("/w/{slug}/t/{}", c.ticket_code);
    let return_to = format!("{home}?c={}", c.id);
    let a = acting(tenant, None);
    let look = view.look.clone();
    let human = match &c.who {
        Who::Human(u) => l.users.get(u).map(PersonView::of),
        Who::Persona(_) => None,
    };
    Ok(serde_json::json!({
        "mode": "detail",
        "c": view,
        "room_key": c.room,
        "room_name": room,
        "stage_name": stage_name,
        "summary": summary,
        "human": human,
        "logs": logs,
        "live": live,
        "can_decide": can_decide,
        "approve": format!("{ticket_base}/stage/approve"),
        "reject": format!("{ticket_base}/stage/reject"),
        "return_to": return_to,
        "panel_href": format!("{home}/panel?c={}", c.id),
        "home": home,
        "runner_href": format!("/w/{slug}/runner/team"),
        "personas_href": format!("/w/{slug}/settings/personas"),
        "can_customize": svc::can_customize(&a, &c.who),
        "look_action": format!("{home}/looks"),
        "subject": if c.persona_key().is_some() { "persona" } else { "user" },
        "subject_key": match &c.who { Who::Persona(k) => k.clone(), Who::Human(u) => u.to_string() },
        "accs": ov::choices("office-acc", ov::ACC_KEYS, &look.acc),
        "styles": ov::choices("office-style", ov::STYLE_KEYS, &look.style),
    }))
}

#[debug_handler]
async fn page(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    tenant: Tenant,
    session: Session,
    Query(q): Query<PanelQuery>,
) -> Result<Response> {
    let (session, flash) = flash::take(session);
    let l = load(&ctx, &tenant).await?;
    let selected = l.office.char(&q.c).cloned();
    let panel = panel_data(&ctx, &tenant, &l, selected.as_ref()).await?;
    let slug = &tenant.workspace.slug;
    let home = base(slug);
    let icons: std::collections::BTreeSet<String> = l
        .labels
        .values()
        .map(|p| p.icon.clone())
        .chain(["robot".to_string(), "bulb".to_string()])
        .collect();
    let rooms: Vec<serde_json::Value> = l
        .office
        .rooms
        .iter()
        .filter(|r| r.kind != crate::models::office::RoomKind::Lounge)
        .map(|r| {
            serde_json::json!({
                "k": r.key,
                "kind": r.kind.as_str(),
                "n": r.name,
                "c": ov::room_color(r),
                "count": l.office.chars.iter().filter(|c| c.room == r.key).count(),
            })
        })
        .collect();
    let counts: Vec<serde_json::Value> = [
        CharStatus::Working,
        CharStatus::Review,
        CharStatus::Decision,
        CharStatus::Idle,
    ]
    .iter()
    .map(|s| {
        serde_json::json!({
            "st": s.as_str(),
            "n": l.office.count(*s),
            "color": ov::status_meta(*s).0,
            "label_key": format!("office-sum-{}", s.as_str()),
        })
    })
    .collect();
    let data = serde_json::json!({
        "home": home,
        "state_href": format!("{home}/state"),
        "panel_base": format!("{home}/panel"),
        "selected": selected.as_ref().map(|c| c.id.clone()),
        "panel": panel,
        "rooms": rooms,
        "total": l.office.chars.len(),
        "counts": counts,
        "hidden": l.office.hidden_projects,
        "statuses": CharStatus::ALL
            .iter()
            .map(|s| serde_json::json!({ "k": s.as_str(), "label_key": format!("office-st-{}", s.as_str()) }))
            .collect::<Vec<_>>(),
        "icons": icons,
        "personas_href": format!("/w/{slug}/settings/personas"),
        "flash": flash,
    });
    let shell = app_shell(&ctx, &tenant.user, Some(&tenant), None)
        .await?
        .at("office");
    let res = views::tickets::render(&v, &shell, "office/page.html", StatusCode::OK, data)?;
    Ok((session, res).into_response())
}

#[debug_handler]
async fn state(State(ctx): State<AppContext>, tenant: Tenant) -> Result<Response> {
    let l = load(&ctx, &tenant).await?;
    let slug = tenant.workspace.slug.clone();
    let cx = Ctx {
        slug: &slug,
        labels: &l.labels,
        users: &l.users,
    };
    let mut res = format::json(ov::state_view(&l.office, &cx))?;
    res.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    Ok(res)
}

#[debug_handler]
async fn panel(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    tenant: Tenant,
    Query(q): Query<PanelQuery>,
) -> Result<Response> {
    let l = load(&ctx, &tenant).await?;
    let selected = if q.c.is_empty() {
        None
    } else {
        Some(l.office.char(&q.c).cloned().ok_or(Error::NotFound)?)
    };
    let data = panel_data(&ctx, &tenant, &l, selected.as_ref()).await?;
    format::render().view(
        &v,
        "office/_panel.html",
        serde_json::json!({ "panel": data }),
    )
}

/// Kustom karakter. Persona: hak kelola persona; manusia: dirinya sendiri atau Admin.
#[debug_handler]
async fn save_look(
    State(ctx): State<AppContext>,
    tenant: Tenant,
    session: Session,
    RequestId(request_id): RequestId,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Result<Response> {
    let get = |k: &str| {
        pairs
            .iter()
            .find(|(key, _)| key == k)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };
    let who = match get("subject").as_str() {
        "persona" => Who::Persona(get("key")),
        "user" => Who::Human(Uuid::parse_str(&get("key")).map_err(|_| Error::NotFound)?),
        _ => return Err(Error::NotFound),
    };
    let input = Look {
        alias: get("alias"),
        shirt: get("shirt"),
        acc: get("acc"),
        skin: get("skin"),
        hair: get("hair"),
        style: get("style"),
    };
    let home = base(&tenant.workspace.slug);
    let back = {
        let c = get("c");
        if c.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-') && !c.is_empty() {
            format!("{home}?c={c}")
        } else {
            home.clone()
        }
    };
    match svc::update_look(&ctx.db, &acting(&tenant, request_id), &who, input).await {
        Ok(_) => {
            let session = flash::set(session, &views::flash::Flash::success(ov::FLASH_LOOK_SAVED))?;
            Ok((session, format::redirect(&back)?).into_response())
        }
        Err(TicketFailure::Invalid(_)) => {
            let session = flash::set(
                session,
                &views::flash::Flash::error(crate::models::office_looks::LOOK_INVALID),
            )?;
            Ok((session, format::redirect(&back)?).into_response())
        }
        Err(TicketFailure::Db(e)) => Err(e.into()),
        Err(_) => Err(Error::NotFound),
    }
}

pub fn routes() -> Routes {
    Routes::new()
        .add("/w/{slug}/office", get(page))
        .add("/w/{slug}/office/state", get(state))
        .add("/w/{slug}/office/panel", get(panel))
        .add("/w/{slug}/office/looks", post(save_look))
}
