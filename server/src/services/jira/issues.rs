//! Nilai field issue Jira ↔ tiket Agent Gate (M11), lewat [`Snapshot`] bersama.
//!
//! Field yang disinkronkan dua arah: judul, deskripsi, label, prioritas, due date, story point
//! (sesuai peta field). Sprint, Fix Version/s, Components, dan field custom hanya dibaca saat
//! impor sekali.

use sea_orm::ActiveEnum;
use serde_json::{Map, Value, json};

use super::api::{self, JiraIssue};
use crate::{
    models::{
        jira_connections::{FieldMap, FieldTarget, JiraEnv},
        jira_links::Snapshot,
        tickets,
    },
    services::imports::fields as import_fields,
};

/// Field yang diminta dari Jira untuk issue (pencarian/ambil).
#[must_use]
pub fn wanted(fm: &FieldMap) -> Vec<String> {
    let mut out: Vec<String> = [
        "summary",
        "status",
        "updated",
        "created",
        "issuetype",
        "assignee",
        "attachment",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    for f in &fm.fields {
        if f.target != FieldTarget::Skip && !out.contains(&f.id) {
            out.push(f.id.clone());
        }
    }
    out
}

fn str_at(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Object(o) => o
            .get("value")
            .or_else(|| o.get("name"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        _ => String::new(),
    }
}

fn field<'a>(issue: &'a JiraIssue, fm: &FieldMap, target: FieldTarget) -> Option<&'a Value> {
    fm.jira_field(target).and_then(|id| issue.fields.get(id))
}

/// Kunci prioritas Agent Gate dari nama prioritas Jira (tak dikenal → `medium`).
#[must_use]
pub fn priority_key(name: &str) -> String {
    import_fields::priority(name)
        .unwrap_or(tickets::TicketPriority::Medium)
        .to_value()
}

/// Nama prioritas Jira untuk kunci Agent Gate.
#[must_use]
pub fn priority_name(key: &str) -> &'static str {
    match key {
        "urgent" => "Highest",
        "high" => "High",
        "low" => "Low",
        _ => "Medium",
    }
}

/// Snapshot issue Jira (field yang tidak dipetakan dibiarkan kosong).
#[must_use]
pub fn of_issue(issue: &JiraIssue, fm: &FieldMap) -> Snapshot {
    let mut s = Snapshot {
        title: field(issue, fm, FieldTarget::Title)
            .map(str_at)
            .unwrap_or_default(),
        description: field(issue, fm, FieldTarget::Description)
            .map(api::body_in)
            .unwrap_or_default(),
        labels: field(issue, fm, FieldTarget::Labels)
            .and_then(Value::as_array)
            .map(|a| a.iter().map(str_at).filter(|l| !l.is_empty()).collect())
            .unwrap_or_default(),
        priority: field(issue, fm, FieldTarget::Priority)
            .map(|p| priority_key(&str_at(p)))
            .unwrap_or_else(|| "medium".into()),
        due_on: field(issue, fm, FieldTarget::DueOn)
            .map(str_at)
            .filter(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").is_ok()),
        story_points: field(issue, fm, FieldTarget::StoryPoints)
            .and_then(Value::as_f64)
            .map(|p| {
                #[allow(clippy::cast_possible_truncation)]
                let n = p.round().clamp(0.0, 1000.0) as i32;
                n
            }),
    };
    if s.title.is_empty() {
        s.title = issue
            .fields
            .get("summary")
            .map(str_at)
            .unwrap_or_default();
    }
    s.normalized()
}

/// Snapshot tiket.
#[must_use]
pub fn of_ticket(t: &tickets::Model) -> Snapshot {
    Snapshot {
        title: t.title.clone(),
        description: t.description.clone(),
        labels: t.labels.clone(),
        priority: t.priority.clone(),
        due_on: t.due_on.map(|d| d.format("%Y-%m-%d").to_string()),
        story_points: t.story_points,
    }
    .normalized()
}

/// Target sinkron dua arah.
pub const SYNCED: [FieldTarget; 6] = [
    FieldTarget::Title,
    FieldTarget::Description,
    FieldTarget::Labels,
    FieldTarget::Priority,
    FieldTarget::DueOn,
    FieldTarget::StoryPoints,
];

/// Nilai satu target sebagai JSON (pembanding).
fn value(s: &Snapshot, t: FieldTarget) -> Value {
    match t {
        FieldTarget::Title => json!(s.title),
        FieldTarget::Description => json!(s.description),
        FieldTarget::Labels => json!(s.labels),
        FieldTarget::Priority => json!(s.priority),
        FieldTarget::DueOn => json!(s.due_on),
        FieldTarget::StoryPoints => json!(s.story_points),
        _ => Value::Null,
    }
}

fn set(s: &mut Snapshot, t: FieldTarget, from: &Snapshot) {
    match t {
        FieldTarget::Title => s.title.clone_from(&from.title),
        FieldTarget::Description => s.description.clone_from(&from.description),
        FieldTarget::Labels => s.labels.clone_from(&from.labels),
        FieldTarget::Priority => s.priority.clone_from(&from.priority),
        FieldTarget::DueOn => s.due_on.clone_from(&from.due_on),
        FieldTarget::StoryPoints => s.story_points = from.story_points,
        _ => {}
    }
}

/// Target dipetakan yang nilainya berbeda antara `a` dan `b`.
#[must_use]
pub fn differing(fm: &FieldMap, a: &Snapshot, b: &Snapshot) -> Vec<FieldTarget> {
    SYNCED
        .into_iter()
        .filter(|t| fm.maps(*t) && value(a, *t) != value(b, *t))
        .collect()
}

/// Field Jira untuk target `targets` dari snapshot `s`.
#[must_use]
pub fn jira_fields(env: JiraEnv, fm: &FieldMap, s: &Snapshot, targets: &[FieldTarget]) -> Value {
    let mut out = Map::new();
    for t in targets {
        let Some(id) = fm.jira_field(*t) else {
            continue;
        };
        let v = match t {
            FieldTarget::Title => json!(s.title),
            FieldTarget::Description => api::body_out(env, &s.description),
            FieldTarget::Labels => json!(
                s.labels
                    .iter()
                    .map(|l| l.split_whitespace().collect::<Vec<_>>().join("-"))
                    .collect::<Vec<_>>()
            ),
            FieldTarget::Priority => json!({ "name": priority_name(&s.priority) }),
            FieldTarget::DueOn => json!(s.due_on),
            FieldTarget::StoryPoints => json!(s.story_points),
            _ => continue,
        };
        out.insert(id.to_string(), v);
    }
    Value::Object(out)
}

/// Hasil penggabungan tiga arah.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Merge {
    pub merged: Snapshot,
    /// Target yang harus ditulis ke tiket (Jira menang).
    pub to_ticket: Vec<FieldTarget>,
    /// Target yang harus ditulis ke Jira (Agent Gate menang).
    pub to_jira: Vec<FieldTarget>,
}

/// Gabung `ticket` dan `jira` terhadap dasar `base`: sisi yang berubah menang; keduanya berubah
/// → `jira_newer` menentukan (perubahan terakhir menang).
#[must_use]
pub fn merge(
    fm: &FieldMap,
    base: &Snapshot,
    ticket: &Snapshot,
    jira: &Snapshot,
    jira_newer: bool,
) -> Merge {
    let mut merged = ticket.clone();
    let mut to_ticket = Vec::new();
    let mut to_jira = Vec::new();
    for t in SYNCED.into_iter().filter(|t| fm.maps(*t)) {
        let (b, a, j) = (value(base, t), value(ticket, t), value(jira, t));
        if a == j {
            continue;
        }
        let ag_changed = a != b;
        let jira_changed = j != b;
        let jira_wins = match (ag_changed, jira_changed) {
            (false, true) => true,
            (true, false) => false,
            _ => jira_newer,
        };
        if jira_wins {
            set(&mut merged, t, jira);
            to_ticket.push(t);
        } else {
            to_jira.push(t);
        }
    }
    Merge {
        merged: merged.normalized(),
        to_ticket,
        to_jira,
    }
}

/// Lampiran issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    pub filename: String,
    pub size: u64,
    pub mime: String,
    pub url: String,
}

#[must_use]
pub fn attachments(issue: &JiraIssue) -> Vec<Attachment> {
    issue
        .fields
        .get("attachment")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|x| Attachment {
                    filename: x
                        .get("filename")
                        .and_then(Value::as_str)
                        .unwrap_or("lampiran")
                        .to_string(),
                    size: x.get("size").and_then(Value::as_u64).unwrap_or(0),
                    mime: x
                        .get("mimeType")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    url: x
                        .get("content")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Nama issue type (`Bug`, `Story`, …) → kunci tipe Agent Gate.
#[must_use]
pub fn type_key(issue: &JiraIssue) -> &'static str {
    let name = issue
        .fields
        .pointer("/issuetype/name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_lowercase();
    match name.as_str() {
        "bug" | "defect" => "bug",
        "story" | "user story" => "story",
        "epic" => "epic",
        _ => "task",
    }
}

/// Nama issue type Jira untuk kunci tipe Agent Gate (issue baru dari mirror).
#[must_use]
pub fn type_name(key: &str) -> &'static str {
    match key {
        "bug" => "Bug",
        "story" => "Story",
        "epic" => "Epic",
        _ => "Task",
    }
}

/// Akun penerima issue.
#[must_use]
pub fn assignee(issue: &JiraIssue, env: JiraEnv) -> Option<String> {
    let a = issue.fields.get("assignee")?;
    let key = match env {
        JiraEnv::Cloud => "accountId",
        JiraEnv::DataCenter => "name",
    };
    a.get(key).and_then(Value::as_str).map(str::to_string)
}

/// Email/nama penerima issue (untuk log user tak dipetakan).
#[must_use]
pub fn assignee_label(issue: &JiraIssue) -> String {
    let a = issue.fields.get("assignee").unwrap_or(&Value::Null);
    a.get("emailAddress")
        .or_else(|| a.get("displayName"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// Sprint terakhir issue: Cloud = array objek `{name, startDate, endDate}`, Data Center = array
/// string `…[id=1,name=Sprint 1,startDate=…]`.
#[must_use]
pub fn sprint(issue: &JiraIssue, fm: &FieldMap) -> Option<SprintInfo> {
    let list = field(issue, fm, FieldTarget::Sprint)?.as_array()?;
    let last = list.last()?;
    match last {
        Value::Object(o) => Some(SprintInfo {
            name: o.get("name").and_then(Value::as_str)?.to_string(),
            starts_on: o
                .get("startDate")
                .and_then(Value::as_str)
                .map(|d| d.chars().take(10).collect()),
            ends_on: o
                .get("endDate")
                .and_then(Value::as_str)
                .map(|d| d.chars().take(10).collect()),
        }),
        Value::String(s) => {
            let part = |k: &str| {
                s.split([',', '['])
                    .find_map(|p| p.strip_prefix(&format!("{k}=")))
                    .map(|v| v.trim_end_matches(']').to_string())
                    .filter(|v| !v.is_empty() && v != "<null>")
            };
            Some(SprintInfo {
                name: part("name")?,
                starts_on: part("startDate").map(|d| d.chars().take(10).collect()),
                ends_on: part("endDate").map(|d| d.chars().take(10).collect()),
            })
        }
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SprintInfo {
    pub name: String,
    pub starts_on: Option<String>,
    pub ends_on: Option<String>,
}

/// Nama pertama dari field daftar bernama (Fix Version/s, Components).
#[must_use]
pub fn first_name(issue: &JiraIssue, fm: &FieldMap, target: FieldTarget) -> Option<String> {
    field(issue, fm, target)?
        .as_array()?
        .first()
        .and_then(|v| v.get("name"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// Nilai mentah field custom Jira yang dipetakan ke field custom Agent Gate `id`.
#[must_use]
pub fn custom_value(issue: &JiraIssue, fm: &FieldMap, id: uuid::Uuid) -> Option<String> {
    let v = field(issue, fm, FieldTarget::Custom(id))?;
    let s = match v {
        Value::Array(a) => a.iter().map(str_at).collect::<Vec<_>>().join(", "),
        other => str_at(other),
    };
    (!s.is_empty()).then_some(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::jira_connections::FieldEntry;

    fn fm() -> FieldMap {
        FieldMap {
            fields: [
                ("summary", FieldTarget::Title),
                ("description", FieldTarget::Description),
                ("labels", FieldTarget::Labels),
                ("priority", FieldTarget::Priority),
                ("duedate", FieldTarget::DueOn),
                ("customfield_10016", FieldTarget::StoryPoints),
            ]
            .iter()
            .map(|(id, t)| FieldEntry {
                id: (*id).into(),
                name: (*id).into(),
                target: *t,
            })
            .collect(),
        }
    }

    fn snap(title: &str, points: Option<i32>) -> Snapshot {
        Snapshot {
            title: title.into(),
            priority: "medium".into(),
            story_points: points,
            ..Snapshot::default()
        }
    }

    #[test]
    fn merge_takes_changed_side_and_last_writer_on_conflict() {
        let base = snap("A", Some(1));
        // Jira ubah judul, AG ubah poin: keduanya diambil.
        let m = merge(&fm(), &base, &snap("A", Some(3)), &snap("B", Some(1)), false);
        assert_eq!(m.merged, snap("B", Some(3)));
        assert_eq!(m.to_ticket, vec![FieldTarget::Title]);
        assert_eq!(m.to_jira, vec![FieldTarget::StoryPoints]);
        // Keduanya ubah judul: yang terakhir menang.
        let jira_newer = merge(&fm(), &base, &snap("AG", Some(1)), &snap("J", Some(1)), true);
        assert_eq!(jira_newer.merged.title, "J");
        let ag_newer = merge(&fm(), &base, &snap("AG", Some(1)), &snap("J", Some(1)), false);
        assert_eq!(ag_newer.merged.title, "AG");
        assert_eq!(ag_newer.to_jira, vec![FieldTarget::Title]);
    }

    #[test]
    fn issue_fields_become_snapshot() {
        let issue = JiraIssue {
            id: "1".into(),
            key: "SR-1".into(),
            status_id: "10".into(),
            status_name: "To Do".into(),
            updated: None,
            fields: json!({
                "summary": " Bayar QRIS ",
                "description": { "type": "doc", "version": 1, "content": [
                    { "type": "paragraph", "content": [{ "type": "text", "text": "Isi" }] }] },
                "labels": ["pos", "bayar"],
                "priority": { "name": "Highest" },
                "duedate": "2026-12-01",
                "customfield_10016": 2.6,
            }),
        };
        let s = of_issue(&issue, &fm());
        assert_eq!(s.title, "Bayar QRIS");
        assert_eq!(s.description, "Isi");
        assert_eq!(s.labels, ["bayar", "pos"]);
        assert_eq!(s.priority, "urgent");
        assert_eq!(s.due_on.as_deref(), Some("2026-12-01"));
        assert_eq!(s.story_points, Some(3));
        let out = jira_fields(JiraEnv::Cloud, &fm(), &s, &SYNCED);
        assert_eq!(out["priority"]["name"], "Highest");
        assert_eq!(out["description"]["type"], "doc");
    }

    #[test]
    fn reads_data_center_sprint_strings() {
        let mut map = fm();
        map.fields.push(FieldEntry {
            id: "customfield_10020".into(),
            name: "Sprint".into(),
            target: FieldTarget::Sprint,
        });
        let issue = JiraIssue {
            id: "1".into(),
            key: "SR-1".into(),
            status_id: String::new(),
            status_name: String::new(),
            updated: None,
            fields: json!({ "customfield_10020": [
                "com.atlassian.greenhopper.service.sprint.Sprint@1[id=3,state=ACTIVE,name=Sprint 4,startDate=2026-11-01T00:00:00.000Z,endDate=<null>]"
            ]}),
        };
        let s = sprint(&issue, &map).unwrap();
        assert_eq!(s.name, "Sprint 4");
        assert_eq!(s.starts_on.as_deref(), Some("2026-11-01"));
        assert_eq!(s.ends_on, None);
    }
}
