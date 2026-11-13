//! Stage tiket (M3): data panel "Stage sekarang" + stepper, dan aksi stage (selesaikan/minta
//! review, approve, reject, keputusan, lompat/paksa, penugasan). Penolakan → 422 halaman detail
//! dengan pesan di panel stage (dialog yang bersangkutan terbuka lagi).
use agentgate_core::workflow::Stage;
use loco_rs::{prelude::*, validation::ModelValidationErrors};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{acting, board::people_options, ticket::render_with_stage_error};
use crate::{
    common::settings::Settings,
    controllers::{
        extractors::{RequestId, TicketAccess},
        flash,
    },
    models::{
        permission::Permission,
        ticket_events::{self, EventKind},
        ticket_stage_people::{self, StagePeople},
        tickets::{self, StageState},
    },
    services::{
        jobs,
        tickets::{
            TicketFailure, gates,
            stages::{self, AdminMove, Decision, Flow},
        },
    },
    views::{
        self,
        flash::Flash,
        tickets::{Lookup, PersonView, ticket_href},
        ui::ComboOption,
        workflow::{GateView, PeopleView, StageView, stepper},
    },
};

/// Stage tujuan (kunci + nama) untuk pilihan.
#[derive(Debug, Clone, Serialize)]
pub struct StageChoice {
    pub key: String,
    pub name: String,
}

fn choice(s: &Stage) -> StageChoice {
    StageChoice {
        key: s.key.clone(),
        name: s.name.clone(),
    }
}

/// Satu putaran gagal (reject) di panel keputusan.
#[derive(Debug, Clone, Serialize)]
pub struct RoundView {
    pub round: i64,
    pub note: String,
    pub actor: Option<PersonView>,
    pub at: String,
}

/// Data panel stage tiket.
#[derive(Debug, Clone, Serialize)]
pub struct StagePanel {
    pub workflow: String,
    pub version: i32,
    pub steps: Vec<StageView>,
    pub current: StageView,
    pub prompt: String,
    pub output: String,
    pub state: &'static str,
    pub gates: Vec<GateView>,
    pub gates_ok: usize,
    pub people: PeopleView,
    pub people_values: PeopleValues,
    pub people_options: Vec<ComboOption>,
    pub can_edit: bool,
    pub can_decide: bool,
    pub is_admin: bool,
    pub allow_force: bool,
    pub next: Option<StageChoice>,
    pub targets: Vec<StageChoice>,
    pub earlier: Vec<StageChoice>,
    pub fail_count: i32,
    pub fail_limit: u8,
    pub rounds: Vec<RoundView>,
    pub cancelled: bool,
    pub decisions: Vec<serde_json::Value>,
    /// Job stage terbaru tiket (M5) dan evidence stage sekarang.
    pub job: Option<views::runner::JobView>,
    pub evidence: Vec<views::runner::EvidenceView>,
    /// Runner milik pelaku (pilihan di dialog Ambil job).
    pub my_runners: Vec<views::runner::RunnerView>,
    /// Path kembali untuk aksi job.
    pub back: String,
}

/// Id penugasan sebagai string (nilai combo).
#[derive(Debug, Clone, Default, Serialize)]
pub struct PeopleValues {
    pub executors: Vec<String>,
    pub approvers: Vec<String>,
    pub watchers: Vec<String>,
}

fn strings(ids: &[Uuid]) -> Vec<String> {
    ids.iter().map(ToString::to_string).collect()
}

/// Bangun panel stage untuk tiket ini.
///
/// # Errors
///
/// Error database.
pub(super) async fn panel(ctx: &AppContext, access: &TicketAccess) -> Result<StagePanel> {
    let db: &DatabaseConnection = &ctx.db;
    let tenant = &access.access.tenant;
    let wid = tenant.workspace_id();
    let t = &access.ticket;
    let flow = Flow::load(db, wid, t).await?;
    let position = flow.position(&t.stage_key);
    let rule = flow.rule(&t.stage_key).cloned().unwrap_or_else(|| {
        Stage::new(
            &t.stage_key,
            &t.stage_key,
            agentgate_core::workflow::Category::Todo,
        )
    });
    let checks = gates::check(db, wid, t, &rule).await?;
    let people = ticket_stage_people::Model::of_stage(db, wid, t.id, &t.stage_key).await?;
    let events = ticket_events::Model::of_ticket(db, wid, t.id).await?;
    let rejected: Vec<&ticket_events::Model> = events
        .iter()
        .filter(|e| e.kind().ok() == Some(EventKind::Rejected))
        .filter(|e| e.changes["stage"].as_str() == Some(rule.name.as_str()))
        .collect();
    let mut ids: Vec<Uuid> = people
        .executors
        .iter()
        .chain(&people.approvers)
        .chain(&people.watchers)
        .copied()
        .collect();
    ids.extend(rejected.iter().filter_map(|e| e.actor_id));
    let users = super::users_by_id(db, ids).await?;
    let lookup = Lookup::new(&tenant.workspace.slug, &[], &users);
    let persons = |list: &[Uuid]| -> Vec<PersonView> {
        list.iter()
            .filter_map(|id| lookup.person(Some(*id)))
            .collect()
    };
    let state = t.stage_state().unwrap_or(StageState::Active);
    let can_decide = stages::can_decide(db, wid, t, tenant.user.id, &access.access.role).await?;
    // Putaran gagal terakhir saja (sejak masuk stage ini dihitung `fail_count`).
    let take = usize::try_from(t.fail_count).unwrap_or(0);
    let rounds: Vec<RoundView> = rejected
        .iter()
        .rev()
        .take(take.max(if state == StageState::Decision { 1 } else { 0 }))
        .rev()
        .map(|e| RoundView {
            round: e.changes["round"].as_i64().unwrap_or(0),
            note: e.changes["note"].as_str().unwrap_or_default().to_string(),
            actor: lookup.person(e.actor_id),
            at: views::time::wib_datetime(&e.created_at),
        })
        .collect();
    let labels = crate::models::personas::Model::labels(db, wid).await?;
    let job = match jobs::latest_of_ticket(db, wid, t.id).await? {
        Some(j) if j.stage_key == t.stage_key => {
            crate::controllers::runner::job_views(db, tenant, &[(j, Some(t.clone()))])
                .await?
                .pop()
        }
        _ => None,
    };
    let evidence: Vec<views::runner::EvidenceView> = jobs::evidence::of_stage(db, wid, t)
        .await?
        .iter()
        .map(|e| views::runner::EvidenceView::of(e, &tenant.workspace.slug))
        .collect();
    let my_runners = if job.as_ref().is_some_and(|j| j.can_claim) {
        crate::controllers::runner::my_runners(db, tenant).await?
    } else {
        Vec::new()
    };
    let mut current = StageView::of(&rule, &flow.current, &labels);
    current.state = "current";
    let options = if access.can(Permission::EditProject) {
        let mut o = people_options(ctx, &access.access, None, None).await?;
        for p in persons(&ids_of(&people)) {
            if !o.iter().any(|x| x.value == p.id.to_string()) {
                o.push(ComboOption::person(p.id, &p.name, &p.email));
            }
        }
        o
    } else {
        Vec::new()
    };
    Ok(StagePanel {
        workflow: flow.workflow.name.clone(),
        version: flow.workflow.version,
        steps: stepper(&flow.current, &t.stage_key, position, &labels),
        prompt: rule.prompt.clone(),
        output: rule.output.clone(),
        state: match state {
            StageState::Active => "active",
            StageState::Review => "review",
            StageState::Decision => "decision",
        },
        gates_ok: checks.iter().filter(|c| c.ok).count(),
        gates: checks.iter().map(|c| GateView::of(c.gate, c.ok)).collect(),
        people: PeopleView {
            executors: persons(&people.executors),
            approvers: persons(&people.approvers),
            watchers: persons(&people.watchers),
        },
        people_values: PeopleValues {
            executors: strings(&people.executors),
            approvers: strings(&people.approvers),
            watchers: strings(&people.watchers),
        },
        people_options: options,
        can_edit: access.can(Permission::EditProject),
        can_decide,
        is_admin: access.can(Permission::AdministerProject),
        allow_force: access.access.project.allow_force_transition,
        next: flow.next(&t.stage_key).map(choice),
        targets: flow
            .current
            .stages
            .iter()
            .filter(|s| s.key != t.stage_key)
            .map(choice)
            .collect(),
        earlier: flow.current.stages[..position.min(flow.current.stages.len())]
            .iter()
            .filter(|s| s.key != t.stage_key)
            .map(choice)
            .collect(),
        fail_count: t.fail_count,
        fail_limit: rule.fail_limit,
        rounds,
        cancelled: t.cancelled_at.is_some(),
        decisions: Decision::ALL
            .iter()
            .map(|d| {
                serde_json::json!({
                    "value": d.as_str(),
                    "label_key": format!("decision-{}", d.as_str()),
                    "hint_key": format!("decision-{}-hint", d.as_str()),
                })
            })
            .collect(),
        current,
        job,
        evidence,
        my_runners,
        back: ticket_href(&tenant.workspace.slug, &t.code),
    })
}

fn ids_of(p: &StagePeople) -> Vec<Uuid> {
    p.executors
        .iter()
        .chain(&p.approvers)
        .chain(&p.watchers)
        .copied()
        .collect()
}

/// Kode error pertama dari field-field aksi stage.
pub(super) fn stage_error(errors: &ModelValidationErrors) -> String {
    for field in [
        "stage", "status", "note", "reason", "choice", "target", "people",
    ] {
        if let Some(e) = errors.errors.get(field).and_then(|l| l.first()) {
            return e.code.clone();
        }
    }
    String::new()
}

async fn done(
    ctx: &AppContext,
    access: &TicketAccess,
    session: Session,
    result: std::result::Result<tickets::Model, TicketFailure>,
    dialog: &'static str,
    v: &TeraView,
    flash_key: &'static str,
) -> Result<Response> {
    match result {
        Ok(ticket) => {
            let session = flash::set(session, &Flash::success(flash_key))?;
            let to = ticket_href(&access.access.tenant.workspace.slug, &ticket.code);
            Ok((session, format::redirect(&to)?).into_response())
        }
        Err(TicketFailure::Invalid(errors)) => {
            render_with_stage_error(ctx, v, access, stage_error(&errors), dialog).await
        }
        Err(e) => Err(e.into()),
    }
}

#[derive(Debug, Deserialize)]
pub struct NoteForm {
    #[serde(default)]
    note: String,
    /// Halaman asal (mis. pratinjau mockup, AI Office) untuk redirect setelah sukses; hanya path
    /// di workspace yang sama.
    #[serde(default)]
    return_to: String,
}

/// `return_to` aman: path lokal di workspace ini, bukan URL lain.
#[must_use]
pub fn safe_return(slug: &str, to: &str) -> bool {
    to.starts_with(&format!("/w/{slug}/"))
        && !to.contains("//")
        && !to.contains('\\')
        && !to.chars().any(char::is_control)
}

/// Ganti tujuan redirect sukses dengan `return_to` yang aman.
fn back(mut res: Response, slug: &str, to: &str) -> Response {
    if res.status() == axum::http::StatusCode::SEE_OTHER && safe_return(slug, to) {
        if let Ok(v) = axum::http::HeaderValue::from_str(to) {
            res.headers_mut().insert(axum::http::header::LOCATION, v);
        }
    }
    res
}

#[debug_handler]
pub(super) async fn complete(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: TicketAccess,
    session: Session,
    RequestId(request_id): RequestId,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    let r = stages::complete(
        &ctx.db,
        &acting(&access.access.tenant, request_id),
        access.ticket.id,
    )
    .await;
    done(&ctx, &access, session, r, "", &v, FLASH_STAGE_MOVED).await
}

#[debug_handler]
pub(super) async fn approve(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: TicketAccess,
    session: Session,
    RequestId(request_id): RequestId,
    Form(form): Form<NoteForm>,
) -> Result<Response> {
    let r = stages::approve(
        &ctx.db,
        &acting(&access.access.tenant, request_id.clone()),
        access.ticket.id,
        &form.note,
    )
    .await;
    if r.is_ok() {
        let settings = Settings::from_ctx(&ctx)?;
        let tenant = &access.access.tenant;
        if let Err(e) = crate::services::git::pulls::after_approve(
            &ctx.db,
            &settings,
            tenant.workspace.workspace_id(),
            tenant.user.id,
            &access.ticket,
            request_id,
        )
        .await
        {
            tracing::warn!(error = %e, "auto merge after approve failed");
        }
    }
    let res = done(&ctx, &access, session, r, "", &v, FLASH_APPROVED).await?;
    Ok(back(
        res,
        &access.access.tenant.workspace.slug,
        &form.return_to,
    ))
}

#[debug_handler]
pub(super) async fn reject(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: TicketAccess,
    session: Session,
    RequestId(request_id): RequestId,
    Form(form): Form<NoteForm>,
) -> Result<Response> {
    let r = stages::reject(
        &ctx.db,
        &acting(&access.access.tenant, request_id),
        access.ticket.id,
        &form.note,
    )
    .await;
    let res = done(
        &ctx,
        &access,
        session,
        r,
        "reject-dialog",
        &v,
        FLASH_REJECTED,
    )
    .await?;
    Ok(back(
        res,
        &access.access.tenant.workspace.slug,
        &form.return_to,
    ))
}

#[derive(Debug, Deserialize)]
pub struct DecideForm {
    #[serde(default)]
    choice: String,
    #[serde(default)]
    note: String,
    #[serde(default)]
    target: String,
}

#[debug_handler]
pub(super) async fn decide(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: TicketAccess,
    session: Session,
    RequestId(request_id): RequestId,
    Form(form): Form<DecideForm>,
) -> Result<Response> {
    let r = stages::decide(
        &ctx.db,
        &acting(&access.access.tenant, request_id),
        access.ticket.id,
        &form.choice,
        &form.note,
        &form.target,
    )
    .await
    .map(|(t, _)| t);
    done(&ctx, &access, session, r, "decision", &v, FLASH_DECIDED).await
}

#[derive(Debug, Deserialize)]
pub struct AdminMoveForm {
    #[serde(default)]
    stage: String,
    #[serde(default)]
    reason: String,
    #[serde(default)]
    kind: String,
}

#[debug_handler]
pub(super) async fn admin_move(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: TicketAccess,
    session: Session,
    RequestId(request_id): RequestId,
    Form(form): Form<AdminMoveForm>,
) -> Result<Response> {
    access.require(Permission::AdministerProject)?;
    let kind = if form.kind == "force" {
        AdminMove::Force
    } else {
        AdminMove::Jump
    };
    let r = stages::admin_move(
        &ctx.db,
        &acting(&access.access.tenant, request_id),
        access.ticket.id,
        &form.stage,
        &form.reason,
        kind,
    )
    .await;
    let dialog = if kind == AdminMove::Force {
        "force-dialog"
    } else {
        "jump-dialog"
    };
    done(&ctx, &access, session, r, dialog, &v, FLASH_STAGE_MOVED).await
}

#[debug_handler]
pub(super) async fn people(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: TicketAccess,
    session: Session,
    RequestId(request_id): RequestId,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    let ids = |name: &str| -> Vec<Uuid> {
        pairs
            .iter()
            .filter(|(k, _)| k == name)
            .filter_map(|(_, v)| Uuid::parse_str(v.trim()).ok())
            .collect()
    };
    let people = StagePeople {
        executors: ids("executors"),
        approvers: ids("approvers"),
        watchers: ids("watchers"),
    };
    let r = stages::set_people(
        &ctx.db,
        &acting(&access.access.tenant, request_id),
        access.ticket.id,
        &access.ticket.stage_key,
        people,
    )
    .await
    .map(|()| access.ticket.clone());
    done(
        &ctx,
        &access,
        session,
        r,
        "people-dialog",
        &v,
        FLASH_PEOPLE_SAVED,
    )
    .await
}

pub const FLASH_STAGE_MOVED: &str = "flash-stage-moved";
pub const FLASH_APPROVED: &str = "flash-stage-approved";
pub const FLASH_REJECTED: &str = "flash-stage-rejected";
pub const FLASH_DECIDED: &str = "flash-stage-decided";
pub const FLASH_PEOPLE_SAVED: &str = "flash-stage-people-saved";

/// Error aksi stage untuk template: kode umum (`stage`) dan per dialog.
#[must_use]
pub(super) fn errors_view(code: &str, dialog: &str) -> serde_json::Value {
    let pick = |names: &[&str]| -> &str { if names.contains(&dialog) { code } else { "" } };
    serde_json::json!({
        "stage": code,
        "in_dialog": !dialog.is_empty(),
        "reject": pick(&["reject-dialog"]),
        "admin_move": pick(&["jump-dialog", "force-dialog"]),
        "people": pick(&["people-dialog"]),
        "decision": pick(&["decision"]),
    })
}
