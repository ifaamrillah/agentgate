//! Tool MCP Agent Gate (`agentgate_core::persona::MCP_TOOLS`). Respons ringkas berlapis:
//! ringkasan dulu, detail (isi artifact, skill) diambil saat diminta. Setiap tool memakai
//! service domain dengan pelaku = pemegang token, jadi izin sama dengan di UI: membaca butuh
//! `ViewProject`, menulis `EditProject` (Viewer ditolak), tiket workspace lain tidak ada.
use sea_orm::{ConnectionTrait, DatabaseConnection};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::models::token_savings::SavingFeature;
use super::{Session, ToolError};

#[path = "graph_tools.rs"]
mod graph;
#[path = "knowledge_tools.rs"]
mod knowledge;
#[path = "planning_tools.rs"]
mod planning;
use crate::{
    models::{
        artifact_versions,
        permission::Permission,
        persona_skills, personas, projects, skill_files,
        skill_packs::{self, SkillStatus},
        ticket_artifacts, ticket_comments, ticket_types,
        tickets::{self, TicketInput, TicketQuery},
        users,
    },
    services::{
        drafts::{self, DraftInput},
        tickets::{
            access_in, artifacts, comments,
            create::{self, TicketForm},
            stages::{self, Flow, Target},
        },
    },
};

/// Definisi satu tool untuk `tools/list`.
#[derive(Debug, Clone)]
pub struct ToolDef {
    pub name: &'static str,
    pub description: &'static str,
    pub schema: Value,
    pub read_only: bool,
}

fn schema(properties: Value, required: &[&str]) -> Value {
    json!({ "type": "object", "properties": properties, "required": required })
}

/// Semua tool (urutan = `MCP_TOOLS`).
#[must_use]
pub fn definitions() -> Vec<ToolDef> {
    let ticket = json!({ "type": "string", "description": "Kode tiket, mis. KSR-31" });
    let project = json!({ "type": "string", "description": "Kode project, mis. KSR (boleh kosong bila Anda hanya punya satu project)" });
    let symbol = json!({ "type": "string", "description": "Id simbol dari ag_search, atau nama (mis. StockService.reserve)" });
    vec![
        ToolDef {
            name: "ag_context",
            description: "Konteks kerja Agent Gate. Tanpa `ticket`: workspace, role Anda, project, dan persona. Dengan `ticket`: ringkasan tiket, stage sekarang (instruksi, persona, input/output, gate), artifact, dan skill persona. Panggil ini dulu sebelum mengerjakan tiket. `include: [\"skills\"]` memuat isi SKILL.md.",
            schema: schema(
                json!({
                    "ticket": ticket,
                    "include": { "type": "array", "items": { "type": "string", "enum": ["skills"] } },
                }),
                &[],
            ),
            read_only: true,
        },
        ToolDef {
            name: "ag_ticket",
            description: "Tiket. action=get: detail satu tiket + komentar terbaru. action=search: cari tiket (judul/kode/deskripsi) di project yang bisa Anda buka. action=create: buat tiket baru di project (butuh izin edit).",
            schema: schema(
                json!({
                    "action": { "type": "string", "enum": ["get", "search", "create"] },
                    "ticket": ticket,
                    "query": { "type": "string" },
                    "project": { "type": "string", "description": "Kunci project, mis. KSR" },
                    "title": { "type": "string" },
                    "description": { "type": "string", "description": "Markdown" },
                    "type": { "type": "string", "description": "Kunci tipe tiket: task, bug, story, epic, subtask" },
                    "parent": { "type": "string", "description": "Kode tiket induk" },
                    "priority": { "type": "string", "enum": ["low", "medium", "high", "urgent"] },
                }),
                &["action"],
            ),
            read_only: false,
        },
        ToolDef {
            name: "ag_draft_tickets",
            description: "Simpan draft tiket hasil brainstorming untuk direview manusia di halaman Draft tiket project. Draft BUKAN tiket: manusia memilih mana yang dibuat. Pakai `ref` dan `parent` untuk hierarki epic → story → subtask. Sertakan `story_points` dan `estimate` (jam AI + manusia per stage) bila bisa.",
            schema: schema(
                json!({
                    "project": { "type": "string", "description": "Kunci project" },
                    "source_ticket": { "type": "string", "description": "Kode tiket asal brainstorming (opsional)" },
                    "drafts": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "ref": { "type": "string" },
                                "parent": { "type": "string", "description": "`ref` draft induk" },
                                "type": { "type": "string" },
                                "title": { "type": "string" },
                                "description": { "type": "string" },
                                "story_points": { "type": "integer" },
                                "estimate": {
                                    "type": "object",
                                    "description": "Estimasi AI: jam AI dan manusia per kunci stage workflow + dasar estimasi",
                                    "properties": {
                                        "stages": { "type": "array", "items": { "type": "object", "properties": { "stage": { "type": "string" }, "ai_hours": { "type": "number" }, "human_hours": { "type": "number" } }, "required": ["stage"] } },
                                        "basis": { "type": "string" },
                                        "persona": { "type": "string" },
                                    },
                                },
                            },
                            "required": ["title"],
                        },
                    },
                }),
                &["project", "drafts"],
            ),
            read_only: false,
        },
        ToolDef {
            name: "ag_artifact",
            description: "Artifact stage tiket (mis. prd.md). action=get tanpa `name`: daftar artifact; dengan `name`: isi versi terbaru. action=save: simpan versi baru (dibuat bila belum ada).",
            schema: schema(
                json!({
                    "action": { "type": "string", "enum": ["get", "save"] },
                    "ticket": ticket,
                    "name": { "type": "string" },
                    "body": { "type": "string", "description": "Isi Markdown lengkap" },
                }),
                &["action", "ticket"],
            ),
            read_only: false,
        },
        ToolDef {
            name: "ag_transition",
            description: "Pindahkan tiket ke stage lain (kunci stage). Server memeriksa gate dan aturan workflow; bila ditolak, alasannya dikembalikan.",
            schema: schema(
                json!({ "ticket": ticket, "to": { "type": "string", "description": "Kunci stage tujuan" } }),
                &["ticket", "to"],
            ),
            read_only: false,
        },
        ToolDef {
            name: "ag_ask",
            description: "Tanya executor atau approver stage sekarang. Pertanyaan ditulis sebagai komentar dan mereka mendapat notifikasi. Jawaban dibaca lewat ag_ticket get.",
            schema: schema(
                json!({ "ticket": ticket, "question": { "type": "string" } }),
                &["ticket", "question"],
            ),
            read_only: false,
        },
        ToolDef {
            name: "ag_comment",
            description: "Tulis komentar Markdown di tiket atas nama Anda.",
            schema: schema(
                json!({ "ticket": ticket, "body": { "type": "string" } }),
                &["ticket", "body"],
            ),
            read_only: false,
        },
        ToolDef {
            name: "ag_search",
            description: "Cari simbol kode (fungsi, method, kelas, route) di code graph project. Cocok dengan pecahan nama (\"reserve stock\" → reserveStock) dan doc comment. Hasil: id, nama, file, baris, signature. Pakai sebelum grep.",
            schema: schema(
                json!({
                    "project": project,
                    "query": { "type": "string" },
                    "kind": { "type": "string", "enum": ["function", "method", "class", "interface", "struct", "enum", "trait", "type", "module", "route"] },
                    "repo": { "type": "string", "description": "Nama repo (opsional)" },
                    "include_tests": { "type": "boolean" },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 50 },
                }),
                &["query"],
            ),
            read_only: true,
        },
        ToolDef {
            name: "ag_trace",
            description: "Telusuri pemanggil (`callers`) atau yang dipanggil (`callees`) sebuah simbol sampai kedalaman 1–5, termasuk lintas repo lewat HTTP/event. Setiap langkah bertanda certain (hasil parsing/SCIP) atau inferred (dugaan).",
            schema: schema(
                json!({
                    "project": project,
                    "symbol": symbol,
                    "direction": { "type": "string", "enum": ["callers", "callees"] },
                    "depth": { "type": "integer", "minimum": 1, "maximum": 5 },
                }),
                &["symbol"],
            ),
            read_only: true,
        },
        ToolDef {
            name: "ag_snippet",
            description: "Kode satu simbol (maks. 200 baris) tanpa membaca file utuh. `context` menambah baris di sekitarnya.",
            schema: schema(
                json!({
                    "project": project,
                    "symbol": symbol,
                    "context": { "type": "integer", "minimum": 0, "maximum": 20 },
                }),
                &["symbol"],
            ),
            read_only: true,
        },
        ToolDef {
            name: "ag_impact",
            description: "Analisis dampak dari sebuah simbol atau dari unified diff (`git diff`): fungsi yang berubah, pemanggil terdampak, tes terkait, repo lain yang ikut terdampak, dan risiko (rendah/sedang/tinggi).",
            schema: schema(
                json!({
                    "project": project,
                    "symbol": symbol,
                    "diff": { "type": "string", "description": "Output git diff" },
                    "depth": { "type": "integer", "minimum": 1, "maximum": 5 },
                }),
                &[],
            ),
            read_only: true,
        },
        ToolDef {
            name: "ag_graph",
            description: "Gambaran code graph project. action=overview: status index per repo, bahasa, modul + node pusat, hotspot git. action=dead: kandidat kode mati. action=links: relasi lintas repo (HTTP, event, package). action=query: query ala Cypher, mis. `MATCH (a:Function {name: 'calcPromo'})<-[:CALLS*1..2]-(b) RETURN b.name, b.file LIMIT 20`.",
            schema: schema(
                json!({
                    "project": project,
                    "action": { "type": "string", "enum": ["overview", "dead", "links", "query"] },
                    "query": { "type": "string" },
                    "repo": { "type": "string" },
                }),
                &["action"],
            ),
            read_only: true,
        },
        ToolDef {
            name: "ag_knowledge",
            description: "Knowledge base project (dokumen arsitektur, aturan bisnis, API, konvensi, dll.). action=list: dokumen yang boleh Anda baca + jenis dokumen. action=read: isi satu dokumen (`slug`). action=propose: usulkan isi baru dokumen (`slug` yang ada) atau dokumen baru (`title` + `type`); usulan menunggu persetujuan pemilik knowledge, dokumen tidak langsung berubah. Sertakan `reason` dan `sources` (repo + path yang Anda baca).",
            schema: schema(
                json!({
                    "project": project,
                    "action": { "type": "string", "enum": ["list", "read", "propose"] },
                    "slug": { "type": "string", "description": "Slug dokumen, mis. arsitektur-sistem" },
                    "title": { "type": "string" },
                    "type": { "type": "string", "description": "Kunci jenis dokumen (dari action=list), wajib untuk dokumen baru" },
                    "body": { "type": "string", "description": "Isi Markdown lengkap (bukan diff)" },
                    "reason": { "type": "string", "description": "Alasan singkat perubahan" },
                    "sources": { "type": "array", "items": { "type": "object", "properties": { "repo": { "type": "string" }, "path": { "type": "string" } }, "required": ["path"] } },
                    "run": { "type": "string", "description": "Id run knowledge dari prompt job (bila ada)" },
                    "persona": { "type": "string", "description": "Kunci persona Anda; membatasi dokumen yang boleh dibaca" },
                }),
                &["action"],
            ),
            read_only: false,
        },
        ToolDef {
            name: "ag_planning",
            description: "Planning project. action=estimate: simpan usulan estimasi tiket (`ticket`, `points` story point, `stages` = jam AI dan jam manusia per kunci stage workflow, `basis` = dasar estimasi, mis. tiket mirip). action=sprint: sprint aktif/rencana project, tiketnya, kapasitas, velocity. action=release_notes: tanpa `body` membaca rilis (`release` = nama, mis. v1.4) beserta tiketnya; dengan `body` (Markdown) menyimpan versi baru release notes.",
            schema: schema(
                json!({
                    "action": { "type": "string", "enum": ["estimate", "sprint", "release_notes"] },
                    "project": project,
                    "ticket": ticket,
                    "points": { "type": "integer", "minimum": 0, "maximum": 1000 },
                    "stages": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "stage": { "type": "string", "description": "Kunci stage workflow, mis. development" },
                                "ai_hours": { "type": "number" },
                                "human_hours": { "type": "number" },
                            },
                            "required": ["stage"],
                        },
                    },
                    "basis": { "type": "string", "description": "Dasar estimasi (maks. 2000 karakter)" },
                    "release": { "type": "string", "description": "Nama rilis, mis. v1.4" },
                    "body": { "type": "string", "description": "Release notes Markdown" },
                    "persona": { "type": "string", "description": "Kunci persona Anda" },
                }),
                &["action"],
            ),
            read_only: false,
        },
    ]
}

#[derive(Debug, Default, Deserialize)]
struct Args {
    #[serde(default)]
    action: String,
    #[serde(default)]
    ticket: String,
    #[serde(default)]
    query: String,
    #[serde(default)]
    project: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default, rename = "type")]
    type_key: String,
    #[serde(default)]
    parent: String,
    #[serde(default)]
    priority: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    to: String,
    #[serde(default)]
    question: String,
    #[serde(default)]
    include: Vec<String>,
    #[serde(default)]
    source_ticket: String,
    #[serde(default)]
    drafts: Vec<DraftInput>,
    #[serde(default)]
    symbol: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    repo: String,
    #[serde(default)]
    direction: String,
    #[serde(default)]
    depth: Option<u8>,
    #[serde(default)]
    limit: Option<u64>,
    #[serde(default)]
    context: Option<u32>,
    #[serde(default)]
    diff: String,
    #[serde(default)]
    include_tests: bool,
    #[serde(default)]
    slug: String,
    #[serde(default)]
    reason: String,
    #[serde(default)]
    run: String,
    #[serde(default)]
    persona: String,
    #[serde(default)]
    sources: Vec<crate::services::knowledge::docs::SourceRef>,
    #[serde(default)]
    points: Option<i32>,
    #[serde(default)]
    stages: Vec<crate::models::ticket_estimates::StageHours>,
    #[serde(default)]
    basis: String,
    #[serde(default)]
    release: String,
}

fn required<'a>(value: &'a str, field: &str) -> Result<&'a str, ToolError> {
    let v = value.trim();
    if v.is_empty() {
        Err(ToolError::new(format!("`{field}` wajib diisi.")))
    } else {
        Ok(v)
    }
}

/// Jalankan tool `name`.
///
/// # Errors
///
/// [`ToolError`] berisi alasan yang bisa dibaca model.
pub async fn call(
    db: &DatabaseConnection,
    s: &Session,
    name: &str,
    args: Value,
) -> Result<Value, ToolError> {
    let a: Args = serde_json::from_value(args)
        .map_err(|e| ToolError::new(format!("Argumen tidak valid: {e}")))?;
    match name {
        "ag_context" => context(db, s, &a).await,
        "ag_ticket" => match a.action.as_str() {
            "get" => ticket_get(db, s, required(&a.ticket, "ticket")?).await,
            "search" => ticket_search(db, s, &a).await,
            "create" => ticket_create(db, s, &a).await,
            _ => Err(ToolError::new("`action` harus get, search, atau create.")),
        },
        "ag_draft_tickets" => draft_tickets(db, s, &a).await,
        "ag_artifact" => match a.action.as_str() {
            "get" => artifact_get(db, s, &a).await,
            "save" => artifact_save(db, s, &a).await,
            _ => Err(ToolError::new("`action` harus get atau save.")),
        },
        "ag_transition" => transition(db, s, &a).await,
        "ag_ask" => ask(db, s, &a).await,
        "ag_comment" => comment(db, s, &a).await,
        "ag_search" => graph::search(db, s, &a).await,
        "ag_trace" => graph::trace(db, s, &a).await,
        "ag_snippet" => graph::snippet(db, s, &a).await,
        "ag_impact" => graph::impact(db, s, &a).await,
        "ag_graph" => graph::overview(db, s, &a).await,
        "ag_knowledge" => knowledge::call(db, s, &a).await,
        "ag_planning" => planning::call(db, s, &a).await,
        _ => Err(ToolError::new(format!("Tool tidak dikenal: {name}"))),
    }
}

/// Tiket `code` yang boleh dibuka pemegang token (bukan di sampah).
async fn find_ticket<C: ConnectionTrait>(
    db: &C,
    s: &Session,
    code: &str,
) -> Result<(tickets::Model, projects::Model), ToolError> {
    let wid = s.workspace.workspace_id();
    let not_found = || ToolError::new(format!("Tiket {code} tidak ditemukan."));
    let (ticket, _) = tickets::Model::find_by_code(db, wid, code)
        .await?
        .filter(|(t, _)| !t.is_trashed())
        .ok_or_else(not_found)?;
    let project = projects::Model::find_in(db, wid, ticket.project_id)
        .await?
        .ok_or_else(not_found)?;
    access_in(db, &s.acting(), project.id, Permission::ViewProject)
        .await
        .map_err(|_| not_found())?;
    Ok((ticket, project))
}

async fn find_project<C: ConnectionTrait>(
    db: &C,
    s: &Session,
    key: &str,
) -> Result<projects::Model, ToolError> {
    let wid = s.workspace.workspace_id();
    let not_found = || ToolError::new(format!("Project {key} tidak ditemukan."));
    let project = projects::Model::find_by_key(db, wid, key)
        .await?
        .ok_or_else(not_found)?;
    access_in(db, &s.acting(), project.id, Permission::ViewProject)
        .await
        .map_err(|_| not_found())?;
    Ok(project)
}

async fn context(db: &DatabaseConnection, s: &Session, a: &Args) -> Result<Value, ToolError> {
    if a.ticket.trim().is_empty() {
        let wid = s.workspace.workspace_id();
        let acting = s.acting();
        let projects: Vec<Value> = projects::Model::accessible_for(db, &acting.accessor())
            .await?
            .iter()
            .map(|p| json!({ "key": p.key, "name": p.name }))
            .collect();
        let personas: Vec<Value> = personas::Model::active_in(db, wid)
            .await?
            .iter()
            .map(|p| json!({ "key": p.key, "name": p.name, "summary": p.summary }))
            .collect();
        return Ok(json!({
            "workspace": { "name": s.workspace.name, "slug": s.workspace.slug },
            "user": { "name": s.user.name, "email": s.user.email, "role": s.role.name },
            "projects": projects,
            "personas": personas,
            "hint": "Panggil ag_context dengan `ticket` untuk konteks stage tiket.",
        }));
    }
    let (ticket, project) = find_ticket(db, s, a.ticket.trim()).await?;
    let wid = s.workspace.workspace_id();
    let flow = Flow::load(db, wid, &ticket).await?;
    let stage = flow.rule(&ticket.stage_key).cloned();
    let stage_persona = stage
        .as_ref()
        .and_then(|st| st.persona.clone())
        .unwrap_or_default();
    let gates = stages::current_gates(db, wid, &ticket, &flow).await?;
    let (artifacts, artifact_bytes) = artifact_list(db, s, &ticket).await?;
    let saver = crate::services::tokens::TokenSaver::of(&project);
    let mut skill_bytes = 0_i64;
    let mut persona_json = Value::Null;
    if let Some(key) = stage.as_ref().and_then(|st| st.persona.clone())
        && let Some(p) = personas::Model::resolve(db, wid, &key).await?
    {
        let mut skills = Vec::new();
        for link in persona_skills::Model::of_persona(db, p.id).await? {
            let Some(pack) = skill_packs::Model::find_in(db, wid, link.pack_id).await? else {
                continue;
            };
            if pack.status() != SkillStatus::Approved {
                continue;
            }
            let mut skill = json!({ "name": pack.name, "description": pack.description, "version": pack.version });
            let files = skill_files::Model::of_pack(db, pack.id).await?;
            if a.include.iter().any(|i| i == "skills") {
                let files: Vec<Value> = files
                    .into_iter()
                    .map(|f| json!({ "path": f.path, "body": f.body }))
                    .collect();
                skill["files"] = json!(files);
            } else {
                skill_bytes += files
                    .iter()
                    .map(|f| i64::try_from(f.body.len()).unwrap_or(0))
                    .sum::<i64>();
            }
            skills.push(skill);
        }
        persona_json = json!({
            "key": p.key,
            "name": p.name,
            "instructions": p.instructions,
            // Gaya ringkas bisa dimatikan per project (Penghemat token).
            "concise": p.concise && saver.concise,
            "mcp_tools": p.mcp_tools,
            "client_tools": p.client_tools,
            "skills": skills,
        });
    }
    let next: Vec<Value> = flow
        .current
        .stages
        .iter()
        .filter(|st| st.key != ticket.stage_key)
        .map(|st| json!({ "key": st.key, "name": st.name, "category": st.category }))
        .collect();
    // Aturan lewat MCP: instruksi stage lain tidak dikirim.
    let other_rules: i64 = flow
        .current
        .stages
        .iter()
        .filter(|st| st.key != ticket.stage_key)
        .map(|st| i64::try_from(st.prompt.len() + st.output_template.len()).unwrap_or(0))
        .sum();
    let res = json!({
        "ticket": summary(&ticket, &project),
        "workflow": flow.workflow.name,
        "stage": stage.map(|st| json!({
            "key": st.key,
            "name": st.name,
            "mode": st.mode,
            "prompt": st.prompt,
            "inputs": st.inputs,
            "output": st.output,
            "output_template": st.output_template,
            "tools": st.tools,
        })),
        "gates": gates.iter().map(|g| json!({ "gate": g.gate, "ok": g.ok })).collect::<Vec<_>>(),
        "persona": persona_json,
        "artifacts": artifacts,
        "stages": next,
        "code_graph": graph::context_section(db, s, &project).await?,
        "knowledge": knowledge::context_section(db, s, &project, &stage_persona).await?,
    });
    note_saving(db, s, &ticket, SavingFeature::McpRules, "ag_context", other_rules, &Value::Null).await;
    note_saving(db, s, &ticket, SavingFeature::Layered, "ag_context", skill_bytes, &Value::Null).await;
    note_saving(db, s, &ticket, SavingFeature::Handoff, "ag_context", artifact_bytes, &Value::Null).await;
    Ok(res)
}

/// Catat penghematan (byte tidak dikirim − ukuran respons) untuk tiket; gagal hanya dicatat log,
/// tidak pernah menggagalkan tool.
async fn note_saving(
    db: &DatabaseConnection,
    s: &Session,
    ticket: &tickets::Model,
    feature: SavingFeature,
    tool: &str,
    withheld: i64,
    response: &Value,
) {
    let sent = if response.is_null() {
        0
    } else {
        i64::try_from(response.to_string().len()).unwrap_or(0)
    };
    if let Err(e) = crate::services::tokens::record(
        db,
        s.workspace.workspace_id(),
        ticket.project_id,
        Some(ticket.id),
        s.user.id,
        feature,
        tool,
        withheld - sent,
    )
    .await
    {
        tracing::warn!(error = %e, tool, "gagal mencatat penghematan token");
    }
}

fn summary(t: &tickets::Model, project: &projects::Model) -> Value {
    json!({
        "code": t.code,
        "title": t.title,
        "project": project.key,
        "status": t.status,
        "priority": t.priority,
        "stage": t.stage_key,
        "story_points": t.story_points,
        "labels": t.labels,
    })
}

async fn ticket_get(db: &DatabaseConnection, s: &Session, code: &str) -> Result<Value, ToolError> {
    let (ticket, project) = find_ticket(db, s, code).await?;
    let wid = s.workspace.workspace_id();
    let ty = ticket_types::Model::find_in(db, wid, ticket.type_id).await?;
    let mut comments = ticket_comments::Model::of_ticket(db, wid, ticket.id).await?;
    let skip = comments.len().saturating_sub(10);
    let comments: Vec<ticket_comments::Model> = comments.drain(skip..).collect();
    let mut ids: Vec<Uuid> = comments.iter().filter_map(|c| c.author_id).collect();
    ids.extend(ticket.assignee_id);
    let names = users::Model::names_by_id(db, &ids).await?;
    let name = |id: Option<Uuid>| id.and_then(|i| names.get(&i).cloned());
    let parent = match ticket.parent_id {
        Some(id) => tickets::Model::find_in(db, wid, id).await?.map(|p| p.code),
        None => None,
    };
    let mut out = summary(&ticket, &project);
    out["type"] = json!(ty.map(|t| t.key));
    out["description"] = json!(ticket.description);
    out["assignee"] = json!(name(ticket.assignee_id));
    out["parent"] = json!(parent);
    out["due_on"] = json!(ticket.due_on);
    out["comments"] = json!(
        comments
            .iter()
            .map(|c| json!({ "author": name(c.author_id), "at": c.created_at, "body": c.body }))
            .collect::<Vec<_>>()
    );
    Ok(out)
}

async fn ticket_search(db: &DatabaseConnection, s: &Session, a: &Args) -> Result<Value, ToolError> {
    let acting = s.acting();
    let accessible = projects::Model::accessible_for(db, &acting.accessor()).await?;
    let project_ids: Vec<Uuid> = if a.project.trim().is_empty() {
        accessible.iter().map(|p| p.id).collect()
    } else {
        vec![find_project(db, s, a.project.trim()).await?.id]
    };
    if project_ids.is_empty() {
        return Ok(json!({ "tickets": [], "total": 0 }));
    }
    let query = TicketQuery {
        project_ids,
        q: Some(a.query.trim().to_string()).filter(|q| !q.is_empty()),
        limit: 20,
        ..TicketQuery::default()
    };
    let (rows, total) = tickets::Model::page(db, s.workspace.workspace_id(), &query).await?;
    let list: Vec<Value> = rows
        .iter()
        .map(|t| {
            let project = accessible.iter().find(|p| p.id == t.project_id);
            json!({
                "code": t.code,
                "title": t.title,
                "project": project.map(|p| p.key.clone()),
                "status": t.status,
                "stage": t.stage_key,
            })
        })
        .collect();
    Ok(json!({ "tickets": list, "total": total }))
}

async fn ticket_create(db: &DatabaseConnection, s: &Session, a: &Args) -> Result<Value, ToolError> {
    let project = find_project(db, s, required(&a.project, "project")?).await?;
    let wid = s.workspace.workspace_id();
    let type_id = if a.type_key.trim().is_empty() {
        String::new()
    } else {
        ticket_types::Model::all_in(db, wid)
            .await?
            .into_iter()
            .find(|t| t.key == a.type_key.trim().to_lowercase())
            .map(|t| t.id.to_string())
            .ok_or_else(|| ToolError::new(format!("Tipe tiket {} tidak dikenal.", a.type_key)))?
    };
    let form = TicketForm {
        input: TicketInput {
            title: a.title.clone(),
            description: a.description.clone(),
            priority: a.priority.clone(),
            ..TicketInput::default()
        },
        type_id,
        parent: a.parent.trim().to_string(),
        ..TicketForm::default()
    };
    let ticket = create::run(db, &s.acting(), project.id, &form).await?;
    Ok(json!({ "created": summary(&ticket, &project) }))
}

async fn draft_tickets(db: &DatabaseConnection, s: &Session, a: &Args) -> Result<Value, ToolError> {
    let project = find_project(db, s, required(&a.project, "project")?).await?;
    let source = if a.source_ticket.trim().is_empty() {
        None
    } else {
        Some(find_ticket(db, s, a.source_ticket.trim()).await?.0.id)
    };
    let saved = drafts::save_from_agent(db, &s.acting(), project.id, source, &a.drafts).await?;
    Ok(json!({
        "saved": saved.len(),
        "review": format!("/w/{}/p/{}/drafts", s.workspace.slug, project.key),
        "message": "Draft tersimpan. Minta manusia meninjau dan membuat tiketnya di halaman Draft tiket.",
    }))
}

/// Daftar artifact (tanpa isi) + total byte isi versi terbaru yang tidak dikirim.
async fn artifact_list(
    db: &DatabaseConnection,
    s: &Session,
    ticket: &tickets::Model,
) -> Result<(Vec<Value>, i64), ToolError> {
    let wid = s.workspace.workspace_id();
    let mut out = Vec::new();
    let mut withheld = 0_i64;
    for art in ticket_artifacts::Model::of_ticket(db, wid, ticket.id).await? {
        let latest = artifact_versions::Model::latest(db, wid, art.id).await?;
        withheld += latest
            .as_ref()
            .map_or(0, |v| i64::try_from(v.body.len()).unwrap_or(0));
        out.push(json!({
            "name": art.name,
            "stage": art.stage_key,
            "version": latest.as_ref().map(|v| v.number),
            "status": latest.map(|v| v.status),
        }));
    }
    Ok((out, withheld))
}

async fn artifact_get(db: &DatabaseConnection, s: &Session, a: &Args) -> Result<Value, ToolError> {
    let (ticket, _) = find_ticket(db, s, a.ticket.trim()).await?;
    if a.name.trim().is_empty() {
        let (list, withheld) = artifact_list(db, s, &ticket).await?;
        let res = json!({ "artifacts": list });
        note_saving(db, s, &ticket, SavingFeature::Layered, "ag_artifact", withheld, &res).await;
        return Ok(res);
    }
    let wid = s.workspace.workspace_id();
    let art = ticket_artifacts::Model::find_named(db, wid, ticket.id, &a.name)
        .await?
        .ok_or_else(|| ToolError::new(format!("Artifact {} belum ada.", a.name.trim())))?;
    let latest = artifact_versions::Model::latest(db, wid, art.id).await?;
    Ok(json!({
        "name": art.name,
        "stage": art.stage_key,
        "version": latest.as_ref().map(|v| v.number),
        "status": latest.as_ref().map(|v| v.status.clone()),
        "body": latest.map(|v| v.body).unwrap_or_default(),
    }))
}

async fn artifact_save(db: &DatabaseConnection, s: &Session, a: &Args) -> Result<Value, ToolError> {
    let (ticket, _) = find_ticket(db, s, a.ticket.trim()).await?;
    let name = required(&a.name, "name")?;
    let wid = s.workspace.workspace_id();
    let acting = s.acting();
    let art = match ticket_artifacts::Model::find_named(db, wid, ticket.id, name).await? {
        Some(art) => art,
        None => artifacts::create(db, &acting, ticket.id, name).await?,
    };
    let version = artifacts::save(db, &acting, ticket.id, art.id, &a.body).await?;
    Ok(json!({ "name": art.name, "version": version.number, "status": version.status }))
}

async fn transition(db: &DatabaseConnection, s: &Session, a: &Args) -> Result<Value, ToolError> {
    let (ticket, project) = find_ticket(db, s, a.ticket.trim()).await?;
    let to = required(&a.to, "to")?;
    let moved = stages::move_to(
        db,
        &s.acting(),
        ticket.id,
        Target::Stage(to.to_string()),
        None,
    )
    .await?;
    Ok(json!({ "ticket": summary(&moved, &project) }))
}

async fn ask(db: &DatabaseConnection, s: &Session, a: &Args) -> Result<Value, ToolError> {
    let (ticket, _) = find_ticket(db, s, a.ticket.trim()).await?;
    let question = required(&a.question, "question")?;
    let (comment, notified) = comments::ask(db, &s.acting(), ticket.id, question).await?;
    Ok(json!({ "comment_id": comment.id, "notified": notified }))
}

async fn comment(db: &DatabaseConnection, s: &Session, a: &Args) -> Result<Value, ToolError> {
    let (ticket, _) = find_ticket(db, s, a.ticket.trim()).await?;
    let body = required(&a.body, "body")?;
    let comment = comments::post(db, &s.acting(), ticket.id, body).await?;
    Ok(json!({ "comment_id": comment.id }))
}
