//! Tampilan Pengaturan → Persona agent dan Edit persona (`assets/views/personas/`).

use std::collections::BTreeMap;

use agentgate_core::persona::{
    ALWAYS_BLOCKED, CLIENT_TOOLS, CLIENTS, DANGER_ACTIONS, Level, MCP_TOOLS,
};
use serde::Serialize;
use uuid::Uuid;

use super::{nav::NavItem, ui::ComboOption};
use crate::models::{personas, skill_packs};

pub const FLASH_CREATED: &str = "persona-flash-created";
pub const FLASH_SAVED: &str = "persona-flash-saved";
pub const FLASH_DELETED: &str = "persona-flash-deleted";

/// Bagian halaman Edit persona (urutan menu).
pub const SECTIONS: &[&str] = &[
    "identity",
    "instructions",
    "skills",
    "tools",
    "knowledge",
    "models",
    "danger",
];

/// Kartu persona di daftar.
#[derive(Debug, Clone, Serialize)]
pub struct PersonaCard {
    pub id: Uuid,
    pub key: String,
    pub name: String,
    pub summary: String,
    pub color: String,
    pub icon: String,
    pub builtin: bool,
    /// Model rekomendasi client pertama yang diisi (`""` = default client).
    pub model: String,
    pub skills: usize,
    /// "Workflow · Stage" yang memakai persona ini.
    pub stages: Vec<String>,
    pub href: String,
}

impl PersonaCard {
    #[must_use]
    pub fn of(p: &personas::Model, skills: usize, stages: Vec<String>, base: &str) -> Self {
        let model = CLIENTS
            .iter()
            .find_map(|c| p.model_for(c.key).filter(|m| !m.model.is_empty()))
            .map(|m| m.model)
            .unwrap_or_default();
        Self {
            id: p.id,
            key: p.key.clone(),
            name: p.name.clone(),
            summary: p.summary.clone(),
            color: p.color.clone(),
            icon: p.icon.clone(),
            builtin: p.builtin,
            model,
            skills,
            stages,
            href: format!("{base}/{}", p.id),
        }
    }
}

/// Isian identitas (form baru dan bagian Identitas).
#[derive(Debug, Clone, Default, Serialize)]
pub struct IdentityForm {
    pub name: String,
    pub summary: String,
    pub color: String,
    pub icon: String,
    /// Persona sumber clone (`""` = kosong).
    pub source: String,
}

impl IdentityForm {
    #[must_use]
    pub fn of(p: &personas::Model) -> Self {
        Self {
            name: p.name.clone(),
            summary: p.summary.clone(),
            color: p.color.clone(),
            icon: p.icon.clone(),
            source: String::new(),
        }
    }
}

/// Satu checkbox tool.
#[derive(Debug, Clone, Serialize)]
pub struct ToolRow {
    pub value: &'static str,
    pub label_key: String,
    pub hint_key: String,
    pub on: bool,
    pub mono: bool,
}

/// Satu aksi berbahaya dengan tingkat izinnya.
#[derive(Debug, Clone, Serialize)]
pub struct DangerRow {
    pub action: &'static str,
    pub label_key: String,
    pub hint_key: String,
    pub level: &'static str,
}

/// Satu baris model per client.
#[derive(Debug, Clone, Serialize)]
pub struct ModelRow {
    pub key: &'static str,
    pub name: &'static str,
    pub models: Vec<&'static str>,
    pub efforts: Vec<&'static str>,
    pub model: String,
    pub effort: String,
}

#[must_use]
pub fn mcp_tools(selected: &[String]) -> Vec<ToolRow> {
    MCP_TOOLS
        .iter()
        .map(|t| ToolRow {
            value: t,
            label_key: String::new(),
            hint_key: format!("mcp-tool-{}", t.replace('_', "-")),
            on: selected.iter().any(|s| s == t),
            mono: true,
        })
        .collect()
}

#[must_use]
pub fn client_tools(selected: &[String]) -> Vec<ToolRow> {
    CLIENT_TOOLS
        .iter()
        .map(|t| ToolRow {
            value: t,
            label_key: format!("persona-client-tool-{t}"),
            hint_key: format!("persona-client-tool-{t}-hint"),
            on: selected.iter().any(|s| s == t),
            mono: false,
        })
        .collect()
}

#[must_use]
pub fn danger_rows(levels: &[(&'static str, Level)]) -> Vec<DangerRow> {
    DANGER_ACTIONS
        .iter()
        .map(|(action, default)| {
            let level = levels
                .iter()
                .find(|(a, _)| a == action)
                .map_or(*default, |(_, l)| *l);
            DangerRow {
                action,
                label_key: format!("persona-danger-{}", action.replace('_', "-")),
                hint_key: format!("persona-danger-{}-hint", action.replace('_', "-")),
                level: level.as_str(),
            }
        })
        .collect()
}

/// Pilihan tingkat izin (Izinkan, Tanya, Blokir).
#[must_use]
pub fn levels() -> Vec<super::workflow::Choice> {
    Level::ALL
        .iter()
        .map(|l| super::workflow::Choice::new(l.as_str(), format!("persona-level-{}", l.as_str())))
        .collect()
}

/// Kunci Fluent aksi yang selalu diblokir.
#[must_use]
pub fn always_blocked() -> Vec<String> {
    ALWAYS_BLOCKED
        .iter()
        .map(|a| format!("persona-blocked-{}", a.replace('_', "-")))
        .collect()
}

#[must_use]
pub fn model_rows(
    choices: &BTreeMap<String, agentgate_core::persona::ModelChoice>,
) -> Vec<ModelRow> {
    CLIENTS
        .iter()
        .map(|c| {
            let choice = choices.get(c.key);
            ModelRow {
                key: c.key,
                name: c.name,
                models: c.models.to_vec(),
                efforts: c.efforts.to_vec(),
                model: choice.map(|m| m.model.clone()).unwrap_or_default(),
                effort: choice.map(|m| m.effort.clone()).unwrap_or_default(),
            }
        })
        .collect()
}

/// Opsi pengganti/sumber clone: persona aktif selain `except`.
#[must_use]
pub fn persona_options(all: &[personas::Model], except: Option<Uuid>) -> Vec<ComboOption> {
    all.iter()
        .filter(|p| Some(p.id) != except)
        .map(|p| ComboOption::new(p.id, &p.name).sub(&p.summary))
        .collect()
}

/// Opsi skill pack yang sudah di-approve.
#[must_use]
pub fn skill_options(approved: &[skill_packs::Model]) -> Vec<ComboOption> {
    approved
        .iter()
        .map(|s| ComboOption::new(s.id, &s.name).sub(format!("v{}", s.version)))
        .collect()
}

/// Satu jenis dokumen di panel Knowledge persona.
#[derive(Debug, Clone, Serialize)]
pub struct KnowledgeTypeRow {
    pub key: String,
    pub name: String,
    pub hint: String,
    pub on: bool,
}

/// Jenis dokumen per kelompok untuk panel Knowledge persona.
#[derive(Debug, Clone, Serialize)]
pub struct KnowledgeGroupRow {
    pub key: &'static str,
    pub name: &'static str,
    pub icon: &'static str,
    pub types: Vec<KnowledgeTypeRow>,
}

/// Kelompok + jenis (bawaan dan custom) dengan tanda terpilih.
#[must_use]
pub fn knowledge_groups(
    custom: &[crate::models::knowledge_types::Model],
    selected: &[String],
) -> Vec<KnowledgeGroupRow> {
    crate::models::knowledge_catalog::GROUPS
        .iter()
        .map(|g| KnowledgeGroupRow {
            key: g.key,
            name: g.name,
            icon: g.icon,
            types: crate::models::knowledge_catalog::types_in(g.key)
                .map(|t| KnowledgeTypeRow {
                    key: t.key.into(),
                    name: t.name.into(),
                    hint: t.hint.into(),
                    on: selected.iter().any(|s| s == t.key),
                })
                .chain(
                    custom
                        .iter()
                        .filter(|c| c.group_key == g.key)
                        .map(|c| KnowledgeTypeRow {
                            key: c.key.clone(),
                            name: c.name.clone(),
                            hint: String::new(),
                            on: selected.contains(&c.key),
                        }),
                )
                .collect(),
        })
        .collect()
}

/// Menu kiri halaman Edit persona.
#[must_use]
pub fn subnav(base: &str, current: &str, skills: usize) -> Vec<NavItem> {
    let link = |key: &'static str, label: &'static str, icon: &'static str, desc: &'static str| {
        NavItem::link(key, label, icon, &format!("{base}?section={key}")).describe(desc)
    };
    let items = vec![
        link(
            "identity",
            "persona-section-identity",
            "user-circle",
            "persona-section-identity-desc",
        ),
        link(
            "instructions",
            "persona-section-instructions",
            "file-text",
            "persona-section-instructions-desc",
        ),
        link(
            "skills",
            "persona-section-skills",
            "package",
            "persona-section-skills-desc",
        )
        .counted(skills as u64, false),
        link(
            "tools",
            "persona-section-tools",
            "shield-lock",
            "persona-section-tools-desc",
        ),
        link(
            "knowledge",
            "persona-section-knowledge",
            "book",
            "persona-section-knowledge-desc",
        ),
        link(
            "models",
            "persona-section-models",
            "cpu",
            "persona-section-models-desc",
        ),
        link(
            "danger",
            "persona-section-danger",
            "trash",
            "persona-section-danger-desc",
        ),
    ];
    super::nav::mark(items, current)
}

/// Semua kunci Fluent yang dibangun di kode untuk halaman persona (dites kontrak i18n).
#[must_use]
pub fn label_keys() -> Vec<String> {
    let mut keys: Vec<String> = Vec::new();
    for t in mcp_tools(&[]).into_iter().chain(client_tools(&[])) {
        keys.push(t.hint_key);
        if !t.label_key.is_empty() {
            keys.push(t.label_key);
        }
    }
    for d in danger_rows(&[]) {
        keys.push(d.label_key);
        keys.push(d.hint_key);
    }
    keys.extend(always_blocked());
    keys.extend(levels().into_iter().map(|l| l.label_key));
    keys.extend(
        subnav("/", "identity", 0)
            .into_iter()
            .filter_map(|i| i.desc_key.map(str::to_string)),
    );
    keys.extend([FLASH_CREATED, FLASH_SAVED, FLASH_DELETED].map(str::to_string));
    keys
}
