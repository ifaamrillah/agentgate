//! Saran autocomplete kotak query JQL (R3): field, operator, kata kunci, dan nilai.
//!
//! [`jql::context`] membaca teks sebelum kursor; modul ini mengubah harapannya menjadi daftar
//! saran. Nilai diambil dari data yang boleh dilihat pelaku: project yang bisa dibuka, stage
//! workflow project itu, anggota workspace, tipe tiket, sprint/rilis/component/label project.
//! Saran disaring dengan kata yang sedang diketik (awalan dulu, lalu yang mengandung) dan dibatasi
//! [`LIMIT`] baris.
use std::collections::BTreeSet;

use sea_orm::{
    ColumnTrait, ConnectionTrait, DbErr, EntityTrait, FromQueryResult, QueryFilter, QuerySelect,
    Statement,
};
use serde::Serialize;
use uuid::Uuid;

use super::jql::{self, Expect, Field};
use crate::{
    common::i18n,
    models::{
        components, projects, releases, sprints, ticket_types, tickets, workflows,
        workspace_members, workspaces::WorkspaceId,
    },
};

/// Saran maksimal per permintaan.
pub const LIMIT: usize = 12;

/// Satu saran: teks yang disisipkan (menggantikan `partial`), label, dan keterangan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Suggestion {
    pub insert: String,
    pub label: String,
    pub hint: String,
    /// `field`, `op`, `value`, `keyword` (ikon/label jenis di daftar).
    pub kind: &'static str,
}

/// Jawaban endpoint saran.
#[derive(Debug, Clone, Serialize)]
pub struct Suggestions {
    pub partial: String,
    pub items: Vec<Suggestion>,
}

fn item(insert: &str, label: &str, hint: String, kind: &'static str) -> Suggestion {
    Suggestion {
        insert: insert.to_string(),
        label: label.to_string(),
        hint,
        kind,
    }
}

fn keyword(insert: &str, label: &str, hint_key: &str) -> Suggestion {
    item(insert, label, i18n::text(hint_key, &[]), "keyword")
}

fn op_hint(op: &str) -> &'static str {
    match op {
        "=" => "jql-op-eq",
        "!=" => "jql-op-ne",
        "~" => "jql-op-like",
        "in" => "jql-op-in",
        "not in" => "jql-op-not-in",
        "is EMPTY" => "jql-op-is-empty",
        "is not EMPTY" => "jql-op-is-not-empty",
        "<" => "jql-op-lt",
        "<=" => "jql-op-le",
        ">" => "jql-op-gt",
        _ => "jql-op-ge",
    }
}

/// Kunci Fluent yang dipakai modul ini (tes kontrak i18n).
#[must_use]
pub fn label_keys() -> Vec<String> {
    [
        "jql-op-eq",
        "jql-op-ne",
        "jql-op-like",
        "jql-op-in",
        "jql-op-not-in",
        "jql-op-is-empty",
        "jql-op-is-not-empty",
        "jql-op-lt",
        "jql-op-le",
        "jql-op-gt",
        "jql-op-ge",
        "jql-kw-and",
        "jql-kw-or",
        "jql-kw-not",
        "jql-kw-open",
        "jql-kw-close",
        "jql-kw-order",
        "jql-kw-asc",
        "jql-kw-desc",
        "jql-kw-list-next",
        "jql-kw-list-close",
        "jql-fn-current-user",
        "jql-fn-open-sprints",
        "jql-value-empty",
    ]
    .iter()
    .map(|k| (*k).to_string())
    .collect()
}

/// Saran field (untuk awal syarat dan ORDER BY).
fn fields(order: bool) -> Vec<Suggestion> {
    jql::FIELDS
        .iter()
        .filter(|(_, f)| !order || *f != Field::Text)
        .map(|(name, _)| {
            item(
                &format!("{name} "),
                name,
                i18n::text(&jql::field_hint_key(name), &[]),
                "field",
            )
        })
        .collect()
}

/// Field yang nilainya bisa kosong (`EMPTY`).
fn nullable(f: Field) -> bool {
    matches!(
        f,
        Field::Assignee
            | Field::Sprint
            | Field::Label
            | Field::Points
            | Field::Release
            | Field::Component
            | Field::Due
    )
}

#[derive(Debug, FromQueryResult)]
struct LabelRow {
    label: String,
}

/// Nilai field `f` dari data project `pids` (label tampil, keterangan).
async fn values<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    pids: &[Uuid],
    f: Field,
) -> Result<Vec<(String, String)>, DbErr> {
    let mut out: Vec<(String, String)> = Vec::new();
    match f {
        Field::Project => {
            for p in projects::Entity::find()
                .filter(projects::Column::WorkspaceId.eq(wid.get()))
                .filter(projects::Column::Id.is_in(pids.to_vec()))
                .all(db)
                .await?
            {
                out.push((p.key, p.name));
            }
        }
        Field::Status => {
            for s in ["backlog", "todo", "in_progress", "done"] {
                let key = format!("ticket-status-{}", s.replace('_', "-"));
                out.push((s.to_string(), i18n::text(&key, &[])));
            }
        }
        Field::Priority => {
            for p in ["urgent", "high", "medium", "low"] {
                out.push((
                    p.to_string(),
                    i18n::text(&format!("ticket-priority-{p}"), &[]),
                ));
            }
        }
        Field::Stage => {
            let used: BTreeSet<Uuid> = tickets::Entity::find()
                .select_only()
                .column(tickets::Column::WorkflowId)
                .distinct()
                .filter(tickets::Column::WorkspaceId.eq(wid.get()))
                .filter(tickets::Column::ProjectId.is_in(pids.to_vec()))
                .into_tuple::<Uuid>()
                .all(db)
                .await?
                .into_iter()
                .collect();
            let mut seen = BTreeSet::new();
            for w in workflows::Model::all_in(db, wid).await? {
                if !used.contains(&w.id) {
                    continue;
                }
                for s in w.definition(db).await?.stages {
                    if seen.insert(s.name.to_lowercase()) {
                        out.push((s.name, s.key));
                    }
                }
            }
        }
        Field::Assignee => {
            out.push((
                "currentUser()".into(),
                i18n::text("jql-fn-current-user", &[]),
            ));
            for (_, u, _) in workspace_members::Model::list_in(db, wid).await? {
                out.push((u.name, u.email));
            }
        }
        Field::Type => {
            for t in ticket_types::Model::all_in(db, wid).await? {
                out.push((t.key, t.name));
            }
        }
        Field::Sprint => {
            out.push((
                "openSprints()".into(),
                i18n::text("jql-fn-open-sprints", &[]),
            ));
            let mut list: Vec<sprints::Model> = Vec::new();
            for pid in pids {
                list.extend(sprints::Model::of_project(db, wid, *pid).await?);
            }
            // Sprint yang belum selesai lebih dulu.
            list.sort_by_key(|s| (s.state() == sprints::SprintState::Completed, s.starts_on));
            for s in list {
                let hint = format!("{} – {}", s.starts_on, s.ends_on);
                out.push((s.name, hint));
            }
        }
        Field::Release => {
            for pid in pids {
                for r in releases::Model::of_project(db, wid, *pid).await? {
                    out.push((r.name, String::new()));
                }
            }
        }
        Field::Component => {
            for pid in pids {
                for c in components::Model::of_project(db, wid, *pid).await? {
                    out.push((c.name, String::new()));
                }
            }
        }
        Field::Label => {
            if !pids.is_empty() {
                let rows = LabelRow::find_by_statement(Statement::from_sql_and_values(
                    db.get_database_backend(),
                    "SELECT DISTINCT unnest(labels) AS label FROM tickets \
                     WHERE workspace_id = $1 AND project_id = ANY($2) AND deleted_at IS NULL \
                     ORDER BY 1 LIMIT 200",
                    [wid.get().into(), pids.to_vec().into()],
                ))
                .all(db)
                .await?;
                out.extend(rows.into_iter().map(|r| (r.label, String::new())));
            }
        }
        Field::Points | Field::Text | Field::Due | Field::Created | Field::Updated => {}
    }
    // Nama sama (mis. sprint "Sprint 1" di dua project) cukup sekali.
    let mut seen = BTreeSet::new();
    out.retain(|(v, _)| seen.insert(v.to_lowercase()));
    Ok(out)
}

/// Saran untuk teks sebelum kursor `prefix`, atas project `pids` yang boleh dibuka pelaku.
///
/// # Errors
///
/// Error database.
pub async fn suggest<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    pids: &[Uuid],
    prefix: &str,
) -> Result<Suggestions, DbErr> {
    let ctx = jql::context(prefix);
    let mut items = match ctx.expect {
        Expect::Field => {
            let mut v = fields(false);
            v.push(keyword("NOT ", "NOT", "jql-kw-not"));
            v.push(keyword("(", "(", "jql-kw-open"));
            v
        }
        Expect::Operator(f) => {
            let mut v: Vec<Suggestion> = jql::operators(f)
                .iter()
                .map(|op| {
                    let insert = if op.ends_with("EMPTY") {
                        format!("{op} ")
                    } else if op.contains("in") {
                        format!("{op} (")
                    } else {
                        format!("{op} ")
                    };
                    item(&insert, op, i18n::text(op_hint(op), &[]), "op")
                })
                .collect();
            if !nullable(f) {
                v.retain(|s| !s.label.ends_with("EMPTY"));
            }
            v
        }
        Expect::InStart(f) => {
            let mut v = vec![keyword("(", "(", "jql-kw-open")];
            match f {
                Field::Sprint => v.push(item(
                    "openSprints() ",
                    "openSprints()",
                    i18n::text("jql-fn-open-sprints", &[]),
                    "value",
                )),
                Field::Assignee => v.push(item(
                    "currentUser() ",
                    "currentUser()",
                    i18n::text("jql-fn-current-user", &[]),
                    "value",
                )),
                _ => {}
            }
            v
        }
        Expect::IsValue => vec![
            keyword("EMPTY ", "EMPTY", "jql-value-empty"),
            keyword("not ", "not", "jql-kw-not"),
        ],
        Expect::Value(f) => {
            let in_list = prefix.trim_end().ends_with('(') || prefix.trim_end().ends_with(',');
            let mut v: Vec<Suggestion> = values(db, wid, pids, f)
                .await?
                .into_iter()
                .map(|(value, hint)| {
                    let insert = if value.ends_with("()") {
                        value.clone()
                    } else {
                        jql::quote(&value)
                    };
                    let insert = if in_list {
                        insert
                    } else {
                        format!("{insert} ")
                    };
                    item(&insert, &value, hint, "value")
                })
                .collect();
            if nullable(f) && !in_list {
                v.push(keyword("EMPTY ", "EMPTY", "jql-value-empty"));
            }
            v
        }
        Expect::ListNext => vec![
            keyword(", ", ",", "jql-kw-list-next"),
            keyword(") ", ")", "jql-kw-list-close"),
        ],
        Expect::Join { open } => {
            let mut v = vec![
                keyword("AND ", "AND", "jql-kw-and"),
                keyword("OR ", "OR", "jql-kw-or"),
            ];
            if open {
                v.push(keyword(") ", ")", "jql-kw-close"));
            } else {
                v.push(keyword("ORDER BY ", "ORDER BY", "jql-kw-order"));
            }
            v
        }
        Expect::OrderBy => vec![keyword("BY ", "BY", "jql-kw-order")],
        Expect::OrderField => fields(true),
        Expect::OrderDir => vec![
            keyword("ASC", "ASC", "jql-kw-asc"),
            keyword("DESC", "DESC", "jql-kw-desc"),
        ],
        Expect::Nothing => Vec::new(),
    };
    let needle = ctx
        .partial
        .trim_start_matches(['"', '\''])
        .trim_end_matches(['"', '\''])
        .to_lowercase();
    // Di dalam kutip hanya nilai teks (bukan fungsi atau kata kunci).
    if ctx.partial.starts_with(['"', '\'']) {
        items.retain(|s| s.kind == "value" && !s.label.ends_with("()"));
    }
    if !needle.is_empty() {
        let starts = |s: &Suggestion| s.label.to_lowercase().starts_with(&needle);
        let has = |s: &Suggestion| {
            s.label.to_lowercase().contains(&needle) || s.hint.to_lowercase().contains(&needle)
        };
        items.retain(has);
        items.sort_by_key(|s| !starts(s));
    }
    items.truncate(LIMIT);
    Ok(Suggestions {
        partial: ctx.partial,
        items,
    })
}
