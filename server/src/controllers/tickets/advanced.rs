//! Pencarian lanjutan (query ala JQL, [`planning::jql`]) lintas project yang boleh dibuka, saved
//! filter (kiri), dan edit massal hasil (prioritas, penerima, sprint, Fix version, tambah label)
//! lewat jalur ubah tiket biasa (izin, validasi rujukan, riwayat, log scope sprint per tiket).
use std::collections::HashMap;

use axum::http::StatusCode;
use loco_rs::prelude::*;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::Serialize;
use uuid::Uuid;

use super::{acting, lookup, ticket::current_form};
use crate::{
    controllers::{
        extractors::{RequestId, Tenant},
        flash,
        shell::app_shell,
    },
    models::{
        components, projects, releases, saved_filters, sprints,
        sprints::SprintState,
        teams, tickets, workflows,
    },
    services::{
        planning::{
            BULK_EMPTY,
            filters::{self, FilterInput},
            jql::{self, Facts, Field},
        },
        tickets::{TicketFailure, update},
    },
    views::{self, flash::Flash, tickets::TicketRow},
};

/// Tiket maksimal yang dievaluasi dan baris maksimal yang ditampilkan.
const SCAN_LIMIT: u64 = 5000;
const SHOW_LIMIT: usize = 200;

pub const FLASH_FILTER_SAVED: &str = "search-flash-filter-saved";
pub const FLASH_FILTER_DELETED: &str = "search-flash-filter-deleted";
pub const FLASH_BULK_DONE: &str = "search-flash-bulk-done";
pub const FLASH_BULK_PARTIAL: &str = "search-flash-bulk-partial";

/// Kunci Fluent yang dibangun di kode (dites kontrak i18n).
#[must_use]
pub fn label_keys() -> Vec<String> {
    [
        FLASH_FILTER_SAVED,
        FLASH_FILTER_DELETED,
        FLASH_BULK_DONE,
        FLASH_BULK_PARTIAL,
    ]
    .iter()
    .map(|k| (*k).to_string())
    .collect()
}

fn base(tenant: &Tenant) -> String {
    format!("/w/{}/search/advanced", tenant.workspace.slug)
}

fn value(fields: &[(String, String)], name: &str) -> String {
    fields
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.clone())
        .unwrap_or_default()
}

#[derive(Debug, Serialize)]
struct Row {
    #[serde(flatten)]
    row: TicketRow,
    stage: String,
}

#[derive(Debug, Serialize)]
struct FilterView {
    id: Uuid,
    name: String,
    query: String,
    visibility: String,
    mine: bool,
    href: String,
    current: bool,
    delete_action: String,
}

#[derive(Debug, Serialize, Clone)]
struct Opt {
    value: String,
    label: String,
}

#[allow(clippy::too_many_lines)]
async fn render(
    ctx: &AppContext,
    v: &TeraView,
    tenant: &Tenant,
    q: &str,
    current_filter: Option<Uuid>,
    flash: Option<Flash>,
    save_error: Option<String>,
) -> Result<Response> {
    let db = &ctx.db;
    let wid = tenant.workspace_id();
    let me = tenant.user.id;
    let base = base(tenant);
    let parsed = jql::parse(q);
    let projects = projects::Model::accessible_for(db, &tenant.accessor()).await?;
    let pids: Vec<Uuid> = projects.iter().map(|p| p.id).collect();

    let mut rows: Vec<Row> = Vec::new();
    let mut total = 0usize;
    let mut sprint_opts: Vec<Opt> = Vec::new();
    let mut release_opts: Vec<Opt> = Vec::new();
    let mut people_opts: Vec<Opt> = Vec::new();
    if let Ok(query) = &parsed {
        let mut list = tickets::Entity::find()
            .filter(tickets::Column::WorkspaceId.eq(wid.get()))
            .filter(tickets::Column::ProjectId.is_in(pids.clone()))
            .filter(tickets::Column::DeletedAt.is_null())
            .order_by_desc(tickets::Column::UpdatedAt)
            .order_by_desc(tickets::Column::Id)
            .limit(SCAN_LIMIT)
            .all(db)
            .await?;
        let lookup = lookup(db, tenant, &list, &[]).await?.with_projects(
            projects
                .iter()
                .map(|p| (p.id, p.name.as_str(), p.key.as_str())),
        );
        // Nama stage per workflow, sprint/rilis/component per id.
        let mut stage_names: HashMap<(Uuid, String), String> = HashMap::new();
        let mut wf_ids: Vec<Uuid> = list.iter().map(|t| t.workflow_id).collect();
        wf_ids.sort();
        wf_ids.dedup();
        for id in wf_ids {
            if let Some(w) = workflows::Model::find_in(db, wid, id).await? {
                for s in w.definition(db).await?.stages {
                    stage_names.insert((id, s.key.clone()), s.name.clone());
                }
            }
        }
        let all_sprints = sprints::Entity::find()
            .filter(sprints::Column::WorkspaceId.eq(wid.get()))
            .filter(sprints::Column::ProjectId.is_in(pids.clone()))
            .all(db)
            .await?;
        let all_releases = releases::Entity::find()
            .filter(releases::Column::WorkspaceId.eq(wid.get()))
            .filter(releases::Column::ProjectId.is_in(pids.clone()))
            .all(db)
            .await?;
        let all_components = components::Entity::find()
            .filter(components::Column::WorkspaceId.eq(wid.get()))
            .filter(components::Column::ProjectId.is_in(pids.clone()))
            .all(db)
            .await?;
        let key_of = |pid: Uuid| {
            projects
                .iter()
                .find(|p| p.id == pid)
                .map(|p| p.key.clone())
                .unwrap_or_default()
        };
        let facts = |t: &tickets::Model| {
            let row = lookup.row(t);
            let sprint = t.sprint_id.and_then(|id| all_sprints.iter().find(|s| s.id == id));
            let assignee = lookup.person(t.assignee_id);
            Facts {
                project: key_of(t.project_id).to_lowercase(),
                stage: vec![
                    t.stage_key.to_lowercase(),
                    stage_names
                        .get(&(t.workflow_id, t.stage_key.clone()))
                        .map(|n| n.to_lowercase())
                        .unwrap_or_default(),
                ],
                status: t.status.clone(),
                assignee: assignee
                    .map(|p| vec![p.name.to_lowercase(), p.email.to_lowercase(), p.id.to_string()])
                    .unwrap_or_default(),
                assignee_id: t.assignee_id,
                ticket_type: vec![
                    row.ticket_type.key.to_lowercase(),
                    row.ticket_type.name.to_lowercase(),
                ],
                sprint: sprint.map(|s| s.name.to_lowercase()),
                sprint_open: sprint.is_some_and(|s| s.state() != SprintState::Completed),
                labels: t.labels.iter().map(|l| l.to_lowercase()).collect(),
                priority: t.priority.clone(),
                points: t.story_points,
                text: format!("{} {} {}", t.code, t.title, t.description).to_lowercase(),
                release: t
                    .release_id
                    .and_then(|id| all_releases.iter().find(|r| r.id == id))
                    .map(|r| r.name.to_lowercase()),
                component: t
                    .component_id
                    .and_then(|id| all_components.iter().find(|c| c.id == id))
                    .map(|c| c.name.to_lowercase()),
                due: t.due_on,
            }
        };
        list.retain(|t| jql::matches(query, &facts(t), me));
        if let Some((field, desc)) = query.order {
            let rank = |p: &str| ["urgent", "high", "medium", "low"].iter().position(|x| *x == p);
            match field {
                Field::Priority => list.sort_by_key(|t| rank(&t.priority)),
                Field::Points => list.sort_by_key(|t| t.story_points),
                Field::Due => list.sort_by_key(|t| t.due_on),
                Field::Created => list.sort_by_key(|t| t.created_at),
                Field::Updated => list.sort_by_key(|t| t.updated_at),
                _ => list.sort_by(|a, b| a.code.cmp(&b.code)),
            }
            // Prioritas: urutan naik = paling mendesak dulu (peringkat 0 = urgent).
            if desc {
                list.reverse();
            }
        }
        total = list.len();
        list.truncate(SHOW_LIMIT);
        for t in &list {
            if let Some(p) = lookup.person(t.assignee_id)
                && !people_opts.iter().any(|o| o.value == p.id.to_string())
            {
                people_opts.push(Opt {
                    value: p.id.to_string(),
                    label: p.name,
                });
            }
            rows.push(Row {
                row: lookup.row(t),
                stage: stage_names
                    .get(&(t.workflow_id, t.stage_key.clone()))
                    .cloned()
                    .unwrap_or_else(|| t.stage_key.clone()),
            });
        }
        let in_results: Vec<Uuid> = list.iter().map(|t| t.project_id).collect();
        for s in all_sprints
            .iter()
            .filter(|s| s.state() != SprintState::Completed && in_results.contains(&s.project_id))
        {
            sprint_opts.push(Opt {
                value: s.id.to_string(),
                label: format!("{} · {}", key_of(s.project_id), s.name),
            });
        }
        for r in all_releases.iter().filter(|r| {
            in_results.contains(&r.project_id)
                && !matches!(
                    r.status(),
                    releases::ReleaseStatus::Released | releases::ReleaseStatus::Archived
                )
        }) {
            release_opts.push(Opt {
                value: r.id.to_string(),
                label: format!("{} · {}", key_of(r.project_id), r.name),
            });
        }
    }

    let acting = acting(tenant, None);
    let saved: Vec<FilterView> = filters::visible(db, &acting)
        .await?
        .into_iter()
        .map(|f| FilterView {
            href: format!(
                "{base}?{}",
                serde_urlencoded::to_string([("q", f.query.as_str()), ("filter", &f.id.to_string())])
                    .unwrap_or_default()
            ),
            current: current_filter == Some(f.id),
            mine: f.owner_id == me,
            delete_action: format!("{base}/filters/{}/delete", f.id),
            visibility: f.visibility.clone(),
            id: f.id,
            name: f.name,
            query: f.query,
        })
        .collect();
    let my_teams = filters::team_ids(db, &acting).await?;
    let team_opts: Vec<Opt> = teams::Model::in_workspace(db, wid)
        .await?
        .into_iter()
        .filter(|t| my_teams.contains(&t.id))
        .map(|t| Opt {
            value: t.id.to_string(),
            label: t.name,
        })
        .collect();
    let fields: Vec<serde_json::Value> = jql::FIELDS
        .iter()
        .map(|(k, _, hint)| serde_json::json!({ "name": k, "hint": hint }))
        .collect();
    let priorities: Vec<Opt> = ["urgent", "high", "medium", "low"]
        .iter()
        .map(|p| Opt {
            value: (*p).to_string(),
            label: format!("ticket-priority-{p}"),
        })
        .collect();
    let shell = app_shell(ctx, &tenant.user, Some(tenant), None)
        .await?
        .at("search");
    let status = if save_error.is_some() {
        StatusCode::UNPROCESSABLE_ENTITY
    } else {
        StatusCode::OK
    };
    views::tickets::render(
        v,
        &shell,
        "tickets/advanced.html",
        status,
        serde_json::json!({
            "base": base,
            "basic": format!("/w/{}/search", tenant.workspace.slug),
            "q": q,
            "error": parsed.as_ref().err(),
            "rows": rows,
            "total": total,
            "limited": total > SHOW_LIMIT,
            "limit": SHOW_LIMIT,
            "saved": saved,
            "teams": team_opts,
            "fields": fields,
            "sprints": sprint_opts,
            "releases": release_opts,
            "people": people_opts,
            "priorities": priorities,
            "me": me.to_string(),
            "save_error": save_error,
            "flash": flash,
            "return_to": format!(
                "{base}?{}",
                serde_urlencoded::to_string([("q", q)]).unwrap_or_default()
            ),
        }),
    )
}

#[derive(Debug, serde::Deserialize, Default)]
pub(super) struct AdvancedQuery {
    #[serde(default)]
    q: String,
    #[serde(default)]
    filter: Option<Uuid>,
}

#[debug_handler]
pub(super) async fn index(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    tenant: Tenant,
    session: Session,
    Query(query): Query<AdvancedQuery>,
) -> Result<Response> {
    let (session, flash) = flash::take(session);
    let q: String = query.q.trim().chars().take(saved_filters::QUERY_MAX_LEN).collect();
    let res = render(&ctx, &v, &tenant, &q, query.filter, flash, None).await?;
    Ok((session, res).into_response())
}

/// Simpan saved filter: `name`, `query`, `visibility` (`private`/`team`/`workspace`), `team`.
#[debug_handler]
pub(super) async fn save_filter(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    tenant: Tenant,
    session: Session,
    RequestId(request_id): RequestId,
    Form(fields): Form<Vec<(String, String)>>,
) -> Result<Response> {
    let input = FilterInput {
        name: value(&fields, "name"),
        query: value(&fields, "query"),
        visibility: value(&fields, "visibility"),
        team: value(&fields, "team"),
    };
    match filters::create(&ctx.db, &acting(&tenant, request_id), &input).await {
        Ok(f) => {
            let session = flash::set(session, &Flash::success(FLASH_FILTER_SAVED))?;
            let to = format!(
                "{}?{}",
                base(&tenant),
                serde_urlencoded::to_string([("q", f.query.as_str()), ("filter", &f.id.to_string())])
                    .unwrap_or_default()
            );
            Ok((session, format::redirect(&to)?).into_response())
        }
        Err(TicketFailure::Invalid(errors)) => {
            let code = errors
                .errors
                .values()
                .flatten()
                .map(|e| e.code.to_string())
                .next()
                .unwrap_or_default();
            let res = render(&ctx, &v, &tenant, input.query.trim(), None, None, Some(code)).await?;
            Ok((session, res).into_response())
        }
        Err(e) => Err(e.into()),
    }
}

#[debug_handler]
pub(super) async fn delete_filter(
    State(ctx): State<AppContext>,
    tenant: Tenant,
    session: Session,
    RequestId(request_id): RequestId,
    Path((_slug, id)): Path<(String, Uuid)>,
) -> Result<Response> {
    match filters::delete(&ctx.db, &acting(&tenant, request_id), id).await {
        Ok(()) => {
            let session = flash::set(session, &Flash::success(FLASH_FILTER_DELETED))?;
            Ok((session, format::redirect(&base(&tenant))?).into_response())
        }
        Err(TicketFailure::NotFound) => Err(Error::NotFound),
        Err(e) => Err(e.into()),
    }
}

/// Edit massal: `ids` (berulang), lalu field yang diisi saja yang berubah: `priority`,
/// `assignee` (id / `none`), `sprint` (id / `backlog`), `release` (id / `none`), `label`
/// (ditambahkan). Tiket yang gagal (izin, rujukan tidak cocok project-nya) dilewati.
#[debug_handler]
pub(super) async fn bulk(
    State(ctx): State<AppContext>,
    tenant: Tenant,
    session: Session,
    RequestId(request_id): RequestId,
    Form(fields): Form<Vec<(String, String)>>,
) -> Result<Response> {
    let ids: Vec<Uuid> = fields
        .iter()
        .filter(|(k, _)| k == "ids")
        .filter_map(|(_, v)| Uuid::parse_str(v).ok())
        .take(SHOW_LIMIT)
        .collect();
    let priority = value(&fields, "priority");
    let assignee = value(&fields, "assignee");
    let sprint = value(&fields, "sprint");
    let release = value(&fields, "release");
    let label = value(&fields, "label").trim().to_string();
    let back = {
        let r = value(&fields, "return_to");
        if r.starts_with(&base(&tenant)) { r } else { base(&tenant) }
    };
    if ids.is_empty()
        || (priority.is_empty()
            && assignee.is_empty()
            && sprint.is_empty()
            && release.is_empty()
            && label.is_empty())
    {
        let session = flash::set(session, &Flash::error(BULK_EMPTY))?;
        return Ok((session, format::redirect(&back)?).into_response());
    }
    let acting = acting(&tenant, request_id);
    let wid = tenant.workspace_id();
    let (mut done, mut skipped) = (0usize, 0usize);
    for id in ids {
        let Some(t) = tickets::Model::find_in(&ctx.db, wid, id).await? else {
            skipped += 1;
            continue;
        };
        let parent = match t.parent_id {
            Some(p) => tickets::Model::find_in(&ctx.db, wid, p).await?.map(|x| x.code),
            None => None,
        };
        let mut form = current_form(&t, parent.as_deref());
        if !priority.is_empty() {
            form.input.priority.clone_from(&priority);
        }
        match assignee.as_str() {
            "" => {}
            "none" => form.assignee = String::new(),
            other => form.assignee = other.to_string(),
        }
        match sprint.as_str() {
            "" => {}
            "backlog" => form.sprint = Some(String::new()),
            other => form.sprint = Some(other.to_string()),
        }
        match release.as_str() {
            "" => {}
            "none" => form.release = Some(String::new()),
            other => form.release = Some(other.to_string()),
        }
        if !label.is_empty() && !t.labels.iter().any(|l| l.eq_ignore_ascii_case(&label)) {
            form.input.labels = if form.input.labels.trim().is_empty() {
                label.clone()
            } else {
                format!("{}, {label}", form.input.labels)
            };
        }
        match update::run(&ctx.db, &acting, t.id, &form).await {
            Ok(_) => done += 1,
            Err(TicketFailure::Invalid(_) | TicketFailure::NotFound) => skipped += 1,
            Err(e) => return Err(e.into()),
        }
    }
    let flash = if skipped == 0 {
        Flash::success(FLASH_BULK_DONE)
    } else if done == 0 {
        Flash::error(FLASH_BULK_PARTIAL)
    } else {
        Flash::success(FLASH_BULK_PARTIAL)
    };
    let session = flash::set(session, &flash)?;
    Ok((session, format::redirect(&back)?).into_response())
}
