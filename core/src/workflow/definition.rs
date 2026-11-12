//! Definisi workflow: daftar stage berurutan beserta aturannya (spec "Workflow, stage & gate").
//!
//! - **Kunci stage** (`key`) stabil selama workflow hidup: tiket menyimpan kunci stage-nya, dan
//!   migrasi workflow membandingkan kunci versi lama dan baru (stage yang kuncinya hilang =
//!   dihapus).
//! - **Kategori** stage adalah himpunan tetap yang sama dengan kolom `tickets.status` M2
//!   (backlog, todo, in_progress, done). Kategori tidak boleh mundur di sepanjang urutan stage
//!   dan stage terakhir berkategori `done`, sehingga "sudah mulai" dan "selesai" punya arti yang
//!   sama di semua workflow.
//! - **Paralel**: stage dengan `parallel = true` berjalan bersamaan dengan stage sebelumnya
//!   (ditampilkan berdampingan di builder). Selama belum ada runner, tiket tetap melewatinya satu
//!   per satu.
//! - Validasi mengembalikan kode Fluent (`workflow-…`) per stage dan field, bukan kalimat.
//!
//! Bentuk JSON (serde) dari [`Definition`] adalah bentuk penyimpanan di database (`jsonb`).
//! Field opsional memakai `#[serde(default)]` supaya definisi lama tetap terbaca saat field baru
//! ditambah.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Batas jumlah stage per workflow.
pub const MAX_STAGES: usize = 30;
pub const NAME_MAX_LEN: usize = 60;
pub const KEY_MAX_LEN: usize = 40;
pub const SKILL_MAX_LEN: usize = 120;
pub const PROMPT_MAX_LEN: usize = 4_000;
pub const TEMPLATE_MAX_LEN: usize = 20_000;
pub const ARTIFACT_NAME_MAX_LEN: usize = 80;
pub const TOOL_MAX_LEN: usize = 60;
pub const LIST_MAX: usize = 10;
pub const FAIL_LIMIT_MAX: u8 = 9;
pub const FAIL_LIMIT_DEFAULT: u8 = 3;

pub const STAGES_EMPTY: &str = "workflow-stages-empty";
pub const STAGES_TOO_MANY: &str = "workflow-stages-too-many";
pub const KEY_INVALID: &str = "workflow-stage-key-invalid";
pub const KEY_TAKEN: &str = "workflow-stage-key-taken";
pub const NAME_REQUIRED: &str = "workflow-stage-name-required";
pub const NAME_TOO_LONG: &str = "workflow-stage-name-too-long";
pub const NAME_TAKEN: &str = "workflow-stage-name-taken";
pub const LAST_NOT_DONE: &str = "workflow-last-stage-not-done";
pub const CATEGORY_ORDER: &str = "workflow-category-order";
pub const FIRST_PARALLEL: &str = "workflow-first-stage-parallel";
pub const FAIL_TO_INVALID: &str = "workflow-fail-to-invalid";
pub const FAIL_LIMIT_INVALID: &str = "workflow-fail-limit-invalid";
pub const ARTIFACT_NAME_INVALID: &str = "workflow-artifact-name-invalid";
pub const TEXT_TOO_LONG: &str = "workflow-text-too-long";
pub const MODE_NEEDS_PERSONA: &str = "workflow-mode-needs-persona";
pub const GATE_DUPLICATE: &str = "workflow-gate-duplicate";
pub const TOOL_INVALID: &str = "workflow-tool-invalid";
pub const LIST_TOO_LONG: &str = "workflow-list-too-long";
pub const PARALLEL_CATEGORY: &str = "workflow-parallel-category";
pub const PERSONA_INVALID: &str = "workflow-persona-invalid";

/// Semua kode error validasi definisi (tes kontrak i18n memastikan semuanya punya teks).
pub const ERROR_CODES: &[&str] = &[
    STAGES_EMPTY,
    STAGES_TOO_MANY,
    KEY_INVALID,
    KEY_TAKEN,
    NAME_REQUIRED,
    NAME_TOO_LONG,
    NAME_TAKEN,
    LAST_NOT_DONE,
    CATEGORY_ORDER,
    FIRST_PARALLEL,
    FAIL_TO_INVALID,
    FAIL_LIMIT_INVALID,
    ARTIFACT_NAME_INVALID,
    TEXT_TOO_LONG,
    MODE_NEEDS_PERSONA,
    GATE_DUPLICATE,
    TOOL_INVALID,
    LIST_TOO_LONG,
    PARALLEL_CATEGORY,
    PERSONA_INVALID,
];

/// Kategori stage = kolom status tiket M2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Backlog,
    Todo,
    InProgress,
    Done,
}

impl Category {
    pub const ALL: [Self; 4] = [Self::Backlog, Self::Todo, Self::InProgress, Self::Done];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Backlog => "backlog",
            Self::Todo => "todo",
            Self::InProgress => "in_progress",
            Self::Done => "done",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.as_str() == value.trim())
    }
}

/// Cara stage dikerjakan: manusia saja (tanpa persona), job otomatis di runner, atau sesi
/// interaktif dengan manusia (mis. brainstorming).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Manual,
    Auto,
    Interactive,
}

impl Mode {
    pub const ALL: [Self; 3] = [Self::Manual, Self::Auto, Self::Interactive];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Auto => "auto",
            Self::Interactive => "interactive",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.as_str() == value.trim())
    }
}

/// Mode transisi keluar stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Exit {
    /// Tiket "Menunggu review" sampai satu approver menyetujui; gate dicek saat itu.
    Approval,
    /// Approve dilewati, gate tetap dicek.
    Auto,
    /// Dipindah manusia lewat board; gate tetap dicek.
    #[default]
    Manual,
}

impl Exit {
    pub const ALL: [Self; 3] = [Self::Approval, Self::Auto, Self::Manual];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Approval => "approval",
            Self::Auto => "auto",
            Self::Manual => "manual",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|e| e.as_str() == value.trim())
    }
}

/// Tipe cek gate yang disediakan (bukan skrip bebas).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Gate {
    /// Artifact output stage ini punya minimal satu versi.
    ArtifactExists,
    /// Deskripsi tiket memuat checklist acceptance criteria (`- [ ]`).
    AcceptanceCriteria,
    /// Semua subtask sudah selesai.
    SubtasksDone,
    /// Evidence runner: screenshot.
    ScreenshotExists,
    /// Evidence runner: hasil test lulus.
    TestsPassed,
    /// Git: PR terbuka.
    PrOpen,
    /// Git: semua PR merged.
    PrsMerged,
    /// Git: CI hijau.
    CiGreen,
}

impl Gate {
    pub const ALL: [Self; 8] = [
        Self::ArtifactExists,
        Self::AcceptanceCriteria,
        Self::SubtasksDone,
        Self::ScreenshotExists,
        Self::TestsPassed,
        Self::PrOpen,
        Self::PrsMerged,
        Self::CiGreen,
    ];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ArtifactExists => "artifact_exists",
            Self::AcceptanceCriteria => "acceptance_criteria",
            Self::SubtasksDone => "subtasks_done",
            Self::ScreenshotExists => "screenshot_exists",
            Self::TestsPassed => "tests_passed",
            Self::PrOpen => "pr_open",
            Self::PrsMerged => "prs_merged",
            Self::CiGreen => "ci_green",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|g| g.as_str() == value.trim())
    }

    /// Gate ini hanya bisa dipenuhi evidence dari runner atau provider git (bukan klaim manusia).
    #[must_use]
    pub fn needs_evidence(self) -> bool {
        !matches!(
            self,
            Self::ArtifactExists | Self::AcceptanceCriteria | Self::SubtasksDone
        )
    }
}

/// Penugasan default stage (user id). Disalin ke tiket saat stage dimulai.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct People {
    #[serde(default)]
    pub executors: Vec<Uuid>,
    #[serde(default)]
    pub approvers: Vec<Uuid>,
    #[serde(default)]
    pub watchers: Vec<Uuid>,
}

impl People {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.executors.is_empty() && self.approvers.is_empty() && self.watchers.is_empty()
    }
}

fn default_fail_limit() -> u8 {
    FAIL_LIMIT_DEFAULT
}

/// Satu stage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stage {
    pub key: String,
    pub name: String,
    pub category: Category,
    /// Kunci persona workspace (format [`valid_key`]); `None` = dikerjakan manusia.
    #[serde(default)]
    pub persona: Option<String>,
    #[serde(default)]
    pub mode: Mode,
    /// Skill yang dipakai agent (teks bebas sampai skill pack M4).
    #[serde(default)]
    pub skill: String,
    #[serde(default)]
    pub prompt: String,
    /// Nama artifact dari stage sebelumnya yang dibaca stage ini.
    #[serde(default)]
    pub inputs: Vec<String>,
    /// Nama artifact yang dihasilkan stage ini (mis. `prd.md`); kosong = tanpa artifact.
    #[serde(default)]
    pub output: String,
    /// Isi awal Markdown artifact output.
    #[serde(default)]
    pub output_template: String,
    #[serde(default)]
    pub gates: Vec<Gate>,
    /// Tooling wajib di runner (mis. `Playwright CLI`).
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub exit: Exit,
    /// Batas putaran gagal sebelum tiket berstatus Butuh keputusan.
    #[serde(default = "default_fail_limit")]
    pub fail_limit: u8,
    /// Stage tujuan saat stage ini gagal/di-reject (mis. Testing → Development); `None` = tetap.
    #[serde(default)]
    pub fail_to: Option<String>,
    #[serde(default)]
    pub parallel: bool,
    #[serde(default)]
    pub people: People,
}

impl Stage {
    /// Stage kosong dengan nama dan kunci (default lainnya).
    #[must_use]
    pub fn new(key: &str, name: &str, category: Category) -> Self {
        Self {
            key: key.to_string(),
            name: name.to_string(),
            category,
            persona: None,
            mode: Mode::Manual,
            skill: String::new(),
            prompt: String::new(),
            inputs: Vec::new(),
            output: String::new(),
            output_template: String::new(),
            gates: Vec::new(),
            tools: Vec::new(),
            exit: Exit::Manual,
            fail_limit: FAIL_LIMIT_DEFAULT,
            fail_to: None,
            parallel: false,
            people: People::default(),
        }
    }

    /// Stage ini menahan tiket: punya gate atau menunggu approve. Melewati stage seperti ini
    /// tanpa menyelesaikannya adalah "lompat stage" (hanya Admin).
    #[must_use]
    pub fn enforces(&self) -> bool {
        !self.gates.is_empty() || self.exit == Exit::Approval
    }

    /// Trim teks dan samakan mode dengan persona (tanpa persona = manual).
    pub fn normalize(&mut self) {
        self.key = self.key.trim().to_string();
        self.name = self.name.trim().to_string();
        self.skill = self.skill.trim().to_string();
        self.prompt = self.prompt.trim().to_string();
        self.persona = self
            .persona
            .take()
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty());
        self.output = self.output.trim().to_string();
        self.output_template = self.output_template.trim_end().to_string();
        for list in [&mut self.inputs, &mut self.tools] {
            let mut out: Vec<String> = Vec::new();
            for item in list.iter().map(|s| s.trim()).filter(|s| !s.is_empty()) {
                if !out.iter().any(|o| o.eq_ignore_ascii_case(item)) {
                    out.push(item.to_string());
                }
            }
            *list = out;
        }
        self.fail_to = self
            .fail_to
            .take()
            .map(|f| f.trim().to_string())
            .filter(|f| !f.is_empty());
        if self.persona.is_none() {
            self.mode = Mode::Manual;
        }
        for list in [
            &mut self.people.executors,
            &mut self.people.approvers,
            &mut self.people.watchers,
        ] {
            let mut seen = Vec::new();
            list.retain(|id| {
                let fresh = !seen.contains(id);
                seen.push(*id);
                fresh
            });
        }
    }
}

/// Satu masalah validasi: stage ke-`stage` (atau seluruh workflow), field, dan kode Fluent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub stage: Option<usize>,
    pub field: &'static str,
    pub code: &'static str,
}

/// Definisi workflow.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Definition {
    pub stages: Vec<Stage>,
}

impl Definition {
    #[must_use]
    pub fn stage(&self, key: &str) -> Option<&Stage> {
        self.stages.iter().find(|s| s.key == key)
    }

    #[must_use]
    pub fn index_of(&self, key: &str) -> Option<usize> {
        self.stages.iter().position(|s| s.key == key)
    }

    #[must_use]
    pub fn first(&self) -> Option<&Stage> {
        self.stages.first()
    }

    /// Stage berikutnya setelah `key`.
    #[must_use]
    pub fn next_after(&self, key: &str) -> Option<&Stage> {
        self.index_of(key).and_then(|i| self.stages.get(i + 1))
    }

    /// Stage pertama berkategori `category`.
    #[must_use]
    pub fn first_in(&self, category: Category) -> Option<&Stage> {
        self.stages.iter().find(|s| s.category == category)
    }

    /// Stage `done` terakhir (tujuan tiket yang dibatalkan).
    #[must_use]
    pub fn final_stage(&self) -> Option<&Stage> {
        self.stages.last()
    }

    /// Kunci baru yang belum dipakai, diturunkan dari `name`.
    #[must_use]
    pub fn fresh_key(&self, name: &str) -> String {
        let base = slug(name);
        let base = if base.is_empty() {
            "stage".to_string()
        } else {
            base
        };
        if self.stage(&base).is_none() {
            return base;
        }
        (2..)
            .map(|n| format!("{base}_{n}"))
            .find(|k| self.stage(k).is_none())
            .unwrap_or(base)
    }

    pub fn normalize(&mut self) {
        for s in &mut self.stages {
            s.normalize();
        }
        if let Some(first) = self.stages.first_mut() {
            first.parallel = false;
        }
    }

    /// Validasi seluruh definisi. Semua masalah dikembalikan sekaligus.
    ///
    /// # Errors
    ///
    /// Daftar [`Issue`] bila ada aturan yang dilanggar.
    pub fn validate(&self) -> Result<(), Vec<Issue>> {
        let mut issues = Vec::new();
        let mut push = |stage: Option<usize>, field: &'static str, code: &'static str| {
            issues.push(Issue { stage, field, code });
        };
        if self.stages.is_empty() {
            push(None, "stages", STAGES_EMPTY);
        }
        if self.stages.len() > MAX_STAGES {
            push(None, "stages", STAGES_TOO_MANY);
        }
        for (i, s) in self.stages.iter().enumerate() {
            let at = Some(i);
            if !valid_key(&s.key) {
                push(at, "key", KEY_INVALID);
            } else if self.stages[..i].iter().any(|o| o.key == s.key) {
                push(at, "key", KEY_TAKEN);
            }
            if s.name.trim().is_empty() {
                push(at, "name", NAME_REQUIRED);
            } else if s.name.chars().count() > NAME_MAX_LEN || s.name.chars().any(char::is_control)
            {
                push(at, "name", NAME_TOO_LONG);
            } else if self.stages[..i]
                .iter()
                .any(|o| o.name.trim().to_lowercase() == s.name.trim().to_lowercase())
            {
                push(at, "name", NAME_TAKEN);
            }
            if i > 0 && s.category < self.stages[i - 1].category {
                push(at, "category", CATEGORY_ORDER);
            }
            if s.parallel {
                if i == 0 {
                    push(at, "parallel", FIRST_PARALLEL);
                } else if s.category != self.stages[i - 1].category {
                    push(at, "parallel", PARALLEL_CATEGORY);
                }
            }
            if s.persona.is_none() && s.mode != Mode::Manual {
                push(at, "mode", MODE_NEEDS_PERSONA);
            }
            if s.persona.as_deref().is_some_and(|p| !valid_key(p)) {
                push(at, "persona", PERSONA_INVALID);
            }
            if s.skill.chars().count() > SKILL_MAX_LEN {
                push(at, "skill", TEXT_TOO_LONG);
            }
            if s.prompt.chars().count() > PROMPT_MAX_LEN {
                push(at, "prompt", TEXT_TOO_LONG);
            }
            if s.output_template.chars().count() > TEMPLATE_MAX_LEN {
                push(at, "output_template", TEXT_TOO_LONG);
            }
            if !s.output.is_empty() && !valid_artifact_name(&s.output) {
                push(at, "output", ARTIFACT_NAME_INVALID);
            }
            if s.inputs.len() > LIST_MAX {
                push(at, "inputs", LIST_TOO_LONG);
            } else if s.inputs.iter().any(|n| !valid_artifact_name(n)) {
                push(at, "inputs", ARTIFACT_NAME_INVALID);
            }
            if s.tools.len() > LIST_MAX {
                push(at, "tools", LIST_TOO_LONG);
            } else if s
                .tools
                .iter()
                .any(|t| t.trim().is_empty() || t.chars().count() > TOOL_MAX_LEN || t.contains(','))
            {
                push(at, "tools", TOOL_INVALID);
            }
            if s.gates
                .iter()
                .enumerate()
                .any(|(gi, g)| s.gates[..gi].contains(g))
            {
                push(at, "gates", GATE_DUPLICATE);
            }
            if !(1..=FAIL_LIMIT_MAX).contains(&s.fail_limit) {
                push(at, "fail_limit", FAIL_LIMIT_INVALID);
            }
            if let Some(to) = &s.fail_to
                && !self.stages[..i].iter().any(|o| &o.key == to)
            {
                push(at, "fail_to", FAIL_TO_INVALID);
            }
            let people = &s.people;
            if people.executors.len() > LIST_MAX
                || people.approvers.len() > LIST_MAX
                || people.watchers.len() > LIST_MAX
            {
                push(at, "people", LIST_TOO_LONG);
            }
        }
        if let Some(last) = self.stages.last()
            && last.category != Category::Done
        {
            push(Some(self.stages.len() - 1), "category", LAST_NOT_DONE);
        }
        if issues.is_empty() {
            Ok(())
        } else {
            Err(issues)
        }
    }
}

/// Kunci stage: huruf kecil ASCII, angka, `_`, `-`; diawali huruf.
#[must_use]
pub fn valid_key(key: &str) -> bool {
    let mut chars = key.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase())
        && key.len() <= KEY_MAX_LEN
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

/// Nama artifact: berkas tanpa folder (`prd.md`, `mockup-bayar.html`).
#[must_use]
pub fn valid_artifact_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().count() <= ARTIFACT_NAME_MAX_LEN
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

/// Kunci dari nama: huruf kecil ASCII, selain huruf/angka menjadi `_`.
#[must_use]
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.trim().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('_') && !out.is_empty() {
            out.push('_');
        }
    }
    let out = out.trim_end_matches('_').to_string();
    let out: String = out
        .trim_start_matches(|c: char| c.is_ascii_digit() || c == '_')
        .chars()
        .take(KEY_MAX_LEN)
        .collect();
    out.trim_end_matches('_').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple() -> Definition {
        Definition {
            stages: vec![
                Stage::new("todo", "To do", Category::Todo),
                Stage::new("dev", "Development", Category::InProgress),
                Stage::new("done", "Selesai", Category::Done),
            ],
        }
    }

    fn codes(d: &Definition) -> Vec<&'static str> {
        d.validate()
            .err()
            .unwrap_or_default()
            .iter()
            .map(|i| i.code)
            .collect()
    }

    #[test]
    fn simple_definition_is_valid() {
        assert!(simple().validate().is_ok());
    }

    #[test]
    fn empty_and_last_not_done_rejected() {
        assert_eq!(codes(&Definition::default()), vec![STAGES_EMPTY]);
        let mut d = simple();
        d.stages.pop();
        assert_eq!(codes(&d), vec![LAST_NOT_DONE]);
    }

    #[test]
    fn keys_and_names_must_be_unique_and_valid() {
        let mut d = simple();
        d.stages[1].key = "todo".into();
        d.stages[2].name = "TO DO".into();
        let c = codes(&d);
        assert!(c.contains(&KEY_TAKEN) && c.contains(&NAME_TAKEN), "{c:?}");
        d.stages[1].key = "1abc".into();
        assert!(codes(&d).contains(&KEY_INVALID));
    }

    #[test]
    fn category_cannot_go_backwards() {
        let mut d = simple();
        d.stages[1].category = Category::Backlog;
        assert_eq!(codes(&d), vec![CATEGORY_ORDER]);
    }

    #[test]
    fn fail_to_must_point_backwards() {
        let mut d = simple();
        d.stages[1].fail_to = Some("done".into());
        assert_eq!(codes(&d), vec![FAIL_TO_INVALID]);
        d.stages[1].fail_to = Some("todo".into());
        assert!(d.validate().is_ok());
        d.stages[1].fail_limit = 0;
        assert_eq!(codes(&d), vec![FAIL_LIMIT_INVALID]);
    }

    #[test]
    fn mode_needs_persona_and_normalize_fixes_it() {
        let mut d = simple();
        d.stages[1].mode = Mode::Auto;
        assert_eq!(codes(&d), vec![MODE_NEEDS_PERSONA]);
        d.normalize();
        assert!(d.validate().is_ok());
    }

    #[test]
    fn persona_is_a_key_and_bad_keys_are_rejected() {
        let mut d = simple();
        d.stages[1].persona = Some("security-auditor".into());
        d.stages[1].mode = Mode::Auto;
        assert!(d.validate().is_ok());
        d.stages[1].persona = Some("Bukan Kunci".into());
        assert_eq!(codes(&d), vec![PERSONA_INVALID]);
        d.stages[1].persona = Some("  ".into());
        d.normalize();
        assert_eq!(d.stages[1].persona, None);
        assert_eq!(d.stages[1].mode, Mode::Manual);
    }

    #[test]
    fn parallel_rules() {
        let mut d = simple();
        d.stages[0].parallel = true;
        assert_eq!(codes(&d), vec![FIRST_PARALLEL]);
        d.normalize();
        d.stages
            .insert(2, Stage::new("arch", "Architect", Category::InProgress));
        d.stages[2].parallel = true;
        assert!(d.validate().is_ok());
        d.stages[2].parallel = false;
        d.stages[3].parallel = true;
        assert_eq!(codes(&d), vec![PARALLEL_CATEGORY]);
    }

    #[test]
    fn artifact_names_and_gates() {
        let mut d = simple();
        d.stages[1].output = "../prd.md".into();
        d.stages[1].gates = vec![Gate::PrOpen, Gate::PrOpen];
        let c = codes(&d);
        assert!(c.contains(&ARTIFACT_NAME_INVALID) && c.contains(&GATE_DUPLICATE));
        assert!(valid_artifact_name("mockup-bayar.html"));
    }

    #[test]
    fn enforces_means_gate_or_approval() {
        let mut s = Stage::new("a", "A", Category::Todo);
        assert!(!s.enforces());
        s.exit = Exit::Approval;
        assert!(s.enforces());
        s.exit = Exit::Auto;
        s.gates.push(Gate::ArtifactExists);
        assert!(s.enforces());
        assert!(!Gate::ArtifactExists.needs_evidence());
        assert!(Gate::CiGreen.needs_evidence());
    }

    #[test]
    fn slug_and_fresh_key() {
        assert_eq!(slug("Desain UI / UX"), "desain_ui_ux");
        assert_eq!(slug("  123 Tes "), "tes");
        let d = simple();
        assert_eq!(d.fresh_key("Development"), "development");
        assert_eq!(d.fresh_key("Dev"), "dev_2");
        assert_eq!(d.fresh_key("***"), "stage");
    }

    #[test]
    fn minimal_json_uses_defaults() {
        let d: Definition = serde_json::from_str(
            r#"{"stages":[{"key":"done","name":"Selesai","category":"done"}]}"#,
        )
        .unwrap_or_default();
        assert_eq!(d.stages.len(), 1);
        assert_eq!(d.stages[0].fail_limit, FAIL_LIMIT_DEFAULT);
        assert_eq!(d.stages[0].exit, Exit::Manual);
    }
}
