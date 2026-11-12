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
    ("ag-search", "slash-search", false),
    ("ag-knowledge-sync", "slash-knowledge-sync", false),
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
