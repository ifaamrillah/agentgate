//! Dokumen YAML workflow (impor/ekspor, template bawaan).
//!
//! Kunci berbahasa Indonesia mengikuti prototype (`workflow`, `nama`, `stages`, `transisi`, …).
//! Nilai enum memakai id yang sama dengan penyimpanan (`kategori: in_progress`,
//! `gate: [artifact_exists]`), kecuali `mode` dan `transisi` yang memakai kata Indonesia
//! (`otomatis`, `interaktif`, `tunggu_approve`, `manual`).
//!
//! Penugasan default berisi **email** anggota (id user tidak bermakna di workspace lain). Saat
//! impor, email yang bukan anggota diabaikan dan dilaporkan sebagai peringatan.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::definition::{
    Category, Definition, Exit, FAIL_LIMIT_DEFAULT, Gate, Mode, People, Stage, slug, valid_key,
};

pub const SYNTAX: &str = "workflow-yaml-syntax";
pub const UNKNOWN_VALUE: &str = "workflow-yaml-unknown-value";
pub const KEY_INVALID: &str = "workflow-yaml-key-invalid";
pub const NAME_REQUIRED: &str = "workflow-yaml-name-required";
pub const ERROR_CODES: &[&str] = &[SYNTAX, UNKNOWN_VALUE, KEY_INVALID, NAME_REQUIRED];

/// Workflow sebagai dokumen: identitas + definisi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    /// Kunci workflow (slug), mis. `superpowers`.
    pub key: String,
    pub name: String,
    pub description: String,
    pub definition: Definition,
}

/// YAML tidak bisa dibaca. `detail` = pesan parser atau nilai yang tidak dikenal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YamlError {
    pub code: &'static str,
    pub detail: String,
}

impl YamlError {
    fn unknown(field: &str, value: &str) -> Self {
        Self {
            code: UNKNOWN_VALUE,
            detail: format!("{field}: {value}"),
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Doc {
    #[serde(default)]
    workflow: String,
    #[serde(default)]
    nama: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    deskripsi: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    versi: Option<i64>,
    #[serde(default)]
    stages: Vec<StageDoc>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StageDoc {
    id: String,
    nama: String,
    kategori: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    persona: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    mode: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    skill: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    prompt: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    input: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    output: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    template_output: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    gate: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tooling: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    transisi: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    batas_gagal: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    gagal_ke: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    paralel: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    penugasan: Option<PeopleDoc>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PeopleDoc {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    executor: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    approver: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    watcher: Vec<String>,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_false(b: &bool) -> bool {
    !*b
}

fn mode_word(mode: Mode) -> &'static str {
    match mode {
        Mode::Manual => "manual",
        Mode::Auto => "otomatis",
        Mode::Interactive => "interaktif",
    }
}

fn exit_word(exit: Exit) -> &'static str {
    match exit {
        Exit::Approval => "tunggu_approve",
        Exit::Auto => "otomatis",
        Exit::Manual => "manual",
    }
}

/// Baca dokumen YAML. `member` memetakan email ke id user anggota workspace. Mengembalikan
/// dokumen (sudah dinormalisasi, belum divalidasi: panggil `definition.validate()`) dan daftar
/// email yang diabaikan.
///
/// # Errors
///
/// [`YamlError`] bila sintaks salah, ada kunci/nilai yang tidak dikenal, atau kunci/nama
/// workflow kosong.
pub fn parse(
    text: &str,
    member: impl Fn(&str) -> Option<Uuid>,
) -> Result<(Document, Vec<String>), YamlError> {
    let doc: Doc = serde_yaml_ng::from_str(text).map_err(|e| YamlError {
        code: SYNTAX,
        detail: e.to_string(),
    })?;
    let name = doc.nama.trim().to_string();
    if name.is_empty() {
        return Err(YamlError {
            code: NAME_REQUIRED,
            detail: String::new(),
        });
    }
    let key = match doc.workflow.trim() {
        "" => slug(&name),
        k => k.to_string(),
    };
    if !valid_key(&key) {
        return Err(YamlError {
            code: KEY_INVALID,
            detail: key,
        });
    }
    let mut ignored = Vec::new();
    let mut stages = Vec::with_capacity(doc.stages.len());
    for s in doc.stages {
        let category = Category::parse(&s.kategori)
            .ok_or_else(|| YamlError::unknown("kategori", &s.kategori))?;
        let persona = match s.persona.as_deref().map(str::trim) {
            None | Some("") => None,
            Some(p) if valid_key(p) => Some(p.to_string()),
            Some(p) => return Err(YamlError::unknown("persona", p)),
        };
        let mode = match s.mode.trim() {
            "" if persona.is_some() => Mode::Auto,
            "" | "manual" => Mode::Manual,
            "otomatis" => Mode::Auto,
            "interaktif" => Mode::Interactive,
            other => return Err(YamlError::unknown("mode", other)),
        };
        let exit = match s.transisi.trim() {
            "" | "manual" => Exit::Manual,
            "otomatis" => Exit::Auto,
            "tunggu_approve" => Exit::Approval,
            other => return Err(YamlError::unknown("transisi", other)),
        };
        let mut gates = Vec::new();
        for g in &s.gate {
            gates.push(Gate::parse(g).ok_or_else(|| YamlError::unknown("gate", g))?);
        }
        let mut resolve = |emails: &[String]| -> Vec<Uuid> {
            emails
                .iter()
                .filter_map(|e| {
                    let id = member(e.trim());
                    if id.is_none() && !ignored.contains(e) {
                        ignored.push(e.clone());
                    }
                    id
                })
                .collect()
        };
        let people = s.penugasan.map_or_else(People::default, |p| People {
            executors: resolve(&p.executor),
            approvers: resolve(&p.approver),
            watchers: resolve(&p.watcher),
        });
        stages.push(Stage {
            key: s.id,
            name: s.nama,
            category,
            persona,
            mode,
            skill: s.skill,
            prompt: s.prompt,
            inputs: s.input,
            output: s.output,
            output_template: s.template_output,
            gates,
            tools: s.tooling,
            exit,
            fail_limit: s.batas_gagal.unwrap_or(FAIL_LIMIT_DEFAULT),
            fail_to: s.gagal_ke,
            parallel: s.paralel,
            people,
        });
    }
    let mut definition = Definition { stages };
    definition.normalize();
    Ok((
        Document {
            key,
            name,
            description: doc.deskripsi.trim().to_string(),
            definition,
        },
        ignored,
    ))
}

/// Tulis dokumen sebagai YAML. `email` memetakan id user ke email (user yang sudah tidak ada
/// dilewati).
#[must_use]
pub fn emit(
    document: &Document,
    version: Option<i64>,
    email: impl Fn(Uuid) -> Option<String>,
) -> String {
    let emails = |ids: &[Uuid]| -> Vec<String> { ids.iter().filter_map(|id| email(*id)).collect() };
    let doc = Doc {
        workflow: document.key.clone(),
        nama: document.name.clone(),
        deskripsi: document.description.clone(),
        versi: version,
        stages: document
            .definition
            .stages
            .iter()
            .map(|s| StageDoc {
                id: s.key.clone(),
                nama: s.name.clone(),
                kategori: s.category.as_str().to_string(),
                persona: s.persona.clone(),
                mode: if s.persona.is_some() {
                    mode_word(s.mode).to_string()
                } else {
                    String::new()
                },
                skill: s.skill.clone(),
                prompt: s.prompt.clone(),
                input: s.inputs.clone(),
                output: s.output.clone(),
                template_output: s.output_template.clone(),
                gate: s.gates.iter().map(|g| g.as_str().to_string()).collect(),
                tooling: s.tools.clone(),
                transisi: exit_word(s.exit).to_string(),
                batas_gagal: (s.fail_limit != FAIL_LIMIT_DEFAULT).then_some(s.fail_limit),
                gagal_ke: s.fail_to.clone(),
                paralel: s.parallel,
                penugasan: (!s.people.is_empty()).then(|| PeopleDoc {
                    executor: emails(&s.people.executors),
                    approver: emails(&s.people.approvers),
                    watcher: emails(&s.people.watchers),
                }),
            })
            .collect(),
    };
    serde_yaml_ng::to_string(&doc).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "workflow: mobile-release
nama: Rilis mobile
stages:
  - id: spec
    nama: Spec
    kategori: todo
    persona: pm
    output: spec.md
    gate: [artifact_exists]
    transisi: tunggu_approve
    penugasan:
      approver: [olivia@contoh.test, hilang@contoh.test]
  - id: build
    nama: Build
    kategori: in_progress
    persona: dev
    gate: [pr_open, tests_passed]
    batas_gagal: 2
    gagal_ke: spec
  - id: done
    nama: Selesai
    kategori: done
";

    fn olivia() -> Uuid {
        Uuid::from_u128(7)
    }

    #[test]
    fn parses_sample_and_reports_unknown_emails() {
        let (doc, ignored) = parse(SAMPLE, |e| (e == "olivia@contoh.test").then(olivia))
            .unwrap_or_else(|e| panic!("{e:?}"));
        assert_eq!(doc.key, "mobile-release");
        assert_eq!(doc.definition.stages.len(), 3);
        let spec = &doc.definition.stages[0];
        assert_eq!(spec.exit, Exit::Approval);
        assert_eq!(spec.mode, Mode::Auto);
        assert_eq!(spec.people.approvers, vec![olivia()]);
        assert_eq!(ignored, vec!["hilang@contoh.test".to_string()]);
        assert_eq!(doc.definition.stages[1].fail_limit, 2);
        assert!(doc.definition.validate().is_ok());
    }

    #[test]
    fn round_trip_keeps_definition() {
        let (doc, _) = parse(SAMPLE, |_| Some(olivia())).unwrap_or_else(|e| panic!("{e:?}"));
        let text = emit(&doc, Some(4), |_| Some("olivia@contoh.test".into()));
        assert!(text.contains("versi: 4"), "{text}");
        let (again, _) = parse(&text, |_| Some(olivia())).unwrap_or_else(|e| panic!("{e:?}"));
        assert_eq!(again, doc);
    }

    #[test]
    fn errors_have_codes() {
        let err = |t: &str| parse(t, |_| None).err().map(|e| e.code);
        assert_eq!(err("nama: [tidak ditutup"), Some(SYNTAX));
        assert_eq!(err("nama: X\nasing: 1"), Some(SYNTAX));
        assert_eq!(err("workflow: x\nstages: []"), Some(NAME_REQUIRED));
        assert_eq!(err("workflow: 'X Y'\nnama: A"), Some(KEY_INVALID));
        assert_eq!(
            err("nama: A\nstages:\n  - {id: a, nama: A, kategori: selesai}"),
            Some(UNKNOWN_VALUE)
        );
        assert_eq!(
            err("nama: A\nstages:\n  - {id: a, nama: A, kategori: done, gate: [sihir]}"),
            Some(UNKNOWN_VALUE)
        );
    }

    #[test]
    fn custom_persona_key_round_trips_and_bad_key_is_rejected() {
        let text = "nama: A\nstages:\n  - {id: a, nama: A, kategori: todo, persona: security-auditor}\n  - {id: b, nama: B, kategori: done}";
        let (doc, _) = parse(text, |_| None).unwrap_or_else(|e| panic!("{e:?}"));
        assert_eq!(
            doc.definition.stages[0].persona.as_deref(),
            Some("security-auditor")
        );
        let again = emit(&doc, None, |_| None);
        assert!(again.contains("persona: security-auditor"), "{again}");
        let bad = "nama: A\nstages:\n  - {id: a, nama: A, kategori: done, persona: 'Bukan Kunci'}";
        assert_eq!(
            parse(bad, |_| None).err().map(|e| e.code),
            Some(UNKNOWN_VALUE)
        );
    }

    #[test]
    fn key_defaults_to_slug_of_name() {
        let (doc, _) =
            parse("nama: Bug Triage Tim\nstages: []", |_| None).unwrap_or_else(|e| panic!("{e:?}"));
        assert_eq!(doc.key, "bug_triage_tim");
    }
}
