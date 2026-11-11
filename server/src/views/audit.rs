//! Pengaturan → Audit log: tabel kejadian (waktu WIB, pelaku, aksi, subjek, ringkasan
//! perubahan), filter kelompok aksi, dan link "Lebih lama" (keyset).
//!
//! Ringkasan perubahan hanya menampilkan field yang dikenal ([`SUMMARY_FIELDS`]), masing-masing
//! dengan label Fluent: id (UUID) dan daftar (izin, grant) tidak berguna dibaca sekilas.
use loco_rs::prelude::*;
use sea_orm::ActiveEnum;
use serde::Serialize;

use super::{
    layout::{AppShell, app_page},
    time,
};
use crate::models::audit_events::{self, AuditAction, AuditGroup, AuditSubject};

/// Field `changes` yang diringkas, urut tampil, beserta kunci label Fluent-nya.
const SUMMARY_FIELDS: &[(&str, &str)] = &[
    ("name", "audit-field-name"),
    ("slug", "audit-field-slug"),
    ("email", "audit-field-email"),
    ("role", "audit-field-role"),
    ("job_title", "audit-field-job-title"),
    ("key", "audit-field-key"),
    ("description", "audit-field-description"),
    ("provider", "audit-field-provider"),
    ("url", "audit-field-url"),
    ("default_branch", "audit-field-default-branch"),
    ("enabled", "audit-field-enabled"),
    ("resent", "audit-field-resent"),
];
const VALUE_YES: &str = "audit-value-yes";
const VALUE_NO: &str = "audit-value-no";
/// Teks panjang (deskripsi) dipotong di ringkasan.
const SUMMARY_MAX_CHARS: usize = 80;

/// Kunci Fluent yang dikirim sebagai variabel ke template (dijaga tes i18n).
pub const MESSAGE_KEYS: &[&str] = &[
    "audit-field-name",
    "audit-field-slug",
    "audit-field-email",
    "audit-field-role",
    "audit-field-job-title",
    "audit-field-key",
    "audit-field-description",
    "audit-field-provider",
    "audit-field-url",
    "audit-field-default-branch",
    "audit-field-enabled",
    "audit-field-resent",
    VALUE_YES,
    VALUE_NO,
];

/// `audit-action-member-role-changed` untuk `member.role_changed`.
#[must_use]
pub fn action_key(action: AuditAction) -> String {
    format!(
        "audit-action-{}",
        action.to_value().replace(['.', '_'], "-")
    )
}

/// `audit-subject-project-grant` untuk `project_grant`.
#[must_use]
pub fn subject_key(subject: AuditSubject) -> String {
    format!("audit-subject-{}", subject.to_value().replace('_', "-"))
}

#[must_use]
pub fn group_key(group: AuditGroup) -> String {
    format!("audit-group-{}", group.as_str())
}

/// Satu nilai di ringkasan: teks apa adanya, atau kunci Fluent (`ya`/`tidak`).
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SummaryValue {
    pub text: String,
    pub key: Option<&'static str>,
}

/// Satu field di ringkasan: `label` (kunci Fluent), dan `from` → `to` untuk perubahan atau
/// hanya `to` untuk data kejadian.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SummaryItem {
    pub label: &'static str,
    pub from: Option<SummaryValue>,
    pub to: SummaryValue,
}

/// Satu baris tabel.
#[derive(Debug, Serialize)]
pub struct AuditRow {
    pub id: String,
    pub action: String,
    pub action_key: String,
    pub datetime: String,
    pub time: String,
    /// Nama pelaku; `None` = sistem (atau user yang sudah dihapus).
    pub actor: Option<String>,
    pub subject_key: String,
    pub summary: Vec<SummaryItem>,
    /// Path halaman ini dengan baris ini terpilih (panel detail), diisi controller.
    pub href: String,
}

impl AuditRow {
    /// Baris dari kejadian. Aksi/subjek yang tidak dikenal (CHECK constraint dilewati) tetap
    /// ditampilkan dengan nilai mentahnya sebagai penanda, bukan menggagalkan halaman.
    #[must_use]
    pub fn new(event: &audit_events::Model, actor: Option<&str>) -> Self {
        Self {
            id: event.id.to_string(),
            action: event.action.clone(),
            action_key: event.action().map_or_else(|_| String::new(), action_key),
            datetime: event.created_at.to_rfc3339(),
            time: time::wib_datetime(&event.created_at),
            actor: actor.map(str::to_string),
            subject_key: event.subject().map_or_else(|_| String::new(), subject_key),
            summary: summarize(&event.changes),
            href: String::new(),
        }
    }
}

/// Pilihan filter kelompok.
#[derive(Debug, Serialize)]
pub struct GroupOption {
    pub value: &'static str,
    pub label: String,
    pub selected: bool,
}

/// Semua kelompok, `selected` menandai filter aktif.
#[must_use]
pub fn group_options(selected: Option<AuditGroup>) -> Vec<GroupOption> {
    use sea_orm::Iterable;
    AuditGroup::iter()
        .map(|g| GroupOption {
            value: g.as_str(),
            label: group_key(g),
            selected: Some(g) == selected,
        })
        .collect()
}

/// Ringkasan `changes`: field dikenal saja, urut [`SUMMARY_FIELDS`].
#[must_use]
pub fn summarize(changes: &serde_json::Value) -> Vec<SummaryItem> {
    SUMMARY_FIELDS
        .iter()
        .filter_map(|(field, label)| {
            let value = changes.get(field)?;
            match value {
                serde_json::Value::Object(change) if change.contains_key("to") => {
                    Some(SummaryItem {
                        label,
                        from: change.get("from").map(display),
                        to: display(change.get("to")?),
                    })
                }
                serde_json::Value::Object(_) | serde_json::Value::Array(_) => None,
                scalar => Some(SummaryItem {
                    label,
                    from: None,
                    to: display(scalar),
                }),
            }
        })
        .collect()
}

fn display(value: &serde_json::Value) -> SummaryValue {
    let text = |text: String| SummaryValue { text, key: None };
    match value {
        serde_json::Value::Bool(b) => SummaryValue {
            text: String::new(),
            key: Some(if *b { VALUE_YES } else { VALUE_NO }),
        },
        serde_json::Value::String(s) if s.chars().count() > SUMMARY_MAX_CHARS => text(format!(
            "{}…",
            s.chars().take(SUMMARY_MAX_CHARS).collect::<String>()
        )),
        serde_json::Value::String(s) => text(s.clone()),
        serde_json::Value::Null => text("—".to_string()),
        other => text(other.to_string()),
    }
}

/// Data halaman audit.
#[derive(Debug, Serialize)]
pub struct AuditPageView<'a> {
    pub rows: &'a [AuditRow],
    /// Id baris yang detailnya ditampilkan di panel (`?event=`, default baris pertama); `""`
    /// bila tidak ada baris.
    pub selected: &'a str,
    pub groups: &'a [GroupOption],
    /// Path halaman (form filter, link "Semua").
    pub action: &'a str,
    /// Path halaman berikutnya (lebih lama), `None` di halaman terakhir.
    pub older_href: Option<String>,
    /// Path halaman pertama dengan filter yang sama; `None` di halaman pertama.
    pub newest_href: Option<String>,
}

pub fn index(
    v: &impl ViewRenderer,
    shell: &AppShell,
    page: &AuditPageView<'_>,
) -> Result<Response> {
    format::render().view(v, "workspaces/audit.html", app_page(shell, page))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_keeps_known_fields_in_order_and_formats_changes() {
        let changes = serde_json::json!({
            "user_id": "0193",
            "role": { "from": "Member", "to": "Admin" },
            "name": "Budi",
            "permissions": ["manage_members"],
            "enabled": { "from": false, "to": true },
            "job_title": { "from": null, "to": "QA" },
        });
        let summary = summarize(&changes);
        let labels: Vec<_> = summary.iter().map(|s| s.label).collect();
        assert_eq!(
            labels,
            [
                "audit-field-name",
                "audit-field-role",
                "audit-field-job-title",
                "audit-field-enabled"
            ]
        );
        assert_eq!(summary[1].from.as_ref().unwrap().text, "Member");
        assert_eq!(summary[1].to.text, "Admin");
        assert_eq!(summary[2].from.as_ref().unwrap().text, "—");
        assert_eq!(summary[3].to.key, Some(VALUE_YES));
        assert_eq!(summary[3].from.as_ref().unwrap().key, Some(VALUE_NO));
    }

    #[test]
    fn long_text_is_truncated() {
        let long = "x".repeat(200);
        let summary = summarize(&serde_json::json!({ "description": long }));
        assert_eq!(summary[0].to.text.chars().count(), SUMMARY_MAX_CHARS + 1);
    }

    #[test]
    fn keys_follow_action_and_subject_values() {
        assert_eq!(
            action_key(AuditAction::MemberRoleChanged),
            "audit-action-member-role-changed"
        );
        assert_eq!(
            action_key(AuditAction::WorkspaceRequire2faChanged),
            "audit-action-workspace-require-2fa-changed"
        );
        assert_eq!(
            subject_key(AuditSubject::ProjectGrant),
            "audit-subject-project-grant"
        );
    }
}
