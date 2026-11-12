//! Halaman tiket di dalam project: board, daftar, tiket baru, opsi orang project, dan sampah.
use std::collections::{BTreeMap, HashMap};

use axum::http::StatusCode;
use loco_rs::{prelude::*, validation::ModelValidationErrors};
use serde::Deserialize;
use uuid::Uuid;

use super::{acting, lookup, project_paths, status_priority_options, summary, table_page};
use crate::{
    controllers::{
        extractors::{IdOr404, ProjectAccess, RequestId},
        flash,
        shell::app_shell,
    },
    models::{
        custom_fields,
        permission::Permission,
        project_access, ticket_type_fields, ticket_types,
        tickets::{self, PersonFilter, TicketInput, TicketQuery},
    },
    services::tickets::{
        self as service, TicketFailure,
        create::{TicketForm, run as create_ticket},
        trash::can_purge,
    },
    views::{
        self,
        datatable::FilterOption,
        flash::Flash,
        nav,
        projects::PROJECT_VIEWS_ACTION,
        tickets::{
            BoardFilter, BoardPaths, LIST_TABLE, TicketFormView, TypeView, board_columns,
            custom_inputs, priority_options, status_options, ticket_href,
        },
        ui::ComboOption,
    },
};

/// Batas kartu yang dimuat board (board besar perlu paging per kolom; tercatat di PROGRESS).
const BOARD_LIMIT: u64 = 500;
/// Batas opsi orang di form tiket.
const PEOPLE_LIMIT: u64 = 50;

/// Isian form tiket dari body mentah. `current` = nilai tiket sekarang untuk field yang tidak
/// dikirim (ubah sebagian dari detail tiket).
pub(super) fn ticket_form(pairs: &[(String, String)], current: Option<&TicketForm>) -> TicketForm {
    let get = |name: &str, fallback: Option<&String>| -> String {
        pairs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
            .or_else(|| fallback.cloned())
            .unwrap_or_default()
    };
    let base = current.cloned().unwrap_or_default();
    let mut custom: BTreeMap<Uuid, String> = base.custom.clone();
    for (key, value) in pairs {
        if let Some(id) = key
            .strip_prefix("custom_")
            .and_then(|id| Uuid::parse_str(id).ok())
        {
            custom.insert(id, value.clone());
        }
    }
    let input = &base.input;
    TicketForm {
        input: TicketInput {
            title: get("title", current.map(|_| &input.title)),
            description: get("description", current.map(|_| &input.description)),
            status: get("status", current.map(|_| &input.status)),
            priority: get("priority", current.map(|_| &input.priority)),
            labels: get("labels", current.map(|_| &input.labels)),
            due_on: get("due_on", current.map(|_| &input.due_on)),
            story_points: get("story_points", current.map(|_| &input.story_points)),
        },
        type_id: get("type_id", current.map(|_| &base.type_id)),
        assignee: get("assignee", current.map(|_| &base.assignee)),
        parent: get("parent", current.map(|_| &base.parent)),
        custom,
        stage: get("stage", None),
    }
}

/// Tampilan form dari isian mentah.
pub(super) fn form_view(
    form: &TicketForm,
    errors: Option<&ModelValidationErrors>,
) -> TicketFormView {
    TicketFormView {
        title: form.input.title.clone(),
        description: form.input.description.clone(),
        type_id: form.type_id.clone(),
        status: form.input.status.clone(),
        priority: form.input.priority.clone(),
        assignee: form.assignee.clone(),
        parent: form.parent.clone(),
        labels: form.input.labels.clone(),
        due_on: form.input.due_on.clone(),
        story_points: form.input.story_points.clone(),
        errors: Default::default(),
    }
    .with_errors(errors)
}

/// Opsi orang project (penerima tugas); `selected` selalu ikut walau di luar batas.
pub(super) async fn people_options(
    ctx: &AppContext,
    access: &ProjectAccess,
    q: Option<&str>,
    selected: Option<Uuid>,
) -> Result<Vec<ComboOption>> {
    let wid = access.tenant.workspace_id();
    let mut people =
        project_access::users_with_access(&ctx.db, wid, access.project.id, q, PEOPLE_LIMIT).await?;
    if let Some(id) = selected.filter(|id| !people.iter().any(|u| u.id == *id)) {
        people.extend(super::users_by_id(&ctx.db, [id]).await?);
    }
    Ok(people
        .iter()
        .map(|u| ComboOption::person(u.id, &u.name, &u.email))
        .collect())
}

/// Field custom form tiket (semua field yang dipakai tipe; JS menyaring per tipe terpilih).
pub(super) async fn custom_for_form(
    ctx: &AppContext,
    access: &ProjectAccess,
    form: &TicketForm,
    errors: Option<&ModelValidationErrors>,
) -> Result<Vec<views::tickets::CustomFieldInput>> {
    let wid = access.tenant.workspace_id();
    let fields = custom_fields::Model::all_in(&ctx.db, wid).await?;
    let links: Vec<(Uuid, Uuid)> = ticket_type_fields::Model::all_in(&ctx.db, wid)
        .await?
        .iter()
        .map(|l| (l.type_id, l.field_id))
        .collect();
    let values: HashMap<Uuid, String> = form.custom.clone().into_iter().collect();
    Ok(custom_inputs(&fields, &links, &values, errors))
}

async fn render_new(
    ctx: &AppContext,
    v: &TeraView,
    access: &ProjectAccess,
    form: &TicketForm,
    errors: Option<&ModelValidationErrors>,
    status: StatusCode,
) -> Result<Response> {
    let wid = access.tenant.workspace_id();
    let home = project_paths(access).home;
    let paths = BoardPaths::new(&home);
    let types: Vec<TypeView> = ticket_types::Model::all_in(&ctx.db, wid)
        .await?
        .iter()
        .map(TypeView::of)
        .collect();
    let selected = Uuid::parse_str(form.assignee.trim()).ok();
    let shell = app_shell(ctx, &access.tenant.user, Some(&access.tenant), Some(access))
        .await?
        .at("project_board");
    let mut view = form_view(form, errors);
    if view.type_id.is_empty() {
        view.type_id = types
            .iter()
            .find(|t| t.key == "task")
            .map(|t| t.id.to_string())
            .unwrap_or_default();
    }
    if view.status.is_empty() {
        view.status = "todo".into();
    }
    if view.priority.is_empty() {
        view.priority = "medium".into();
    }
    views::tickets::render(
        v,
        &shell,
        "tickets/new.html",
        status,
        data!({
            "project": summary(access),
            "views": nav::project_views(&home, &access.tenant.user.project_views, "board"),
            "views_action": PROJECT_VIEWS_ACTION,
            "return_to": paths.new_ticket,
            "form": view,
            "types": types,
            "statuses": status_options(),
            "priorities": priority_options(),
            "people": people_options(ctx, access, None, selected).await?,
            "custom": custom_for_form(ctx, access, form, errors).await?,
            "people_src": paths.people,
            "paths": paths,
        }),
    )
}

#[derive(Debug, Default, Deserialize)]
pub struct NewQuery {
    #[serde(default)]
    status: String,
    #[serde(default)]
    parent: String,
}

#[debug_handler]
pub(super) async fn new(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: ProjectAccess,
    Query(query): Query<NewQuery>,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    let mut form = TicketForm::default();
    form.input.status = tickets::TicketStatus::parse(&query.status)
        .map(|s| views::tickets::status_view(s).value.to_string())
        .unwrap_or_default();
    form.parent = query.parent.trim().chars().take(64).collect();
    render_new(&ctx, &v, &access, &form, None, StatusCode::OK).await
}

/// `ProjectAccess` + `EditProject` sebelum body dibaca (rules §6).
#[debug_handler]
pub(super) async fn create(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: ProjectAccess,
    RequestId(request_id): RequestId,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    let form = ticket_form(&pairs, None);
    match create_ticket(
        &ctx.db,
        &acting(&access.tenant, request_id),
        access.project.id,
        &form,
    )
    .await
    {
        Ok(ticket) => format::redirect(&ticket_href(&access.tenant.workspace.slug, &ticket.code)),
        Err(TicketFailure::Invalid(errors)) => {
            render_new(
                &ctx,
                &v,
                &access,
                &form,
                Some(&errors),
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => Err(e.into()),
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct BoardQuery {
    #[serde(default)]
    q: String,
    #[serde(default)]
    who: String,
}

/// Href board dengan filter `q` dan `who`.
fn board_href(base: &str, q: &str, who: &str) -> String {
    let mut pairs = Vec::new();
    if !q.is_empty() {
        pairs.push(("q", q));
    }
    if !who.is_empty() {
        pairs.push(("who", who));
    }
    if pairs.is_empty() {
        base.to_string()
    } else {
        format!(
            "{base}?{}",
            serde_urlencoded::to_string(&pairs).unwrap_or_default()
        )
    }
}

#[debug_handler]
pub(super) async fn board(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: ProjectAccess,
    session: Session,
    Query(query): Query<BoardQuery>,
) -> Result<Response> {
    access.require(Permission::ViewProject)?;
    let (session, flash) = flash::take(session);
    let wid = access.tenant.workspace_id();
    let q: String = query.q.trim().chars().take(200).collect();
    let who = match query.who.as_str() {
        "me" | "none" => query.who.as_str(),
        _ => "",
    };
    let people = match who {
        "me" => vec![PersonFilter::AssignedTo(access.tenant.user.id)],
        "none" => vec![PersonFilter::Unassigned],
        _ => Vec::new(),
    };
    let tickets = tickets::Model::board(
        &ctx.db,
        wid,
        &TicketQuery {
            project_ids: vec![access.project.id],
            q: Some(q.clone()).filter(|q| !q.is_empty()),
            people,
            limit: BOARD_LIMIT,
            ..TicketQuery::default()
        },
    )
    .await?;
    let lookup = lookup(&ctx.db, &access.tenant, &tickets, &[]).await?;
    let home = project_paths(&access).home;
    let paths = BoardPaths::new(&home);
    let filter = BoardFilter {
        mine_href: board_href(&paths.board, &q, if who == "me" { "" } else { "me" }),
        unassigned_href: board_href(&paths.board, &q, if who == "none" { "" } else { "none" }),
        clear_href: paths.board.clone(),
        mine: who == "me",
        unassigned: who == "none",
        total: tickets.len(),
        q,
    };
    let shell = app_shell(
        &ctx,
        &access.tenant.user,
        Some(&access.tenant),
        Some(&access),
    )
    .await?
    .at("project_board");
    let res = views::tickets::render(
        &v,
        &shell,
        "tickets/board.html",
        StatusCode::OK,
        data!({
            "project": summary(&access),
            "views": nav::project_views(&home, &access.tenant.user.project_views, "board"),
            "views_action": PROJECT_VIEWS_ACTION,
            "return_to": paths.board,
            "columns": board_columns(&lookup, &tickets),
            "statuses": status_options(),
            "filter": filter,
            "who": who,
            "paths": paths,
            "can_edit": access.role.has(Permission::EditProject),
            "flash": flash,
        }),
    )?;
    Ok((session, res).into_response())
}

#[debug_handler]
pub(super) async fn list(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: ProjectAccess,
    Query(query): Query<Vec<(String, String)>>,
) -> Result<Response> {
    access.require(Permission::ViewProject)?;
    let wid = access.tenant.workspace_id();
    let home = project_paths(&access).home;
    let paths = BoardPaths::new(&home);
    let state = LIST_TABLE.parse(&query);
    let ticket_query = super::ticket_query(&state, access.tenant.user.id, vec![access.project.id]);
    let (rows, table) =
        table_page(&ctx.db, wid, &LIST_TABLE, &state, &paths.list, ticket_query).await?;
    let lookup = lookup(&ctx.db, &access.tenant, &rows, &[]).await?;
    let mut types: Vec<_> = lookup.types.values().cloned().collect();
    types.sort_by(|a, b| a.name.cmp(&b.name));
    let table = status_priority_options(table)
        .with_filter_options(
            "type",
            types
                .iter()
                .map(|t| FilterOption::raw(&t.id.to_string(), &t.name))
                .collect(),
        )
        .with_filter_options(
            "who",
            vec![
                FilterOption::key("me", "ticket-filter-me"),
                FilterOption::key("none", "ticket-unassigned"),
            ],
        );
    let tickets: Vec<_> = rows.iter().map(|t| lookup.row(t)).collect();
    let shell = app_shell(
        &ctx,
        &access.tenant.user,
        Some(&access.tenant),
        Some(&access),
    )
    .await?
    .at("project_list");
    views::tickets::render(
        &v,
        &shell,
        "tickets/list.html",
        StatusCode::OK,
        data!({
            "project": summary(&access),
            "views": nav::project_views(&home, &access.tenant.user.project_views, "list"),
            "views_action": PROJECT_VIEWS_ACTION,
            "return_to": paths.list,
            "table": table,
            "tickets": tickets,
            "paths": paths,
            "can_edit": access.role.has(Permission::EditProject),
        }),
    )
}

#[derive(Debug, Default, Deserialize)]
pub struct PeopleQuery {
    #[serde(default)]
    q: String,
}

/// `GET …/people?q=`: opsi combobox orang yang punya akses project. Tidak pernah menulis.
#[debug_handler]
pub(super) async fn people(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: ProjectAccess,
    Query(query): Query<PeopleQuery>,
) -> Result<Response> {
    access.require(Permission::ViewProject)?;
    let q = query.q.trim();
    let options = people_options(&ctx, &access, Some(q).filter(|q| !q.is_empty()), None).await?;
    views::workspaces::people_options(&v, &options)
}

#[derive(Debug, Default, Deserialize)]
pub struct TrashQuery {
    #[serde(default)]
    q: String,
}

#[debug_handler]
pub(super) async fn trash(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: ProjectAccess,
    session: Session,
    Query(query): Query<TrashQuery>,
) -> Result<Response> {
    access.require(Permission::ViewProject)?;
    let (session, flash) = flash::take(session);
    let wid = access.tenant.workspace_id();
    let q = query.q.trim();
    let trashed = tickets::Model::trashed_in(
        &ctx.db,
        wid,
        access.project.id,
        Some(q).filter(|q| !q.is_empty()),
    )
    .await?;
    let deleters: Vec<Uuid> = trashed.iter().filter_map(|t| t.deleted_by).collect();
    let lookup = lookup(&ctx.db, &access.tenant, &trashed, &deleters).await?;
    let home = project_paths(&access).home;
    let paths = BoardPaths::new(&home);
    let can_edit = access.role.has(Permission::EditProject);
    let rows: Vec<_> = trashed
        .iter()
        .map(|t| {
            let deleted_at = t.deleted_at.unwrap_or(t.updated_at);
            let purge_on = deleted_at + chrono::Duration::days(service::trash::RETENTION_DAYS);
            serde_json::json!({
                "ticket": lookup.row(t),
                "deleted_by": lookup.person(t.deleted_by),
                "deleted_at": views::time::wib_datetime(&deleted_at),
                "purge_on": views::tickets::format_date(purge_on.date_naive()),
                "restore_action": format!("{}/{}/restore", paths.trash, t.id),
                "purge_action": format!("{}/{}/purge", paths.trash, t.id),
                "can_restore": can_edit,
                "can_purge": can_purge(&access.role, t, access.tenant.user.id),
            })
        })
        .collect();
    let shell = app_shell(
        &ctx,
        &access.tenant.user,
        Some(&access.tenant),
        Some(&access),
    )
    .await?
    .at("project_board");
    let res = views::tickets::render(
        &v,
        &shell,
        "tickets/trash.html",
        StatusCode::OK,
        data!({
            "project": summary(&access),
            "views": nav::project_views(&home, &access.tenant.user.project_views, "board"),
            "views_action": PROJECT_VIEWS_ACTION,
            "return_to": paths.trash,
            "rows": rows,
            "q": q,
            "paths": paths,
            "retention_days": service::trash::RETENTION_DAYS,
            "flash": flash,
        }),
    )?;
    Ok((session, res).into_response())
}

#[debug_handler]
pub(super) async fn restore(
    State(ctx): State<AppContext>,
    access: ProjectAccess,
    session: Session,
    RequestId(request_id): RequestId,
    id: IdOr404<tickets::Entity>,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    ensure_in_project(&ctx, &access, id.id).await?;
    let ticket =
        service::trash::restore(&ctx.db, &acting(&access.tenant, request_id), id.id).await?;
    let session = flash::set(session, &Flash::success(views::tickets::FLASH_RESTORED))?;
    let to = ticket_href(&access.tenant.workspace.slug, &ticket.code);
    Ok((session, format::redirect(&to)?).into_response())
}

#[debug_handler]
pub(super) async fn purge(
    State(ctx): State<AppContext>,
    access: ProjectAccess,
    session: Session,
    RequestId(request_id): RequestId,
    id: IdOr404<tickets::Entity>,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    ensure_in_project(&ctx, &access, id.id).await?;
    service::trash::purge(
        &ctx.db,
        &ctx.storage,
        &acting(&access.tenant, request_id),
        id.id,
    )
    .await?;
    let session = flash::set(session, &Flash::success(views::tickets::FLASH_PURGED))?;
    let to = BoardPaths::new(&project_paths(&access).home).trash;
    Ok((session, format::redirect(&to)?).into_response())
}

/// Tiket `id` milik project di path, selain itu 404.
async fn ensure_in_project(ctx: &AppContext, access: &ProjectAccess, id: Uuid) -> Result<()> {
    tickets::Model::find_in(&ctx.db, access.tenant.workspace_id(), id)
        .await?
        .filter(|t| t.project_id == access.project.id)
        .map(|_| ())
        .ok_or(Error::NotFound)
}
