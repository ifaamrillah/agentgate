//! Rencana per baris CSV: siap, peringatan (tetap diimpor), atau dilewati. Dipakai pratinjau
//! (semua baris) dan worker (baris demi baris), sehingga keduanya tidak berbeda pendapat.
//!
//! Aturan:
//! - **Dilewati**: judul kosong; baris duplikat (semua nilai yang dipetakan sama dengan baris
//!   sebelumnya); kode lama bentrok dengan kode/alias yang sudah ada di workspace atau baris
//!   sebelumnya di berkas.
//! - **Peringatan**: penerima bukan anggota project (dikosongkan); status dipetakan ke stage
//!   lain atau ke stage awal (tidak dikenal, atau stage tujuannya didahului gate); tipe tidak
//!   dikenal (Task); prioritas/story point/tanggal/sprint tidak terbaca (dikosongkan); label
//!   baru di project; judul/deskripsi dipotong; kode lama tidak layak jadi alias.
use std::collections::{HashMap, HashSet};

use agentgate_core::workflow::Definition;
use chrono::{DateTime, Utc};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QuerySelect,
    sea_query::ExprTrait,
};
use uuid::Uuid;

use super::fields;
use crate::{
    models::{
        project_access, project_workflows, projects, sprints,
        ticket_imports::ImportField,
        ticket_types,
        tickets::{self, TicketInput, TicketStatus},
        workflows,
        workspaces::WorkspaceId,
    },
    services::tickets::{create::TicketForm, stages},
};

pub const SKIP_TITLE_EMPTY: &str = "csv-skip-title-empty";
pub const SKIP_DUPLICATE: &str = "csv-skip-duplicate";
pub const SKIP_ALIAS_TAKEN: &str = "csv-skip-alias-taken";
pub const SKIP_FAILED: &str = "csv-skip-failed";
pub const WARN_ASSIGNEE: &str = "csv-warn-assignee";
pub const WARN_STATUS_MAPPED: &str = "csv-warn-status-mapped";
pub const WARN_STATUS_UNKNOWN: &str = "csv-warn-status-unknown";
pub const WARN_STATUS_GATED: &str = "csv-warn-status-gated";
pub const WARN_TYPE: &str = "csv-warn-type";
pub const WARN_PRIORITY: &str = "csv-warn-priority";
pub const WARN_POINTS: &str = "csv-warn-points";
pub const WARN_DATE: &str = "csv-warn-date";
pub const WARN_SPRINT: &str = "csv-warn-sprint";
pub const WARN_LABEL_NEW: &str = "csv-warn-label-new";
pub const WARN_LABEL_DROPPED: &str = "csv-warn-label-dropped";
pub const WARN_TRUNCATED: &str = "csv-warn-truncated";
pub const WARN_ALIAS_INVALID: &str = "csv-warn-alias-invalid";

/// Semua kode di atas (tes kelengkapan Fluent).
pub const CODES: &[&str] = &[
    SKIP_TITLE_EMPTY,
    SKIP_DUPLICATE,
    SKIP_ALIAS_TAKEN,
    SKIP_FAILED,
    WARN_ASSIGNEE,
    WARN_STATUS_MAPPED,
    WARN_STATUS_UNKNOWN,
    WARN_STATUS_GATED,
    WARN_TYPE,
    WARN_PRIORITY,
    WARN_POINTS,
    WARN_DATE,
    WARN_SPRINT,
    WARN_LABEL_NEW,
    WARN_LABEL_DROPPED,
    WARN_TRUNCATED,
    WARN_ALIAS_INVALID,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Ready,
    Warning,
    Skipped,
}

/// Catatan satu baris (kode Fluent + argumen).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Note {
    pub code: &'static str,
    pub arg: String,
}

/// Hasil rencana satu baris.
#[derive(Debug, Clone)]
pub struct RowPlan {
    /// Nomor baris di berkas (header = 1).
    pub row: u32,
    pub outcome: Outcome,
    pub notes: Vec<Note>,
    pub title: String,
    pub type_key: String,
    pub stage_name: String,
    pub form: TicketForm,
    pub alias: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

/// Data project yang dibaca sekali per pratinjau/impor.
pub struct Context {
    types: Vec<ticket_types::Model>,
    /// Definisi workflow efektif per id tipe.
    flows: HashMap<Uuid, Definition>,
    /// Email (huruf kecil) → id user yang boleh membuka project.
    members: HashMap<String, Uuid>,
    sprints: Vec<sprints::Model>,
    known_labels: HashSet<String>,
    /// Kode/alias yang sudah ada di workspace (huruf besar), dari kolom kode lama berkas.
    taken: HashSet<String>,
}

impl Context {
    /// Muat data project. `aliases` = semua nilai kolom kode lama di berkas (dicek sekaligus).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn load<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        project: &projects::Model,
        aliases: &[String],
    ) -> Result<Self, DbErr> {
        let types = ticket_types::Model::all_in(db, wid).await?;
        let mut flows = HashMap::new();
        for t in &types {
            let wf_id = project_workflows::Model::effective(db, wid, project.id, t.id).await?;
            if let Some(wf) = workflows::Model::find_in(db, wid, wf_id).await? {
                flows.insert(t.id, wf.definition(db).await?);
            }
        }
        let members = project_access::users_with_access(db, wid, project.id, None, u64::MAX >> 1)
            .await?
            .into_iter()
            .map(|u| (u.email.to_lowercase(), u.id))
            .collect();
        let sprints = sprints::Model::of_project(db, wid, project.id).await?;
        let label_rows: Vec<Vec<String>> = tickets::Entity::find()
            .select_only()
            .column(tickets::Column::Labels)
            .filter(tickets::Column::WorkspaceId.eq(wid.get()))
            .filter(tickets::Column::ProjectId.eq(project.id))
            .into_tuple()
            .all(db)
            .await?;
        let known_labels = label_rows
            .into_iter()
            .flatten()
            .map(|l| l.to_lowercase())
            .collect();
        let upper: Vec<String> = aliases
            .iter()
            .map(|a| a.trim().to_uppercase())
            .filter(|a| !a.is_empty())
            .collect();
        let mut taken = HashSet::new();
        for chunk in upper.chunks(500) {
            let rows: Vec<String> = crate::models::ticket_codes::Entity::find()
                .select_only()
                .column(crate::models::ticket_codes::Column::Code)
                .filter(crate::models::ticket_codes::Column::WorkspaceId.eq(wid.get()))
                .filter(
                    sea_orm::sea_query::Expr::expr(sea_orm::sea_query::Func::upper(
                        sea_orm::sea_query::Expr::col(crate::models::ticket_codes::Column::Code),
                    ))
                    .is_in(chunk.to_vec()),
                )
                .into_tuple()
                .all(db)
                .await?;
            taken.extend(rows.into_iter().map(|c| c.to_uppercase()));
        }
        Ok(Self {
            types,
            flows,
            members,
            sprints,
            known_labels,
            taken,
        })
    }

    /// Kode `code` sudah dipakai (sebelum impor ini dimulai).
    #[must_use]
    pub fn alias_taken(&self, code: &str) -> bool {
        self.taken.contains(&code.trim().to_uppercase())
    }
}

/// Keadaan yang terbawa antarbaris (duplikat dan alias di berkas yang sama).
#[derive(Debug, Default)]
pub struct Seen {
    rows: HashMap<Vec<String>, u32>,
    aliases: HashSet<String>,
    labels: HashSet<String>,
}

fn note(code: &'static str, arg: impl Into<String>) -> Note {
    Note {
        code,
        arg: arg.into(),
    }
}

fn cut(s: &str, max: usize) -> (String, bool) {
    if s.chars().count() > max {
        (s.chars().take(max).collect(), true)
    } else {
        (s.to_string(), false)
    }
}

/// Rencana baris `values` (nomor baris `row`) menurut `mapping`.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn plan_row(
    ctx: &Context,
    seen: &mut Seen,
    mapping: &[ImportField],
    row: u32,
    values: &[String],
) -> RowPlan {
    let get = |field: ImportField| -> &str {
        mapping
            .iter()
            .position(|f| *f == field)
            .and_then(|i| values.get(i))
            .map_or("", |v| v.trim())
    };
    let mut notes = Vec::new();
    let mut plan = RowPlan {
        row,
        outcome: Outcome::Ready,
        notes: Vec::new(),
        title: String::new(),
        type_key: String::new(),
        stage_name: String::new(),
        form: TicketForm::default(),
        alias: None,
        created_at: None,
    };
    let (title, title_cut) = cut(get(ImportField::Title), tickets::TITLE_MAX_LEN);
    plan.title.clone_from(&title);
    let skip = |mut plan: RowPlan, code: &'static str, arg: String| {
        plan.outcome = Outcome::Skipped;
        plan.notes = vec![note(code, arg)];
        plan
    };
    if title.is_empty() {
        return skip(plan, SKIP_TITLE_EMPTY, String::new());
    }
    let mapped: Vec<String> = mapping
        .iter()
        .zip(values)
        .filter(|(f, _)| **f != ImportField::Skip)
        .map(|(_, v)| v.trim().to_string())
        .collect();
    if let Some(first) = seen.rows.get(&mapped) {
        return skip(plan, SKIP_DUPLICATE, first.to_string());
    }
    let alias_raw = get(ImportField::Alias);
    if !alias_raw.is_empty() {
        if fields::valid_alias(alias_raw) {
            let upper = alias_raw.to_uppercase();
            if ctx.alias_taken(alias_raw) || seen.aliases.contains(&upper) {
                return skip(plan, SKIP_ALIAS_TAKEN, alias_raw.to_string());
            }
            seen.aliases.insert(upper);
            plan.alias = Some(alias_raw.to_string());
        } else {
            notes.push(note(WARN_ALIAS_INVALID, alias_raw));
        }
    }
    seen.rows.insert(mapped, row);
    if title_cut {
        notes.push(note(WARN_TRUNCATED, "title"));
    }
    let (description, desc_cut) = cut(get(ImportField::Description), tickets::DESCRIPTION_MAX_LEN);
    if desc_cut {
        notes.push(note(WARN_TRUNCATED, "description"));
    }
    // Tipe: kunci atau nama (tanpa membedakan huruf besar); "Sub-task" → subtask.
    let type_raw = get(ImportField::Type);
    let norm = type_raw.to_lowercase().replace(['-', ' ', '_'], "");
    let ticket_type = ctx
        .types
        .iter()
        .find(|t| {
            !norm.is_empty()
                && (t.key.to_lowercase().replace(['-', '_'], "") == norm
                    || t.name.to_lowercase().replace(['-', ' ', '_'], "") == norm)
        })
        .or_else(|| ctx.types.iter().find(|t| t.key == "task"))
        .or(ctx.types.first());
    if !type_raw.is_empty()
        && ticket_type.is_some_and(|t| {
            t.key.to_lowercase().replace(['-', '_'], "") != norm
                && t.name.to_lowercase().replace(['-', ' ', '_'], "") != norm
        })
    {
        notes.push(note(WARN_TYPE, type_raw));
    }
    plan.type_key = ticket_type.map(|t| t.key.clone()).unwrap_or_default();
    // Stage.
    let mut stage_key = String::new();
    let mut status = TicketStatus::Todo;
    if let Some(def) = ticket_type.and_then(|t| ctx.flows.get(&t.id)) {
        let status_raw = get(ImportField::Stage);
        let exact = def.stages.iter().find(|s| {
            !status_raw.is_empty()
                && (s.name.eq_ignore_ascii_case(status_raw)
                    || s.key.eq_ignore_ascii_case(status_raw))
        });
        let category = fields::status_category(status_raw);
        let wanted = match (exact, category) {
            (Some(s), _) => Some(s),
            (None, Some(c)) => def.first_in(c),
            (None, None) => None,
        };
        if let Some(c) = category {
            status = TicketStatus::parse(c.as_str()).unwrap_or(TicketStatus::Todo);
        }
        let start = |def: &Definition| -> Option<(String, String)> {
            stages::initial_stage(def, "", TicketStatus::Todo)
                .ok()
                .or_else(|| def.first())
                .map(|s| (s.key.clone(), s.name.clone()))
        };
        match wanted {
            Some(s) if stages::initial_stage(def, &s.key, status).is_ok() => {
                stage_key.clone_from(&s.key);
                plan.stage_name.clone_from(&s.name);
                if exact.is_none() {
                    notes.push(note(WARN_STATUS_MAPPED, format!("{status_raw}|{}", s.name)));
                }
            }
            Some(s) => {
                if let Some((k, n)) = start(def) {
                    stage_key = k;
                    plan.stage_name.clone_from(&n);
                    notes.push(note(WARN_STATUS_GATED, format!("{}|{n}", s.name)));
                }
                status = TicketStatus::Todo;
            }
            None => {
                if let Some((k, n)) = start(def) {
                    stage_key = k;
                    plan.stage_name.clone_from(&n);
                    if !status_raw.is_empty() {
                        notes.push(note(WARN_STATUS_UNKNOWN, format!("{status_raw}|{n}")));
                    }
                }
                status = TicketStatus::Todo;
            }
        }
        if stage_key.is_empty() {
            status = TicketStatus::Todo;
        }
    }
    // Penerima: email anggota project.
    let assignee_raw = get(ImportField::Assignee);
    let assignee = if assignee_raw.is_empty() {
        String::new()
    } else if let Some(id) = ctx.members.get(&assignee_raw.to_lowercase()) {
        id.to_string()
    } else {
        notes.push(note(WARN_ASSIGNEE, assignee_raw));
        String::new()
    };
    let priority_raw = get(ImportField::Priority);
    let priority = fields::priority(priority_raw).unwrap_or_else(|| {
        notes.push(note(WARN_PRIORITY, priority_raw));
        tickets::TicketPriority::Medium
    });
    let points = match fields::story_points(get(ImportField::StoryPoints)) {
        Ok(p) => p,
        Err(fields::BadValue) => {
            notes.push(note(WARN_POINTS, get(ImportField::StoryPoints)));
            None
        }
    };
    let due_on = match fields::date(get(ImportField::DueOn)) {
        Ok(d) => d,
        Err(fields::BadValue) => {
            notes.push(note(WARN_DATE, get(ImportField::DueOn)));
            None
        }
    };
    plan.created_at = match fields::datetime(get(ImportField::CreatedAt)) {
        Ok(d) => d,
        Err(fields::BadValue) => {
            notes.push(note(WARN_DATE, get(ImportField::CreatedAt)));
            None
        }
    };
    // Label: semua kolom Label digabung.
    let label_values: Vec<&str> = mapping
        .iter()
        .zip(values)
        .filter(|(f, _)| **f == ImportField::Labels)
        .map(|(_, v)| v.as_str())
        .collect();
    let mut labels = Vec::new();
    for l in fields::labels(&label_values) {
        if labels.len() >= tickets::LABELS_MAX
            || tickets::parse_labels(&l).map_or(true, |v| v.is_empty())
        {
            notes.push(note(WARN_LABEL_DROPPED, l));
            continue;
        }
        let lower = l.to_lowercase();
        if !ctx.known_labels.contains(&lower) && !seen.labels.contains(&lower) {
            notes.push(note(WARN_LABEL_NEW, l.clone()));
            seen.labels.insert(lower);
        }
        labels.push(l);
    }
    let sprint_raw = get(ImportField::Sprint);
    let sprint = if sprint_raw.is_empty() {
        String::new()
    } else {
        // Jira menulis beberapa sprint dipisah koma; yang terakhir = sprint sekarang.
        let name = sprint_raw.rsplit(',').next().unwrap_or(sprint_raw).trim();
        match ctx.sprints.iter().find(|s| {
            s.name.eq_ignore_ascii_case(name) && s.state() != sprints::SprintState::Completed
        }) {
            Some(s) => s.id.to_string(),
            None => {
                notes.push(note(WARN_SPRINT, name));
                String::new()
            }
        }
    };
    plan.form = TicketForm {
        input: TicketInput {
            title,
            description,
            status: sea_orm::ActiveEnum::to_value(&status),
            priority: sea_orm::ActiveEnum::to_value(&priority),
            labels: labels.join(", "),
            due_on: due_on
                .map(|d| d.format("%Y-%m-%d").to_string())
                .unwrap_or_default(),
            story_points: points.map(|p| p.to_string()).unwrap_or_default(),
        },
        type_id: ticket_type.map(|t| t.id.to_string()).unwrap_or_default(),
        assignee,
        stage: stage_key,
        sprint: Some(sprint),
        ..TicketForm::default()
    };
    plan.outcome = if notes.is_empty() {
        Outcome::Ready
    } else {
        Outcome::Warning
    };
    plan.notes = notes;
    plan
}
