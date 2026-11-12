//! Tampilan workflow (M3): label stage, persona, mode, transisi, gate; stepper tiket; data
//! builder. Semua label berupa kunci Fluent (`ui-workflow.ftl`).

use agentgate_core::workflow::{Category, Definition, Exit, Gate, Mode, Stage};
use serde::Serialize;

use super::tickets::PersonView;
pub use crate::models::personas::{PersonaLabel, PersonaLabels};

/// Label persona `key`: dari workspace, lalu persona bawaan, lalu kuncinya sendiri.
#[must_use]
pub fn persona_label(key: &str, labels: &PersonaLabels) -> PersonaLabel {
    labels.get(key).cloned().unwrap_or_else(|| {
        let builtin = agentgate_core::persona::builtin(key);
        PersonaLabel {
            key: key.to_string(),
            name: builtin.map_or_else(|| key.to_string(), |b| b.name.to_string()),
            icon: builtin.map_or("robot", |b| b.icon).to_string(),
            color: builtin.map_or("#64748b", |b| b.color).to_string(),
        }
    })
}

#[must_use]
pub fn mode_key(m: Mode) -> String {
    format!("workflow-mode-{}", m.as_str())
}

#[must_use]
pub fn exit_key(e: Exit) -> String {
    format!("workflow-exit-{}", e.as_str())
}

#[must_use]
pub fn gate_key(g: Gate) -> String {
    format!("workflow-gate-{}", g.as_str().replace('_', "-"))
}

#[must_use]
pub fn category_key(c: Category) -> String {
    format!("ticket-status-{}", c.as_str().replace('_', "-"))
}

/// Pilihan persona untuk form stage: persona aktif workspace (nilai = kunci, label = nama).
#[must_use]
pub fn persona_options(labels: &[PersonaLabel]) -> Vec<Choice> {
    labels
        .iter()
        .map(|p| Choice::new(&p.key, p.name.clone()))
        .collect()
}

#[must_use]
pub fn mode_options() -> Vec<Choice> {
    [Mode::Manual, Mode::Auto, Mode::Interactive]
        .iter()
        .map(|m| Choice::new(m.as_str(), mode_key(*m)))
        .collect()
}

#[must_use]
pub fn exit_options() -> Vec<Choice> {
    [Exit::Approval, Exit::Auto, Exit::Manual]
        .iter()
        .map(|e| {
            let mut c = Choice::new(e.as_str(), exit_key(*e));
            c.hint_key = format!("{}-hint", exit_key(*e));
            c
        })
        .collect()
}

#[must_use]
pub fn gate_options() -> Vec<Choice> {
    Gate::ALL
        .iter()
        .map(|g| {
            let mut c = Choice::new(g.as_str(), gate_key(*g));
            c.evidence = g.needs_evidence();
            c
        })
        .collect()
}

#[must_use]
pub fn category_options() -> Vec<Choice> {
    Category::ALL
        .iter()
        .map(|c| Choice::new(c.as_str(), category_key(*c)))
        .collect()
}

/// Satu pilihan (radio/select/checkbox).
#[derive(Debug, Clone, Serialize)]
pub struct Choice {
    pub value: String,
    pub label_key: String,
    pub hint_key: String,
    /// Gate yang butuh evidence runner/provider (belum ada sumbernya di M3).
    pub evidence: bool,
}

impl Choice {
    fn new(value: &str, label_key: String) -> Self {
        Self {
            value: value.to_string(),
            label_key,
            hint_key: String::new(),
            evidence: false,
        }
    }
}

/// Ringkasan satu stage (stepper, kanvas builder, pilihan stage).
#[derive(Debug, Clone, Serialize)]
pub struct StageView {
    pub key: String,
    pub name: String,
    pub category: &'static str,
    pub category_key: String,
    /// Kunci persona (kosong = manusia).
    pub persona: String,
    /// Nama persona (kosong = manusia).
    pub persona_name: String,
    pub icon: String,
    pub mode_key: String,
    pub exit: &'static str,
    pub exit_key: String,
    pub gates: usize,
    pub parallel: bool,
    pub fail_to: String,
    pub fail_limit: u8,
    /// `done` (sudah dilewati), `current`, `todo`.
    pub state: &'static str,
    pub href: String,
}

impl StageView {
    #[must_use]
    pub fn of(s: &Stage, definition: &Definition, labels: &PersonaLabels) -> Self {
        let label = s.persona.as_deref().map(|p| persona_label(p, labels));
        Self {
            key: s.key.clone(),
            name: s.name.clone(),
            category: s.category.as_str(),
            category_key: category_key(s.category),
            persona: s.persona.clone().unwrap_or_default(),
            persona_name: label.as_ref().map(|l| l.name.clone()).unwrap_or_default(),
            icon: label.map_or_else(|| "user".to_string(), |l| l.icon),
            mode_key: mode_key(s.mode),
            exit: s.exit.as_str(),
            exit_key: exit_key(s.exit),
            gates: s.gates.len(),
            parallel: s.parallel,
            fail_to: s
                .fail_to
                .as_deref()
                .and_then(|k| definition.stage(k))
                .map(|t| t.name.clone())
                .unwrap_or_default(),
            fail_limit: s.fail_limit,
            state: "todo",
            href: String::new(),
        }
    }
}

/// Stepper tiket: semua stage versi aktif; yang sebelum stage sekarang `done`.
#[must_use]
pub fn stepper(
    definition: &Definition,
    current: &str,
    position: usize,
    labels: &PersonaLabels,
) -> Vec<StageView> {
    definition
        .stages
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let mut v = StageView::of(s, definition, labels);
            v.state = if s.key == current {
                "current"
            } else if i < position {
                "done"
            } else {
                "todo"
            };
            v
        })
        .collect()
}

/// Hasil satu gate untuk panel stage.
#[derive(Debug, Clone, Serialize)]
pub struct GateView {
    pub key: &'static str,
    pub label_key: String,
    pub hint_key: String,
    pub ok: bool,
}

impl GateView {
    #[must_use]
    pub fn of(gate: Gate, ok: bool) -> Self {
        Self {
            key: gate.as_str(),
            label_key: gate_key(gate),
            hint_key: format!("{}-hint", gate_key(gate)),
            ok,
        }
    }
}

/// Penugasan stage untuk tampilan.
#[derive(Debug, Clone, Default, Serialize)]
pub struct PeopleView {
    pub executors: Vec<PersonView>,
    pub approvers: Vec<PersonView>,
    pub watchers: Vec<PersonView>,
}

/// Kunci Fluent semua label workflow (tes kontrak i18n).
#[must_use]
pub fn label_keys() -> Vec<String> {
    let mut keys = Vec::new();
    for m in [Mode::Manual, Mode::Auto, Mode::Interactive] {
        keys.push(mode_key(m));
    }
    for e in [Exit::Approval, Exit::Auto, Exit::Manual] {
        keys.push(exit_key(e));
        keys.push(format!("{}-hint", exit_key(e)));
    }
    for g in Gate::ALL {
        keys.push(gate_key(g));
        keys.push(format!("{}-hint", gate_key(g)));
    }
    for c in Category::ALL {
        keys.push(category_key(c));
    }
    keys
}

/// Kunci flash halaman workflow, stage, dan artifact (tes kontrak i18n).
pub const MESSAGE_KEYS: &[&str] = &[
    "flash-workflow-created",
    "flash-workflow-draft-saved",
    "flash-workflow-published",
    "flash-workflow-discarded",
    "flash-workflow-deleted",
    "flash-workflow-defaults-saved",
    "flash-workflow-renamed",
    "flash-project-workflow-saved",
    "flash-workflow-imported",
    "flash-stage-moved",
    "flash-stage-approved",
    "flash-stage-rejected",
    "flash-stage-decided",
    "flash-stage-people-saved",
    "flash-artifact-saved",
    "flash-artifact-created",
    "artifact-status-draft",
    "artifact-status-pending",
    "artifact-status-approved",
    "artifact-status-rejected",
    "decision-hint",
    "decision-self",
    "decision-requirement",
    "decision-accept",
    "decision-cancel",
    "decision-hint-hint",
    "decision-self-hint",
    "decision-requirement-hint",
    "decision-accept-hint",
    "decision-cancel-hint",
    "workflow-tab-templates",
    "workflow-tab-import",
    "workflow-tab-builder",
    "inbox-tab-decisions",
    "workflow-source-blank",
    "workflow-source-template",
    "workflow-source-clone",
    "workflow-migrate-removed",
    "workflow-migrate-to-map",
    "workflow-migrate-others",
];
