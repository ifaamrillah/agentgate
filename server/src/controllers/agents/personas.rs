//! Pengaturan → Persona agent (`Require<ManageTicketSettings>`): daftar kartu (bawaan, custom),
//! persona baru (dialog: kosong atau clone), Edit persona (subpage per bagian, satu form per
//! bagian, PRG), hapus dengan pengganti. Persona workspace lain, id tidak valid, atau persona
//! yang sudah dihapus → 404.
use std::collections::{BTreeMap, HashMap};

use agentgate_core::persona::ModelChoice;
use axum::{extract::Query, http::StatusCode};
use loco_rs::{prelude::*, validation::ModelValidationErrors};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    controllers::{
        extractors::{IdOr404, RequestId, Require, Tenant, require::ManageTicketSettings},
        flash,
        shell::app_shell,
    },
    models::{
        persona_skills,
        personas::{self, Identity},
        skill_packs::{self, SkillStatus},
        workflows,
    },
    services::{
        personas::{self as service, Edit},
        tickets::TicketFailure,
        workspaces::Acting,
    },
    views::{
        self,
        flash::Flash,
        form::field_errors,
        personas::{
            IdentityForm, PersonaCard, always_blocked, client_tools, danger_rows, mcp_tools,
            model_rows, persona_options, skill_options, subnav,
        },
        ui::Stat,
    },
};

pub(super) fn base(slug: &str) -> String {
    format!("/w/{slug}/settings/personas")
}

fn acting(tenant: &Tenant, request_id: Option<String>) -> Acting<'_> {
    Acting {
        workspace: &tenant.workspace,
        user: &tenant.user,
        role: &tenant.role,
        request_id,
    }
}

fn get<'a>(pairs: &'a [(String, String)], name: &str) -> &'a str {
    pairs
        .iter()
        .find(|(k, _)| k == name)
        .map_or("", |(_, v)| v.as_str())
}

fn all(pairs: &[(String, String)], name: &str) -> Vec<String> {
    pairs
        .iter()
        .filter(|(k, _)| k == name)
        .map(|(_, v)| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .collect()
}

fn identity_form(pairs: &[(String, String)]) -> IdentityForm {
    IdentityForm {
        name: get(pairs, "name").trim().to_string(),
        summary: get(pairs, "summary").trim().to_string(),
        color: get(pairs, "color").trim().to_string(),
        icon: get(pairs, "icon").trim().to_string(),
        source: get(pairs, "source").trim().to_string(),
    }
}

fn identity_of(form: &IdentityForm) -> Identity {
    Identity {
        name: form.name.clone(),
        summary: form.summary.clone(),
        color: form.color.clone(),
        icon: form.icon.clone(),
    }
}

/// "Workflow · Stage" per kunci persona (setelah mengikuti pengganti), dari versi aktif setiap
/// workflow workspace.
async fn stage_usage(ctx: &AppContext, tenant: &Tenant) -> Result<HashMap<String, Vec<String>>> {
    let wid = tenant.workspace_id();
    let labels = personas::Model::labels(&ctx.db, wid).await?;
    let mut usage: HashMap<String, Vec<String>> = HashMap::new();
    for w in workflows::Model::all_in(&ctx.db, wid).await? {
        let definition = w.definition(&ctx.db).await?;
        for stage in &definition.stages {
            if let Some(key) = &stage.persona {
                let resolved = labels.get(key).map_or(key.clone(), |l| l.key.clone());
                usage
                    .entry(resolved)
                    .or_default()
                    .push(format!("{} · {}", w.name, stage.name));
            }
        }
    }
    Ok(usage)
}

async fn render_index(
    ctx: &AppContext,
    v: &TeraView,
    tenant: &Tenant,
    new: IdentityForm,
    errors: Option<&ModelValidationErrors>,
    flash: Option<Flash>,
    status: StatusCode,
) -> Result<Response> {
    let wid = tenant.workspace_id();
    let slug = &tenant.workspace.slug;
    let base = base(slug);
    let list = personas::Model::active_in(&ctx.db, wid).await?;
    let links = persona_skills::Model::all_in(&ctx.db, wid).await?;
    let usage = stage_usage(ctx, tenant).await?;
    let cards: Vec<PersonaCard> = list
        .iter()
        .map(|p| {
            PersonaCard::of(
                p,
                links.iter().filter(|l| l.persona_id == p.id).count(),
                usage.get(&p.key).cloned().unwrap_or_default(),
                &base,
            )
        })
        .collect();
    let packs = skill_packs::Model::all_in(&ctx.db, wid).await?;
    let used = cards.iter().filter(|c| !c.stages.is_empty()).count();
    let skills_href = format!("/w/{slug}/settings/skills");
    let stats = vec![
        Stat::new("persona-stat-total", list.len(), "robot").hint(
            "persona-stat-total-hint",
            list.iter().filter(|p| !p.builtin).count(),
        ),
        Stat::new("persona-stat-used", used, "route").tone("accent"),
        Stat::new(
            "persona-stat-skills",
            packs
                .iter()
                .filter(|p| p.status() == SkillStatus::Approved)
                .count(),
            "package",
        )
        .link(&skills_href),
        Stat::new(
            "persona-stat-pending",
            packs
                .iter()
                .filter(|p| p.status() == SkillStatus::Pending)
                .count(),
            "eye",
        )
        .tone("warn")
        .link(&skills_href),
    ];
    let new_open = errors.is_some();
    let shell = app_shell(ctx, &tenant.user, Some(tenant), None)
        .await?
        .at("personas");
    views::tickets::render(
        v,
        &shell,
        "personas/index.html",
        status,
        data!({
            "builtin": cards.iter().filter(|c| c.builtin).collect::<Vec<_>>(),
            "custom": cards.iter().filter(|c| !c.builtin).collect::<Vec<_>>(),
            "stats": stats,
            "new": new,
            "new_open": new_open,
            "errors": field_errors(&["name", "summary", "source"], errors),
            "sources": persona_options(&list, None),
            "create_action": base,
            "flash": flash,
        }),
    )
}

#[debug_handler]
pub(super) async fn index(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageTicketSettings>,
    session: Session,
) -> Result<Response> {
    let (session, flash) = flash::take(session);
    let res = render_index(
        &ctx,
        &v,
        &access.tenant,
        IdentityForm::default(),
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
    let form = identity_form(&pairs);
    let source = if form.source.is_empty() {
        None
    } else {
        match Uuid::try_parse(&form.source) {
            Ok(id) => Some(id),
            Err(_) => return Err(Error::NotFound),
        }
    };
    let r = service::create(
        &ctx.db,
        &acting(tenant, request_id),
        &identity_of(&form),
        source,
    )
    .await;
    match r {
        Ok(p) => {
            let session = flash::set(session, &Flash::success(views::personas::FLASH_CREATED))?;
            let to = format!("{}/{}", base(&tenant.workspace.slug), p.id);
            Ok((session, format::redirect(&to)?).into_response())
        }
        Err(TicketFailure::Invalid(errors)) => {
            render_index(
                &ctx,
                &v,
                tenant,
                form,
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
pub(super) struct EditQuery {
    #[serde(default)]
    section: Option<String>,
}

/// Tool MCP, tools client, dan tingkat aksi berbahaya dari isian form.
type ToolsDraft = (Vec<String>, Vec<String>, BTreeMap<String, String>);

/// Isian bagian yang gagal disimpan (ditampilkan ulang dengan 422).
#[derive(Debug, Default)]
struct Draft {
    identity: Option<IdentityForm>,
    instructions: Option<(String, bool)>,
    tools: Option<ToolsDraft>,
    models: Option<BTreeMap<String, ModelChoice>>,
    skills: Option<Vec<String>>,
}

async fn find_active(ctx: &AppContext, tenant: &Tenant, id: Uuid) -> Result<personas::Model> {
    personas::Model::find_active_by_id(&ctx.db, tenant.workspace_id(), id)
        .await?
        .ok_or(Error::NotFound)
}

#[allow(clippy::too_many_arguments)]
async fn render_edit(
    ctx: &AppContext,
    v: &TeraView,
    tenant: &Tenant,
    persona: &personas::Model,
    section: &str,
    draft: Draft,
    errors: Option<&ModelValidationErrors>,
    flash: Option<Flash>,
    status: StatusCode,
) -> Result<Response> {
    let wid = tenant.workspace_id();
    let slug = &tenant.workspace.slug;
    let base = base(slug);
    let href = format!("{base}/{}", persona.id);
    let section = if views::personas::SECTIONS.contains(&section) && section != "knowledge" {
        section
    } else {
        "identity"
    };
    let installed = persona_skills::Model::of_persona(&ctx.db, persona.id).await?;
    let approved = skill_packs::Model::approved_in(&ctx.db, wid).await?;
    let all_packs = skill_packs::Model::all_in(&ctx.db, wid).await?;
    let mut selected_skills: Vec<String> =
        installed.iter().map(|l| l.pack_id.to_string()).collect();
    if let Some(s) = &draft.skills {
        selected_skills.clone_from(s);
    }
    let skills: Vec<_> = installed
        .iter()
        .filter_map(|l| all_packs.iter().find(|p| p.id == l.pack_id))
        .map(|p| views::skills::SkillRow::of(p, &format!("/w/{slug}/settings/skills")))
        .collect();
    let identity = draft.identity.unwrap_or_else(|| IdentityForm::of(persona));
    let (instructions, concise) = draft
        .instructions
        .unwrap_or_else(|| (persona.instructions.clone(), persona.concise));
    let (mcp_sel, client_sel, danger) = match draft.tools {
        Some((m, c, d)) => (
            m,
            c,
            d.into_iter()
                .filter_map(|(a, l)| {
                    let action = agentgate_core::persona::DANGER_ACTIONS
                        .iter()
                        .find(|(k, _)| *k == a)?
                        .0;
                    Some((action, agentgate_core::persona::Level::parse(&l)?))
                })
                .collect(),
        ),
        None => (
            persona.mcp_tools.clone(),
            persona.client_tools.clone(),
            persona.danger_levels(),
        ),
    };
    let models = draft.models.unwrap_or_else(|| persona.model_choices());
    let usage = stage_usage(ctx, tenant).await?;
    let active = personas::Model::active_in(&ctx.db, wid).await?;
    let shell = app_shell(ctx, &tenant.user, Some(tenant), None)
        .await?
        .at("personas");
    views::tickets::render(
        v,
        &shell,
        "personas/edit.html",
        status,
        data!({
            "persona": persona,
            "section": section,
            "subnav": subnav(&href, section, installed.len()),
            "href": href,
            "back": base,
            "stages": usage.get(&persona.key).cloned().unwrap_or_default(),
            "identity": identity,
            "icons": personas::ICONS,
            "instructions": instructions,
            "concise": concise,
            "mcp_tools": mcp_tools(&mcp_sel),
            "client_tools": client_tools(&client_sel),
            "danger": danger_rows(&danger),
            "always_blocked": always_blocked(),
            "levels": views::personas::levels(),
            "models": model_rows(&models),
            "skills": skills,
            "skill_options": skill_options(&approved),
            "selected_skills": selected_skills,
            "replacements": persona_options(&active, Some(persona.id)),
            "errors": field_errors(
                &["name", "summary", "color", "icon", "instructions", "mcp_tools",
                  "client_tools", "danger", "models", "skills", "replacement"],
                errors,
            ),
            "flash": flash,
        }),
    )
}

#[debug_handler]
pub(super) async fn edit(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageTicketSettings>,
    id: IdOr404<personas::Entity>,
    Query(q): Query<EditQuery>,
    session: Session,
) -> Result<Response> {
    let tenant = &access.tenant;
    let persona = find_active(&ctx, tenant, id.id).await?;
    let (session, flash) = flash::take(session);
    let res = render_edit(
        &ctx,
        &v,
        tenant,
        &persona,
        q.section.as_deref().unwrap_or("identity"),
        Draft::default(),
        None,
        flash,
        StatusCode::OK,
    )
    .await?;
    Ok((session, res).into_response())
}

/// Bagian dari isian form: `Edit` untuk service dan `Draft` untuk menampilkan ulang.
fn parse_section(section: &str, pairs: &[(String, String)]) -> Option<(Edit, Draft)> {
    match section {
        "identity" => {
            let form = identity_form(pairs);
            Some((
                Edit::Identity(identity_of(&form)),
                Draft {
                    identity: Some(form),
                    ..Draft::default()
                },
            ))
        }
        "instructions" => {
            let text = get(pairs, "instructions").to_string();
            let concise = get(pairs, "concise") == "on";
            Some((
                Edit::Instructions {
                    instructions: text.clone(),
                    concise,
                },
                Draft {
                    instructions: Some((text, concise)),
                    ..Draft::default()
                },
            ))
        }
        "tools" => {
            let mcp = all(pairs, "mcp_tools");
            let client = all(pairs, "client_tools");
            let danger: BTreeMap<String, String> = pairs
                .iter()
                .filter_map(|(k, v)| {
                    k.strip_prefix("danger_")
                        .map(|a| (a.to_string(), v.trim().to_string()))
                })
                .collect();
            Some((
                Edit::Tools {
                    mcp: mcp.clone(),
                    client: client.clone(),
                    danger: danger.clone(),
                },
                Draft {
                    tools: Some((mcp, client, danger)),
                    ..Draft::default()
                },
            ))
        }
        "models" => {
            let mut models: BTreeMap<String, ModelChoice> = BTreeMap::new();
            for c in agentgate_core::persona::CLIENTS {
                let model = get(pairs, &format!("model_{}", c.key)).trim().to_string();
                let effort = get(pairs, &format!("effort_{}", c.key)).trim().to_string();
                models.insert(c.key.to_string(), ModelChoice { model, effort });
            }
            Some((
                Edit::Models(models.clone()),
                Draft {
                    models: Some(models),
                    ..Draft::default()
                },
            ))
        }
        "skills" => {
            let raw = all(pairs, "skills");
            let ids = raw.iter().filter_map(|s| Uuid::try_parse(s).ok()).collect();
            Some((
                Edit::Skills(ids),
                Draft {
                    skills: Some(raw),
                    ..Draft::default()
                },
            ))
        }
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
async fn save_section(
    ctx: &AppContext,
    v: &TeraView,
    tenant: &Tenant,
    session: Session,
    request_id: Option<String>,
    id: Uuid,
    section: &'static str,
    pairs: &[(String, String)],
) -> Result<Response> {
    let persona = find_active(ctx, tenant, id).await?;
    let Some((edit, draft)) = parse_section(section, pairs) else {
        return Err(Error::NotFound);
    };
    match service::update(&ctx.db, &acting(tenant, request_id), id, edit).await {
        Ok(_) => {
            let session = flash::set(session, &Flash::success(views::personas::FLASH_SAVED))?;
            let to = format!("{}/{id}?section={section}", base(&tenant.workspace.slug));
            Ok((session, format::redirect(&to)?).into_response())
        }
        Err(TicketFailure::Invalid(errors)) => {
            render_edit(
                ctx,
                v,
                tenant,
                &persona,
                section,
                draft,
                Some(&errors),
                None,
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => Err(e.into()),
    }
}

macro_rules! section_handler {
    ($name:ident, $section:literal) => {
        #[debug_handler]
        pub(super) async fn $name(
            State(ctx): State<AppContext>,
            ViewEngine(v): ViewEngine<TeraView>,
            access: Require<ManageTicketSettings>,
            id: IdOr404<personas::Entity>,
            session: Session,
            RequestId(request_id): RequestId,
            Form(pairs): Form<Vec<(String, String)>>,
        ) -> Result<Response> {
            save_section(
                &ctx,
                &v,
                &access.tenant,
                session,
                request_id,
                id.id,
                $section,
                &pairs,
            )
            .await
        }
    };
}

section_handler!(save_identity, "identity");
section_handler!(save_instructions, "instructions");
section_handler!(save_tools, "tools");
section_handler!(save_models, "models");
section_handler!(save_skills, "skills");

#[debug_handler]
pub(super) async fn delete(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageTicketSettings>,
    id: IdOr404<personas::Entity>,
    session: Session,
    RequestId(request_id): RequestId,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let persona = find_active(&ctx, tenant, id.id).await?;
    let replacement = Uuid::try_parse(get(&pairs, "replacement").trim()).ok();
    match service::delete(&ctx.db, &acting(tenant, request_id), id.id, replacement).await {
        Ok(()) => {
            let session = flash::set(session, &Flash::success(views::personas::FLASH_DELETED))?;
            Ok((session, format::redirect(&base(&tenant.workspace.slug))?).into_response())
        }
        Err(TicketFailure::Invalid(errors)) => {
            render_edit(
                &ctx,
                &v,
                tenant,
                &persona,
                "danger",
                Draft::default(),
                Some(&errors),
                None,
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => Err(e.into()),
    }
}
