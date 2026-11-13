//! Isi ZIP ekspor. Struktur (semua jalur di bawah `agentgate-export/`):
//!
//! ```text
//! manifest.json                                   workspace, isi, project, waktu
//! projects/<KEY>/tickets.json                     project + tiket (komentar, riwayat, relasi,
//!                                                 alias, field custom bernama)
//! projects/<KEY>/artifacts/<KODE>/<nama>/v<n>.md   versi artifact (.html untuk mockup)
//! projects/<KEY>/.agentgate/knowledge/<slug>.md   format sama dengan `agentgate knowledge pull`
//! projects/<KEY>/attachments/<KODE>/<id>-<berkas> lampiran tiket
//! projects/<KEY>/evidence/<KODE>/<id>-<nama>      evidence job
//! workspace/.agentgate/knowledge/<slug>.md        knowledge tingkat workspace
//! config/workflows/<key>.yaml                     YAML impor ulang (sama dengan tombol Ekspor)
//! config/personas/<key>.yaml, config/skill-packs/<key>/pack.yaml + files/<path>
//! audit/audit-log.jsonl                           12 bulan terakhir, satu kejadian per baris
//! ```
//!
//! Semua query difilter workspace ekspor dan daftar project cakupan, jadi data workspace atau
//! project lain tidak pernah ikut.
use std::{
    collections::{HashMap, HashSet},
    io::{Cursor, Write},
    path::Path,
};

use chrono::{Duration, Utc};
use loco_rs::app::AppContext;
use sea_orm::{ColumnTrait, DbErr, EntityTrait, QueryFilter, QueryOrder};
use serde::Serialize;
use serde_json::json;
use uuid::Uuid;
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

use crate::models::{
    artifact_versions, audit_events, custom_fields,
    data_exports::{self, ExportPart},
    job_evidence, knowledge_docs, persona_skills, personas, projects, skill_files, skill_packs,
    sprints, ticket_artifacts, ticket_attachments, ticket_codes, ticket_comments, ticket_events,
    ticket_links, ticket_types, tickets, users, workflows,
    workspaces::{self, WorkspaceId},
};

const ROOT: &str = "agentgate-export";
/// Versi format arsip (naik bila struktur berubah tidak kompatibel).
pub const FORMAT_VERSION: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error(transparent)]
    Db(#[from] DbErr),
    #[error("zip: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("storage: {0}")]
    Storage(String),
}

/// Nama aman untuk satu segmen jalur di ZIP (tanpa `/`, `\`, `..`, kontrol).
#[must_use]
pub fn safe_segment(raw: &str) -> String {
    let s: String = raw
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | ' ') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let s = s.trim().trim_matches('.').to_string();
    if s.is_empty() {
        "_".into()
    } else {
        s.chars().take(120).collect()
    }
}

struct Zip {
    w: ZipWriter<Cursor<Vec<u8>>>,
}

impl Zip {
    fn new() -> Self {
        Self {
            w: ZipWriter::new(Cursor::new(Vec::new())),
        }
    }

    fn add(&mut self, path: &str, data: &[u8]) -> Result<(), BuildError> {
        let opts = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .large_file(data.len() > u32::MAX as usize / 2);
        self.w.start_file(format!("{ROOT}/{path}"), opts)?;
        self.w.write_all(data)?;
        Ok(())
    }

    fn json<T: Serialize>(&mut self, path: &str, value: &T) -> Result<(), BuildError> {
        let mut data = serde_json::to_vec_pretty(value)?;
        data.push(b'\n');
        self.add(path, &data)
    }

    fn finish(self) -> Result<Vec<u8>, BuildError> {
        Ok(self.w.finish()?.into_inner())
    }
}

/// Email per id user (dimuat sekali untuk semua id yang dirujuk).
struct People(HashMap<Uuid, String>);

impl People {
    async fn load(ctx: &AppContext, ids: HashSet<Uuid>) -> Result<Self, DbErr> {
        let mut map = HashMap::new();
        let ids: Vec<Uuid> = ids.into_iter().collect();
        for chunk in ids.chunks(500) {
            for u in users::Entity::find()
                .filter(users::Column::Id.is_in(chunk.to_vec()))
                .all(&ctx.db)
                .await?
            {
                map.insert(u.id, u.email);
            }
        }
        Ok(Self(map))
    }

    fn email(&self, id: Option<Uuid>) -> Option<String> {
        id.and_then(|i| self.0.get(&i).cloned())
    }
}

/// Kemajuan worker: `done` dari `total` langkah.
struct Progress<'a> {
    ctx: &'a AppContext,
    id: Uuid,
    total: usize,
    done: usize,
}

impl Progress<'_> {
    async fn step(&mut self, phase: ExportPart) -> Result<(), DbErr> {
        self.done += 1;
        let pct = i32::try_from(self.done * 100 / self.total.max(1)).unwrap_or(99);
        data_exports::ActiveModel::progress(&self.ctx.db, self.id, phase, pct).await
    }
}

/// Isi knowledge seperti `agentgate knowledge pull` (front matter + body).
fn knowledge_md(d: &knowledge_docs::Model) -> String {
    format!(
        "---\ntitle: {}\ntype: {}\ngroup: {}\nversion: {}\n---\n\n{}\n",
        d.title.replace('\n', " "),
        d.type_key,
        d.group_key,
        d.version,
        d.body.trim_end()
    )
}

/// Susun ZIP ekspor `row` milik `workspace`.
///
/// # Errors
///
/// Error database, storage (lampiran), atau penulisan ZIP.
#[allow(clippy::too_many_lines)]
pub async fn build(
    ctx: &AppContext,
    workspace: &workspaces::Model,
    row: &data_exports::Model,
) -> Result<Vec<u8>, BuildError> {
    let db = &ctx.db;
    let wid = workspace.workspace_id();
    let parts = row.parts();
    let mut project_rows = projects::Entity::find()
        .filter(projects::Column::WorkspaceId.eq(wid.get()))
        .order_by_asc(projects::Column::Key)
        .all(db)
        .await?;
    if !row.project_ids.is_empty() {
        project_rows.retain(|p| row.project_ids.contains(&p.id));
    }
    let per_project = [
        ExportPart::Tickets,
        ExportPart::Artifacts,
        ExportPart::Knowledge,
        ExportPart::Attachments,
    ];
    let total = parts
        .iter()
        .map(|p| {
            if per_project.contains(p) {
                project_rows.len().max(1)
            } else {
                1
            }
        })
        .sum();
    let mut progress = Progress {
        ctx,
        id: row.id,
        total,
        done: 0,
    };
    let mut zip = Zip::new();
    zip.json(
        "manifest.json",
        &json!({
            "format": "agentgate-export",
            "format_version": FORMAT_VERSION,
            "workspace": { "name": workspace.name, "slug": workspace.slug },
            "created_at": Utc::now().to_rfc3339(),
            "parts": parts.iter().map(|p| p.key()).collect::<Vec<_>>(),
            "projects": project_rows.iter().map(|p| json!({ "key": p.key, "name": p.name })).collect::<Vec<_>>(),
        }),
    )?;
    let types = ticket_types::Model::all_in(db, wid).await?;
    let fields = custom_fields::Model::all_in(db, wid).await?;
    for part in &parts {
        match part {
            ExportPart::Tickets => {
                for p in &project_rows {
                    tickets_json(ctx, wid, p, &types, &fields, &mut zip).await?;
                    progress.step(*part).await?;
                }
            }
            ExportPart::Artifacts => {
                for p in &project_rows {
                    artifacts(ctx, wid, p, &mut zip).await?;
                    progress.step(*part).await?;
                }
            }
            ExportPart::Knowledge => {
                for p in &project_rows {
                    for d in knowledge_docs::Model::of_project(db, wid, p.id).await? {
                        zip.add(
                            &format!(
                                "projects/{}/.agentgate/knowledge/{}.md",
                                safe_segment(&p.key),
                                safe_segment(&d.slug)
                            ),
                            knowledge_md(&d).as_bytes(),
                        )?;
                    }
                    progress.step(*part).await?;
                }
                if row.project_ids.is_empty() {
                    for d in knowledge_docs::Entity::find()
                        .filter(knowledge_docs::Column::WorkspaceId.eq(wid.get()))
                        .filter(knowledge_docs::Column::ProjectId.is_null())
                        .filter(knowledge_docs::Column::ArchivedAt.is_null())
                        .all(db)
                        .await?
                    {
                        zip.add(
                            &format!(
                                "workspace/.agentgate/knowledge/{}.md",
                                safe_segment(&d.slug)
                            ),
                            knowledge_md(&d).as_bytes(),
                        )?;
                    }
                }
            }
            ExportPart::Attachments => {
                for p in &project_rows {
                    files(ctx, wid, p, &mut zip).await?;
                    progress.step(*part).await?;
                }
            }
            ExportPart::Config => {
                config(ctx, wid, &mut zip).await?;
                progress.step(*part).await?;
            }
            ExportPart::Audit => {
                audit_log(ctx, wid, &mut zip).await?;
                progress.step(*part).await?;
            }
        }
    }
    zip.finish()
}

/// Tiket aktif project (bukan di sampah).
async fn project_tickets(
    ctx: &AppContext,
    wid: WorkspaceId,
    project_id: Uuid,
) -> Result<Vec<tickets::Model>, DbErr> {
    tickets::Entity::find()
        .filter(tickets::Column::WorkspaceId.eq(wid.get()))
        .filter(tickets::Column::ProjectId.eq(project_id))
        .filter(tickets::active())
        .order_by_asc(tickets::Column::CreatedAt)
        .order_by_asc(tickets::Column::Id)
        .all(&ctx.db)
        .await
}

#[allow(clippy::too_many_lines)]
async fn tickets_json(
    ctx: &AppContext,
    wid: WorkspaceId,
    project: &projects::Model,
    types: &[ticket_types::Model],
    fields: &[custom_fields::Model],
    zip: &mut Zip,
) -> Result<(), BuildError> {
    let db = &ctx.db;
    let rows = project_tickets(ctx, wid, project.id).await?;
    let ids: Vec<Uuid> = rows.iter().map(|t| t.id).collect();
    let mut comments: HashMap<Uuid, Vec<ticket_comments::Model>> = HashMap::new();
    let mut events: HashMap<Uuid, Vec<ticket_events::Model>> = HashMap::new();
    let mut codes: HashMap<Uuid, Vec<String>> = HashMap::new();
    let mut links: Vec<ticket_links::Model> = Vec::new();
    for chunk in ids.chunks(500) {
        for c in ticket_comments::Entity::find()
            .filter(ticket_comments::Column::WorkspaceId.eq(wid.get()))
            .filter(ticket_comments::Column::TicketId.is_in(chunk.to_vec()))
            .order_by_asc(ticket_comments::Column::CreatedAt)
            .all(db)
            .await?
        {
            comments.entry(c.ticket_id).or_default().push(c);
        }
        for e in ticket_events::Entity::find()
            .filter(ticket_events::Column::WorkspaceId.eq(wid.get()))
            .filter(ticket_events::Column::TicketId.is_in(chunk.to_vec()))
            .order_by_asc(ticket_events::Column::CreatedAt)
            .all(db)
            .await?
        {
            events.entry(e.ticket_id).or_default().push(e);
        }
        for c in ticket_codes::Entity::find()
            .filter(ticket_codes::Column::WorkspaceId.eq(wid.get()))
            .filter(ticket_codes::Column::TicketId.is_in(chunk.to_vec()))
            .all(db)
            .await?
        {
            codes.entry(c.ticket_id).or_default().push(c.code);
        }
        links.extend(
            ticket_links::Entity::find()
                .filter(ticket_links::Column::WorkspaceId.eq(wid.get()))
                .filter(ticket_links::Column::FromId.is_in(chunk.to_vec()))
                .all(db)
                .await?,
        );
    }
    // Kode tiket tujuan relasi (bisa di project lain di workspace yang sama: kodenya saja).
    let targets: HashSet<Uuid> = links
        .iter()
        .map(|l| l.to_id)
        .chain(rows.iter().filter_map(|t| t.parent_id))
        .collect();
    let mut code_of: HashMap<Uuid, String> = rows.iter().map(|t| (t.id, t.code.clone())).collect();
    let missing: Vec<Uuid> = targets
        .iter()
        .filter(|id| !code_of.contains_key(id))
        .copied()
        .collect();
    for chunk in missing.chunks(500) {
        for t in tickets::Entity::find()
            .filter(tickets::Column::WorkspaceId.eq(wid.get()))
            .filter(tickets::Column::Id.is_in(chunk.to_vec()))
            .all(db)
            .await?
        {
            code_of.insert(t.id, t.code);
        }
    }
    let sprint_rows = sprints::Model::of_project(db, wid, project.id).await?;
    let mut people_ids: HashSet<Uuid> = HashSet::new();
    for t in &rows {
        people_ids.extend(
            [t.assignee_id, t.reporter_id, t.created_by]
                .into_iter()
                .flatten(),
        );
    }
    people_ids.extend(comments.values().flatten().filter_map(|c| c.author_id));
    people_ids.extend(events.values().flatten().filter_map(|e| e.actor_id));
    let people = People::load(ctx, people_ids).await?;
    let field_name: HashMap<Uuid, &str> = fields.iter().map(|f| (f.id, f.name.as_str())).collect();
    let tickets_out: Vec<serde_json::Value> = rows
        .iter()
        .map(|t| {
            let custom: serde_json::Map<String, serde_json::Value> =
                serde_json::from_value::<tickets::CustomValues>(t.custom_values.clone())
                    .unwrap_or_default()
                    .0
                    .into_iter()
                    .map(|(id, v)| {
                        (
                            field_name.get(&id).map_or_else(|| id.to_string(), |n| (*n).to_string()),
                            serde_json::Value::String(v),
                        )
                    })
                    .collect();
            let aliases: Vec<&String> = codes
                .get(&t.id)
                .map(|c| c.iter().filter(|c| !c.eq_ignore_ascii_case(&t.code)).collect())
                .unwrap_or_default();
            json!({
                "code": t.code,
                "aliases": aliases,
                "title": t.title,
                "description": t.description,
                "type": types.iter().find(|ty| ty.id == t.type_id).map(|ty| ty.key.clone()),
                "status": t.status,
                "stage": t.stage_key,
                "stage_state": t.stage_state,
                "priority": t.priority,
                "assignee": people.email(t.assignee_id),
                "reporter": people.email(t.reporter_id),
                "labels": t.labels,
                "story_points": t.story_points,
                "due_on": t.due_on,
                "sprint": t.sprint_id.and_then(|s| sprint_rows.iter().find(|x| x.id == s)).map(|s| s.name.clone()),
                "parent": t.parent_id.and_then(|p| code_of.get(&p).cloned()),
                "custom_fields": custom,
                "created_at": t.created_at,
                "updated_at": t.updated_at,
                "done_at": t.done_at,
                "comments": comments.get(&t.id).map(|cs| cs.iter().map(|c| json!({
                    "author": people.email(c.author_id),
                    "body": c.body,
                    "created_at": c.created_at,
                    "edited_at": c.edited_at,
                })).collect::<Vec<_>>()).unwrap_or_default(),
                "history": events.get(&t.id).map(|es| es.iter().map(|e| json!({
                    "actor": people.email(e.actor_id),
                    "kind": e.kind,
                    "changes": e.changes,
                    "created_at": e.created_at,
                })).collect::<Vec<_>>()).unwrap_or_default(),
                "links": links.iter().filter(|l| l.from_id == t.id).map(|l| json!({
                    "kind": l.kind,
                    "to": code_of.get(&l.to_id),
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    zip.json(
        &format!("projects/{}/tickets.json", safe_segment(&project.key)),
        &json!({
            "project": {
                "key": project.key,
                "name": project.name,
                "description": project.description,
                "archived_at": project.archived_at,
                "created_at": project.created_at,
            },
            "ticket_types": types.iter().map(|t| json!({ "key": t.key, "name": t.name })).collect::<Vec<_>>(),
            "custom_fields": fields.iter().map(|f| json!({ "name": f.name, "kind": f.kind, "options": f.options, "required": f.required })).collect::<Vec<_>>(),
            "sprints": sprint_rows.iter().map(|s| json!({ "name": s.name, "goal": s.goal, "starts_on": s.starts_on, "ends_on": s.ends_on, "state": s.state })).collect::<Vec<_>>(),
            "tickets": tickets_out,
        }),
    )?;
    Ok(())
}

async fn artifacts(
    ctx: &AppContext,
    wid: WorkspaceId,
    project: &projects::Model,
    zip: &mut Zip,
) -> Result<(), BuildError> {
    let db = &ctx.db;
    let rows = project_tickets(ctx, wid, project.id).await?;
    for chunk in rows.chunks(200) {
        let ids: Vec<Uuid> = chunk.iter().map(|t| t.id).collect();
        let arts = ticket_artifacts::Entity::find()
            .filter(ticket_artifacts::Column::WorkspaceId.eq(wid.get()))
            .filter(ticket_artifacts::Column::TicketId.is_in(ids))
            .all(db)
            .await?;
        if arts.is_empty() {
            continue;
        }
        let versions = artifact_versions::Entity::find()
            .filter(artifact_versions::Column::WorkspaceId.eq(wid.get()))
            .filter(artifact_versions::Column::ArtifactId.is_in(arts.iter().map(|a| a.id)))
            .order_by_asc(artifact_versions::Column::Number)
            .all(db)
            .await?;
        for a in &arts {
            let Some(t) = chunk.iter().find(|t| t.id == a.ticket_id) else {
                continue;
            };
            let html = crate::services::mockups::is_mockup(&a.name);
            let stem = a
                .name
                .trim_end_matches(".html")
                .trim_end_matches(".htm")
                .trim_end_matches(".md");
            for v in versions.iter().filter(|v| v.artifact_id == a.id) {
                zip.add(
                    &format!(
                        "projects/{}/artifacts/{}/{}/v{}.{}",
                        safe_segment(&project.key),
                        safe_segment(&t.code),
                        safe_segment(stem),
                        v.number,
                        if html { "html" } else { "md" }
                    ),
                    v.body.as_bytes(),
                )?;
            }
        }
    }
    Ok(())
}

async fn files(
    ctx: &AppContext,
    wid: WorkspaceId,
    project: &projects::Model,
    zip: &mut Zip,
) -> Result<(), BuildError> {
    let db = &ctx.db;
    let rows = project_tickets(ctx, wid, project.id).await?;
    let key = safe_segment(&project.key);
    for chunk in rows.chunks(200) {
        let ids: Vec<Uuid> = chunk.iter().map(|t| t.id).collect();
        let code = |id: Uuid| {
            chunk
                .iter()
                .find(|t| t.id == id)
                .map_or_else(String::new, |t| safe_segment(&t.code))
        };
        for a in ticket_attachments::Entity::find()
            .filter(ticket_attachments::Column::WorkspaceId.eq(wid.get()))
            .filter(ticket_attachments::Column::TicketId.is_in(ids.clone()))
            .all(db)
            .await?
        {
            let data: Vec<u8> = ctx
                .storage
                .download(Path::new(&a.storage_key))
                .await
                .map_err(|e| BuildError::Storage(e.to_string()))?;
            zip.add(
                &format!(
                    "projects/{key}/attachments/{}/{}-{}",
                    code(a.ticket_id),
                    a.id,
                    safe_segment(&a.filename)
                ),
                &data,
            )?;
        }
        for e in job_evidence::Entity::find()
            .filter(job_evidence::Column::WorkspaceId.eq(wid.get()))
            .filter(job_evidence::Column::TicketId.is_in(ids))
            .all(db)
            .await?
        {
            // Evidence lama yang berkasnya sudah tidak ada dilewati (metadatanya tetap di DB).
            let Ok(data) = ctx
                .storage
                .download::<Vec<u8>>(Path::new(&e.storage_key))
                .await
            else {
                tracing::warn!(evidence_id = %e.id, "evidence tidak ada di storage, dilewati");
                continue;
            };
            zip.add(
                &format!(
                    "projects/{key}/evidence/{}/{}-{}",
                    code(e.ticket_id),
                    e.id,
                    safe_segment(&e.name)
                ),
                &data,
            )?;
        }
    }
    Ok(())
}

#[derive(Serialize)]
struct PersonaDoc<'a> {
    key: &'a str,
    name: &'a str,
    summary: &'a str,
    color: &'a str,
    icon: &'a str,
    instructions: &'a str,
    concise: bool,
    mcp_tools: &'a [String],
    client_tools: &'a [String],
    knowledge_types: &'a [String],
    danger: &'a serde_json::Value,
    models: &'a serde_json::Value,
    skill_packs: Vec<String>,
}

#[derive(Serialize)]
struct PackDoc<'a> {
    key: &'a str,
    name: &'a str,
    description: &'a str,
    source: &'a str,
    repo_url: &'a Option<String>,
    repo_ref: &'a Option<String>,
    repo_path: &'a Option<String>,
    commit_sha: &'a Option<String>,
    version: &'a str,
    requested_tools: &'a [String],
    status: &'a str,
}

fn yaml<T: Serialize>(value: &T) -> Vec<u8> {
    serde_yaml_ng::to_string(value)
        .unwrap_or_default()
        .into_bytes()
}

async fn config(ctx: &AppContext, wid: WorkspaceId, zip: &mut Zip) -> Result<(), BuildError> {
    let db = &ctx.db;
    for w in workflows::Model::all_in(db, wid).await? {
        let text = crate::services::workflows::export(db, wid, &w, false).await?;
        zip.add(
            &format!("config/workflows/{}.yaml", safe_segment(&w.key)),
            text.as_bytes(),
        )?;
    }
    let packs = skill_packs::Model::all_in(db, wid).await?;
    let links = persona_skills::Entity::find()
        .filter(persona_skills::Column::WorkspaceId.eq(wid.get()))
        .all(db)
        .await?;
    for p in personas::Model::all_in(db, wid)
        .await?
        .into_iter()
        .filter(|p| p.deleted_at.is_none())
    {
        let doc = PersonaDoc {
            key: &p.key,
            name: &p.name,
            summary: &p.summary,
            color: &p.color,
            icon: &p.icon,
            instructions: &p.instructions,
            concise: p.concise,
            mcp_tools: &p.mcp_tools,
            client_tools: &p.client_tools,
            knowledge_types: &p.knowledge_types,
            danger: &p.danger,
            models: &p.models,
            skill_packs: links
                .iter()
                .filter(|l| l.persona_id == p.id)
                .filter_map(|l| {
                    packs
                        .iter()
                        .find(|k| k.id == l.pack_id)
                        .map(|k| k.key.clone())
                })
                .collect(),
        };
        zip.add(
            &format!("config/personas/{}.yaml", safe_segment(&p.key)),
            &yaml(&doc),
        )?;
    }
    for k in &packs {
        let dir = format!("config/skill-packs/{}", safe_segment(&k.key));
        let doc = PackDoc {
            key: &k.key,
            name: &k.name,
            description: &k.description,
            source: &k.source,
            repo_url: &k.repo_url,
            repo_ref: &k.repo_ref,
            repo_path: &k.repo_path,
            commit_sha: &k.commit_sha,
            version: &k.version,
            requested_tools: &k.requested_tools,
            status: &k.status,
        };
        zip.add(&format!("{dir}/pack.yaml"), &yaml(&doc))?;
        for f in skill_files::Entity::find()
            .filter(skill_files::Column::WorkspaceId.eq(wid.get()))
            .filter(skill_files::Column::PackId.eq(k.id))
            .all(db)
            .await?
        {
            let path: Vec<String> = f
                .path
                .split('/')
                .filter(|s| !s.is_empty())
                .map(safe_segment)
                .collect();
            zip.add(
                &format!("{dir}/files/{}", path.join("/")),
                f.body.as_bytes(),
            )?;
        }
    }
    Ok(())
}

async fn audit_log(ctx: &AppContext, wid: WorkspaceId, zip: &mut Zip) -> Result<(), BuildError> {
    let since = Utc::now() - Duration::days(365);
    let rows = audit_events::Entity::find()
        .filter(audit_events::Column::WorkspaceId.eq(wid.get()))
        .filter(audit_events::Column::CreatedAt.gte(since))
        .order_by_asc(audit_events::Column::CreatedAt)
        .order_by_asc(audit_events::Column::Id)
        .all(&ctx.db)
        .await?;
    let people = People::load(ctx, rows.iter().filter_map(|r| r.actor_id).collect()).await?;
    let mut out = Vec::new();
    for r in &rows {
        serde_json::to_writer(
            &mut out,
            &json!({
                "id": r.id,
                "at": r.created_at,
                "actor": people.email(r.actor_id),
                "action": r.action,
                "subject_type": r.subject_type,
                "subject_id": r.subject_id,
                "changes": r.changes,
                "request_id": r.request_id,
            }),
        )?;
        out.push(b'\n');
    }
    zip.add("audit/audit-log.jsonl", &out)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_segment_blocks_traversal() {
        assert_eq!(safe_segment("../etc/passwd"), "_etc_passwd");
        assert_eq!(safe_segment("prd.md"), "prd.md");
        assert_eq!(safe_segment(".."), "_");
    }
}
