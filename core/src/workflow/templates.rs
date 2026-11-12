//! Template workflow bawaan (spec: Scrum, Kanban, BMAD, Spec-driven, Shape Up, Bug Triage,
//! Waterfall, Superpowers-style, Hotfix/Incident) dan workflow "Subtask ringkas" yang dibuat untuk
//! setiap workspace baru. Isinya berkas YAML di `core/templates/`, dibaca dengan parser impor yang
//! sama, sehingga template sekaligus menjadi contoh format YAML.

use super::yaml::{self, Document};

/// Satu template bawaan.
#[derive(Debug, Clone, Copy)]
pub struct Template {
    pub key: &'static str,
    pub yaml: &'static str,
}

impl Template {
    /// Dokumen template. Template bawaan selalu valid (dijaga tes); bila tidak, hasilnya
    /// dokumen kosong.
    #[must_use]
    pub fn document(&self) -> Document {
        yaml::parse(self.yaml, |_| None).map_or_else(
            |_| Document {
                key: self.key.to_string(),
                name: self.key.to_string(),
                description: String::new(),
                definition: super::Definition::default(),
            },
            |(doc, _)| doc,
        )
    }
}

/// Sembilan template yang bisa di-clone, urut tampil.
pub const TEMPLATES: [Template; 9] = [
    Template {
        key: "scrum",
        yaml: include_str!("../../templates/scrum.yaml"),
    },
    Template {
        key: "kanban",
        yaml: include_str!("../../templates/kanban.yaml"),
    },
    Template {
        key: "bmad",
        yaml: include_str!("../../templates/bmad.yaml"),
    },
    Template {
        key: "spec_driven",
        yaml: include_str!("../../templates/spec_driven.yaml"),
    },
    Template {
        key: "shape_up",
        yaml: include_str!("../../templates/shape_up.yaml"),
    },
    Template {
        key: "bug_triage",
        yaml: include_str!("../../templates/bug_triage.yaml"),
    },
    Template {
        key: "waterfall",
        yaml: include_str!("../../templates/waterfall.yaml"),
    },
    Template {
        key: "superpowers",
        yaml: include_str!("../../templates/superpowers.yaml"),
    },
    Template {
        key: "hotfix",
        yaml: include_str!("../../templates/hotfix.yaml"),
    },
];

/// Workflow ringkas untuk tipe Subtask (spec: "Dev → Review → Done").
pub const SUBTASK: Template = Template {
    key: "subtask",
    yaml: include_str!("../../templates/subtask.yaml"),
};

/// Template dengan kunci `key`.
#[must_use]
pub fn find(key: &str) -> Option<Template> {
    TEMPLATES.iter().copied().find(|t| t.key == key)
}

/// Kanban: workflow bawaan setiap tipe selain Subtask.
#[must_use]
pub fn kanban() -> Template {
    TEMPLATES[1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_templates_parse_and_validate() {
        for t in TEMPLATES.iter().chain(std::iter::once(&SUBTASK)) {
            let (doc, ignored) =
                yaml::parse(t.yaml, |_| None).unwrap_or_else(|e| panic!("{}: {e:?}", t.key));
            assert_eq!(doc.key, t.key);
            assert!(ignored.is_empty());
            if let Err(issues) = doc.definition.validate() {
                panic!("{}: {issues:?}", t.key);
            }
        }
    }

    #[test]
    fn kanban_keys_match_ticket_statuses() {
        let keys: Vec<String> = kanban()
            .document()
            .definition
            .stages
            .into_iter()
            .map(|s| s.key)
            .collect();
        assert_eq!(keys, ["backlog", "todo", "in_progress", "done"]);
    }

    #[test]
    fn nine_distinct_templates() {
        let mut keys: Vec<&str> = TEMPLATES.iter().map(|t| t.key).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), 9);
        assert!(find("hotfix").is_some() && find("subtask").is_none());
    }
}
