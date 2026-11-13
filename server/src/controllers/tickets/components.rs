//! Pengaturan project → Components (prototype M9 "Components"): subpage "Daftar component"
//! (datatable: cari, filter code graph, sort; dialog buat/ubah; hapus) dan "Aturan otomatis"
//! (pemilik component jadi penerima default). Butuh `AdministerProject`.
use axum::http::StatusCode;
use loco_rs::prelude::*;
use serde::Serialize;
use uuid::Uuid;

use super::{acting, board::people_options, project_paths, summary, users_by_id};
use crate::{
    controllers::{
        extractors::{ProjectAdmin, RequestId},
        flash,
        shell::app_shell,
    },
    models::components as model,
    services::{
        planning::components::{self as service, ComponentInput, PlanningRules},
        tickets::TicketFailure,
    },
    views::{
        self,
        datatable::{ColumnSpec, FilterOption, FilterSpec, TableSpec, TableView},
        flash::Flash,
        form::{FieldErrors, field_errors},
        nav::NavItem,
        ui::ComboOption,
    },
};

const FIELDS: &[&str] = &["name", "description", "owner", "modules"];

pub const FLASH_CREATED: &str = "component-flash-created";
pub const FLASH_SAVED: &str = "component-flash-saved";
pub const FLASH_DELETED: &str = "component-flash-deleted";
pub const FLASH_RULES: &str = "component-flash-rules";

/// Kunci Fluent yang dibangun di kode (dites kontrak i18n).
#[must_use]
pub fn label_keys() -> Vec<String> {
    [
        FLASH_CREATED,
        FLASH_SAVED,
        FLASH_DELETED,
        FLASH_RULES,
        "component-panel-list",
        "component-panel-list-desc",
        "component-panel-rules",
        "component-panel-rules-desc",
        "component-graph-linked",
        "component-graph-unlinked",
    ]
    .iter()
    .map(|k| (*k).to_string())
    .collect()
}

static COMPONENTS_TABLE: TableSpec = TableSpec {
    id: "components",
    columns: &[
        ColumnSpec::new("component-col-name").sort("name"),
        ColumnSpec::new("component-col-modules").hide("sm"),
        ColumnSpec::new("component-col-owner").sort("owner"),
        ColumnSpec::new("component-col-tickets")
            .sort("tickets")
            .right(),
        ColumnSpec::new("component-col-actions").right(),
    ],
    filters: &[FilterSpec {
        key: "graph",
        label_key: "component-col-modules",
        icon: "topology-star-3",
    }],
    default_sort: "name",
    default_desc: false,
    default_per: 25,
    search_key: Some("component-search"),
};

fn base(access: &crate::controllers::extractors::ProjectAccess) -> String {
    format!("{}/settings/components", project_paths(access).home)
}

fn value(fields: &[(String, String)], name: &str) -> String {
    fields
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.clone())
        .unwrap_or_default()
}

fn input(fields: &[(String, String)]) -> ComponentInput {
    ComponentInput {
        name: value(fields, "name"),
        description: value(fields, "description"),
        owner: value(fields, "owner"),
        modules: fields
            .iter()
            .filter(|(k, _)| k == "modules")
            .flat_map(|(_, v)| v.lines().map(str::to_string).collect::<Vec<_>>())
            .collect(),
    }
}

#[derive(Debug, Serialize, Default)]
struct FormValues {
    name: String,
    description: String,
    owner: String,
    modules: Vec<String>,
}

#[derive(Debug, Serialize)]
struct Row {
    id: Uuid,
    name: String,
    description: String,
    modules: Vec<String>,
    owner: Option<views::tickets::PersonView>,
    #[serde(skip)]
    owner_sort: String,
    tickets: u64,
    action: String,
    form: FormValues,
}

/// Dialog yang gagal divalidasi: id dialog (`component-new` / `component-edit-<id>`).
struct Failed {
    dialog: String,
    errors: FieldErrors,
    input: ComponentInput,
}

fn subnav(
    access: &crate::controllers::extractors::ProjectAccess,
    panel: &str,
    count: usize,
) -> Vec<NavItem> {
    let base = base(access);
    [
        (
            "list",
            "component-panel-list",
            "puzzle",
            "component-panel-list-desc",
            base.clone(),
        ),
        (
            "rules",
            "component-panel-rules",
            "sparkles",
            "component-panel-rules-desc",
            format!("{base}/rules"),
        ),
    ]
    .into_iter()
    .map(|(k, label, icon, desc, href)| {
        let mut item = NavItem::link(k, label, icon, &href).describe(desc);
        if k == "list" {
            item.count = Some(count as u64);
        }
        item.current = k == panel;
        item
    })
    .collect()
}

#[allow(clippy::too_many_lines)]
async fn render(
    ctx: &AppContext,
    v: &TeraView,
    access: &crate::controllers::extractors::ProjectAccess,
    panel: &str,
    pairs: &[(String, String)],
    flash: Option<Flash>,
    failed: Option<Failed>,
) -> Result<Response> {
    let db = &ctx.db;
    let wid = access.tenant.workspace_id();
    let base = base(access);
    let all = model::Model::of_project(db, wid, access.project.id).await?;
    let ids: Vec<Uuid> = all.iter().map(|c| c.id).collect();
    let counts = service::ticket_counts(db, wid, &ids).await?;
    let owner_ids: Vec<Uuid> = all.iter().filter_map(|c| c.owner_id).collect();
    let owners = users_by_id(db, owner_ids).await?;
    let mut rows: Vec<Row> = all
        .iter()
        .map(|c| {
            let owner = c
                .owner_id
                .and_then(|id| owners.iter().find(|u| u.id == id))
                .map(views::tickets::PersonView::of);
            Row {
                id: c.id,
                name: c.name.clone(),
                description: c.description.clone(),
                modules: c.modules.clone(),
                owner_sort: owner
                    .as_ref()
                    .map(|o| o.name.to_lowercase())
                    .unwrap_or_default(),
                owner,
                tickets: counts.get(&c.id).copied().unwrap_or(0),
                action: format!("{base}/{}", c.id),
                form: FormValues {
                    name: c.name.clone(),
                    description: c.description.clone(),
                    owner: c.owner_id.map(|u| u.to_string()).unwrap_or_default(),
                    modules: c.modules.clone(),
                },
            }
        })
        .collect();
    let total_all = rows.len();
    let state = COMPONENTS_TABLE.parse(pairs);
    if let Some(q) = state.query() {
        let q = q.to_lowercase();
        rows.retain(|r| {
            r.name.to_lowercase().contains(&q)
                || r.description.to_lowercase().contains(&q)
                || r.modules.iter().any(|m| m.to_lowercase().contains(&q))
        });
    }
    let graph = state.filter("graph").to_vec();
    if !graph.is_empty() {
        rows.retain(|r| {
            let linked = if r.modules.is_empty() {
                "unlinked"
            } else {
                "linked"
            };
            graph.iter().any(|g| g == linked)
        });
    }
    match state.sort {
        "owner" => rows.sort_by_key(|r| r.owner_sort.clone()),
        "tickets" => rows.sort_by_key(|r| r.tickets),
        _ => rows.sort_by_key(|r| r.name.to_lowercase()),
    }
    if state.desc {
        rows.reverse();
    }
    let total = rows.len() as u64;
    let page: Vec<Row> = rows
        .into_iter()
        .skip(usize::try_from(state.offset()).unwrap_or(0))
        .take(usize::try_from(state.per).unwrap_or(25))
        .collect();
    let table = TableView::new(&COMPONENTS_TABLE, &state, &base, total).with_filter_options(
        "graph",
        vec![
            FilterOption::key("linked", "component-graph-linked"),
            FilterOption::key("unlinked", "component-graph-unlinked"),
        ],
    );

    // Opsi dialog: modul code graph (+ modul yang sudah dipakai) dan orang project.
    let mut modules = service::graph_modules(db, wid, access.project.id).await?;
    let graph_indexed = !modules.is_empty();
    for c in &all {
        modules.extend(c.modules.iter().cloned());
    }
    modules.sort();
    modules.dedup();
    let module_options: Vec<ComboOption> = modules
        .iter()
        .map(|m| {
            let mut o = ComboOption::new(m, m);
            o.group = m.split('/').next().unwrap_or_default().to_string();
            o
        })
        .collect();
    let mut people = people_options(ctx, access, None, None).await?;
    for c in &all {
        if let Some(id) = c
            .owner_id
            .filter(|id| !people.iter().any(|p| p.value == id.to_string()))
        {
            people.extend(
                users_by_id(db, [id])
                    .await?
                    .iter()
                    .map(|u| ComboOption::person(u.id, &u.name, &u.email)),
            );
        }
    }

    let status = if failed.is_some() {
        StatusCode::UNPROCESSABLE_ENTITY
    } else {
        StatusCode::OK
    };
    let (failed_dialog, errors, mut failed_input) = match failed {
        Some(f) => (
            f.dialog,
            f.errors,
            Some(FormValues {
                name: f.input.name,
                description: f.input.description,
                owner: f.input.owner,
                modules: f.input.modules,
            }),
        ),
        None => (String::new(), field_errors(FIELDS, None), None),
    };
    // Dialog buat + ubah per baris; dialog yang gagal memakai isian yang dikirim.
    let new_form = if failed_dialog == "component-new" {
        failed_input.take().unwrap_or_default()
    } else {
        FormValues::default()
    };
    let mut dialogs = vec![serde_json::json!({
        "id": "component-new", "action": base, "new": true, "form": new_form,
    })];
    for r in &page {
        let id = format!("component-edit-{}", r.id);
        let form = if failed_dialog == id {
            failed_input.take()
        } else {
            None
        };
        dialogs.push(serde_json::json!({
            "id": id, "action": r.action, "new": false,
            "form": form.map_or_else(|| serde_json::to_value(&r.form).unwrap_or_default(), |f| serde_json::to_value(f).unwrap_or_default()),
        }));
    }
    let rules = PlanningRules::of(&access.project);
    let paths = project_paths(access);
    let shell = app_shell(ctx, &access.tenant.user, Some(&access.tenant), Some(access))
        .await?
        .at("project_components");
    views::tickets::render(
        v,
        &shell,
        "tickets/components.html",
        status,
        serde_json::json!({
            "project": summary(access),
            "paths": paths,
            "panel": panel,
            "subnav": subnav(access, panel, total_all),
            "table": table,
            "components": page,
            "base": base,
            "rules_action": format!("{base}/rules"),
            "rules": rules,
            "module_options": module_options,
            "graph_indexed": graph_indexed,
            "graph_href": paths.graph,
            "people": people,
            "failed_dialog": failed_dialog,
            "dialogs": dialogs,
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
    ProjectAdmin(access): ProjectAdmin,
    session: Session,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Result<Response> {
    let (session, flash) = flash::take(session);
    let res = render(&ctx, &v, &access, "list", &pairs, flash, None).await?;
    Ok((session, res).into_response())
}

#[debug_handler]
pub(super) async fn rules(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    ProjectAdmin(access): ProjectAdmin,
    session: Session,
) -> Result<Response> {
    let (session, flash) = flash::take(session);
    let res = render(&ctx, &v, &access, "rules", &[], flash, None).await?;
    Ok((session, res).into_response())
}

#[debug_handler]
pub(super) async fn save_rules(
    State(ctx): State<AppContext>,
    ProjectAdmin(access): ProjectAdmin,
    session: Session,
    RequestId(request_id): RequestId,
    Form(fields): Form<Vec<(String, String)>>,
) -> Result<Response> {
    let rules = PlanningRules {
        component_assignee: !value(&fields, "component_assignee").is_empty(),
    };
    service::save_rules(
        &ctx.db,
        &acting(&access.tenant, request_id),
        access.project.id,
        &rules,
    )
    .await?;
    let session = flash::set(session, &Flash::success(FLASH_RULES))?;
    Ok((
        session,
        format::redirect(&format!("{}/rules", base(&access)))?,
    )
        .into_response())
}

#[allow(clippy::too_many_arguments)]
async fn finish<T>(
    ctx: &AppContext,
    v: &TeraView,
    access: &crate::controllers::extractors::ProjectAccess,
    session: Session,
    result: Result<T, TicketFailure>,
    ok: &str,
    dialog: String,
    input: ComponentInput,
) -> Result<Response> {
    match result {
        Ok(_) => {
            let session = flash::set(session, &Flash::success(ok))?;
            Ok((session, format::redirect(&base(access))?).into_response())
        }
        Err(TicketFailure::Invalid(errors)) => {
            let failed = Failed {
                dialog,
                errors: field_errors(FIELDS, Some(&errors)),
                input,
            };
            let res = render(ctx, v, access, "list", &[], None, Some(failed)).await?;
            Ok((session, res).into_response())
        }
        Err(TicketFailure::NotFound) => Err(Error::NotFound),
        Err(e) => Err(e.into()),
    }
}

#[debug_handler]
pub(super) async fn create(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    ProjectAdmin(access): ProjectAdmin,
    session: Session,
    RequestId(request_id): RequestId,
    Form(fields): Form<Vec<(String, String)>>,
) -> Result<Response> {
    let input = input(&fields);
    let result = service::create(
        &ctx.db,
        &acting(&access.tenant, request_id),
        access.project.id,
        &input,
    )
    .await;
    finish(
        &ctx,
        &v,
        &access,
        session,
        result,
        FLASH_CREATED,
        "component-new".into(),
        input,
    )
    .await
}

async fn find(
    ctx: &AppContext,
    access: &crate::controllers::extractors::ProjectAccess,
    id: Uuid,
) -> Result<model::Model> {
    model::Model::find_in(&ctx.db, access.tenant.workspace_id(), id)
        .await?
        .filter(|c| c.project_id == access.project.id)
        .ok_or(Error::NotFound)
}

#[debug_handler]
pub(super) async fn update(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    ProjectAdmin(access): ProjectAdmin,
    session: Session,
    RequestId(request_id): RequestId,
    Path((_slug, _key, id)): Path<(String, String, Uuid)>,
    Form(fields): Form<Vec<(String, String)>>,
) -> Result<Response> {
    let c = find(&ctx, &access, id).await?;
    let input = input(&fields);
    let result = service::update(&ctx.db, &acting(&access.tenant, request_id), c.id, &input).await;
    finish(
        &ctx,
        &v,
        &access,
        session,
        result,
        FLASH_SAVED,
        format!("component-edit-{id}"),
        input,
    )
    .await
}

#[debug_handler]
pub(super) async fn delete(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    ProjectAdmin(access): ProjectAdmin,
    session: Session,
    RequestId(request_id): RequestId,
    Path((_slug, _key, id)): Path<(String, String, Uuid)>,
) -> Result<Response> {
    let c = find(&ctx, &access, id).await?;
    let result = service::delete(&ctx.db, &acting(&access.tenant, request_id), c.id).await;
    finish(
        &ctx,
        &v,
        &access,
        session,
        result,
        FLASH_DELETED,
        String::new(),
        ComponentInput::default(),
    )
    .await
}
