//! Pengaturan → Knowledge workspace (M8, `Require<ManageKnowledge>`):
//!
//! - `docs` — dokumen workspace (standar lintas project): daftar, dokumen baru, editor (halaman
//!   sendiri) + pengaturan berlaku-untuk/persona/pemilik, arsipkan;
//! - `types` — katalog bawaan (baca saja) + jenis custom (template + instruksi AI), halaman sendiri;
//! - `auto` — pemicu update otomatis, audit mingguan, kedalaman bawaan, sinkron repo.
use axum::http::StatusCode;
use loco_rs::{prelude::*, validation::ModelValidationErrors};
use sea_orm::{ActiveEnum, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::{
    controllers::{
        extractors::{RequestId, Require, Tenant, require::ManageKnowledge},
        flash,
        shell::app_shell,
    },
    models::{
        knowledge_catalog, knowledge_docs, knowledge_runs::Depth, knowledge_settings,
        knowledge_types, knowledge_versions, personas, projects, users, workspace_members,
    },
    services::{
        knowledge::{self as kb, Failure, docs, settings as service},
        workspaces::Acting,
    },
    views::{
        self,
        flash::Flash,
        form::field_errors,
        knowledge::{self as kv, VersionRow},
        ui::ComboOption,
    },
};

fn base(slug: &str) -> String {
    format!("/w/{slug}/settings/knowledge")
}

fn acting(tenant: &Tenant, request_id: Option<String>) -> Acting<'_> {
    Acting {
        workspace: &tenant.workspace,
        user: &tenant.user,
        role: &tenant.role,
        request_id,
    }
}

fn field<'a>(pairs: &'a [(String, String)], name: &str) -> &'a str {
    pairs
        .iter()
        .find(|(k, _)| k == name)
        .map_or("", |(_, v)| v.as_str())
}

fn all<'a>(pairs: &'a [(String, String)], name: &str) -> Vec<&'a str> {
    pairs
        .iter()
        .filter(|(k, v)| k == name && !v.trim().is_empty())
        .map(|(_, v)| v.as_str())
        .collect()
}

fn uuids(pairs: &[(String, String)], name: &str) -> Vec<Uuid> {
    all(pairs, name)
        .into_iter()
        .filter_map(|v| Uuid::parse_str(v.trim()).ok())
        .collect()
}

fn first_code(errors: &ModelValidationErrors) -> String {
    errors
        .errors
        .iter()
        .find_map(|(_, es)| es.first().map(|e| e.code.to_string()))
        .unwrap_or_else(|| "kb-flash-invalid".into())
}

fn ok_to(session: Session, to: &str, key: &str) -> Result<Response> {
    let session = flash::set(session, &Flash::success(key))?;
    Ok((session, format::redirect(to)?).into_response())
}

fn fail_to(session: Session, to: &str, e: Failure) -> Result<Response> {
    match e {
        Failure::Invalid(errors) => {
            let session = flash::set(session, &Flash::error(&first_code(&errors)))?;
            Ok((session, format::redirect(to)?).into_response())
        }
        Failure::NotFound => Err(Error::NotFound),
        Failure::Db(e) => Err(e.into()),
    }
}

async fn render(
    ctx: &AppContext,
    v: &TeraView,
    tenant: &Tenant,
    template: &str,
    status: StatusCode,
    mut data: serde_json::Value,
) -> Result<Response> {
    let shell = app_shell(ctx, &tenant.user, Some(tenant), None)
        .await?
        .at("knowledge");
    let b = base(&tenant.workspace.slug);
    let current = data["section"].as_str().unwrap_or("docs").to_string();
    if let Some(d) = data.as_object_mut() {
        d.insert("base".into(), serde_json::json!(b));
        d.insert(
            "subnav".into(),
            serde_json::to_value(kv::settings_subnav(&b, &current))?,
        );
    }
    views::tickets::render(v, &shell, template, status, data)
}

async fn project_options(ctx: &AppContext, tenant: &Tenant) -> Result<Vec<ComboOption>> {
    let wid = tenant.workspace_id();
    Ok(projects::Entity::find()
        .filter(projects::Column::WorkspaceId.eq(wid.get()))
        .filter(projects::Column::ArchivedAt.is_null())
        .all(&ctx.db)
        .await?
        .into_iter()
        .map(|p| {
            ComboOption::new(p.id, &p.name)
                .sub(&p.key)
                .color(p.color.as_deref().unwrap_or_default())
        })
        .collect())
}

#[debug_handler]
async fn index(
    State(_ctx): State<AppContext>,
    access: Require<ManageKnowledge>,
) -> Result<Response> {
    format::redirect(&format!("{}/docs", base(&access.tenant.workspace.slug)))
}

// ---------------------------------------------------------------- dokumen workspace

#[debug_handler]
async fn docs_index(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageKnowledge>,
    session: Session,
) -> Result<Response> {
    let (session, flash) = flash::take(session);
    let tenant = &access.tenant;
    let wid = tenant.workspace_id();
    let b = base(&tenant.workspace.slug);
    let list = knowledge_docs::Model::of_workspace(&ctx.db, wid).await?;
    let projects = projects::Entity::find()
        .filter(projects::Column::WorkspaceId.eq(wid.get()))
        .all(&ctx.db)
        .await?;
    let owners: Vec<Uuid> = list.iter().filter_map(|d| d.owner_id).collect();
    let people = users::Entity::find()
        .filter(users::Column::Id.is_in(owners))
        .all(&ctx.db)
        .await?;
    let rows: Vec<serde_json::Value> = list
        .iter()
        .map(|d| {
            let applies: Vec<&str> = projects
                .iter()
                .filter(|p| d.applies_to.contains(&p.id))
                .map(|p| p.name.as_str())
                .collect();
            serde_json::json!({
                "slug": d.slug,
                "title": d.title,
                "href": format!("{b}/docs/{}", d.slug),
                "applies": applies.join(", "),
                "personas": d.personas.join(", "),
                "updated": views::time::wib_datetime(&d.updated_at),
                "owner": d.owner_id.and_then(|o| people.iter().find(|u| u.id == o)).map(views::tickets::PersonView::of),
            })
        })
        .collect();
    let res = render(
        &ctx,
        &v,
        tenant,
        "knowledge/settings_docs.html",
        StatusCode::OK,
        serde_json::json!({ "section": "docs", "rows": rows, "flash": flash }),
    )
    .await?;
    Ok((session, res).into_response())
}

#[debug_handler]
async fn doc_new(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageKnowledge>,
) -> Result<Response> {
    render_doc_form(
        &ctx,
        &v,
        &access.tenant,
        None,
        &[],
        (None, None),
        StatusCode::OK,
    )
    .await
}

/// Isian editor dokumen workspace (baru atau ubah).
async fn render_doc_form(
    ctx: &AppContext,
    v: &TeraView,
    tenant: &Tenant,
    doc: Option<&knowledge_docs::Model>,
    pairs: &[(String, String)],
    (error, flash): (Option<String>, Option<Flash>),
    status: StatusCode,
) -> Result<Response> {
    let wid = tenant.workspace_id();
    let b = base(&tenant.workspace.slug);
    let (title, body, base_version) = match doc {
        Some(d) if pairs.is_empty() => (d.title.clone(), d.body.clone(), d.version),
        Some(d) => (
            field(pairs, "title").to_string(),
            field(pairs, "body").to_string(),
            field(pairs, "base_version").parse().unwrap_or(d.version),
        ),
        None => (
            field(pairs, "title").to_string(),
            field(pairs, "body").to_string(),
            0,
        ),
    };
    let persona_options: Vec<ComboOption> = personas::Model::active_in(&ctx.db, wid)
        .await?
        .into_iter()
        .map(|p| ComboOption::new(&p.key, &p.name).sub(&p.summary))
        .collect();
    let people: Vec<ComboOption> = workspace_members::Model::list_in(&ctx.db, wid)
        .await?
        .iter()
        .map(|(_, u, _)| ComboOption::person(u.id, &u.name, &u.email))
        .collect();
    let (versions, href) = match doc {
        Some(d) => {
            let href = format!("{b}/docs/{}", d.slug);
            let list = knowledge_versions::Model::of_doc(&ctx.db, wid, d.id).await?;
            let ids: Vec<Uuid> = list.iter().filter_map(|x| x.author_id).collect();
            let people: std::collections::HashMap<Uuid, users::Model> = users::Entity::find()
                .filter(users::Column::Id.is_in(ids))
                .all(&ctx.db)
                .await?
                .into_iter()
                .map(|u| (u.id, u))
                .collect();
            (
                list.iter()
                    .take(6)
                    .map(|x| VersionRow::of(x, &format!("{href}/versions"), d.version, &people))
                    .collect::<Vec<_>>(),
                href,
            )
        }
        None => (Vec::new(), format!("{b}/docs/new")),
    };
    render(
        ctx,
        v,
        tenant,
        "knowledge/settings_doc.html",
        status,
        serde_json::json!({
            "section": "docs",
            "is_new": doc.is_none(),
            "doc": doc.map(|d| serde_json::json!({
                "slug": d.slug,
                "path": d.repo_path(),
                "version": d.version,
                "applies_to": d.applies_to.iter().map(Uuid::to_string).collect::<Vec<_>>(),
                "personas": d.personas,
                "owner": d.owner_id.map(|o| o.to_string()).unwrap_or_default(),
                "updated": views::time::wib_datetime(&d.updated_at),
            })),
            "doc_title": title,
            "body": body,
            "base_version": base_version,
            "error": error,
            "action": href,
            "project_options": project_options(ctx, tenant).await?,
            "persona_options": persona_options,
            "people": people,
            "versions": versions,
            "flash": flash,
        }),
    )
    .await
}

#[debug_handler]
async fn doc_create(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageKnowledge>,
    session: Session,
    RequestId(request_id): RequestId,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let new = docs::NewDoc {
        project_id: None,
        title: field(&pairs, "title").trim().to_string(),
        type_key: "konvensi".into(),
        body: field(&pairs, "body").to_string(),
        slug: String::new(),
    };
    match docs::create(&ctx.db, &acting(tenant, request_id), &new).await {
        Ok(d) => ok_to(
            session,
            &format!("{}/docs/{}", base(&tenant.workspace.slug), d.slug),
            "kb-flash-created",
        ),
        Err(Failure::Invalid(errors)) => {
            render_doc_form(
                &ctx,
                &v,
                tenant,
                None,
                &pairs,
                (Some(first_code(&errors)), None),
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => fail_to(session, &base(&tenant.workspace.slug), e),
    }
}

async fn workspace_doc(
    ctx: &AppContext,
    tenant: &Tenant,
    slug: &str,
) -> Result<knowledge_docs::Model> {
    knowledge_docs::Model::by_slug(&ctx.db, tenant.workspace_id(), None, slug)
        .await?
        .filter(|d| d.archived_at.is_none())
        .ok_or(Error::NotFound)
}

#[debug_handler]
async fn doc_edit(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageKnowledge>,
    session: Session,
    Path((_, slug)): Path<(String, String)>,
) -> Result<Response> {
    let (session, flash) = flash::take(session);
    let doc = workspace_doc(&ctx, &access.tenant, &slug).await?;
    let res = render_doc_form(
        &ctx,
        &v,
        &access.tenant,
        Some(&doc),
        &[],
        (None, flash),
        StatusCode::OK,
    )
    .await?;
    Ok((session, res).into_response())
}

#[debug_handler]
async fn doc_save(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageKnowledge>,
    session: Session,
    RequestId(request_id): RequestId,
    Path((_, slug)): Path<(String, String)>,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let doc = workspace_doc(&ctx, tenant, &slug).await?;
    let acting = acting(tenant, request_id);
    let base_version = field(&pairs, "base_version").parse().unwrap_or(0);
    let saved = docs::save(
        &ctx.db,
        &acting,
        doc.id,
        field(&pairs, "title"),
        field(&pairs, "body"),
        base_version,
        field(&pairs, "message"),
    )
    .await;
    let saved = match saved {
        // Isi sama: tetap simpan pengaturan meta di bawah.
        Err(Failure::Invalid(errors)) if first_code(&errors) == kb::UNCHANGED => Ok(doc.clone()),
        other => other,
    };
    match saved {
        Ok(d) => {
            let owner = Uuid::parse_str(field(&pairs, "owner").trim()).ok();
            let personas = all(&pairs, "personas")
                .into_iter()
                .map(str::to_string)
                .collect();
            if let Err(e) = docs::set_workspace_meta(
                &ctx.db,
                &acting,
                d.id,
                uuids(&pairs, "applies_to"),
                personas,
                owner,
            )
            .await
            {
                return fail_to(
                    session,
                    &format!("{}/docs/{}", base(&tenant.workspace.slug), d.slug),
                    e,
                );
            }
            ok_to(
                session,
                &format!("{}/docs/{}", base(&tenant.workspace.slug), d.slug),
                "kb-flash-saved",
            )
        }
        Err(Failure::Invalid(errors)) => {
            let code = first_code(&errors);
            let status = if code == kb::CONFLICT {
                StatusCode::CONFLICT
            } else {
                StatusCode::UNPROCESSABLE_ENTITY
            };
            render_doc_form(
                &ctx,
                &v,
                tenant,
                Some(&doc),
                &pairs,
                (Some(code), None),
                status,
            )
            .await
        }
        Err(e) => fail_to(session, &base(&tenant.workspace.slug), e),
    }
}

#[debug_handler]
async fn doc_archive(
    State(ctx): State<AppContext>,
    access: Require<ManageKnowledge>,
    session: Session,
    RequestId(request_id): RequestId,
    Path((_, slug)): Path<(String, String)>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let doc = workspace_doc(&ctx, tenant, &slug).await?;
    let to = format!("{}/docs", base(&tenant.workspace.slug));
    match docs::archive(&ctx.db, &acting(tenant, request_id), doc.id).await {
        Ok(_) => ok_to(session, &to, "kb-flash-archived"),
        Err(e) => fail_to(session, &to, e),
    }
}

// ---------------------------------------------------------------- jenis dokumen

#[debug_handler]
async fn types_index(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageKnowledge>,
    session: Session,
) -> Result<Response> {
    let (session, flash) = flash::take(session);
    let tenant = &access.tenant;
    let wid = tenant.workspace_id();
    let b = base(&tenant.workspace.slug);
    let custom = knowledge_types::Model::active_in(&ctx.db, wid).await?;
    let projects = projects::Entity::find()
        .filter(projects::Column::WorkspaceId.eq(wid.get()))
        .all(&ctx.db)
        .await?;
    let rows: Vec<serde_json::Value> = custom
        .iter()
        .map(|t| {
            let ps: Vec<&str> = projects
                .iter()
                .filter(|p| t.projects.contains(&p.id))
                .map(|p| p.name.as_str())
                .collect();
            serde_json::json!({
                "name": t.name,
                "key": t.key,
                "group": knowledge_catalog::group(&t.group_key).map_or("", |g| g.name),
                "projects": ps.join(", "),
                "href": format!("{b}/types/{}", t.id),
            })
        })
        .collect();
    let builtin: Vec<serde_json::Value> = knowledge_catalog::GROUPS
        .iter()
        .map(|g| {
            let types: Vec<&str> = knowledge_catalog::TYPES
                .iter()
                .filter(|t| t.group == g.key)
                .map(|t| t.name)
                .collect();
            serde_json::json!({ "name": g.name, "icon": g.icon, "desc": g.desc, "types": types.join(" · ") })
        })
        .collect();
    let res = render(
        &ctx,
        &v,
        tenant,
        "knowledge/settings_types.html",
        StatusCode::OK,
        serde_json::json!({ "section": "types", "rows": rows, "builtin": builtin, "flash": flash }),
    )
    .await?;
    Ok((session, res).into_response())
}

fn type_form(pairs: &[(String, String)]) -> service::TypeForm {
    service::TypeForm {
        name: field(pairs, "name").to_string(),
        group_key: field(pairs, "group_key").to_string(),
        template: field(pairs, "template").to_string(),
        instructions: field(pairs, "instructions").to_string(),
        projects: uuids(pairs, "projects"),
        ai_scan: field(pairs, "ai_scan") == "on",
        on_done: field(pairs, "on_done") == "on",
    }
}

async fn render_type_form(
    ctx: &AppContext,
    v: &TeraView,
    tenant: &Tenant,
    t: Option<&knowledge_types::Model>,
    pairs: &[(String, String)],
    errors: Option<&ModelValidationErrors>,
    status: StatusCode,
) -> Result<Response> {
    let b = base(&tenant.workspace.slug);
    let form = if pairs.is_empty() {
        t.map_or_else(
            || {
                serde_json::json!({
                    "name": "", "group_key": "operasional", "template": "", "instructions": "",
                    "projects": Vec::<String>::new(), "ai_scan": true, "on_done": false,
                })
            },
            |t| {
                serde_json::json!({
                    "name": t.name, "group_key": t.group_key, "template": t.template,
                    "instructions": t.instructions,
                    "projects": t.projects.iter().map(Uuid::to_string).collect::<Vec<_>>(),
                    "ai_scan": t.ai_scan, "on_done": t.on_done,
                })
            },
        )
    } else {
        serde_json::json!({
            "name": field(pairs, "name"), "group_key": field(pairs, "group_key"),
            "template": field(pairs, "template"), "instructions": field(pairs, "instructions"),
            "projects": all(pairs, "projects"),
            "ai_scan": field(pairs, "ai_scan") == "on", "on_done": field(pairs, "on_done") == "on",
        })
    };
    let used = match t {
        Some(t) => knowledge_docs::Entity::find()
            .filter(knowledge_docs::Column::WorkspaceId.eq(tenant.workspace_id().get()))
            .filter(knowledge_docs::Column::TypeKey.eq(t.key.clone()))
            .filter(knowledge_docs::Column::ArchivedAt.is_null())
            .all(&ctx.db)
            .await?
            .len(),
        None => 0,
    };
    let groups: Vec<ComboOption> = knowledge_catalog::GROUPS
        .iter()
        .map(|g| ComboOption::new(g.key, g.name))
        .collect();
    render(
        ctx,
        v,
        tenant,
        "knowledge/settings_type.html",
        status,
        serde_json::json!({
            "section": "types",
            "is_new": t.is_none(),
            "type": t.map(|t| serde_json::json!({ "id": t.id, "name": t.name, "key": t.key })),
            "used": used,
            "form": form,
            "errors": field_errors(&["name", "group_key"], errors),
            "action": t.map_or_else(|| format!("{b}/types/new"), |t| format!("{b}/types/{}", t.id)),
            "groups": groups,
            "project_options": project_options(ctx, tenant).await?,
        }),
    )
    .await
}

#[debug_handler]
async fn type_new(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageKnowledge>,
) -> Result<Response> {
    render_type_form(&ctx, &v, &access.tenant, None, &[], None, StatusCode::OK).await
}

#[debug_handler]
async fn type_create(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageKnowledge>,
    session: Session,
    RequestId(request_id): RequestId,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let b = base(&tenant.workspace.slug);
    match service::create_type(&ctx.db, &acting(tenant, request_id), &type_form(&pairs)).await {
        Ok(_) => ok_to(session, &format!("{b}/types"), "kb-flash-type-created"),
        Err(Failure::Invalid(errors)) => {
            render_type_form(
                &ctx,
                &v,
                tenant,
                None,
                &pairs,
                Some(&errors),
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => fail_to(session, &b, e),
    }
}

async fn find_type(ctx: &AppContext, tenant: &Tenant, id: &str) -> Result<knowledge_types::Model> {
    let id = Uuid::parse_str(id).map_err(|_| Error::NotFound)?;
    knowledge_types::Model::find_in(&ctx.db, tenant.workspace_id(), id)
        .await?
        .filter(|t| t.archived_at.is_none())
        .ok_or(Error::NotFound)
}

#[debug_handler]
async fn type_edit(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageKnowledge>,
    Path((_, id)): Path<(String, String)>,
) -> Result<Response> {
    let t = find_type(&ctx, &access.tenant, &id).await?;
    render_type_form(
        &ctx,
        &v,
        &access.tenant,
        Some(&t),
        &[],
        None,
        StatusCode::OK,
    )
    .await
}

#[debug_handler]
async fn type_save(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageKnowledge>,
    session: Session,
    RequestId(request_id): RequestId,
    Path((_, id)): Path<(String, String)>,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let t = find_type(&ctx, tenant, &id).await?;
    let b = base(&tenant.workspace.slug);
    match service::update_type(
        &ctx.db,
        &acting(tenant, request_id),
        t.id,
        &type_form(&pairs),
    )
    .await
    {
        Ok(_) => ok_to(session, &format!("{b}/types"), "kb-flash-type-saved"),
        Err(Failure::Invalid(errors)) => {
            render_type_form(
                &ctx,
                &v,
                tenant,
                Some(&t),
                &pairs,
                Some(&errors),
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => fail_to(session, &b, e),
    }
}

#[debug_handler]
async fn type_archive(
    State(ctx): State<AppContext>,
    access: Require<ManageKnowledge>,
    session: Session,
    RequestId(request_id): RequestId,
    Path((_, id)): Path<(String, String)>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let t = find_type(&ctx, tenant, &id).await?;
    let to = format!("{}/types", base(&tenant.workspace.slug));
    match service::archive_type(&ctx.db, &acting(tenant, request_id), t.id).await {
        Ok(_) => ok_to(session, &to, "kb-flash-type-archived"),
        Err(e) => fail_to(session, &to, e),
    }
}

// ---------------------------------------------------------------- update otomatis

const DAYS: [&str; 7] = [
    "kb-day-0", "kb-day-1", "kb-day-2", "kb-day-3", "kb-day-4", "kb-day-5", "kb-day-6",
];

#[debug_handler]
async fn auto(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageKnowledge>,
    session: Session,
) -> Result<Response> {
    let (session, flash) = flash::take(session);
    let tenant = &access.tenant;
    let s = knowledge_settings::Model::of(&ctx.db, tenant.workspace_id()).await?;
    let days: Vec<serde_json::Value> = DAYS
        .iter()
        .enumerate()
        .map(|(i, k)| serde_json::json!({ "id": i.to_string(), "name": k }))
        .collect();
    let depths: Vec<serde_json::Value> = ["cepat", "standar", "mendalam"]
        .iter()
        .map(|d| serde_json::json!({ "id": d, "name": format!("kb-depth-{d}") }))
        .collect();
    let res = render(
        &ctx,
        &v,
        tenant,
        "knowledge/settings_auto.html",
        StatusCode::OK,
        serde_json::json!({
            "section": "auto",
            "settings": {
                "on_done": s.on_done, "on_merge": s.on_merge, "audit": s.audit,
                "audit_day": s.audit_day.to_string(), "default_depth": s.default_depth,
                "repo_sync": s.repo_sync,
                "last_audit": s.last_audit_at.as_ref().map(views::time::wib_datetime),
            },
            "days": days,
            "depths": depths,
            "flash": flash,
        }),
    )
    .await?;
    Ok((session, res).into_response())
}

#[debug_handler]
async fn auto_save(
    State(ctx): State<AppContext>,
    access: Require<ManageKnowledge>,
    session: Session,
    RequestId(request_id): RequestId,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let to = format!("{}/auto", base(&tenant.workspace.slug));
    let form = service::SettingsForm {
        on_done: field(&pairs, "on_done") == "on",
        on_merge: field(&pairs, "on_merge") == "on",
        audit: field(&pairs, "audit") == "on",
        audit_day: field(&pairs, "audit_day").parse().unwrap_or(-1),
        default_depth: Depth::try_from_value(&field(&pairs, "default_depth").to_string())
            .unwrap_or(Depth::Standar),
        repo_sync: field(&pairs, "repo_sync") == "on",
    };
    match service::save(&ctx.db, &acting(tenant, request_id), &form).await {
        Ok(_) => ok_to(session, &to, "kb-flash-settings"),
        Err(e) => fail_to(session, &to, e),
    }
}

/// Kunci Fluent yang dibangun di kode (dites kontrak i18n).
#[must_use]
pub fn label_keys() -> Vec<String> {
    DAYS.iter().map(|d| (*d).to_string()).collect()
}

pub fn routes() -> Routes {
    Routes::new()
        .add("/w/{slug}/settings/knowledge", get(index))
        .add("/w/{slug}/settings/knowledge/docs", get(docs_index))
        .add(
            "/w/{slug}/settings/knowledge/docs/new",
            get(doc_new).post(doc_create),
        )
        .add(
            "/w/{slug}/settings/knowledge/docs/{doc}",
            get(doc_edit).post(doc_save),
        )
        .add(
            "/w/{slug}/settings/knowledge/docs/{doc}/archive",
            post(doc_archive),
        )
        .add("/w/{slug}/settings/knowledge/types", get(types_index))
        .add(
            "/w/{slug}/settings/knowledge/types/new",
            get(type_new).post(type_create),
        )
        .add(
            "/w/{slug}/settings/knowledge/types/{id}",
            get(type_edit).post(type_save),
        )
        .add(
            "/w/{slug}/settings/knowledge/types/{id}/archive",
            post(type_archive),
        )
        .add(
            "/w/{slug}/settings/knowledge/auto",
            get(auto).post(auto_save),
        )
}
