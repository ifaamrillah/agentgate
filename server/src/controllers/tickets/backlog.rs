//! Project → Backlog: sprint yang belum selesai (aktif lalu rencana) dan backlog (tiket tanpa
//! sprint yang belum selesai), urut `backlog_rank`. Buat/ubah/hapus/mulai/selesaikan sprint dan
//! pindahkan tiket (menu ⋯ di baris, drag-drop `backlog-dnd.js` memakai form yang sama).
//! Melihat butuh `ViewProject`, merencanakan `EditProject`.
use std::collections::HashMap;

use axum::http::StatusCode;
use loco_rs::prelude::*;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use super::{acting, board::project_definitions, lookup, project_paths, summary};
use crate::{
    controllers::{
        extractors::{ProjectAccess, RequestId},
        flash,
        shell::app_shell,
    },
    models::{
        permission::Permission,
        sprints::{self, SprintState},
        tickets,
    },
    services::{
        planning::{
            self, Failure, estimates,
            sprints::{self as service, CompleteTarget, Place, SprintInput},
        },
        tickets::TicketFailure,
    },
    views::{
        self, flash::Flash, form::field_errors, nav, planning as pv, projects::PROJECT_VIEWS_ACTION,
        ui::Stat,
    },
};

const FIELDS: &[&str] = &["name", "goal", "starts_on", "ends_on", "capacity", "sprint"];

fn base(access: &ProjectAccess) -> String {
    format!("{}/backlog", project_paths(access).home)
}

fn value(fields: &[(String, String)], name: &str) -> String {
    fields
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.clone())
        .unwrap_or_default()
}

fn sprint_input(fields: &[(String, String)]) -> SprintInput {
    SprintInput {
        name: value(fields, "name"),
        goal: value(fields, "goal"),
        starts_on: value(fields, "starts_on"),
        ends_on: value(fields, "ends_on"),
        capacity: value(fields, "capacity"),
    }
}

/// Dialog yang gagal divalidasi: id dialog, error, dan isian yang dikirim.
struct Failed<'a> {
    dialog: String,
    errors: &'a loco_rs::validation::ModelValidationErrors,
    input: SprintInput,
}

#[derive(Debug, serde::Deserialize, Default)]
pub(super) struct BacklogQuery {
    #[serde(default)]
    q: String,
}

fn matches(t: &tickets::Model, q: &str) -> bool {
    q.is_empty() || {
        let q = q.to_lowercase();
        t.code.to_lowercase().contains(&q) || t.title.to_lowercase().contains(&q)
    }
}

#[allow(clippy::too_many_lines)]
async fn render(
    ctx: &AppContext,
    v: &TeraView,
    access: &ProjectAccess,
    q: &str,
    flash: Option<Flash>,
    failed: Option<Failed<'_>>,
    status: StatusCode,
) -> Result<Response> {
    let db = &ctx.db;
    let wid = access.tenant.workspace_id();
    let pid = access.project.id;
    let home = project_paths(access).home;
    let base = base(access);
    let mut open: Vec<sprints::Model> = sprints::Model::of_project(db, wid, pid)
        .await?
        .into_iter()
        .filter(|s| s.state() != SprintState::Completed)
        .collect();
    open.sort_by_key(|s| (s.state() != SprintState::Active, s.starts_on, s.id));
    let open_ids: Vec<Uuid> = open.iter().map(|s| s.id).collect();
    let mut all: Vec<tickets::Model> = tickets::Entity::find()
        .filter(tickets::Column::WorkspaceId.eq(wid.get()))
        .filter(tickets::Column::ProjectId.eq(pid))
        .filter(tickets::Column::DeletedAt.is_null())
        .order_by_asc(tickets::Column::BacklogRank)
        .order_by_asc(tickets::Column::Id)
        .all(db)
        .await?;
    all.retain(|t| match t.sprint_id {
        Some(s) => open_ids.contains(&s),
        None => t.status != "done",
    });
    let total = all.len();
    let shown: Vec<tickets::Model> = all.iter().filter(|t| matches(t, q)).cloned().collect();
    let lookup = lookup(db, &access.tenant, &shown, &[]).await?;
    let stage_names: HashMap<String, String> = project_definitions(ctx, access, &shown)
        .await?
        .iter()
        .flat_map(|d| d.stages.iter().map(|s| (s.key.clone(), s.name.clone())))
        .collect();
    let ids: Vec<Uuid> = all.iter().map(|t| t.id).collect();
    let estimates = estimates::latest(db, wid, &ids).await?;
    let move_base = format!("{base}/move");
    let done = service::velocity(db, wid, pid, 6).await?;
    let avg = service::average_velocity(&done);

    let mut blocks = Vec::new();
    for s in &open {
        let mine: Vec<tickets::Model> = shown
            .iter()
            .filter(|t| t.sprint_id == Some(s.id))
            .cloned()
            .collect();
        let rows = pv::rows(&lookup, &mine, &stage_names, &estimates, &move_base);
        let today = crate::services::planning::today();
        let workdays_left = if today > s.ends_on {
            0
        } else {
            i64::try_from(crate::services::planning::workdays(today.max(s.starts_on), s.ends_on).len())
                .unwrap_or(0)
        };
        let mut block = pv::sprint_block(
            s,
            rows,
            service::capacity(s, avg),
            today,
            workdays_left,
            &format!("{base}/sprints"),
        );
        // Angka kepala sprint dihitung dari semua tiketnya, bukan hanya yang lolos pencarian.
        let every: Vec<&tickets::Model> =
            all.iter().filter(|t| t.sprint_id == Some(s.id)).collect();
        block.count = every.len();
        block.points = every.iter().filter_map(|t| t.story_points).sum();
        block.done_points = every
            .iter()
            .filter(|t| t.status == "done")
            .filter_map(|t| t.story_points)
            .sum();
        block.over = block.capacity.is_some_and(|c| block.points > c);
        blocks.push(block);
    }
    let targets: Vec<pv::SprintTarget> = blocks
        .iter()
        .filter(|b| b.state == "planned")
        .map(pv::SprintBlock::as_target)
        .collect();
    for b in &mut blocks {
        b.finish_setup(&targets);
    }
    let backlog_tickets: Vec<tickets::Model> = shown
        .iter()
        .filter(|t| t.sprint_id.is_none())
        .cloned()
        .collect();
    let mut backlog = pv::backlog_block(pv::rows(
        &lookup,
        &backlog_tickets,
        &stage_names,
        &estimates,
        &move_base,
    ));
    let every_backlog: Vec<&tickets::Model> = all.iter().filter(|t| t.sprint_id.is_none()).collect();
    backlog.count = every_backlog.len();
    backlog.points = every_backlog.iter().filter_map(|t| t.story_points).sum();

    // Statistik kepala.
    let calib = estimates::calibration(db, wid, pid).await?;
    let mut stats = Vec::new();
    match blocks.iter().find(|b| b.state == "active") {
        Some(b) => {
            stats.push(
                Stat::new("backlog-stat-active", b.done_points, "flag")
                    .of(b.points)
                    .tone("accent")
                    .hint("backlog-stat-active-hint", &b.name),
            );
        }
        None => stats.push(Stat::new("backlog-stat-no-active", "–", "flag")),
    }
    stats.push(match avg {
        Some(a) => Stat::new("backlog-stat-velocity", pv::decimal(a), "chart-line")
            .hint("backlog-stat-velocity-hint", done.len().min(planning::sprints::VELOCITY_WINDOW)),
        None => Stat::new("backlog-stat-velocity", "–", "chart-line")
            .hint("backlog-stat-velocity-none", ""),
    });
    stats.push(
        Stat::new(
            "backlog-stat-accuracy",
            calib
                .accuracy
                .map_or_else(|| "–".to_string(), |a| format!("{}%", (a * 100.0).round())),
            "sparkles",
        )
        .tone(if calib.accuracy.is_some() { "success" } else { "" })
        .hint("backlog-stat-accuracy-hint", calib.points.len())
        .link(format!("{home}/reports?tab=calibration")),
    );
    if let Some(active) = open.iter().find(|s| s.state() == SprintState::Active) {
        let in_sprint: Vec<&tickets::Model> =
            all.iter().filter(|t| t.sprint_id == Some(active.id)).collect();
        let left: i32 = in_sprint
            .iter()
            .filter(|t| t.status != "done")
            .filter_map(|t| t.story_points)
            .sum();
        let hours: f64 = in_sprint
            .iter()
            .filter_map(|t| {
                estimates::remaining_hours(
                    t,
                    estimates.get(&t.id).and_then(|e| e.current.as_ref()),
                    &calib,
                )
            })
            .sum();
        let finish = estimates::finish_date(planning::today(), hours);
        let late = hours > 0.0 && finish > active.ends_on;
        stats.push(
            Stat::new("backlog-stat-forecast", left, "calendar")
                .tone(if late { "warn" } else { "" })
                .hint(
                    if late {
                        "backlog-stat-forecast-late"
                    } else {
                        "backlog-stat-forecast-hint"
                    },
                    pv::long_day(finish),
                ),
        );
    }

    let suggestion = service::suggestion(db, wid, pid).await?;
    let (failed_dialog, errors, input) = match &failed {
        Some(f) => (
            f.dialog.clone(),
            field_errors(FIELDS, Some(f.errors)),
            Some(&f.input),
        ),
        None => (String::new(), field_errors(FIELDS, None), None),
    };
    let new_input = input.filter(|_| failed_dialog == "sprint-new").unwrap_or(&suggestion);
    let shell = app_shell(ctx, &access.tenant.user, Some(&access.tenant), Some(access))
        .await?
        .at("project_backlog");
    views::tickets::render(
        v,
        &shell,
        "tickets/backlog.html",
        status,
        data!({
            "project": summary(access),
            "views": nav::project_views(&home, &access.tenant.user.project_views, "backlog"),
            "views_action": PROJECT_VIEWS_ACTION,
            "return_to": base,
            "base": base,
            "q": q,
            "total": total,
            "shown": shown.len(),
            "stats": stats,
            "blocks": blocks.iter().chain(std::iter::once(&backlog)).collect::<Vec<_>>(),
            "sprints": blocks,
            "can_edit": access.role.has(Permission::EditProject),
            "new_ticket": format!("{home}/tickets/new"),
            "reports": format!("{home}/reports"),
            "new_action": format!("{base}/sprints"),
            "new_input": {
                "name": new_input.name, "goal": new_input.goal, "starts_on": new_input.starts_on,
                "ends_on": new_input.ends_on, "capacity": new_input.capacity,
            },
            "failed_dialog": failed_dialog,
            "failed_input": input.map(|i| serde_json::json!({
                "name": i.name, "goal": i.goal, "starts_on": i.starts_on,
                "ends_on": i.ends_on, "capacity": i.capacity, "capacity_input": i.capacity,
            })),
            "errors": errors,
            "blank": field_errors(FIELDS, None),
            "flash": flash,
        }),
    )
}

#[debug_handler]
pub(super) async fn index(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: ProjectAccess,
    session: Session,
    Query(query): Query<BacklogQuery>,
) -> Result<Response> {
    access.require(Permission::ViewProject)?;
    let (session, flash) = flash::take(session);
    let res = render(&ctx, &v, &access, query.q.trim(), flash, None, StatusCode::OK).await?;
    Ok((session, res).into_response())
}

/// Hasil POST: berhasil → flash + kembali ke backlog; validasi gagal → render ulang dengan
/// dialog terbuka; lainnya → error.
async fn finish<T>(
    ctx: &AppContext,
    v: &TeraView,
    access: &ProjectAccess,
    session: Session,
    result: Result<T, Failure>,
    ok_flash: &str,
    dialog: String,
    input: SprintInput,
) -> Result<Response> {
    match result {
        Ok(_) => {
            let session = flash::set(session, &Flash::success(ok_flash))?;
            Ok((session, format::redirect(&base(access))?).into_response())
        }
        Err(TicketFailure::Invalid(errors)) => {
            let res = render(
                ctx,
                v,
                access,
                "",
                None,
                Some(Failed {
                    dialog,
                    errors: &errors,
                    input,
                }),
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await?;
            Ok((session, res).into_response())
        }
        Err(e) => Err(e.into()),
    }
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
    let input = sprint_input(&fields);
    let result = service::create(
        &ctx.db,
        &acting(&access.tenant, request_id),
        access.project.id,
        &input,
    )
    .await;
    finish(&ctx, &v, &access, session, result, pv::FLASH_CREATED, "sprint-new".into(), input).await
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
    let input = sprint_input(&fields);
    let result = service::update(&ctx.db, &acting(&access.tenant, request_id), id, &input).await;
    finish(&ctx, &v, &access, session, result, pv::FLASH_SAVED, format!("sprint-edit-{id}"), input)
        .await
}

#[debug_handler]
pub(super) async fn delete(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: ProjectAccess,
    session: Session,
    RequestId(request_id): RequestId,
    Path((_slug, _key, id)): Path<(String, String, Uuid)>,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    let result = service::delete(&ctx.db, &acting(&access.tenant, request_id), id).await;
    finish(&ctx, &v, &access, session, result, pv::FLASH_DELETED, String::new(), SprintInput::default())
        .await
}

#[debug_handler]
pub(super) async fn start(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: ProjectAccess,
    session: Session,
    RequestId(request_id): RequestId,
    Path((_slug, _key, id)): Path<(String, String, Uuid)>,
    Form(fields): Form<Vec<(String, String)>>,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    let input = sprint_input(&fields);
    let given = !input.name.trim().is_empty();
    let result = service::start(
        &ctx.db,
        &acting(&access.tenant, request_id),
        id,
        given.then_some(&input),
    )
    .await;
    finish(&ctx, &v, &access, session, result, pv::FLASH_STARTED, format!("sprint-start-{id}"), input)
        .await
}

#[debug_handler]
pub(super) async fn complete(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: ProjectAccess,
    session: Session,
    RequestId(request_id): RequestId,
    Path((_slug, _key, id)): Path<(String, String, Uuid)>,
    Form(fields): Form<Vec<(String, String)>>,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    let target = CompleteTarget::parse(&value(&fields, "target"))
        .ok_or_else(|| Error::BadRequest("target".into()))?;
    let result = service::complete(&ctx.db, &acting(&access.tenant, request_id), id, target).await;
    finish(
        &ctx,
        &v,
        &access,
        session,
        result,
        pv::FLASH_COMPLETED,
        format!("sprint-complete-{id}"),
        SprintInput::default(),
    )
    .await
}

/// Pindah tiket: `to` = id sprint atau `backlog` (kosong = tetap), `place` = top|bottom|keep.
#[debug_handler]
pub(super) async fn move_ticket(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: ProjectAccess,
    session: Session,
    RequestId(request_id): RequestId,
    Path((_slug, _key, id)): Path<(String, String, Uuid)>,
    Form(fields): Form<Vec<(String, String)>>,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    let acting = acting(&access.tenant, request_id);
    let ticket = tickets::Model::find_in(&ctx.db, access.tenant.workspace_id(), id)
        .await?
        .filter(|t| t.project_id == access.project.id && !t.is_trashed())
        .ok_or(Error::NotFound)?;
    let to = match value(&fields, "to").trim() {
        "" => ticket.sprint_id,
        "backlog" => None,
        raw => Some(Uuid::parse_str(raw).map_err(|_| Error::BadRequest("to".into()))?),
    };
    let place = match value(&fields, "place").as_str() {
        "top" => Place::Top,
        "bottom" => Place::Bottom,
        _ => Place::Keep,
    };
    let result = service::move_ticket(&ctx.db, &acting, id, to, place).await;
    finish(&ctx, &v, &access, session, result, pv::FLASH_MOVED, String::new(), SprintInput::default())
        .await
}
