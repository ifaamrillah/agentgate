//! Tool knowledge (M8): `ag_knowledge` (list, read, propose) dan bagian knowledge di `ag_search`
//! serta `ag_context`. Membaca butuh `ViewProject`; `propose` butuh `EditProject` dan hanya
//! membuat usulan (tidak pernah mengubah dokumen langsung).
use sea_orm::DatabaseConnection;
use serde_json::{Value, json};

use super::{Args, Session, ToolError, required};
use crate::{
    common::markdown,
    models::{
        knowledge_catalog, knowledge_docs,
        knowledge_proposals::ProposalTrigger,
        knowledge_runs::{self, RunKind, RunTrigger},
        permission::Permission,
        personas, projects,
    },
    services::{
        knowledge::{
            self, Failure,
            context::{self, Brief},
            docs, proposals,
            search::{self, Hit},
        },
        tickets::access_in,
    },
};

/// Project tool (sama dengan tool graph).
async fn project_of(
    db: &DatabaseConnection,
    s: &Session,
    a: &Args,
) -> Result<projects::Model, ToolError> {
    super::graph::project_of(db, s, a).await
}

fn failure(e: Failure) -> ToolError {
    match e {
        Failure::NotFound => ToolError::new("Tidak ditemukan atau Anda tidak punya akses."),
        Failure::Invalid(errors) => {
            let text = serde_json::to_string(&errors).unwrap_or_default();
            let msg = if text.contains(knowledge::UNCHANGED) {
                "Isi usulan sama dengan dokumen sekarang; tidak ada yang diusulkan."
            } else if text.contains(knowledge::TYPE_INVALID) {
                "`type` tidak dikenal. Pakai kunci jenis dari `ag_knowledge list` (types)."
            } else if text.contains(knowledge::TITLE_INVALID) {
                "`title` wajib diisi untuk dokumen baru (maks. 160 karakter)."
            } else if text.contains(knowledge::BODY_TOO_LONG) {
                "`body` terlalu panjang (maks. 200.000 karakter)."
            } else {
                "Usulan tidak valid."
            };
            ToolError::new(msg)
        }
        Failure::Db(e) => e.into(),
    }
}

async fn persona_of(
    db: &DatabaseConnection,
    s: &Session,
    key: &str,
) -> Result<Option<personas::Model>, ToolError> {
    if key.trim().is_empty() {
        return Ok(None);
    }
    Ok(personas::Model::resolve(db, s.workspace.workspace_id(), key.trim()).await?)
}

fn brief_json(b: &Brief) -> Value {
    json!({
        "slug": b.slug,
        "title": b.title,
        "type": b.type_key,
        "group": b.group,
        "version": b.version,
        "workspace": b.workspace,
        "stale": if b.stale.is_empty() { Value::Null } else { Value::from(b.stale.clone()) },
    })
}

/// `ag_knowledge`.
pub(super) async fn call(
    db: &DatabaseConnection,
    s: &Session,
    a: &Args,
) -> Result<Value, ToolError> {
    match a.action.as_str() {
        "list" => list(db, s, a).await,
        "read" => read(db, s, a).await,
        "propose" => propose(db, s, a).await,
        _ => Err(ToolError::new("`action` harus list, read, atau propose.")),
    }
}

async fn list(db: &DatabaseConnection, s: &Session, a: &Args) -> Result<Value, ToolError> {
    let project = project_of(db, s, a).await?;
    let wid = s.workspace.workspace_id();
    let briefs = context::briefs(db, wid, project.id, a.persona.trim()).await?;
    let types = knowledge::catalog::available_types(db, wid, project.id).await?;
    let relevant = knowledge::catalog::relevant_groups(db, wid, project.id).await?;
    Ok(json!({
        "project": project.key,
        "docs": briefs.iter().map(brief_json).collect::<Vec<_>>(),
        "groups": knowledge_catalog::GROUPS
            .iter()
            .filter(|g| relevant.contains(&g.key))
            .map(|g| json!({ "key": g.key, "name": g.name }))
            .collect::<Vec<_>>(),
        "types": types
            .iter()
            .filter(|t| relevant.contains(&t.group.as_str()) || t.custom)
            .map(|t| json!({ "key": t.key, "name": t.name, "group": t.group, "hint": t.hint }))
            .collect::<Vec<_>>(),
        "hint": "Baca isi dengan action=read + slug. Usulkan perubahan/dokumen baru dengan action=propose.",
    }))
}

async fn find_doc(
    db: &DatabaseConnection,
    s: &Session,
    project: &projects::Model,
    slug: &str,
) -> Result<knowledge_docs::Model, ToolError> {
    let wid = s.workspace.workspace_id();
    let not_found = || {
        ToolError::new(format!(
            "Dokumen {slug} tidak ada di project {}.",
            project.key
        ))
    };
    if let Some(d) = knowledge_docs::Model::by_slug(db, wid, Some(project.id), slug)
        .await?
        .filter(|d| d.archived_at.is_none())
    {
        return Ok(d);
    }
    knowledge_docs::Model::by_slug(db, wid, None, slug)
        .await?
        .filter(|d| {
            d.archived_at.is_none()
                && (d.applies_to.is_empty() || d.applies_to.contains(&project.id))
        })
        .ok_or_else(not_found)
}

async fn read(db: &DatabaseConnection, s: &Session, a: &Args) -> Result<Value, ToolError> {
    let project = project_of(db, s, a).await?;
    let slug = required(&a.slug, "slug")?;
    let doc = find_doc(db, s, &project, slug).await?;
    if let Some(p) = persona_of(db, s, &a.persona).await?
        && !docs::persona_reads(&p, &doc)
    {
        return Err(ToolError::new(format!(
            "Persona {} tidak diizinkan membaca dokumen {slug}.",
            p.name
        )));
    }
    let wid = s.workspace.workspace_id();
    let sources: Vec<Value> = docs::sources_of(db, wid, doc.id)
        .await?
        .into_iter()
        .map(|src| json!({ "repo_id": src.repo_id, "path": src.path }))
        .collect();
    Ok(json!({
        "slug": doc.slug,
        "title": doc.title,
        "type": doc.type_key,
        "group": doc.group_key,
        "version": doc.version,
        "workspace": doc.is_workspace_doc(),
        "stale": if doc.stale_reason.is_empty() { Value::Null } else { Value::from(doc.stale_reason.clone()) },
        "outline": markdown::outline(&doc.body).iter().map(|h| json!({ "level": h.level, "text": h.text })).collect::<Vec<_>>(),
        "sources": sources,
        "body": doc.body,
    }))
}

fn trigger_of(run: &knowledge_runs::Model) -> ProposalTrigger {
    if run.kind() == RunKind::Scan {
        return ProposalTrigger::Scan;
    }
    match run.trigger() {
        RunTrigger::Done => ProposalTrigger::Done,
        RunTrigger::Merge => ProposalTrigger::Merge,
        RunTrigger::Audit => ProposalTrigger::Audit,
        RunTrigger::Init => ProposalTrigger::Init,
        RunTrigger::Manual => ProposalTrigger::Manual,
    }
}

async fn propose(db: &DatabaseConnection, s: &Session, a: &Args) -> Result<Value, ToolError> {
    let project = project_of(db, s, a).await?;
    let acting = s.acting();
    access_in(db, &acting, project.id, Permission::EditProject)
        .await
        .map_err(|_| ToolError::new("Anda tidak boleh mengusulkan knowledge di project ini."))?;
    let body = required(&a.body, "body")?;
    if a.slug.trim().is_empty() && a.title.trim().is_empty() {
        return Err(ToolError::new(
            "Isi `slug` (dokumen yang ada) atau `title` (dokumen baru).",
        ));
    }
    let wid = s.workspace.workspace_id();
    let run = match a.run.trim() {
        "" => None,
        id => {
            let id = uuid::Uuid::parse_str(id)
                .map_err(|_| ToolError::new("`run` bukan id run yang valid."))?;
            knowledge_runs::Model::find_in(db, wid, id)
                .await?
                .filter(|r| r.project_id == project.id)
        }
    };
    let mut new = proposals::NewProposal::new(
        project.id,
        run.as_ref().map_or(ProposalTrigger::Manual, trigger_of),
    );
    new.slug = a.slug.trim().to_string();
    new.title = a.title.trim().to_string();
    new.type_key = a.type_key.trim().to_string();
    new.body = body.to_string();
    new.reason = a.reason.clone();
    new.ticket_id = run.as_ref().and_then(|r| r.ticket_id);
    new.trigger_ref = run
        .as_ref()
        .map(|r| r.trigger_ref.clone())
        .unwrap_or_default();
    new.run_id = run.as_ref().map(|r| r.id);
    new.persona = if a.persona.trim().is_empty() {
        knowledge::runs::PERSONA.to_string()
    } else {
        a.persona.trim().to_string()
    };
    new.sources = a.sources.clone();
    new.created_by = Some(s.user.id);
    let p = proposals::propose(db, wid, new).await.map_err(failure)?;
    Ok(json!({
        "proposal": p.id,
        "slug": p.slug,
        "title": p.title,
        "new_doc": p.doc_id.is_none(),
        "status": "pending",
        "hint": "Usulan menunggu persetujuan pemilik knowledge; dokumen belum berubah.",
    }))
}

/// Hasil knowledge untuk `ag_search`.
pub(super) async fn search_section(
    db: &DatabaseConnection,
    s: &Session,
    project: &projects::Model,
    q: &str,
) -> Result<Vec<Value>, ToolError> {
    let hits = search::search(db, s.workspace.workspace_id(), Some(project.id), q, 5).await?;
    Ok(hits
        .iter()
        .map(|Hit { doc, snippet }| {
            json!({ "slug": doc.slug, "title": doc.title, "type": doc.type_key, "snippet": snippet })
        })
        .collect())
}

/// Bagian `knowledge` di `ag_context` tiket: dokumen yang boleh dibaca persona stage.
pub(super) async fn context_section(
    db: &DatabaseConnection,
    s: &Session,
    project: &projects::Model,
    persona: &str,
) -> Result<Value, ToolError> {
    let briefs = context::briefs(db, s.workspace.workspace_id(), project.id, persona).await?;
    Ok(json!({
        "docs": briefs.iter().map(brief_json).collect::<Vec<_>>(),
        "hint": if briefs.is_empty() {
            "Project belum punya dokumen knowledge."
        } else {
            "Baca dokumen yang relevan dengan ag_knowledge action=read sebelum bekerja."
        },
    }))
}
