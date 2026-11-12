//! Pengaturan → Workflow (`Require<ManageTicketSettings>`): daftar (workflow workspace, template
//! bawaan, ekspor & impor), workflow default per tipe, builder (draf, YAML, riwayat versi),
//! terbitkan + migrasi tiket; dan Pengaturan project → Workflow (`ProjectAdmin`).
use std::collections::BTreeMap;

use agentgate_core::workflow::{
    Category, Definition, Exit, Gate, Mode, People, Persona, Stage, templates,
};
use axum::{
    body::Body,
    http::{HeaderValue, StatusCode, header},
};
use loco_rs::{prelude::*, validation::ModelValidationErrors};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{acting, project_paths, summary};
use crate::{
    controllers::{
        extractors::{
            IdOr404, ProjectAdmin, RequestId, Require, Tenant, require::ManageTicketSettings,
        },
        flash,
        shell::app_shell,
    },
    models::{project_workflows, ticket_types, tickets, workflow_versions, workflows},
    services::{
        tickets::TicketFailure,
        workflows::{self as service, DraftEdit, ProjectWorkflows, Source},
    },
    views::{
        self,
        flash::Flash,
        nav::{self, NavItem},
        tickets::{SelectOption, TypeView},
        ui::{ComboOption, Stat},
        workflow::{
            StageView, category_options, exit_options, gate_options, mode_options, persona_options,
        },
    },
};

pub const FLASH_CREATED: &str = "flash-workflow-created";
pub const FLASH_DRAFT_SAVED: &str = "flash-workflow-draft-saved";
pub const FLASH_PUBLISHED: &str = "flash-workflow-published";
pub const FLASH_DISCARDED: &str = "flash-workflow-discarded";
pub const FLASH_DELETED: &str = "flash-workflow-deleted";
pub const FLASH_DEFAULTS_SAVED: &str = "flash-workflow-defaults-saved";
pub const FLASH_RENAMED: &str = "flash-workflow-renamed";
pub const FLASH_PROJECT_SAVED: &str = "flash-project-workflow-saved";
pub const FLASH_IMPORTED: &str = "flash-workflow-imported";

pub fn add(routes: Routes) -> Routes {
    routes
        .add("/w/{slug}/settings/workflows", get(list).post(create))
        .add("/w/{slug}/settings/workflows/import", post(import))
        .add("/w/{slug}/settings/workflows/defaults", post(defaults))
        .add("/w/{slug}/settings/workflows/{id}", get(builder))
        .add("/w/{slug}/settings/workflows/{id}/identity", post(rename))
        .add("/w/{slug}/settings/workflows/{id}/stage", post(save_stage))
        .add("/w/{slug}/settings/workflows/{id}/stages", post(add_stage))
        .add(
            "/w/{slug}/settings/workflows/{id}/stages/remove",
            post(remove_stage),
        )
        .add(
            "/w/{slug}/settings/workflows/{id}/stages/move",
            post(move_stage),
        )
        .add("/w/{slug}/settings/workflows/{id}/yaml", post(save_yaml))
        .add("/w/{slug}/settings/workflows/{id}/export", get(export))
        .add("/w/{slug}/settings/workflows/{id}/publish", post(publish))
        .add(
            "/w/{slug}/settings/workflows/{id}/migrate",
            get(migrate).post(apply_migration),
        )
        .add("/w/{slug}/settings/workflows/{id}/discard", post(discard))
        .add("/w/{slug}/settings/workflows/{id}/delete", post(delete))
        .add(
            "/w/{slug}/p/{key}/settings/workflow",
            get(project).post(save_project),
        )
}

fn base(slug: &str) -> String {
    format!("/w/{slug}/settings/workflows")
}

fn get_one<'a>(pairs: &'a [(String, String)], name: &str) -> &'a str {
    pairs
        .iter()
        .find(|(k, _)| k == name)
        .map_or("", |(_, v)| v.as_str())
}

fn all<'a>(pairs: &'a [(String, String)], name: &str) -> Vec<&'a str> {
    pairs
        .iter()
        .filter(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
        .collect()
}

fn uuids(pairs: &[(String, String)], name: &str) -> Vec<Uuid> {
    all(pairs, name)
        .into_iter()
        .filter_map(|v| Uuid::parse_str(v.trim()).ok())
        .collect()
}

fn csv(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// Semua field yang bisa membawa error di halaman workflow (template membaca kunci yang selalu
/// ada).
const ERROR_FIELDS: &[&str] = &[
    "name",
    "description",
    "source",
    "yaml",
    "workflow",
    "stages",
    "stage",
    "people",
    "mapping",
    "draft",
    "workflow_name",
    "workflow_description",
    "key",
    "category",
    "persona",
    "mode",
    "skill",
    "prompt",
    "inputs",
    "output",
    "output_template",
    "gates",
    "tools",
    "exit",
    "fail_limit",
    "fail_to",
    "parallel",
];

fn error_map(
    errors: Option<&ModelValidationErrors>,
    pick: impl Fn(&loco_rs::validation::ValidationError) -> String,
) -> BTreeMap<String, String> {
    let mut out: BTreeMap<String, String> = ERROR_FIELDS
        .iter()
        .map(|f| ((*f).to_string(), String::new()))
        .collect();
    if let Some(e) = errors {
        for (k, l) in &e.errors {
            if let Some(x) = l.first() {
                out.insert(k.to_string(), pick(x));
            }
        }
    }
    out
}

/// Kode error per field (kunci = nama field form).
fn codes(errors: Option<&ModelValidationErrors>) -> BTreeMap<String, String> {
    error_map(errors, |x| x.code.clone())
}

/// Detail error (`detail`, nama stage) per field.
fn details(errors: Option<&ModelValidationErrors>) -> BTreeMap<String, String> {
    error_map(errors, |x| {
        x.params
            .get("detail")
            .and_then(|d| d.as_str())
            .unwrap_or_default()
            .to_string()
    })
}

async fn find(ctx: &AppContext, tenant: &Tenant, id: Uuid) -> Result<workflows::Model> {
    workflows::Model::find_in(&ctx.db, tenant.workspace_id(), id)
        .await?
        .ok_or(Error::NotFound)
}

// ---------------------------------------------------------------- daftar

#[derive(Debug, Default, Deserialize)]
pub struct ListQuery {
    #[serde(default)]
    tab: String,
}

#[derive(Debug, Serialize)]
struct WorkflowRow {
    id: Uuid,
    name: String,
    description: String,
    version: i32,
    stages: usize,
    draft: bool,
    types: Vec<String>,
    tickets: u64,
    template: String,
    href: String,
}

#[derive(Debug, Serialize)]
struct TemplateRow {
    key: &'static str,
    name: String,
    description: String,
    stages: Vec<String>,
    gates: usize,
    approvals: usize,
}

#[allow(clippy::too_many_arguments)]
async fn render_list(
    ctx: &AppContext,
    v: &TeraView,
    tenant: &Tenant,
    tab: &str,
    new_form: serde_json::Value,
    new_open: bool,
    yaml: &str,
    errors: Option<&ModelValidationErrors>,
    flash: Option<Flash>,
    status: StatusCode,
) -> Result<Response> {
    let wid = tenant.workspace_id();
    let slug = &tenant.workspace.slug;
    let all = workflows::Model::all_in(&ctx.db, wid).await?;
    let types = ticket_types::Model::all_in(&ctx.db, wid).await?;
    let mut rows = Vec::new();
    for w in &all {
        let def = w.definition(&ctx.db).await?;
        let used = tickets::Entity::find()
            .filter(tickets::Column::WorkspaceId.eq(wid.get()))
            .filter(tickets::Column::WorkflowId.eq(w.id))
            .filter(tickets::active())
            .count(&ctx.db)
            .await?;
        rows.push(WorkflowRow {
            id: w.id,
            name: w.name.clone(),
            description: w.description.clone(),
            version: w.version,
            stages: def.stages.len(),
            draft: w.draft.is_some(),
            types: types
                .iter()
                .filter(|t| t.workflow_id == w.id)
                .map(|t| t.name.clone())
                .collect(),
            tickets: used,
            template: w.template_key.clone().unwrap_or_default(),
            href: format!("{}/{}", base(slug), w.id),
        });
    }
    let template_rows: Vec<TemplateRow> = templates::TEMPLATES
        .iter()
        .map(|t| {
            let doc = t.document();
            TemplateRow {
                key: t.key,
                name: doc.name.clone(),
                description: doc.description.clone(),
                stages: doc
                    .definition
                    .stages
                    .iter()
                    .map(|s| s.name.clone())
                    .collect(),
                gates: doc.definition.stages.iter().map(|s| s.gates.len()).sum(),
                approvals: doc
                    .definition
                    .stages
                    .iter()
                    .filter(|s| s.exit == Exit::Approval)
                    .count(),
            }
        })
        .collect();
    let tab = match tab {
        "templates" | "import" => tab,
        _ => "workspace",
    };
    let list_href = base(slug);
    let mut tabs = vec![
        NavItem::link(
            "workspace",
            "workflow-tab-workspace",
            "git-merge",
            &format!("{list_href}?tab=workspace"),
        )
        .counted(rows.len() as u64, false),
        NavItem::link(
            "templates",
            "workflow-tab-templates",
            "template",
            &format!("{list_href}?tab=templates"),
        )
        .counted(template_rows.len() as u64, false),
        NavItem::link(
            "import",
            "workflow-tab-import",
            "file-import",
            &format!("{list_href}?tab=import"),
        ),
    ];
    for t in &mut tabs {
        t.current = t.key == tab;
    }
    let sources: Vec<serde_json::Value> = std::iter::once(serde_json::json!({
        "id": "blank", "label_key": "workflow-source-blank", "name": "",
    }))
    .chain(template_rows.iter().map(|t| serde_json::json!({
        "id": format!("template:{}", t.key), "label_key": "workflow-source-template", "name": t.name,
    })))
    .chain(all.iter().map(|w| serde_json::json!({
        "id": format!("clone:{}", w.id), "label_key": "workflow-source-clone", "name": w.name,
    })))
    .collect();
    let shell = app_shell(ctx, &tenant.user, Some(tenant), None)
        .await?
        .at("workflows");
    views::tickets::render(
        v,
        &shell,
        "workflows/list.html",
        status,
        data!({
            "tab": tab,
            "tabs": tabs,
            "workflows": rows,
            "templates": template_rows,
            "types": types.iter().map(|t| serde_json::json!({
                "type": TypeView::of(t),
                "workflow": t.workflow_id.to_string(),
            })).collect::<Vec<_>>(),
            "options": all.iter().map(|w| SelectOption { id: w.id.to_string(), name: w.name.clone() }).collect::<Vec<_>>(),
            "sources": sources,
            "new": new_form,
            "new_open": new_open,
            "yaml": yaml,
            "errors": codes(errors),
            "details": details(errors),
            "create_action": list_href,
            "import_action": format!("{list_href}/import"),
            "defaults_action": format!("{list_href}/defaults"),
            "flash": flash,
        }),
    )
}

fn empty_new() -> serde_json::Value {
    serde_json::json!({ "name": "", "description": "", "source": "blank" })
}

#[debug_handler]
pub(super) async fn list(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageTicketSettings>,
    session: Session,
    Query(query): Query<ListQuery>,
) -> Result<Response> {
    let (session, flash) = flash::take(session);
    let res = render_list(
        &ctx,
        &v,
        &access.tenant,
        &query.tab,
        empty_new(),
        false,
        "",
        None,
        flash,
        StatusCode::OK,
    )
    .await?;
    Ok((session, res).into_response())
}

#[debug_handler]
pub(super) async fn create(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageTicketSettings>,
    session: Session,
    RequestId(request_id): RequestId,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let name = get_one(&pairs, "name");
    let description = get_one(&pairs, "description");
    let source = get_one(&pairs, "source");
    let source_parsed = Source::parse(source);
    // Template tanpa nama memakai nama template.
    let name = match (&source_parsed, name.trim()) {
        (Some(Source::Template(k)), "") => templates::find(k)
            .map(|t| t.document().name)
            .unwrap_or_default(),
        (_, n) => n.to_string(),
    };
    match service::create(
        &ctx.db,
        &acting(tenant, request_id),
        &name,
        description,
        source_parsed,
    )
    .await
    {
        Ok(created) => {
            let session = flash::set(session, &Flash::success(FLASH_CREATED))?;
            let to = format!("{}/{}", base(&tenant.workspace.slug), created.id);
            Ok((session, format::redirect(&to)?).into_response())
        }
        Err(TicketFailure::Invalid(errors)) => {
            render_list(
                &ctx,
                &v,
                tenant,
                if source.starts_with("template:") {
                    "templates"
                } else {
                    "workspace"
                },
                serde_json::json!({ "name": name, "description": description, "source": source }),
                true,
                "",
                Some(&errors),
                None,
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => Err(e.into()),
    }
}

#[derive(Debug, Deserialize)]
pub struct YamlForm {
    #[serde(default)]
    yaml: String,
}

#[debug_handler]
pub(super) async fn import(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageTicketSettings>,
    session: Session,
    RequestId(request_id): RequestId,
    Form(form): Form<YamlForm>,
) -> Result<Response> {
    let tenant = &access.tenant;
    match service::import(&ctx.db, &acting(tenant, request_id), &form.yaml).await {
        Ok((created, _ignored)) => {
            let session = flash::set(session, &Flash::success(FLASH_IMPORTED))?;
            let to = format!("{}/{}", base(&tenant.workspace.slug), created.id);
            Ok((session, format::redirect(&to)?).into_response())
        }
        Err(TicketFailure::Invalid(errors)) => {
            render_list(
                &ctx,
                &v,
                tenant,
                "import",
                empty_new(),
                false,
                &form.yaml,
                Some(&errors),
                None,
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => Err(e.into()),
    }
}

#[debug_handler]
pub(super) async fn defaults(
    State(ctx): State<AppContext>,
    access: Require<ManageTicketSettings>,
    session: Session,
    RequestId(request_id): RequestId,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let acting = acting(tenant, request_id);
    for (k, value) in &pairs {
        let Some(type_id) = k
            .strip_prefix("type_")
            .and_then(|id| Uuid::parse_str(id).ok())
        else {
            continue;
        };
        let workflow_id = crate::controllers::extractors::form_id(value)?;
        service::set_type_default(&ctx.db, &acting, type_id, workflow_id).await?;
    }
    let session = flash::set(session, &Flash::success(FLASH_DEFAULTS_SAVED))?;
    Ok((session, format::redirect(&base(&tenant.workspace.slug))?).into_response())
}

// ---------------------------------------------------------------- builder

#[derive(Debug, Default, Deserialize)]
pub struct BuilderQuery {
    #[serde(default)]
    stage: String,
    #[serde(default)]
    tab: String,
}

/// Isian form stage untuk template.
#[derive(Debug, Serialize)]
struct StageForm {
    key: String,
    name: String,
    category: &'static str,
    persona: String,
    mode: &'static str,
    skill: String,
    prompt: String,
    inputs: String,
    output: String,
    output_template: String,
    gates: Vec<&'static str>,
    tools: String,
    exit: &'static str,
    fail_limit: u8,
    fail_to: String,
    parallel: bool,
    executors: Vec<String>,
    approvers: Vec<String>,
    watchers: Vec<String>,
}

impl StageForm {
    fn of(s: &Stage) -> Self {
        let ids = |l: &[Uuid]| l.iter().map(ToString::to_string).collect();
        Self {
            key: s.key.clone(),
            name: s.name.clone(),
            category: s.category.as_str(),
            persona: s
                .persona
                .map(|p| p.as_str().to_string())
                .unwrap_or_default(),
            mode: s.mode.as_str(),
            skill: s.skill.clone(),
            prompt: s.prompt.clone(),
            inputs: s.inputs.join(", "),
            output: s.output.clone(),
            output_template: s.output_template.clone(),
            gates: s.gates.iter().map(|g| g.as_str()).collect(),
            tools: s.tools.join(", "),
            exit: s.exit.as_str(),
            fail_limit: s.fail_limit,
            fail_to: s.fail_to.clone().unwrap_or_default(),
            parallel: s.parallel,
            executors: ids(&s.people.executors),
            approvers: ids(&s.people.approvers),
            watchers: ids(&s.people.watchers),
        }
    }
}

/// Stage dari isian form (nilai yang tidak dikenal jadi bawaan; validasi di service).
fn stage_from(pairs: &[(String, String)]) -> Stage {
    let key = get_one(pairs, "key").trim();
    let mut s = Stage::new(
        key,
        get_one(pairs, "name"),
        Category::parse(get_one(pairs, "category")).unwrap_or(Category::Todo),
    );
    s.name = get_one(pairs, "name").trim().to_string();
    s.persona = Persona::parse(get_one(pairs, "persona"));
    s.mode = Mode::parse(get_one(pairs, "mode")).unwrap_or_default();
    s.skill = get_one(pairs, "skill").trim().to_string();
    s.prompt = get_one(pairs, "prompt")
        .replace("\r\n", "\n")
        .trim()
        .to_string();
    s.inputs = csv(get_one(pairs, "inputs"));
    s.output = get_one(pairs, "output").trim().to_string();
    s.output_template = get_one(pairs, "output_template").replace("\r\n", "\n");
    s.gates = all(pairs, "gates")
        .into_iter()
        .filter_map(Gate::parse)
        .collect();
    s.tools = csv(get_one(pairs, "tools"));
    s.exit = Exit::parse(get_one(pairs, "exit")).unwrap_or_default();
    s.fail_limit = get_one(pairs, "fail_limit").trim().parse().unwrap_or(0);
    s.fail_to = Some(get_one(pairs, "fail_to").trim())
        .filter(|v| !v.is_empty())
        .map(str::to_string);
    s.parallel = !get_one(pairs, "parallel").is_empty();
    s.people = People {
        executors: uuids(pairs, "executors"),
        approvers: uuids(pairs, "approvers"),
        watchers: uuids(pairs, "watchers"),
    };
    s
}

struct BuilderState<'a> {
    selected: Option<String>,
    stage_override: Option<Stage>,
    errors: Option<&'a ModelValidationErrors>,
    tab: String,
    yaml: Option<String>,
    flash: Option<Flash>,
}

async fn render_builder(
    ctx: &AppContext,
    v: &TeraView,
    tenant: &Tenant,
    workflow: &workflows::Model,
    state: BuilderState<'_>,
    status: StatusCode,
) -> Result<Response> {
    let wid = tenant.workspace_id();
    let slug = &tenant.workspace.slug;
    let href = format!("{}/{}", base(slug), workflow.id);
    let draft = service::working_copy(&ctx.db, workflow).await?;
    let selected = state
        .selected
        .as_deref()
        .and_then(|k| draft.stage(k))
        .or(draft.first())
        .cloned();
    let form = state
        .stage_override
        .as_ref()
        .or(selected.as_ref())
        .map(StageForm::of);
    let selected_key = selected.as_ref().map(|s| s.key.clone()).unwrap_or_default();
    let nodes: Vec<StageView> = draft
        .stages
        .iter()
        .map(|s| {
            let mut v = StageView::of(s, &draft);
            v.state = if s.key == selected_key {
                "current"
            } else {
                "todo"
            };
            v.href = format!("{href}?stage={}", s.key);
            v
        })
        .collect();
    let versions = workflow_versions::Model::all_of(&ctx.db, wid, workflow.id).await?;
    let creators: Vec<Uuid> = versions.iter().filter_map(|v| v.created_by).collect();
    let users = super::users_by_id(&ctx.db, creators).await?;
    let lookup = views::tickets::Lookup::new(slug, &[], &users);
    let plan = if workflow.draft.is_some() {
        service::plan(&ctx.db, workflow).await.ok()
    } else {
        None
    };
    let yaml = match state.yaml {
        Some(y) => y,
        None => service::export(&ctx.db, wid, workflow, true).await?,
    };
    let people: Vec<ComboOption> = crate::models::workspace_members::Model::list_in(&ctx.db, wid)
        .await?
        .iter()
        .map(|(_, u, _)| ComboOption::person(u.id, &u.name, &u.email))
        .collect();
    let tab = match state.tab.as_str() {
        "yaml" | "versions" => state.tab.clone(),
        _ => "builder".to_string(),
    };
    let mut tabs = vec![
        NavItem::link("builder", "workflow-tab-builder", "sitemap", &href),
        NavItem::link(
            "yaml",
            "workflow-tab-yaml",
            "code",
            &format!("{href}?tab=yaml"),
        ),
        NavItem::link(
            "versions",
            "workflow-tab-versions",
            "history",
            &format!("{href}?tab=versions"),
        )
        .counted(versions.len() as u64, false),
    ];
    for t in &mut tabs {
        t.current = t.key == tab;
    }
    let in_use = service::in_use(&ctx.db, workflow).await?;
    let shell = app_shell(ctx, &tenant.user, Some(tenant), None)
        .await?
        .at("workflows");
    views::tickets::render(
        v,
        &shell,
        "workflows/builder.html",
        status,
        data!({
            "workflow": {
                "id": workflow.id,
                "name": workflow.name,
                "description": workflow.description,
                "version": workflow.version,
                "draft": workflow.draft.is_some(),
                "next_version": workflow.version + 1,
                "in_use": in_use,
            },
            "tab": tab,
            "tabs": tabs,
            "nodes": nodes,
            "stage": form,
            "earlier": draft.stages.iter().take_while(|s| s.key != selected_key).map(|s| SelectOption { id: s.key.clone(), name: s.name.clone() }).collect::<Vec<_>>(),
            "categories": category_options(),
            "personas": persona_options(),
            "modes": mode_options(),
            "exits": exit_options(),
            "gate_options": gate_options(),
            "people": people,
            "yaml": yaml,
            "versions": versions.iter().map(|v| serde_json::json!({
                "version": v.version,
                "stages": v.definition().stages.len(),
                "author": lookup.person(v.created_by),
                "at": views::time::wib_datetime(&v.created_at),
                "active": v.version == workflow.version,
            })).collect::<Vec<_>>(),
            "plan": plan.as_ref().map(|p| serde_json::json!({
                "removed": p.removed.iter().filter(|r| r.tickets > 0).count(),
                "needs_mapping": p.needs_mapping(),
                "others": p.others,
            })),
            "errors": codes(state.errors),
            "details": details(state.errors),
            "paths": {
                "self": href,
                "list": base(slug),
                "identity": format!("{href}/identity"),
                "stage": format!("{href}/stage"),
                "stages": format!("{href}/stages"),
                "remove": format!("{href}/stages/remove"),
                "move": format!("{href}/stages/move"),
                "yaml": format!("{href}/yaml"),
                "export": format!("{href}/export"),
                "publish": format!("{href}/publish"),
                "discard": format!("{href}/discard"),
                "delete": format!("{href}/delete"),
            },
            "flash": state.flash,
        }),
    )
}

fn state(selected: Option<String>) -> BuilderState<'static> {
    BuilderState {
        selected,
        stage_override: None,
        errors: None,
        tab: String::new(),
        yaml: None,
        flash: None,
    }
}

#[debug_handler]
pub(super) async fn builder(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageTicketSettings>,
    session: Session,
    id: IdOr404<workflows::Entity>,
    Query(query): Query<BuilderQuery>,
) -> Result<Response> {
    let workflow = find(&ctx, &access.tenant, id.id).await?;
    let (session, flash) = flash::take(session);
    let mut st = state(Some(query.stage));
    st.tab = query.tab;
    st.flash = flash;
    let res = render_builder(&ctx, &v, &access.tenant, &workflow, st, StatusCode::OK).await?;
    Ok((session, res).into_response())
}

async fn after_edit(
    ctx: &AppContext,
    v: &TeraView,
    tenant: &Tenant,
    session: Session,
    id: Uuid,
    result: std::result::Result<(Definition, Option<String>), TicketFailure>,
    on_error: BuilderState<'_>,
) -> Result<Response> {
    match result {
        Ok((_, selected)) => {
            let session = flash::set(session, &Flash::success(FLASH_DRAFT_SAVED))?;
            let mut to = format!("{}/{}", base(&tenant.workspace.slug), id);
            if let Some(k) = selected {
                to = format!("{to}?stage={k}");
            }
            Ok((session, format::redirect(&to)?).into_response())
        }
        Err(TicketFailure::Invalid(errors)) => {
            let workflow = find(ctx, tenant, id).await?;
            let st = BuilderState {
                errors: Some(&errors),
                ..on_error
            };
            render_builder(
                ctx,
                v,
                tenant,
                &workflow,
                st,
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => Err(e.into()),
    }
}

#[debug_handler]
pub(super) async fn save_stage(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageTicketSettings>,
    session: Session,
    RequestId(request_id): RequestId,
    id: IdOr404<workflows::Entity>,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let stage = stage_from(&pairs);
    let key = stage.key.clone();
    let r = service::edit_draft(
        &ctx.db,
        &acting(tenant, request_id),
        id.id,
        DraftEdit::Stage {
            key: key.clone(),
            stage: Box::new(stage.clone()),
        },
    )
    .await;
    let mut st = state(Some(key));
    st.stage_override = Some(stage);
    after_edit(&ctx, &v, tenant, session, id.id, r, st).await
}

#[debug_handler]
pub(super) async fn add_stage(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageTicketSettings>,
    session: Session,
    RequestId(request_id): RequestId,
    id: IdOr404<workflows::Entity>,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let name = get_one(&pairs, "name").trim().to_string();
    let category = Category::parse(get_one(&pairs, "category")).unwrap_or(Category::InProgress);
    let r = service::edit_draft(
        &ctx.db,
        &acting(tenant, request_id),
        id.id,
        DraftEdit::Add { name, category },
    )
    .await;
    after_edit(&ctx, &v, tenant, session, id.id, r, state(None)).await
}

#[derive(Debug, Deserialize)]
pub struct KeyForm {
    #[serde(default)]
    key: String,
    #[serde(default)]
    by: i8,
}

#[debug_handler]
pub(super) async fn remove_stage(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageTicketSettings>,
    session: Session,
    RequestId(request_id): RequestId,
    id: IdOr404<workflows::Entity>,
    Form(form): Form<KeyForm>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let r = service::edit_draft(
        &ctx.db,
        &acting(tenant, request_id),
        id.id,
        DraftEdit::Remove {
            key: form.key.clone(),
        },
    )
    .await;
    after_edit(&ctx, &v, tenant, session, id.id, r, state(Some(form.key))).await
}

#[debug_handler]
pub(super) async fn move_stage(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageTicketSettings>,
    session: Session,
    RequestId(request_id): RequestId,
    id: IdOr404<workflows::Entity>,
    Form(form): Form<KeyForm>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let r = service::edit_draft(
        &ctx.db,
        &acting(tenant, request_id),
        id.id,
        DraftEdit::Move {
            key: form.key.clone(),
            by: form.by.signum(),
        },
    )
    .await;
    after_edit(&ctx, &v, tenant, session, id.id, r, state(Some(form.key))).await
}

#[debug_handler]
pub(super) async fn save_yaml(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageTicketSettings>,
    session: Session,
    RequestId(request_id): RequestId,
    id: IdOr404<workflows::Entity>,
    Form(form): Form<YamlForm>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let r = service::edit_draft(
        &ctx.db,
        &acting(tenant, request_id),
        id.id,
        DraftEdit::Yaml(form.yaml.clone()),
    )
    .await;
    let mut st = state(None);
    st.tab = "yaml".into();
    st.yaml = Some(form.yaml);
    after_edit(&ctx, &v, tenant, session, id.id, r, st).await
}

#[derive(Debug, Deserialize)]
pub struct IdentityForm {
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
}

#[debug_handler]
pub(super) async fn rename(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageTicketSettings>,
    session: Session,
    RequestId(request_id): RequestId,
    id: IdOr404<workflows::Entity>,
    Form(form): Form<IdentityForm>,
) -> Result<Response> {
    let tenant = &access.tenant;
    match service::rename(
        &ctx.db,
        &acting(tenant, request_id),
        id.id,
        &form.name,
        &form.description,
    )
    .await
    {
        Ok(w) => {
            let session = flash::set(session, &Flash::success(FLASH_RENAMED))?;
            let to = format!("{}/{}", base(&tenant.workspace.slug), w.id);
            Ok((session, format::redirect(&to)?).into_response())
        }
        Err(TicketFailure::Invalid(mut errors)) => {
            for (from, to) in [
                ("name", "workflow_name"),
                ("description", "workflow_description"),
            ] {
                if let Some(list) = errors.errors.remove(from) {
                    errors.errors.insert(to.into(), list);
                }
            }
            let workflow = find(&ctx, tenant, id.id).await?;
            let st = BuilderState {
                errors: Some(&errors),
                ..state(None)
            };
            render_builder(
                &ctx,
                &v,
                tenant,
                &workflow,
                st,
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => Err(e.into()),
    }
}

/// Unduh YAML versi aktif.
#[debug_handler]
pub(super) async fn export(
    State(ctx): State<AppContext>,
    access: Require<ManageTicketSettings>,
    id: IdOr404<workflows::Entity>,
) -> Result<Response> {
    let workflow = find(&ctx, &access.tenant, id.id).await?;
    let text = service::export(&ctx.db, access.tenant.workspace_id(), &workflow, false).await?;
    let mut res = Response::new(Body::from(text));
    let headers = res.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/yaml; charset=utf-8"),
    );
    let disposition = format!("attachment; filename=\"{}.yaml\"", workflow.key);
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&disposition).unwrap_or(HeaderValue::from_static("attachment")),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    Ok(res)
}

#[debug_handler]
pub(super) async fn publish(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageTicketSettings>,
    session: Session,
    RequestId(request_id): RequestId,
    id: IdOr404<workflows::Entity>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let workflow = find(&ctx, tenant, id.id).await?;
    let href = format!("{}/{}", base(&tenant.workspace.slug), workflow.id);
    if service::plan(&ctx.db, &workflow)
        .await
        .is_ok_and(|p| p.needs_mapping())
    {
        return Ok(format::redirect(&format!("{href}/migrate"))?.into_response());
    }
    finish_publish(
        &ctx,
        &v,
        tenant,
        session,
        request_id,
        &workflow,
        &BTreeMap::new(),
    )
    .await
}

async fn finish_publish(
    ctx: &AppContext,
    v: &TeraView,
    tenant: &Tenant,
    session: Session,
    request_id: Option<String>,
    workflow: &workflows::Model,
    mapping: &BTreeMap<String, String>,
) -> Result<Response> {
    let href = format!("{}/{}", base(&tenant.workspace.slug), workflow.id);
    match service::publish(&ctx.db, &acting(tenant, request_id), workflow.id, mapping).await {
        Ok(_) => {
            let session = flash::set(session, &Flash::success(FLASH_PUBLISHED))?;
            Ok((session, format::redirect(&href)?).into_response())
        }
        Err(TicketFailure::Invalid(errors)) => {
            if errors.errors.contains_key("mapping") {
                return render_migrate(ctx, v, tenant, workflow, mapping, Some(&errors)).await;
            }
            let st = BuilderState {
                errors: Some(&errors),
                ..state(None)
            };
            render_builder(
                ctx,
                v,
                tenant,
                workflow,
                st,
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => Err(e.into()),
    }
}

async fn render_migrate(
    ctx: &AppContext,
    v: &TeraView,
    tenant: &Tenant,
    workflow: &workflows::Model,
    mapping: &BTreeMap<String, String>,
    errors: Option<&ModelValidationErrors>,
) -> Result<Response> {
    let plan = service::plan(&ctx.db, workflow)
        .await
        .map_err(Error::from)?;
    let href = format!("{}/{}", base(&tenant.workspace.slug), workflow.id);
    let added: Vec<String> = plan
        .draft
        .stages
        .iter()
        .filter(|s| plan.current.stage(&s.key).is_none())
        .map(|s| s.name.clone())
        .collect();
    let changed: Vec<String> = plan
        .draft
        .stages
        .iter()
        .filter(|s| plan.current.stage(&s.key).is_some_and(|o| o != *s))
        .map(|s| s.name.clone())
        .collect();
    let removed: Vec<_> = plan
        .removed
        .iter()
        .map(|r| {
            // Saran: stage sekategori pertama di draf.
            let suggested = mapping
                .get(&r.key)
                .cloned()
                .or_else(|| plan.draft.first_in(r.category).map(|s| s.key.clone()))
                .unwrap_or_default();
            serde_json::json!({
                "key": r.key,
                "name": r.name,
                "category_key": views::workflow::category_key(r.category),
                "tickets": r.tickets,
                "selected": suggested,
            })
        })
        .collect();
    let status = if errors.is_some() {
        StatusCode::UNPROCESSABLE_ENTITY
    } else {
        StatusCode::OK
    };
    let mut removed_stat = Stat::new("workflow-migrate-removed", plan.removed.len(), "trash");
    removed_stat.tone = "danger";
    let mut map_stat = Stat::new(
        "workflow-migrate-to-map",
        plan.removed.iter().map(|r| r.tickets).sum::<u64>(),
        "ticket",
    );
    map_stat.tone = "warn";
    let stats = vec![
        removed_stat,
        map_stat,
        Stat::new("workflow-migrate-others", plan.others, "checks"),
    ];
    let targets: Vec<SelectOption> = plan
        .draft
        .stages
        .iter()
        .map(|s| SelectOption {
            id: s.key.clone(),
            name: s.name.clone(),
        })
        .collect();
    let next_version = workflow.version + 1;
    let action = format!("{href}/migrate");
    let shell = app_shell(ctx, &tenant.user, Some(tenant), None)
        .await?
        .at("workflows");
    views::tickets::render(
        v,
        &shell,
        "workflows/migrate.html",
        status,
        data!({
            "workflow": { "id": workflow.id, "name": workflow.name, "version": workflow.version, "next_version": next_version },
            "removed": removed,
            "stats": stats,
            "added": added,
            "changed": changed,
            "targets": targets,
            "errors": codes(errors),
            "details": details(errors),
            "action": action,
            "back": href,
        }),
    )
}

#[debug_handler]
pub(super) async fn migrate(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageTicketSettings>,
    id: IdOr404<workflows::Entity>,
) -> Result<Response> {
    let workflow = find(&ctx, &access.tenant, id.id).await?;
    if workflow.draft.is_none() {
        return format::redirect(&format!(
            "{}/{}",
            base(&access.tenant.workspace.slug),
            workflow.id
        ));
    }
    render_migrate(&ctx, &v, &access.tenant, &workflow, &BTreeMap::new(), None).await
}

#[debug_handler]
pub(super) async fn apply_migration(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageTicketSettings>,
    session: Session,
    RequestId(request_id): RequestId,
    id: IdOr404<workflows::Entity>,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Result<Response> {
    let workflow = find(&ctx, &access.tenant, id.id).await?;
    let mapping: BTreeMap<String, String> = pairs
        .iter()
        .filter_map(|(k, v)| {
            k.strip_prefix("map_")
                .filter(|_| !v.trim().is_empty())
                .map(|k| (k.to_string(), v.trim().to_string()))
        })
        .collect();
    finish_publish(
        &ctx,
        &v,
        &access.tenant,
        session,
        request_id,
        &workflow,
        &mapping,
    )
    .await
}

#[debug_handler]
pub(super) async fn discard(
    State(ctx): State<AppContext>,
    access: Require<ManageTicketSettings>,
    session: Session,
    RequestId(request_id): RequestId,
    id: IdOr404<workflows::Entity>,
) -> Result<Response> {
    let tenant = &access.tenant;
    service::discard_draft(&ctx.db, &acting(tenant, request_id), id.id).await?;
    let session = flash::set(session, &Flash::success(FLASH_DISCARDED))?;
    let to = format!("{}/{}", base(&tenant.workspace.slug), id.id);
    Ok((session, format::redirect(&to)?).into_response())
}

#[debug_handler]
pub(super) async fn delete(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageTicketSettings>,
    session: Session,
    RequestId(request_id): RequestId,
    id: IdOr404<workflows::Entity>,
) -> Result<Response> {
    let tenant = &access.tenant;
    match service::delete(&ctx.db, &acting(tenant, request_id), id.id).await {
        Ok(()) => {
            let session = flash::set(session, &Flash::success(FLASH_DELETED))?;
            Ok((session, format::redirect(&base(&tenant.workspace.slug))?).into_response())
        }
        Err(TicketFailure::Invalid(errors)) => {
            let workflow = find(&ctx, tenant, id.id).await?;
            let st = BuilderState {
                errors: Some(&errors),
                ..state(None)
            };
            render_builder(
                &ctx,
                &v,
                tenant,
                &workflow,
                st,
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => Err(e.into()),
    }
}

// ---------------------------------------------------------------- project

async fn render_project(
    ctx: &AppContext,
    v: &TeraView,
    access: &crate::controllers::extractors::ProjectAccess,
    flash: Option<Flash>,
    status: StatusCode,
) -> Result<Response> {
    let wid = access.tenant.workspace_id();
    let paths = project_paths(access);
    let types = ticket_types::Model::all_in(&ctx.db, wid).await?;
    let all = workflows::Model::all_in(&ctx.db, wid).await?;
    let overrides = project_workflows::Model::of_project(&ctx.db, wid, access.project.id).await?;
    let mut rows = Vec::new();
    for t in &types {
        let chosen = overrides
            .iter()
            .find(|o| o.type_id == t.id)
            .map(|o| o.workflow_id);
        let effective = chosen.unwrap_or(t.workflow_id);
        let def = all
            .iter()
            .find(|w| w.id == effective)
            .map(|w| w.definition(&ctx.db));
        let stages = match def {
            Some(f) => f
                .await?
                .stages
                .iter()
                .map(|s| s.name.clone())
                .collect::<Vec<_>>(),
            None => Vec::new(),
        };
        let count = tickets::Entity::find()
            .filter(tickets::Column::WorkspaceId.eq(wid.get()))
            .filter(tickets::Column::ProjectId.eq(access.project.id))
            .filter(tickets::Column::TypeId.eq(t.id))
            .filter(tickets::active())
            .count(&ctx.db)
            .await?;
        rows.push(serde_json::json!({
            "type": TypeView::of(t),
            "field": format!("type_{}", t.id),
            "selected": chosen.map(|c| c.to_string()).unwrap_or_default(),
            "default_name": all.iter().find(|w| w.id == t.workflow_id).map(|w| w.name.clone()).unwrap_or_default(),
            "stages": stages,
            "tickets": count,
        }));
    }
    let shell = app_shell(ctx, &access.tenant.user, Some(&access.tenant), Some(access))
        .await?
        .at("project_workflow");
    views::tickets::render(
        v,
        &shell,
        "workflows/project.html",
        status,
        data!({
            "project": summary(access),
            "paths": paths,
            "subnav": nav::project_settings_nav(&paths, "workflow"),
            "rows": rows,
            "options": all.iter().map(|w| SelectOption { id: w.id.to_string(), name: w.name.clone() }).collect::<Vec<_>>(),
            "allow_force": access.project.allow_force_transition,
            "action": paths.workflow,
            "manage_href": access.tenant.can(crate::models::permission::Permission::ManageTicketSettings).then(|| base(&access.tenant.workspace.slug)),
            "flash": flash,
        }),
    )
}

#[debug_handler]
pub(super) async fn project(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    ProjectAdmin(access): ProjectAdmin,
    session: Session,
) -> Result<Response> {
    let (session, flash) = flash::take(session);
    let res = render_project(&ctx, &v, &access, flash, StatusCode::OK).await?;
    Ok((session, res).into_response())
}

#[debug_handler]
pub(super) async fn save_project(
    State(ctx): State<AppContext>,
    ProjectAdmin(access): ProjectAdmin,
    session: Session,
    RequestId(request_id): RequestId,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Result<Response> {
    let mut input = ProjectWorkflows {
        overrides: Vec::new(),
        allow_force_transition: !get_one(&pairs, "allow_force_transition").is_empty(),
    };
    for (k, value) in &pairs {
        let Some(type_id) = k
            .strip_prefix("type_")
            .and_then(|id| Uuid::parse_str(id).ok())
        else {
            continue;
        };
        let choice = match value.trim() {
            "" => None,
            v => Some(crate::controllers::extractors::form_id(v)?),
        };
        input.overrides.push((type_id, choice));
    }
    service::set_project_workflows(
        &ctx.db,
        &acting(&access.tenant, request_id),
        access.project.id,
        &input,
    )
    .await?;
    let session = flash::set(session, &Flash::success(FLASH_PROJECT_SAVED))?;
    let to = project_paths(&access).workflow;
    Ok((session, format::redirect(&to)?).into_response())
}
