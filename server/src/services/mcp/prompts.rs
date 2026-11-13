//! Slash command Agent Gate = prompt MCP. `ag-<stage>` untuk setiap stage ber-persona di
//! workflow workspace (kunci stage sama di beberapa workflow → satu prompt; instruksi diambil dari
//! workflow tiket saat dipanggil), ditambah utilitas. Claude Code menampilkannya sebagai
//! `/mcp__agentgate__ag-…`.

use sea_orm::{ConnectionTrait, DbErr};
use serde::Serialize;

use crate::models::{
    personas::{self, PersonaLabel},
    roles, workflows, workspaces,
};

/// Prompt utilitas: (nama, kunci Fluent deskripsi, sudah tersedia).
pub const UTILITIES: &[(&str, &str, bool)] = &[
    ("ag-status", "slash-status", true),
    ("ag-ticket", "slash-ticket", true),
    ("ag-next", "slash-next", false),
    ("ag-search", "slash-search", true),
    ("ag-knowledge-sync", "slash-knowledge-sync", true),
];

/// Satu prompt stage.
#[derive(Debug, Clone, Serialize)]
pub struct StagePrompt {
    /// Nama prompt (`ag-brainstorming`).
    pub name: String,
    pub stage_key: String,
    pub stage_name: String,
    /// Workflow yang memuat stage ini (nama).
    pub workflows: Vec<String>,
    pub persona: Option<PersonaLabel>,
}

/// Slash command satu workspace.
#[derive(Debug, Clone, Serialize)]
pub struct Group {
    pub workspace: String,
    pub stages: Vec<StagePrompt>,
}

impl Group {
    #[must_use]
    pub fn len(&self) -> usize {
        self.stages.len() + UTILITIES.iter().filter(|u| u.2).count()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Nama prompt untuk kunci stage.
#[must_use]
pub fn stage_prompt_name(stage_key: &str) -> String {
    format!("ag-{}", stage_key.replace('_', "-"))
}

/// Prompt stage satu workspace (urutan: workflow lalu urutan stage).
///
/// # Errors
///
/// Error database.
pub async fn stage_prompts<C: ConnectionTrait>(
    db: &C,
    workspace: &workspaces::Model,
) -> Result<Vec<StagePrompt>, DbErr> {
    let wid = workspace.workspace_id();
    let labels = personas::Model::labels(db, wid).await?;
    let mut out: Vec<StagePrompt> = Vec::new();
    for wf in workflows::Model::all_in(db, wid).await? {
        let def = wf.definition(db).await?;
        for stage in &def.stages {
            let Some(persona) = stage.persona.as_deref() else {
                continue;
            };
            let name = stage_prompt_name(&stage.key);
            if let Some(found) = out.iter_mut().find(|p| p.name == name) {
                if !found.workflows.contains(&wf.name) {
                    found.workflows.push(wf.name.clone());
                }
                continue;
            }
            out.push(StagePrompt {
                name,
                stage_key: stage.key.clone(),
                stage_name: stage.name.clone(),
                workflows: vec![wf.name.clone()],
                persona: labels.get(persona).cloned(),
            });
        }
    }
    Ok(out)
}

/// Katalog slash command per workspace (halaman Hubungkan client AI).
///
/// # Errors
///
/// Error database.
pub async fn catalog<C: ConnectionTrait>(
    db: &C,
    workspaces: &[(workspaces::Model, roles::Model)],
) -> Result<Vec<Group>, DbErr> {
    let mut out = Vec::new();
    for (w, _) in workspaces {
        out.push(Group {
            workspace: w.name.clone(),
            stages: stage_prompts(db, w).await?,
        });
    }
    Ok(out)
}

/// Satu prompt untuk `prompts/list`.
#[derive(Debug, Clone)]
pub struct PromptDef {
    pub name: String,
    pub title: String,
    pub description: String,
    /// Butuh argumen `ticket`.
    pub takes_ticket: bool,
}

/// Prompt yang tersedia untuk pemegang token: stage ber-persona + utilitas yang sudah ada.
///
/// # Errors
///
/// Error database.
pub async fn list<C: ConnectionTrait>(db: &C, s: &super::Session) -> Result<Vec<PromptDef>, DbErr> {
    let mut out: Vec<PromptDef> = stage_prompts(db, &s.workspace)
        .await?
        .into_iter()
        .map(|p| PromptDef {
            title: format!("Stage {}", p.stage_name),
            description: format!(
                "Kerjakan stage {} tiket{}.",
                p.stage_name,
                p.persona
                    .as_ref()
                    .map(|l| format!(" sebagai {}", l.name))
                    .unwrap_or_default()
            ),
            name: p.name,
            takes_ticket: true,
        })
        .collect();
    out.push(PromptDef {
        name: "ag-status".into(),
        title: "Status".into(),
        description: "Ringkasan tiket yang sedang Anda kerjakan.".into(),
        takes_ticket: false,
    });
    out.push(PromptDef {
        name: "ag-ticket".into(),
        title: "Tiket".into(),
        description: "Muat konteks satu tiket.".into(),
        takes_ticket: true,
    });
    out.push(PromptDef {
        name: "ag-search".into(),
        title: "Cari".into(),
        description: "Cari di knowledge dan code graph project.".into(),
        takes_ticket: false,
    });
    out.push(PromptDef {
        name: "ag-knowledge-sync".into(),
        title: "Sinkronkan knowledge".into(),
        description: "Periksa knowledge project terhadap kode dan usulkan perbaikan.".into(),
        takes_ticket: false,
    });
    Ok(out)
}

/// Isi prompt `name` (teks untuk pesan user). `None` bila prompt tidak dikenal.
///
/// # Errors
///
/// Error database.
pub async fn render(
    db: &sea_orm::DatabaseConnection,
    s: &super::Session,
    name: &str,
    ticket: &str,
) -> Result<Option<(String, String)>, DbErr> {
    let ticket = ticket.trim();
    let target = if ticket.is_empty() {
        "tiket yang saya sebutkan (tanyakan kodenya bila belum jelas)".to_string()
    } else {
        format!("tiket {ticket}")
    };
    match name {
        "ag-status" => {
            let acting = s.acting();
            let projects =
                crate::models::projects::Model::accessible_for(db, &acting.accessor()).await?;
            let query = crate::models::tickets::TicketQuery {
                project_ids: projects.iter().map(|p| p.id).collect(),
                people: vec![crate::models::tickets::PersonFilter::AssignedTo(s.user.id)],
                statuses: vec![
                    crate::models::tickets::TicketStatus::Todo,
                    crate::models::tickets::TicketStatus::InProgress,
                ],
                limit: 20,
                ..Default::default()
            };
            let (rows, _) = if query.project_ids.is_empty() {
                (Vec::new(), 0)
            } else {
                crate::models::tickets::Model::page(db, s.workspace.workspace_id(), &query).await?
            };
            let mut text = format!(
                "Ringkas pekerjaan saya di workspace {} (Agent Gate). Tiket yang ditugaskan ke saya:\n",
                s.workspace.name
            );
            if rows.is_empty() {
                text.push_str("- (tidak ada)\n");
            }
            for t in &rows {
                text.push_str(&format!(
                    "- {} · {} · stage {}\n",
                    t.code, t.title, t.stage_key
                ));
            }
            text.push_str(
                "\nUntuk setiap tiket, panggil ag_context bila perlu detail, lalu sebutkan langkah berikutnya.",
            );
            Ok(Some(("Status pekerjaan".into(), text)))
        }
        "ag-ticket" => Ok(Some((
            "Konteks tiket".into(),
            format!(
                "Muat konteks {target} di Agent Gate: panggil ag_context dengan ticket tersebut, lalu ringkas tujuan, stage sekarang, gate yang belum terpenuhi, dan artifact yang ada."
            ),
        ))),
        "ag-search" => Ok(Some((
            "Cari knowledge dan kode".into(),
            "Cari jawaban pertanyaan saya di Agent Gate: panggil ag_search dengan kata kunci (hasilnya berisi simbol kode dan dokumen knowledge), baca dokumen yang cocok dengan ag_knowledge action=read dan kode dengan ag_snippet, lalu jawab ringkas dengan menyebut sumbernya.".into(),
        ))),
        "ag-knowledge-sync" => Ok(Some((
            "Sinkronkan knowledge".into(),
            "Sinkronkan knowledge project di Agent Gate dengan kode sekarang:\n\n             1. Panggil ag_knowledge action=list (tanyakan kode project bila Anda punya lebih dari satu).\n             2. Untuk dokumen yang basi atau terkait kode yang berubah, baca isinya (action=read) lalu periksa kodenya lewat ag_search, ag_trace, dan ag_snippet.\n             3. Usulkan perbaikan lewat ag_knowledge action=propose (isi Markdown lengkap, `reason`, `sources`). Dokumen yang masih benar tidak perlu diusulkan.\n             4. Bila jenis dokumen penting belum ada, usulkan dokumen baru dengan `title` + `type`.\n\n             Semua usulan menunggu persetujuan pemilik knowledge; jangan mengubah file repo.".into(),
        ))),
        _ => {
            let Some(p) = stage_prompts(db, &s.workspace)
                .await?
                .into_iter()
                .find(|p| p.name == name)
            else {
                return Ok(None);
            };
            let persona = p
                .persona
                .as_ref()
                .map(|l| format!(" sebagai persona {}", l.name))
                .unwrap_or_default();
            let text = format!(
                "Kerjakan stage \"{stage}\" untuk {target}{persona}.\n\n\
                 1. Panggil ag_context dengan ticket tersebut (include: [\"skills\"]) dan ikuti instruksi persona, prompt stage, dan skill-nya.\n\
                 2. Baca artifact input stage lewat ag_artifact bila ada.\n\
                 3. Simpan hasil ke artifact output stage lewat ag_artifact action=save.\n\
                 4. Bila butuh keputusan manusia, pakai ag_ask. Jangan menebak.\n\
                 5. Setelah gate stage terpenuhi, ajukan pindah stage lewat ag_transition.\n\n\
                 Stage key: {key}. Workflow: {workflows}.",
                stage = p.stage_name,
                key = p.stage_key,
                workflows = p.workflows.join(", "),
            );
            Ok(Some((format!("Stage {}", p.stage_name), text)))
        }
    }
}
