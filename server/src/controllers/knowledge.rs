//! Knowledge base project (M8), tab project `knowledge` (`ViewProject`; mengubah `EditProject`,
//! memutuskan usulan = pemilik knowledge atau `AdministerProject`):
//!
//! - `GET …/knowledge` — beranda: angka, Perlu tindakan (usulan + basi), kesehatan, kartu
//!   kelompok; `?q=` hasil pencarian;
//! - `GET …/knowledge/g/{group}` — dokumen satu kelompok;
//! - `GET …/knowledge/proposals?p=` — tinjau usulan (mode fokus berdampingan) + approve/tolak/
//!   batal;
//! - `GET …/knowledge/{doc}` — editor dokumen (halaman sendiri), simpan dengan cek versi dasar;
//! - `GET …/knowledge/{doc}/versions?v=` — riwayat + diff versi, pulihkan;
//! - `POST` scan/dokumen baru/pemilik/masih benar/minta AI/arsipkan.
use std::collections::HashMap;

use axum::http::{StatusCode, header};
use loco_rs::prelude::*;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    common::markdown,
    controllers::{
        extractors::{ProjectAccess, RequestId},
        flash,
        shell::app_shell,
    },
    models::{
        knowledge_catalog, knowledge_docs,
        knowledge_proposals::{self, ProposalStatus},
        knowledge_runs::{Depth, RunKind},
        knowledge_versions,
        permission::Permission,
        project_access, users,
    },
    services::{
        knowledge::{self as kb, Failure, catalog, docs, proposals, runs, search},
        workspaces::Acting,
    },
    views::{
        self,
        flash::Flash,
        knowledge::{self as kv, DocRow, DocState, Donut, GroupCard, ProposalRow, VersionRow},
        nav,
        projects::{ProjectPaths, ProjectSummary},
        ui::{ComboOption, Stat},
    },
};

fn acting(access: &ProjectAccess, request_id: Option<String>) -> Acting<'_> {
    Acting {
        workspace: &access.tenant.workspace,
        user: &access.tenant.user,
        role: &access.tenant.role,
        request_id,
    }
}

fn base_of(access: &ProjectAccess) -> String {
    ProjectPaths::new(&access.tenant.workspace.slug, &access.project.key).knowledge
}

/// Kode Fluent pertama dari error validasi (pesan flash).
fn first_code(errors: &loco_rs::validation::ModelValidationErrors) -> String {
    errors
        .errors
        .iter()
        .find_map(|(_, es)| es.first().map(|e| e.code.to_string()))
        .unwrap_or_else(|| "kb-flash-invalid".into())
}

/// Ubah kegagalan service menjadi redirect dengan flash error (PRG), atau error HTTP.
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

fn ok_to(session: Session, to: &str, key: &str) -> Result<Response> {
    let session = flash::set(session, &Flash::success(key))?;
    Ok((session, format::redirect(to)?).into_response())
}

async fn users_map(db: &DatabaseConnection, ids: Vec<Uuid>) -> Result<HashMap<Uuid, users::Model>> {
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

/// Dokumen project + usulan pending, dalam bentuk baris.
struct Loaded {
    docs: Vec<knowledge_docs::Model>,
    pending: Vec<knowledge_proposals::Model>,
    rows: Vec<DocRow>,
    people: HashMap<Uuid, users::Model>,
}

impl Loaded {
    async fn of(ctx: &AppContext, access: &ProjectAccess) -> Result<Self> {
        let wid = access.tenant.workspace_id();
        let docs = knowledge_docs::Model::of_project(&ctx.db, wid, access.project.id).await?;
        let pending = knowledge_proposals::Model::of_project(
            &ctx.db,
            wid,
            access.project.id,
            ProposalStatus::Pending,
            200,
        )
        .await?;
        let mut ids: Vec<Uuid> = docs
            .iter()
            .filter_map(|d| d.owner_id.or(d.updated_by))
            .collect();
        ids.extend(access.project.knowledge_owner_id);
        ids.extend(pending.iter().filter_map(|p| p.created_by));
        ids.sort();
        ids.dedup();
        let people = users_map(&ctx.db, ids).await?;
        let base = base_of(access);
        let rows = docs
            .iter()
            .map(|d| {
                let p = pending
                    .iter()
                    .find(|p| p.doc_id == Some(d.id))
                    .map(|p| p.id);
                DocRow::of(d, &base, p, &people)
            })
            .collect();
        Ok(Self {
            docs,
            pending,
            rows,
            people,
        })
    }

    fn count(&self, state: DocState) -> usize {
        self.rows.iter().filter(|r| r.state == state).count()
    }
}

async fn render(
    ctx: &AppContext,
    v: &TeraView,
    access: &ProjectAccess,
    template: &str,
    return_to: &str,
    status: StatusCode,
    mut data: serde_json::Value,
) -> Result<Response> {
    let shell = app_shell(ctx, &access.tenant.user, Some(&access.tenant), Some(access))
        .await?
        .at("project_knowledge");
    let p = ProjectPaths::new(&access.tenant.workspace.slug, &access.project.key);
    let wid = access.tenant.workspace_id();
    let pending = proposals::pending_count(&ctx.db, wid, access.project.id).await?;
    let can_approve = kb::can_approve(&ctx.db, &acting(access, None), &access.project).await?;
    let owner = match access.project.knowledge_owner_id {
        Some(u) => users::Model::find_by_id(&ctx.db, u)
            .await?
            .map(|u| views::tickets::PersonView::of(&u)),
        None => None,
    };
    let extra = serde_json::json!({
        "project": ProjectSummary::new(&access.project, access.role.label(), &p),
        "views": nav::project_views(&p.home, &access.tenant.user.project_views, "knowledge"),
        "views_action": views::projects::PROJECT_VIEWS_ACTION,
        "return_to": return_to,
        "paths": p,
        "base": p.knowledge,
        "can_edit": access.role.has(Permission::EditProject),
        "can_approve": can_approve,
        "can_admin": access.role.has(Permission::AdministerProject),
        "pending_count": pending,
        "owner": owner,
        "kb_tabs": kv::tabs(&p.knowledge, data["tab"].as_str().unwrap_or("home"), pending),
    });
    if let (Some(d), serde_json::Value::Object(e)) = (data.as_object_mut(), extra) {
        d.extend(e);
    }
    views::tickets::render(v, &shell, template, status, data)
}

/// Data dialog Scan + Dokumen baru (beranda, kelompok, editor).
async fn dialogs(ctx: &AppContext, access: &ProjectAccess) -> Result<serde_json::Value> {
    let wid = access.tenant.workspace_id();
    let estimates = runs::estimates(&ctx.db, wid, access.project.id).await?;
    let types = catalog::available_types(&ctx.db, wid, access.project.id).await?;
    let relevant = catalog::relevant_groups(&ctx.db, wid, access.project.id).await?;
    let type_options: Vec<ComboOption> = types
        .iter()
        .filter(|t| t.custom || relevant.contains(&t.group.as_str()))
        .map(|t| {
            ComboOption::new(&t.key, &t.name)
                .sub(&t.hint)
                .group(knowledge_catalog::group(&t.group).map_or("", |g| g.name))
        })
        .collect();
    let settings = crate::models::knowledge_settings::Model::of(&ctx.db, wid).await?;
    Ok(serde_json::json!({
        "estimates": estimates,
        "default_depth": settings.default_depth,
        "type_options": type_options,
        "my_runners": crate::controllers::runner::my_runners(&ctx.db, &access.tenant).await?,
        "repo_names": crate::models::repos::Model::of_project(&ctx.db, wid, access.project.id)
            .await?
            .into_iter()
            .map(|r| r.name)
            .collect::<Vec<_>>(),
    }))
}

#[derive(Debug, Default, Deserialize)]
struct HomeQuery {
    #[serde(default)]
    q: String,
}

#[debug_handler]
async fn home(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: ProjectAccess,
    session: Session,
    Query(hq): Query<HomeQuery>,
) -> Result<Response> {
    access.require(Permission::ViewProject)?;
    let (session, flash) = flash::take(session);
    let wid = access.tenant.workspace_id();
    let base = base_of(&access);
    let l = Loaded::of(&ctx, &access).await?;
    let q: String = hq.q.trim().chars().take(200).collect();
    let results: Vec<serde_json::Value> = if q.is_empty() {
        Vec::new()
    } else {
        search::search(&ctx.db, wid, Some(access.project.id), &q, 30)
            .await?
            .into_iter()
            .map(|h| {
                serde_json::json!({
                    "title": h.doc.title,
                    "href": if h.doc.project_id.is_some() { format!("{base}/{}", h.doc.slug) } else { String::new() },
                    "workspace": h.doc.project_id.is_none(),
                    "group": kv::group_label_key(&h.doc.group_key),
                    "snippet": h.snippet,
                })
            })
            .collect()
    };
    let fresh = l.count(DocState::Fresh);
    let stale = l.count(DocState::Stale);
    let with_prop = l.count(DocState::Proposal);
    let relevant = catalog::relevant_groups(&ctx.db, wid, access.project.id).await?;
    let cards: Vec<GroupCard> = knowledge_catalog::GROUPS
        .iter()
        .filter(|g| relevant.contains(&g.key) || l.rows.iter().any(|r| r.group == g.key))
        .map(|g| GroupCard::of(g, &l.rows, &base))
        .collect();
    let stale_rows: Vec<&DocRow> = l
        .rows
        .iter()
        .filter(|r| r.state == DocState::Stale)
        .collect();
    let pending_rows: Vec<ProposalRow> = l
        .pending
        .iter()
        .map(|p| {
            let old = l
                .docs
                .iter()
                .find(|d| Some(d.id) == p.doc_id)
                .map_or("", |d| d.body.as_str());
            ProposalRow::of(p, &base, proposals::stat(old, &p.body), &l.people)
        })
        .collect();
    let open_run = runs::open_run(&ctx.db, wid, access.project.id, None).await?;
    let last_run = runs::latest(&ctx.db, wid, access.project.id).await?;
    let stats = vec![
        Stat::new("kb-stat-docs", l.rows.len(), "file-text")
            .hint("kb-stat-docs-hint", cards.len().to_string()),
        Stat::new("kb-stat-fresh", fresh, "circle-check").tone("success"),
        {
            let s =
                Stat::new("kb-stat-proposals", l.pending.len(), "git-pull-request").tone("accent");
            if l.pending.is_empty() {
                s
            } else {
                s.link(format!("{base}/proposals"))
            }
        },
        Stat::new("kb-stat-stale", stale, "clock").tone("warn"),
    ];
    let hidden: Vec<&str> = knowledge_catalog::GROUPS
        .iter()
        .filter(|g| !relevant.contains(&g.key))
        .map(|g| g.name)
        .collect();
    let mut data = serde_json::json!({
        "tab": "home",
        "q": q,
        "results": results,
        "stats": stats,
        "cards": cards,
        "stale_rows": stale_rows,
        "pending_rows": pending_rows,
        "action_count": stale_rows.len() + pending_rows.len(),
        "doc_count": l.rows.len(),
        "donut": Donut::of(fresh, with_prop, stale),
        "open_run": open_run.as_ref().map(run_json),
        "last_run": last_run.as_ref().map(run_json),
        "hidden_groups": hidden.join(", "),
        "owner_options": owner_options(&ctx, &access).await?,
        "flash": flash,
        "settings_href": access.tenant.can(Permission::ManageKnowledge).then(|| format!("/w/{}/settings/knowledge", access.tenant.workspace.slug)),
    });
    if let (Some(d), serde_json::Value::Object(e)) =
        (data.as_object_mut(), dialogs(&ctx, &access).await?)
    {
        d.extend(e);
    }
    let res = render(
        &ctx,
        &v,
        &access,
        "knowledge/home.html",
        &base,
        StatusCode::OK,
        data,
    )
    .await?;
    Ok((session, res).into_response())
}

fn run_json(r: &crate::models::knowledge_runs::Model) -> serde_json::Value {
    serde_json::json!({
        "kind": r.kind,
        "depth_label": runs::depth_label(r.depth()),
        "status": r.status,
        "scope": r.scope,
        "proposals": r.proposals,
        "tokens": r.estimate_tokens,
        "created": views::time::wib_datetime(&r.created_at),
        "trigger": r.trigger,
    })
}

async fn owner_options(ctx: &AppContext, access: &ProjectAccess) -> Result<Vec<ComboOption>> {
    let wid = access.tenant.workspace_id();
    let people =
        project_access::users_with_access(&ctx.db, wid, access.project.id, None, 200).await?;
    let mut out = Vec::new();
    for u in people {
        if kb::user_can(
            &ctx.db,
            wid,
            u.id,
            access.project.id,
            Permission::EditProject,
        )
        .await?
        {
            out.push(ComboOption::person(u.id, &u.name, &u.email));
        }
    }
    Ok(out)
}

#[debug_handler]
async fn group(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: ProjectAccess,
    session: Session,
    Path((_, _, key)): Path<(String, String, String)>,
) -> Result<Response> {
    access.require(Permission::ViewProject)?;
    let g = knowledge_catalog::group(&key).ok_or(Error::NotFound)?;
    let (session, flash) = flash::take(session);
    let base = base_of(&access);
    let l = Loaded::of(&ctx, &access).await?;
    let rows: Vec<&DocRow> = l.rows.iter().filter(|r| r.group == g.key).collect();
    let count = |s: DocState| rows.iter().filter(|r| r.state == s).count();
    let stats = vec![
        Stat::new("kb-stat-fresh", count(DocState::Fresh), "circle-check").tone("success"),
        {
            let s = Stat::new(
                "kb-stat-proposals",
                count(DocState::Proposal),
                "git-pull-request",
            )
            .tone("accent");
            if count(DocState::Proposal) > 0 {
                s.link(format!("{base}/proposals"))
            } else {
                s
            }
        },
        Stat::new("kb-stat-stale", count(DocState::Stale), "clock").tone("warn"),
    ];
    let mut data = serde_json::json!({
        "tab": "home",
        "group": { "key": g.key, "name": g.name, "icon": g.icon, "desc": g.desc },
        "group_tabs": kv::group_tabs(&base, g.key, &l.rows),
        "rows": rows,
        "stats": stats,
        "flash": flash,
    });
    if let (Some(d), serde_json::Value::Object(e)) =
        (data.as_object_mut(), dialogs(&ctx, &access).await?)
    {
        d.extend(e);
    }
    let res = render(
        &ctx,
        &v,
        &access,
        "knowledge/group.html",
        &format!("{base}/g/{key}"),
        StatusCode::OK,
        data,
    )
    .await?;
    Ok((session, res).into_response())
}

#[derive(Debug, Default, Deserialize)]
struct ReviewQuery {
    #[serde(default)]
    p: String,
}

#[debug_handler]
async fn review(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: ProjectAccess,
    session: Session,
    Query(rq): Query<ReviewQuery>,
) -> Result<Response> {
    access.require(Permission::ViewProject)?;
    let (session, flash) = flash::take(session);
    let wid = access.tenant.workspace_id();
    let base = base_of(&access);
    let (pending, decided) = proposals::queue(&ctx.db, wid, access.project.id).await?;
    let docs = knowledge_docs::Model::of_project(&ctx.db, wid, access.project.id).await?;
    let mut ids: Vec<Uuid> = pending
        .iter()
        .chain(decided.iter())
        .filter_map(|p| p.decided_by.or(p.created_by))
        .collect();
    ids.sort();
    ids.dedup();
    let people = users_map(&ctx.db, ids).await?;
    let old_of = |p: &knowledge_proposals::Model| -> String {
        docs.iter()
            .find(|d| Some(d.id) == p.doc_id || (p.doc_id.is_none() && d.slug == p.slug))
            .map(|d| d.body.clone())
            .unwrap_or_default()
    };
    let rows: Vec<ProposalRow> = pending
        .iter()
        .map(|p| ProposalRow::of(p, &base, proposals::stat(&old_of(p), &p.body), &people))
        .collect();
    let decided_rows: Vec<ProposalRow> = decided
        .iter()
        .map(|p| ProposalRow::of(p, &base, (0, 0), &people))
        .collect();
    let selected = Uuid::parse_str(rq.p.trim())
        .ok()
        .and_then(|id| pending.iter().position(|p| p.id == id))
        .unwrap_or(0);
    let current = pending.get(selected).map(|p| {
        let old = old_of(p);
        let doc = docs.iter().find(|d| Some(d.id) == p.doc_id);
        serde_json::json!({
            "row": rows[selected],
            "index": selected + 1,
            "path": format!(".agentgate/knowledge/{}.md", p.slug),
            "group": knowledge_catalog::group(&p.group_key).map(|g| serde_json::json!({ "key": g.key, "name": g.name, "icon": g.icon, "href": format!("{base}/g/{}", g.key) })),
            "base_version": p.base_version,
            "doc_version": doc.map(|d| d.version),
            "outdated": doc.is_some_and(|d| d.version != p.base_version),
            "sbs": proposals::side_by_side(&old, &p.body),
            "approve_action": format!("{base}/proposals/{}/approve", p.id),
            "reject_action": format!("{base}/proposals/{}/reject", p.id),
            "edit_href": format!("{base}/{}?proposal={}", p.slug, p.id),
            "prev_href": selected.checked_sub(1).and_then(|i| rows.get(i)).map(|r| r.href.clone()),
            "next_href": rows.get(selected + 1).map(|r| r.href.clone()),
        })
    });
    let res = render(
        &ctx,
        &v,
        &access,
        "knowledge/proposals.html",
        &format!("{base}/proposals"),
        StatusCode::OK,
        serde_json::json!({
            "tab": "proposals",
            "queue": rows,
            "decided": decided_rows,
            "current": current,
            "steps": (0..pending.len()).map(|i| if i == selected { "on" } else { "" }).collect::<Vec<_>>(),
            "owner_options": owner_options(&ctx, &access).await?,
            "flash": flash,
        }),
    )
    .await?;
    Ok((session, res).into_response())
}

/// Usulan berikutnya setelah memutuskan `id` (antrean pending sekarang).
async fn next_review(ctx: &AppContext, access: &ProjectAccess) -> Result<String> {
    let base = base_of(access);
    let wid = access.tenant.workspace_id();
    let (pending, _) = proposals::queue(&ctx.db, wid, access.project.id).await?;
    Ok(pending.first().map_or_else(
        || format!("{base}/proposals"),
        |p| format!("{base}/proposals?p={}", p.id),
    ))
}

#[derive(Debug, Default, Deserialize)]
struct DecideForm {
    #[serde(default)]
    note: String,
    /// Isi yang diedit penyetuju ("Edit dulu") + versi dokumen saat diedit.
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    base_version: Option<i32>,
}

#[debug_handler]
async fn approve(
    State(ctx): State<AppContext>,
    access: ProjectAccess,
    session: Session,
    RequestId(request_id): RequestId,
    Path((_, _, id)): Path<(String, String, Uuid)>,
    Form(form): Form<DecideForm>,
) -> Result<Response> {
    access.require(Permission::ViewProject)?;
    let edited = form.body.as_deref().zip(form.base_version);
    let back = format!("{}/proposals?p={id}", base_of(&access));
    match proposals::approve(&ctx.db, &acting(&access, request_id), id, edited).await {
        Ok((_, doc)) => {
            if edited.is_some() {
                let to = format!("{}/{}", base_of(&access), doc.slug);
                return ok_to(session, &to, "kb-flash-approved");
            }
            let to = next_review(&ctx, &access).await?;
            ok_to(session, &to, "kb-flash-approved")
        }
        Err(e) => fail_to(session, &back, e),
    }
}

#[debug_handler]
async fn reject(
    State(ctx): State<AppContext>,
    access: ProjectAccess,
    session: Session,
    RequestId(request_id): RequestId,
    Path((_, _, id)): Path<(String, String, Uuid)>,
    Form(form): Form<DecideForm>,
) -> Result<Response> {
    access.require(Permission::ViewProject)?;
    let back = format!("{}/proposals?p={id}", base_of(&access));
    match proposals::reject(&ctx.db, &acting(&access, request_id), id, &form.note).await {
        Ok(_) => {
            let to = next_review(&ctx, &access).await?;
            ok_to(session, &to, "kb-flash-rejected")
        }
        Err(e) => fail_to(session, &back, e),
    }
}

#[debug_handler]
async fn undo(
    State(ctx): State<AppContext>,
    access: ProjectAccess,
    session: Session,
    RequestId(request_id): RequestId,
    Path((_, _, id)): Path<(String, String, Uuid)>,
) -> Result<Response> {
    access.require(Permission::ViewProject)?;
    let to = format!("{}/proposals?p={id}", base_of(&access));
    match proposals::undo(&ctx.db, &acting(&access, request_id), id).await {
        Ok(_) => ok_to(session, &to, "kb-flash-undone"),
        Err(e) => fail_to(session, &format!("{}/proposals", base_of(&access)), e),
    }
}

#[derive(Debug, Default, Deserialize)]
struct ScanForm {
    #[serde(default)]
    depth: String,
    #[serde(default)]
    scope: String,
    #[serde(default)]
    runner: String,
    /// Slug dokumen: minta AI memperbarui dokumen ini saja.
    #[serde(default)]
    doc: String,
    #[serde(default)]
    next: String,
}

fn safe_next(next: &str, fallback: &str) -> String {
    if next.starts_with('/') && !next.starts_with("//") {
        next.to_string()
    } else {
        fallback.to_string()
    }
}

#[debug_handler]
async fn scan(
    State(ctx): State<AppContext>,
    access: ProjectAccess,
    session: Session,
    RequestId(request_id): RequestId,
    Form(form): Form<ScanForm>,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    let wid = access.tenant.workspace_id();
    let base = base_of(&access);
    let to = safe_next(&form.next, &base);
    let doc_ids = if form.doc.trim().is_empty() {
        Vec::new()
    } else {
        let doc =
            knowledge_docs::Model::by_slug(&ctx.db, wid, Some(access.project.id), form.doc.trim())
                .await?
                .ok_or(Error::NotFound)?;
        vec![doc.id]
    };
    let kind = if doc_ids.is_empty() {
        RunKind::Scan
    } else {
        RunKind::Update
    };
    let req = runs::RunRequest {
        kind,
        depth: sea_orm::ActiveEnum::try_from_value(&form.depth.trim().to_string())
            .unwrap_or(Depth::Standar),
        scope: form.scope.trim().to_string(),
        doc_ids,
        runner: Uuid::parse_str(form.runner.trim()).ok(),
    };
    match runs::request(
        &ctx.db,
        &acting(&access, request_id),
        access.project.id,
        req,
    )
    .await
    {
        Ok(_) => ok_to(
            session,
            &to,
            if kind == RunKind::Scan {
                "kb-flash-scan"
            } else {
                "kb-flash-update"
            },
        ),
        Err(e) => fail_to(session, &to, e),
    }
}

#[derive(Debug, Default, Deserialize)]
struct NewForm {
    #[serde(default)]
    type_key: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    ai_draft: String,
}

#[debug_handler]
async fn create(
    State(ctx): State<AppContext>,
    access: ProjectAccess,
    session: Session,
    RequestId(request_id): RequestId,
    Form(form): Form<NewForm>,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    let wid = access.tenant.workspace_id();
    let base = base_of(&access);
    let acting = acting(&access, request_id);
    let types = catalog::available_types(&ctx.db, wid, access.project.id).await?;
    let Some(t) = types.iter().find(|t| t.key == form.type_key.trim()) else {
        let session = flash::set(session, &Flash::error(kb::TYPE_INVALID))?;
        return Ok((session, format::redirect(&base)?).into_response());
    };
    let title = if form.title.trim().is_empty() {
        t.name.clone()
    } else {
        form.title.trim().to_string()
    };
    if form.ai_draft == "on" {
        let req = runs::RunRequest {
            kind: RunKind::Scan,
            depth: Depth::Standar,
            scope: format!("dokumen baru \"{title}\" (jenis `{}`)", t.key),
            doc_ids: Vec::new(),
            runner: None,
        };
        return match runs::request(&ctx.db, &acting, access.project.id, req).await {
            Ok(_) => ok_to(session, &base, "kb-flash-draft"),
            Err(e) => fail_to(session, &base, e),
        };
    }
    let new = docs::NewDoc {
        project_id: Some(access.project.id),
        title,
        type_key: t.key.clone(),
        body: t.template.clone(),
        slug: String::new(),
    };
    match docs::create(&ctx.db, &acting, &new).await {
        Ok(doc) => ok_to(session, &format!("{base}/{}", doc.slug), "kb-flash-created"),
        Err(e) => fail_to(session, &base, e),
    }
}

#[derive(Debug, Default, Deserialize)]
struct OwnerForm {
    #[serde(default)]
    owner: String,
    #[serde(default)]
    next: String,
}

#[debug_handler]
async fn set_owner(
    State(ctx): State<AppContext>,
    access: ProjectAccess,
    session: Session,
    RequestId(request_id): RequestId,
    Form(form): Form<OwnerForm>,
) -> Result<Response> {
    access.require(Permission::AdministerProject)?;
    let to = safe_next(&form.next, &base_of(&access));
    let owner = Uuid::parse_str(form.owner.trim()).ok();
    match docs::set_owner(
        &ctx.db,
        &acting(&access, request_id),
        access.project.id,
        owner,
    )
    .await
    {
        Ok(_) => ok_to(session, &to, "kb-flash-owner"),
        Err(e) => fail_to(session, &to, e),
    }
}

async fn find_doc(
    ctx: &AppContext,
    access: &ProjectAccess,
    slug: &str,
) -> Result<knowledge_docs::Model> {
    knowledge_docs::Model::by_slug(
        &ctx.db,
        access.tenant.workspace_id(),
        Some(access.project.id),
        slug,
    )
    .await?
    .filter(|d| d.archived_at.is_none())
    .ok_or(Error::NotFound)
}

#[derive(Debug, Default, Deserialize)]
struct DocQuery {
    #[serde(default)]
    proposal: String,
}

/// Isian editor yang gagal disimpan (konflik) ditampilkan ulang.
struct Draft {
    title: String,
    body: String,
    base_version: i32,
    error: Option<String>,
}

#[allow(clippy::too_many_arguments)]
async fn render_doc(
    ctx: &AppContext,
    v: &TeraView,
    access: &ProjectAccess,
    doc: &knowledge_docs::Model,
    draft: Option<Draft>,
    proposal_id: Option<Uuid>,
    flash: Option<Flash>,
    status: StatusCode,
) -> Result<Response> {
    let wid = access.tenant.workspace_id();
    let base = base_of(access);
    let href = format!("{base}/{}", doc.slug);
    let versions = knowledge_versions::Model::of_doc(&ctx.db, wid, doc.id).await?;
    let pending = knowledge_proposals::Model::of_project(
        &ctx.db,
        wid,
        access.project.id,
        ProposalStatus::Pending,
        200,
    )
    .await?;
    let doc_pending: Vec<&knowledge_proposals::Model> = pending
        .iter()
        .filter(|p| p.doc_id == Some(doc.id))
        .collect();
    // "Edit dulu": editor berisi isi usulan, simpan = approve dengan isi yang diedit.
    let editing = proposal_id.and_then(|id| doc_pending.iter().find(|p| p.id == id).copied());
    let mut ids: Vec<Uuid> = versions.iter().filter_map(|x| x.author_id).collect();
    ids.extend(doc.owner_id);
    ids.extend(doc.updated_by);
    ids.sort();
    ids.dedup();
    let people = users_map(&ctx.db, ids).await?;
    let row = DocRow::of(doc, &base, doc_pending.first().map(|p| p.id), &people);
    let repos = crate::models::repos::Model::of_project(&ctx.db, wid, access.project.id).await?;
    let sources: Vec<serde_json::Value> = docs::sources_of(&ctx.db, wid, doc.id)
        .await?
        .into_iter()
        .map(|s| {
            let repo = repos
                .iter()
                .find(|r| r.id == s.repo_id)
                .map_or_else(String::new, |r| r.name.clone());
            serde_json::json!({ "repo": repo, "path": s.path })
        })
        .collect();
    let tickets: Vec<serde_json::Value> = {
        let mut seen = Vec::new();
        let mut out = Vec::new();
        for p in knowledge_proposals::Entity::find()
            .filter(knowledge_proposals::Column::WorkspaceId.eq(wid.get()))
            .filter(knowledge_proposals::Column::DocId.eq(doc.id))
            .filter(knowledge_proposals::Column::TicketId.is_not_null())
            .all(&ctx.db)
            .await?
        {
            let Some(tid) = p.ticket_id else { continue };
            if seen.contains(&tid) {
                continue;
            }
            seen.push(tid);
            if let Some(t) = crate::models::tickets::Model::find_in(&ctx.db, wid, tid).await? {
                out.push(serde_json::json!({
                    "code": t.code,
                    "title": t.title,
                    "href": format!("/w/{}/t/{}", access.tenant.workspace.slug, t.code),
                }));
            }
        }
        out
    };
    let (title, body, base_version, error) = match (draft, editing) {
        (Some(d), _) => (d.title, d.body, d.base_version, d.error),
        (None, Some(p)) => (p.title.clone(), p.body.clone(), doc.version, None),
        (None, None) => (doc.title.clone(), doc.body.clone(), doc.version, None),
    };
    let types = catalog::available_types(&ctx.db, wid, access.project.id).await?;
    let mut data = serde_json::json!({
        "tab": "home",
        "doc": row,
        "doc_title": title,
        "body": body,
        "base_version": base_version,
        "error": error,
        "type_name": catalog::type_name(&types, &doc.type_key),
        "group": knowledge_catalog::group(&doc.group_key).map(|g| serde_json::json!({ "key": g.key, "name": g.name, "icon": g.icon, "href": format!("{base}/g/{}", g.key) })),
        "outline": markdown::outline(&doc.body),
        "sources": sources,
        "tickets": tickets,
        "versions": versions.iter().take(4).map(|x| VersionRow::of(x, &format!("{href}/versions"), doc.version, &people)).collect::<Vec<_>>(),
        "version_count": versions.len(),
        "proposal": doc_pending.first().map(|p| format!("{base}/proposals?p={}", p.id)),
        "editing": editing.map(|p| serde_json::json!({
            "id": p.id,
            "action": format!("{base}/proposals/{}/approve", p.id),
            "review": format!("{base}/proposals?p={}", p.id),
        })),
        "save_action": href,
        "href": href,
        "flash": flash,
    });
    if let (Some(d), serde_json::Value::Object(e)) =
        (data.as_object_mut(), dialogs(ctx, access).await?)
    {
        d.extend(e);
    }
    render(ctx, v, access, "knowledge/doc.html", &href, status, data).await
}

#[debug_handler]
async fn doc_page(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: ProjectAccess,
    session: Session,
    Path((_, _, slug)): Path<(String, String, String)>,
    Query(dq): Query<DocQuery>,
) -> Result<Response> {
    access.require(Permission::ViewProject)?;
    let (session, flash) = flash::take(session);
    let doc = find_doc(&ctx, &access, &slug).await?;
    let res = render_doc(
        &ctx,
        &v,
        &access,
        &doc,
        None,
        Uuid::parse_str(dq.proposal.trim()).ok(),
        flash,
        StatusCode::OK,
    )
    .await?;
    Ok((session, res).into_response())
}

#[derive(Debug, Default, Deserialize)]
struct SaveForm {
    #[serde(default)]
    title: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    base_version: i32,
    #[serde(default)]
    message: String,
}

#[debug_handler]
async fn save(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: ProjectAccess,
    session: Session,
    RequestId(request_id): RequestId,
    Path((_, _, slug)): Path<(String, String, String)>,
    Form(form): Form<SaveForm>,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    let doc = find_doc(&ctx, &access, &slug).await?;
    match docs::save(
        &ctx.db,
        &acting(&access, request_id),
        doc.id,
        &form.title,
        &form.body,
        form.base_version,
        &form.message,
    )
    .await
    {
        Ok(doc) => ok_to(
            session,
            &format!("{}/{}", base_of(&access), doc.slug),
            "kb-flash-saved",
        ),
        Err(Failure::Invalid(errors)) => {
            let code = first_code(&errors);
            let status = if code == kb::CONFLICT {
                StatusCode::CONFLICT
            } else {
                StatusCode::UNPROCESSABLE_ENTITY
            };
            // Isian tetap (tidak hilang); konflik: versi dasar tetap yang lama agar simpan ulang
            // tetap ditolak sampai Anda memuat versi terbaru.
            render_doc(
                &ctx,
                &v,
                &access,
                &doc,
                Some(Draft {
                    title: form.title,
                    body: form.body,
                    base_version: form.base_version,
                    error: Some(code),
                }),
                None,
                None,
                status,
            )
            .await
        }
        Err(e) => fail_to(session, &base_of(&access), e),
    }
}

#[debug_handler]
async fn fresh(
    State(ctx): State<AppContext>,
    access: ProjectAccess,
    session: Session,
    RequestId(request_id): RequestId,
    Path((_, _, slug)): Path<(String, String, String)>,
    Form(form): Form<ScanForm>,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    let doc = find_doc(&ctx, &access, &slug).await?;
    let to = safe_next(&form.next, &base_of(&access));
    match docs::mark_fresh(&ctx.db, &acting(&access, request_id), doc.id).await {
        Ok(_) => ok_to(session, &to, "kb-flash-fresh"),
        Err(e) => fail_to(session, &to, e),
    }
}

#[debug_handler]
async fn archive(
    State(ctx): State<AppContext>,
    access: ProjectAccess,
    session: Session,
    RequestId(request_id): RequestId,
    Path((_, _, slug)): Path<(String, String, String)>,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    let doc = find_doc(&ctx, &access, &slug).await?;
    let base = base_of(&access);
    match docs::archive(&ctx.db, &acting(&access, request_id), doc.id).await {
        Ok(_) => ok_to(session, &base, "kb-flash-archived"),
        Err(e) => fail_to(session, &base, e),
    }
}

/// Unduh Markdown dokumen (`<slug>.md`).
#[debug_handler]
async fn raw(
    State(ctx): State<AppContext>,
    access: ProjectAccess,
    Path((_, _, slug)): Path<(String, String, String)>,
) -> Result<Response> {
    access.require(Permission::ViewProject)?;
    let doc = find_doc(&ctx, &access, &slug).await?;
    Ok((
        [
            (
                header::CONTENT_TYPE,
                "text/markdown; charset=utf-8".to_string(),
            ),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}.md\"", doc.slug),
            ),
        ],
        doc.body,
    )
        .into_response())
}

#[derive(Debug, Default, Deserialize)]
struct VersionQuery {
    #[serde(default)]
    v: Option<i32>,
}

#[debug_handler]
async fn versions(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: ProjectAccess,
    session: Session,
    Path((_, _, slug)): Path<(String, String, String)>,
    Query(vq): Query<VersionQuery>,
) -> Result<Response> {
    access.require(Permission::ViewProject)?;
    let (session, flash) = flash::take(session);
    let wid = access.tenant.workspace_id();
    let doc = find_doc(&ctx, &access, &slug).await?;
    let base = base_of(&access);
    let href = format!("{base}/{}/versions", doc.slug);
    let list = knowledge_versions::Model::of_doc(&ctx.db, wid, doc.id).await?;
    let people = users_map(&ctx.db, list.iter().filter_map(|x| x.author_id).collect()).await?;
    let selected =
        vq.v.and_then(|n| list.iter().find(|x| x.number == n))
            .or_else(|| list.iter().find(|x| x.number == doc.version - 1))
            .or_else(|| list.first());
    let diff = selected.map(|s| {
        serde_json::json!({
            "number": s.number,
            "current": s.number == doc.version,
            "sbs": proposals::side_by_side(&s.body, &doc.body),
            "added": proposals::stat(&s.body, &doc.body).0,
            "removed": proposals::stat(&s.body, &doc.body).1,
            "restore_action": format!("{href}/{}/restore", s.number),
        })
    });
    let res = render(
        &ctx,
        &v,
        &access,
        "knowledge/versions.html",
        &href,
        StatusCode::OK,
        serde_json::json!({
            "tab": "home",
            "doc": DocRow::of(&doc, &base, None, &people),
            "versions": list.iter().map(|x| VersionRow::of(x, &href, doc.version, &people)).collect::<Vec<_>>(),
            "diff": diff,
            "flash": flash,
        }),
    )
    .await?;
    Ok((session, res).into_response())
}

#[debug_handler]
async fn restore(
    State(ctx): State<AppContext>,
    access: ProjectAccess,
    session: Session,
    RequestId(request_id): RequestId,
    Path((_, _, slug, number)): Path<(String, String, String, i32)>,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    let doc = find_doc(&ctx, &access, &slug).await?;
    let to = format!("{}/{}", base_of(&access), doc.slug);
    match docs::restore(&ctx.db, &acting(&access, request_id), doc.id, number).await {
        Ok(_) => ok_to(session, &to, "kb-flash-restored"),
        Err(e) => fail_to(session, &format!("{to}/versions"), e),
    }
}

pub fn routes() -> Routes {
    Routes::new()
        .add("/w/{slug}/p/{key}/knowledge", get(home))
        .add("/w/{slug}/p/{key}/knowledge/g/{group}", get(group))
        .add("/w/{slug}/p/{key}/knowledge/proposals", get(review))
        .add(
            "/w/{slug}/p/{key}/knowledge/proposals/{id}/approve",
            post(approve),
        )
        .add(
            "/w/{slug}/p/{key}/knowledge/proposals/{id}/reject",
            post(reject),
        )
        .add(
            "/w/{slug}/p/{key}/knowledge/proposals/{id}/undo",
            post(undo),
        )
        .add("/w/{slug}/p/{key}/knowledge/scan", post(scan))
        .add("/w/{slug}/p/{key}/knowledge/new", post(create))
        .add("/w/{slug}/p/{key}/knowledge/owner", post(set_owner))
        .add(
            "/w/{slug}/p/{key}/knowledge/{doc}",
            get(doc_page).post(save),
        )
        .add("/w/{slug}/p/{key}/knowledge/{doc}/fresh", post(fresh))
        .add("/w/{slug}/p/{key}/knowledge/{doc}/archive", post(archive))
        .add("/w/{slug}/p/{key}/knowledge/{doc}/raw", get(raw))
        .add("/w/{slug}/p/{key}/knowledge/{doc}/versions", get(versions))
        .add(
            "/w/{slug}/p/{key}/knowledge/{doc}/versions/{n}/restore",
            post(restore),
        )
}
