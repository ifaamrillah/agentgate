//! Pengaturan → Skill pack (`Require<ManageTicketSettings>`): daftar, buat sendiri (dialog →
//! editor), impor dari repo GitHub (dialog), editor berkas (draf/ditolak), kirim ke review,
//! laporan pindai, approve/tolak, hapus. Skill pack workspace lain atau id tidak valid → 404.
use axum::{extract::Query, http::StatusCode};
use loco_rs::{prelude::*, validation::ModelValidationErrors};
use sea_orm::EntityTrait;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    common::settings::Settings,
    controllers::{
        extractors::{IdOr404, RequestId, Require, Tenant, require::ManageTicketSettings},
        flash,
        shell::app_shell,
    },
    models::{
        persona_skills, personas, skill_files,
        skill_packs::{self, SKILL_TOOLS, SkillSource, SkillStatus},
        users,
    },
    services::{skills as service, tickets::TicketFailure, workspaces::Acting},
    views::{
        self,
        flash::Flash,
        form::field_errors,
        skills::{FileView, SkillRow, checks},
        ui::Stat,
    },
};

pub(super) fn base(slug: &str) -> String {
    format!("/w/{slug}/settings/skills")
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

/// Dialog yang terbuka saat halaman daftar dirender ulang (422).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Open {
    #[default]
    None,
    New,
    Import,
}

#[allow(clippy::too_many_arguments)]
async fn render_index(
    ctx: &AppContext,
    v: &TeraView,
    tenant: &Tenant,
    pairs: &[(String, String)],
    open: Open,
    errors: Option<&ModelValidationErrors>,
    flash: Option<Flash>,
    status: StatusCode,
) -> Result<Response> {
    let wid = tenant.workspace_id();
    let base = base(&tenant.workspace.slug);
    let packs = skill_packs::Model::all_in(&ctx.db, wid).await?;
    let links = persona_skills::Model::all_in(&ctx.db, wid).await?;
    let rows: Vec<serde_json::Value> = packs
        .iter()
        .map(|p| {
            serde_json::json!({
                "pack": SkillRow::of(p, &base),
                "used": links.iter().filter(|l| l.pack_id == p.id).count(),
            })
        })
        .collect();
    let count = |s: SkillStatus| packs.iter().filter(|p| p.status() == s).count();
    let stats = vec![
        Stat::new(
            "skill-stat-approved",
            count(SkillStatus::Approved),
            "package",
        )
        .tone("success"),
        Stat::new("skill-stat-pending", count(SkillStatus::Pending), "eye").tone("warn"),
        Stat::new(
            "skill-stat-custom",
            packs
                .iter()
                .filter(|p| p.source() == SkillSource::Custom)
                .count(),
            "pencil",
        )
        .tone("accent"),
        Stat::new(
            "skill-stat-rejected",
            count(SkillStatus::Rejected),
            "shield-x",
        ),
    ];
    let shell = app_shell(ctx, &tenant.user, Some(tenant), None)
        .await?
        .at("skills");
    views::tickets::render(
        v,
        &shell,
        "skills/index.html",
        status,
        data!({
            "rows": rows,
            "stats": stats,
            "new_open": open == Open::New,
            "import_open": open == Open::Import,
            "form": {
                "name": get(pairs, "name").trim(),
                "description": get(pairs, "description").trim(),
                "url": get(pairs, "url").trim(),
                "ref": get(pairs, "ref").trim(),
                "path": get(pairs, "path").trim(),
            },
            "errors": field_errors(&["name", "description", "url", "path"], errors),
            "create_action": base,
            "import_action": format!("{base}/import"),
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
        &[],
        Open::None,
        None,
        flash,
        StatusCode::OK,
    )
    .await?;
    Ok((session, res).into_response())
}

#[allow(clippy::too_many_arguments)]
async fn index_or_error(
    ctx: &AppContext,
    v: &TeraView,
    tenant: &Tenant,
    session: Session,
    pairs: &[(String, String)],
    open: Open,
    result: std::result::Result<skill_packs::Model, TicketFailure>,
    flash_key: &str,
) -> Result<Response> {
    match result {
        Ok(p) => {
            let session = flash::set(session, &Flash::success(flash_key))?;
            let to = format!("{}/{}", base(&tenant.workspace.slug), p.id);
            Ok((session, format::redirect(&to)?).into_response())
        }
        Err(TicketFailure::Invalid(errors)) => {
            render_index(
                ctx,
                v,
                tenant,
                pairs,
                open,
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
pub(super) async fn create(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageTicketSettings>,
    session: Session,
    RequestId(request_id): RequestId,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let r = service::create_custom(
        &ctx.db,
        &acting(tenant, request_id),
        get(&pairs, "name"),
        get(&pairs, "description"),
    )
    .await;
    index_or_error(
        &ctx,
        &v,
        tenant,
        session,
        &pairs,
        Open::New,
        r,
        views::skills::FLASH_CREATED,
    )
    .await
}

#[debug_handler]
pub(super) async fn import(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageTicketSettings>,
    session: Session,
    RequestId(request_id): RequestId,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let settings = Settings::from_ctx(&ctx)?;
    let r = service::import(
        &ctx.db,
        &acting(tenant, request_id),
        &settings.skills,
        get(&pairs, "url"),
        get(&pairs, "ref"),
        get(&pairs, "path"),
    )
    .await;
    index_or_error(
        &ctx,
        &v,
        tenant,
        session,
        &pairs,
        Open::Import,
        r,
        views::skills::FLASH_IMPORTED,
    )
    .await
}

#[derive(Debug, Deserialize)]
pub(super) struct ShowQuery {
    #[serde(default)]
    file: Option<String>,
}

/// Isian yang gagal disimpan (ditampilkan ulang dengan 422).
#[derive(Debug, Default)]
struct Draft {
    info: Option<(String, String, Vec<String>)>,
    file: Option<(String, String)>,
    new_path: Option<String>,
}

async fn find(ctx: &AppContext, tenant: &Tenant, id: Uuid) -> Result<skill_packs::Model> {
    skill_packs::Model::find_in(&ctx.db, tenant.workspace_id(), id)
        .await?
        .ok_or(Error::NotFound)
}

#[allow(clippy::too_many_arguments)]
async fn render_show(
    ctx: &AppContext,
    v: &TeraView,
    tenant: &Tenant,
    pack: &skill_packs::Model,
    selected: Option<&str>,
    draft: Draft,
    errors: Option<&ModelValidationErrors>,
    flash: Option<Flash>,
    status: StatusCode,
) -> Result<Response> {
    let wid = tenant.workspace_id();
    let base = base(&tenant.workspace.slug);
    let href = format!("{base}/{}", pack.id);
    let files = skill_files::Model::of_pack(&ctx.db, pack.id).await?;
    let current_path = draft
        .file
        .as_ref()
        .map(|(p, _)| p.clone())
        .or_else(|| selected.map(str::to_string))
        .filter(|p| files.iter().any(|f| &f.path == p))
        .or_else(|| files.first().map(|f| f.path.clone()))
        .unwrap_or_default();
    let current = files.iter().find(|f| f.path == current_path);
    let body = draft
        .file
        .as_ref()
        .map(|(_, b)| b.clone())
        .or_else(|| current.map(|f| f.body.clone()))
        .unwrap_or_default();
    let tree: Vec<FileView> = files
        .iter()
        .map(|f| FileView {
            path: f.path.clone(),
            kind: f.kind(),
            href: format!("{href}?file={}", urlencode(&f.path)),
            current: f.path == current_path,
            lines: f.body.lines().count(),
            icon: views::skills::file_icon(f.kind()),
        })
        .collect();
    let (name, description, tools) = draft.info.unwrap_or_else(|| {
        (
            pack.name.clone(),
            pack.description.clone(),
            pack.requested_tools.clone(),
        )
    });
    let tool_rows: Vec<serde_json::Value> = SKILL_TOOLS
        .iter()
        .map(|t| serde_json::json!({ "value": t, "on": tools.iter().any(|x| x == t) }))
        .collect();
    let all_personas = personas::Model::active_in(&ctx.db, wid).await?;
    let used: Vec<String> = persona_skills::Model::all_in(&ctx.db, wid)
        .await?
        .iter()
        .filter(|l| l.pack_id == pack.id)
        .filter_map(|l| all_personas.iter().find(|p| p.id == l.persona_id))
        .map(|p| p.name.clone())
        .collect();
    let person = |id: Option<Uuid>| async move {
        match id {
            Some(id) => users::Entity::find_by_id(id)
                .one(&ctx.db)
                .await
                .map(|u| u.map(|u| u.name)),
            None => Ok(None),
        }
    };
    let creator = person(pack.created_by).await?;
    let reviewer = person(pack.reviewed_by).await?;
    let editable = pack.editable() && pack.source() == SkillSource::Custom;
    let skills = files
        .iter()
        .filter(|f| f.path.ends_with("SKILL.md"))
        .count();
    let shell = app_shell(ctx, &tenant.user, Some(tenant), None)
        .await?
        .at("skills");
    views::tickets::render(
        v,
        &shell,
        "skills/show.html",
        status,
        data!({
            "pack": SkillRow::of(pack, &base),
            "raw": pack,
            "href": href,
            "back": base,
            "editable": editable,
            "pending": pack.status() == SkillStatus::Pending,
            "danger": pack.risk() == Some(skill_packs::SkillRisk::Danger),
            "files": tree,
            "current": current_path,
            "current_icon": views::skills::file_icon(current.map_or("md", skill_files::Model::kind)),
            "body": body,
            "skills": skills,
            "info": { "name": name, "description": description },
            "tools": tool_rows,
            "checks": checks(&pack.report),
            "used": used,
            "creator": creator,
            "reviewer": reviewer,
            "new_path": draft.new_path.unwrap_or_default(),
            "errors": field_errors(
                &["name", "description", "tools", "path", "body", "pack", "note", "confirm",
                  "decision"],
                errors,
            ),
            "flash": flash,
        }),
    )
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'/' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[debug_handler]
pub(super) async fn show(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageTicketSettings>,
    id: IdOr404<skill_packs::Entity>,
    Query(q): Query<ShowQuery>,
    session: Session,
) -> Result<Response> {
    let tenant = &access.tenant;
    let pack = find(&ctx, tenant, id.id).await?;
    let (session, flash) = flash::take(session);
    let res = render_show(
        &ctx,
        &v,
        tenant,
        &pack,
        q.file.as_deref(),
        Draft::default(),
        None,
        flash,
        StatusCode::OK,
    )
    .await?;
    Ok((session, res).into_response())
}

/// PRG ke halaman skill pack (berkas `file` terpilih), atau render ulang 422 dengan isian.
#[allow(clippy::too_many_arguments)]
async fn after(
    ctx: &AppContext,
    v: &TeraView,
    tenant: &Tenant,
    session: Session,
    id: Uuid,
    file: Option<&str>,
    result: std::result::Result<(), TicketFailure>,
    draft: Draft,
    flash_key: &str,
) -> Result<Response> {
    match result {
        Ok(()) => {
            let session = flash::set(session, &Flash::success(flash_key))?;
            let mut to = format!("{}/{id}", base(&tenant.workspace.slug));
            if let Some(f) = file {
                to = format!("{to}?file={}", urlencode(f));
            }
            Ok((session, format::redirect(&to)?).into_response())
        }
        Err(TicketFailure::Invalid(errors)) => {
            let pack = find(ctx, tenant, id).await?;
            render_show(
                ctx,
                v,
                tenant,
                &pack,
                file,
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

#[debug_handler]
pub(super) async fn update(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageTicketSettings>,
    id: IdOr404<skill_packs::Entity>,
    session: Session,
    RequestId(request_id): RequestId,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Result<Response> {
    let tenant = &access.tenant;
    find(&ctx, tenant, id.id).await?;
    let tools: Vec<String> = pairs
        .iter()
        .filter(|(k, _)| k == "tools")
        .map(|(_, v)| v.clone())
        .collect();
    let name = get(&pairs, "name").to_string();
    let description = get(&pairs, "description").to_string();
    let r = service::update_info(
        &ctx.db,
        &acting(tenant, request_id),
        id.id,
        &name,
        &description,
        &tools,
    )
    .await
    .map(|_| ());
    let draft = Draft {
        info: Some((name, description, tools)),
        ..Draft::default()
    };
    after(
        &ctx,
        &v,
        tenant,
        session,
        id.id,
        None,
        r,
        draft,
        views::skills::FLASH_SAVED,
    )
    .await
}

#[debug_handler]
pub(super) async fn save_file(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageTicketSettings>,
    id: IdOr404<skill_packs::Entity>,
    session: Session,
    RequestId(request_id): RequestId,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Result<Response> {
    let tenant = &access.tenant;
    find(&ctx, tenant, id.id).await?;
    let path = get(&pairs, "path").trim().to_string();
    let body = get(&pairs, "body").to_string();
    let r = service::save_file(&ctx.db, &acting(tenant, request_id), id.id, &path, &body).await;
    let draft = Draft {
        file: Some((path.clone(), body)),
        ..Draft::default()
    };
    after(
        &ctx,
        &v,
        tenant,
        session,
        id.id,
        Some(&path),
        r,
        draft,
        views::skills::FLASH_SAVED,
    )
    .await
}

/// Tambah berkas baru (isi awal menurut jenisnya).
#[debug_handler]
pub(super) async fn add_file(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageTicketSettings>,
    id: IdOr404<skill_packs::Entity>,
    session: Session,
    RequestId(request_id): RequestId,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Result<Response> {
    let tenant = &access.tenant;
    find(&ctx, tenant, id.id).await?;
    let path = get(&pairs, "path")
        .trim()
        .trim_start_matches('/')
        .to_string();
    let files = skill_files::Model::of_pack(&ctx.db, id.id).await?;
    let r = if files.iter().any(|f| f.path == path) {
        Err(TicketFailure::field("path", skill_packs::PATH_INVALID))
    } else {
        let starter = if path.ends_with(".md") {
            format!(
                "# {}\n",
                path.rsplit('/')
                    .next()
                    .unwrap_or_default()
                    .trim_end_matches(".md")
            )
        } else if path.ends_with(".py") {
            "#!/usr/bin/env python3\n\n".to_string()
        } else if path.ends_with(".sh") {
            "#!/usr/bin/env bash\nset -euo pipefail\n\n".to_string()
        } else {
            String::new()
        };
        service::save_file(&ctx.db, &acting(tenant, request_id), id.id, &path, &starter).await
    };
    let ok = r.is_ok();
    let draft = Draft {
        new_path: Some(path.clone()),
        ..Draft::default()
    };
    after(
        &ctx,
        &v,
        tenant,
        session,
        id.id,
        ok.then_some(path.as_str()),
        r,
        draft,
        views::skills::FLASH_SAVED,
    )
    .await
}

#[debug_handler]
pub(super) async fn delete_file(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageTicketSettings>,
    id: IdOr404<skill_packs::Entity>,
    session: Session,
    RequestId(request_id): RequestId,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Result<Response> {
    let tenant = &access.tenant;
    find(&ctx, tenant, id.id).await?;
    let r = service::delete_file(
        &ctx.db,
        &acting(tenant, request_id),
        id.id,
        get(&pairs, "path").trim(),
    )
    .await;
    after(
        &ctx,
        &v,
        tenant,
        session,
        id.id,
        None,
        r,
        Draft::default(),
        views::skills::FLASH_SAVED,
    )
    .await
}

macro_rules! decision {
    ($name:ident, $field:literal, $flash:path, |$db:ident, $acting:ident, $id:ident, $value:ident| $call:expr) => {
        #[debug_handler]
        pub(super) async fn $name(
            State(ctx): State<AppContext>,
            ViewEngine(v): ViewEngine<TeraView>,
            access: Require<ManageTicketSettings>,
            id: IdOr404<skill_packs::Entity>,
            session: Session,
            RequestId(request_id): RequestId,
            Form(pairs): Form<Vec<(String, String)>>,
        ) -> Result<Response> {
            let tenant = &access.tenant;
            find(&ctx, tenant, id.id).await?;
            let $db = &ctx.db;
            let $acting = acting(tenant, request_id);
            let $id = id.id;
            let $value = get(&pairs, $field).to_string();
            let r = $call.await.map(|_| ());
            after(
                &ctx,
                &v,
                tenant,
                session,
                id.id,
                None,
                r,
                Draft::default(),
                $flash,
            )
            .await
        }
    };
}

decision!(
    submit,
    "note",
    views::skills::FLASH_SUBMITTED,
    |db, acting, id, note| { service::submit(db, &acting, id, &note) }
);
decision!(
    approve,
    "confirm",
    views::skills::FLASH_APPROVED,
    |db, acting, id, confirm| { service::approve(db, &acting, id, &confirm) }
);
decision!(
    reject,
    "note",
    views::skills::FLASH_REJECTED,
    |db, acting, id, note| { service::reject(db, &acting, id, &note) }
);

#[debug_handler]
pub(super) async fn delete(
    State(ctx): State<AppContext>,
    access: Require<ManageTicketSettings>,
    id: IdOr404<skill_packs::Entity>,
    session: Session,
    RequestId(request_id): RequestId,
) -> Result<Response> {
    let tenant = &access.tenant;
    find(&ctx, tenant, id.id).await?;
    service::delete(&ctx.db, &acting(tenant, request_id), id.id).await?;
    let session = flash::set(session, &Flash::success(views::skills::FLASH_DELETED))?;
    Ok((session, format::redirect(&base(&tenant.workspace.slug))?).into_response())
}
