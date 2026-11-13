//! Saran pemetaan Jira ↔ Agent Gate (M11). Dipanggil setiap kali katalog Jira diperbarui
//! (koneksi disimpan/project dipilih): pilihan yang sudah ada dipertahankan untuk status/field/
//! user yang masih ada, yang baru diberi saran otomatis.
//!
//! - **Status**: nama stage sama (tanpa membedakan huruf besar), lalu kategori (nama status
//!   umum → kategori, atau `statusCategory` Jira) → stage pertama berkategori itu.
//! - **Field**: field standar Jira ke field Agent Gate yang setara; field custom Jira hanya
//!   ditampilkan bila namanya sama dengan field custom Agent Gate.
//! - **User**: email Jira sama dengan email anggota workspace.

use agentgate_core::workflow::{Category, Stage};
use sea_orm::{ConnectionTrait, DbErr};
use uuid::Uuid;

use super::api::{JiraField, JiraStatus, JiraUser};
use crate::{
    models::{
        custom_fields,
        jira_connections::{
            FieldEntry, FieldMap, FieldTarget, StatusCategory, StatusEntry, StatusMap, UserEntry,
            UserMap,
        },
        project_workflows, projects, ticket_types, workflows,
        workspaces::WorkspaceId,
    },
    services::imports::fields as import_fields,
};

/// Stage semua workflow project (gabungan tipe tiket, urutan workflow tipe Task dulu).
///
/// # Errors
///
/// Error database.
pub async fn project_stages<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project: &projects::Model,
) -> Result<Vec<Stage>, DbErr> {
    let mut types = ticket_types::Model::all_in(db, wid).await?;
    types.sort_by_key(|t| (t.key != "task", t.key == "subtask"));
    let mut seen = Vec::new();
    let mut out: Vec<Stage> = Vec::new();
    for t in types {
        let wf_id = project_workflows::Model::effective(db, wid, project.id, t.id).await?;
        if seen.contains(&wf_id) {
            continue;
        }
        seen.push(wf_id);
        let Some(wf) = workflows::Model::find_in(db, wid, wf_id).await? else {
            continue;
        };
        for stage in wf.definition(db).await?.stages {
            if !out.iter().any(|s| s.key == stage.key) {
                out.push(stage);
            }
        }
    }
    Ok(out)
}

fn category_of(status: &JiraStatus) -> Category {
    import_fields::status_category(&status.name).unwrap_or(match status.category {
        StatusCategory::New => Category::Todo,
        StatusCategory::Indeterminate => Category::InProgress,
        StatusCategory::Done => Category::Done,
    })
}

/// Stage yang disarankan untuk status Jira.
#[must_use]
pub fn suggest_stage(status: &JiraStatus, stages: &[Stage]) -> Option<String> {
    let name = status.name.trim().to_lowercase();
    if let Some(s) = stages
        .iter()
        .find(|s| s.name.trim().to_lowercase() == name || s.key == name.replace(' ', "_"))
    {
        return Some(s.key.clone());
    }
    let cat = category_of(status);
    stages
        .iter()
        .find(|s| s.category == cat)
        .or_else(|| {
            (cat == Category::Todo)
                .then(|| stages.iter().find(|s| s.category == Category::Backlog))
                .flatten()
        })
        .map(|s| s.key.clone())
}

/// Peta status dari status Jira terbaru; pilihan lama dipertahankan bila stage-nya masih ada.
#[must_use]
pub fn statuses(jira: &[JiraStatus], stages: &[Stage], current: &StatusMap) -> StatusMap {
    StatusMap {
        statuses: jira
            .iter()
            .map(|st| {
                let stage = match current.statuses.iter().find(|c| c.id == st.id) {
                    Some(c) => c
                        .stage
                        .clone()
                        .filter(|k| stages.iter().any(|s| &s.key == k)),
                    None => suggest_stage(st, stages),
                };
                StatusEntry {
                    id: st.id.clone(),
                    name: st.name.clone(),
                    category: st.category,
                    stage,
                }
            })
            .collect(),
    }
}

/// Field standar Jira (id sistem) → target bawaan.
const SYSTEM_FIELDS: &[(&str, FieldTarget)] = &[
    ("summary", FieldTarget::Title),
    ("description", FieldTarget::Description),
    ("fixVersions", FieldTarget::Release),
    ("components", FieldTarget::Component),
    ("duedate", FieldTarget::DueOn),
    ("labels", FieldTarget::Labels),
    ("priority", FieldTarget::Priority),
];

fn custom_target(name: &str) -> Option<FieldTarget> {
    match name.trim().to_lowercase().as_str() {
        "story points" | "story point estimate" | "story point" => Some(FieldTarget::StoryPoints),
        "sprint" => Some(FieldTarget::Sprint),
        _ => None,
    }
}

/// Peta field dari katalog field Jira dan field custom Agent Gate.
#[must_use]
pub fn fields(
    jira: &[JiraField],
    custom: &[custom_fields::Model],
    current: &FieldMap,
) -> FieldMap {
    let mut out: Vec<FieldEntry> = Vec::new();
    let keep = |id: &str, suggested: FieldTarget| {
        current
            .fields
            .iter()
            .find(|c| c.id == id)
            .map_or(suggested, |c| match c.target {
                FieldTarget::Custom(cid) if !custom.iter().any(|f| f.id == cid) => {
                    FieldTarget::Skip
                }
                t => t,
            })
    };
    for (id, target) in SYSTEM_FIELDS {
        if let Some(f) = jira.iter().find(|f| f.id == *id) {
            out.push(FieldEntry {
                id: f.id.clone(),
                name: f.name.clone(),
                target: keep(&f.id, *target),
            });
        }
    }
    for f in jira.iter().filter(|f| f.custom) {
        let suggested = custom_target(&f.name).or_else(|| {
            custom
                .iter()
                .find(|c| c.name.trim().eq_ignore_ascii_case(f.name.trim()))
                .map(|c| FieldTarget::Custom(c.id))
        });
        let Some(suggested) = suggested else {
            // Field custom lain tetap tampil bila sebelumnya dipetakan.
            if let Some(c) = current.fields.iter().find(|c| c.id == f.id) {
                out.push(FieldEntry {
                    id: f.id.clone(),
                    name: f.name.clone(),
                    target: keep(&c.id, FieldTarget::Skip),
                });
            }
            continue;
        };
        // Story points/sprint: satu field Jira saja (yang pertama).
        if matches!(suggested, FieldTarget::StoryPoints | FieldTarget::Sprint)
            && out.iter().any(|e| e.target == suggested)
            && !current.fields.iter().any(|c| c.id == f.id)
        {
            continue;
        }
        out.push(FieldEntry {
            id: f.id.clone(),
            name: f.name.clone(),
            target: keep(&f.id, suggested),
        });
    }
    FieldMap { fields: out }
}

/// Peta user: pilihan lama dipertahankan bila anggotanya masih ada, sisanya dicocokkan lewat
/// email. `members` = (id user, email huruf kecil).
#[must_use]
pub fn users(jira: &[JiraUser], members: &[(Uuid, String)], current: &UserMap) -> UserMap {
    let member = |id: Uuid| members.iter().any(|(m, _)| *m == id);
    UserMap {
        users: jira
            .iter()
            .filter(|u| !u.account.is_empty())
            .map(|u| {
                let kept = current
                    .users
                    .iter()
                    .find(|c| c.account == u.account)
                    .and_then(|c| c.user_id)
                    .filter(|id| member(*id));
                let by_email = (!u.email.is_empty())
                    .then(|| {
                        members
                            .iter()
                            .find(|(_, e)| e.eq_ignore_ascii_case(&u.email))
                            .map(|(id, _)| *id)
                    })
                    .flatten();
                UserEntry {
                    account: u.account.clone(),
                    name: u.name.clone(),
                    email: u.email.clone(),
                    user_id: kept.or(by_email),
                }
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(id: &str, name: &str, cat: StatusCategory) -> JiraStatus {
        JiraStatus {
            id: id.into(),
            name: name.into(),
            category: cat,
        }
    }

    fn stages() -> Vec<Stage> {
        vec![
            Stage::new("backlog", "Backlog", Category::Backlog),
            Stage::new("todo", "To do", Category::Todo),
            Stage::new("review", "In Review", Category::InProgress),
            Stage::new("in_progress", "Dikerjakan", Category::InProgress),
            Stage::new("done", "Selesai", Category::Done),
        ]
    }

    #[test]
    fn suggests_by_name_then_category() {
        let s = stages();
        assert_eq!(
            suggest_stage(&st("1", "To Do", StatusCategory::New), &s).as_deref(),
            Some("todo")
        );
        assert_eq!(
            suggest_stage(&st("2", "In Review", StatusCategory::Indeterminate), &s).as_deref(),
            Some("review")
        );
        assert_eq!(
            suggest_stage(&st("3", "Closed", StatusCategory::Done), &s).as_deref(),
            Some("done")
        );
        assert_eq!(
            suggest_stage(&st("4", "Blocked", StatusCategory::Indeterminate), &s).as_deref(),
            Some("review")
        );
    }

    #[test]
    fn keeps_existing_choices() {
        let jira = [st("1", "To Do", StatusCategory::New)];
        let current = StatusMap {
            statuses: vec![StatusEntry {
                id: "1".into(),
                name: "To Do".into(),
                category: StatusCategory::New,
                stage: None,
            }],
        };
        assert_eq!(statuses(&jira, &stages(), &current).statuses[0].stage, None);
        let fresh = statuses(&jira, &stages(), &StatusMap::default());
        assert_eq!(fresh.statuses[0].stage.as_deref(), Some("todo"));
    }

    #[test]
    fn users_match_by_email() {
        let id = Uuid::now_v7();
        let jira = [
            JiraUser {
                account: "a1".into(),
                name: "Dewi".into(),
                email: "dewi@contoh.id".into(),
            },
            JiraUser {
                account: "a2".into(),
                name: "Budi".into(),
                email: "budi@contoh.id".into(),
            },
        ];
        let map = users(&jira, &[(id, "dewi@contoh.id".into())], &UserMap::default());
        assert_eq!(map.user_of("a1"), Some(id));
        assert_eq!(map.user_of("a2"), None);
    }
}
