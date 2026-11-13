//! Sprint: buat/ubah/hapus, mulai, selesaikan (laporan beku + pindah tiket belum selesai), pindah
//! tiket antar sprint/backlog dan urutan backlog, kapasitas dari velocity.
//!
//! Epic dan Subtask tidak direncanakan di sprint (Subtask mengikuti induknya di board), jadi
//! tidak bisa dipindah ke sprint lewat jalur ini.
use chrono::NaiveDate;
use sea_orm::{
    ActiveEnum, ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait,
    DatabaseConnection, DbErr, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder, QuerySelect,
    TransactionTrait,
};
use uuid::Uuid;

use super::{
    CAPACITY_INVALID, DATES_INVALID, Failure, NAME_INVALID, SPRINT_STATE, TARGET_INVALID,
    TEXT_TOO_LONG, scope,
};
use crate::{
    models::{
        permission::Permission,
        projects,
        sprints::{self, GOAL_MAX_LEN, NAME_MAX_LEN, ReportTicket, SprintReport, SprintState},
        ticket_events::{self, EventKind},
        ticket_types, tickets,
        workspaces::WorkspaceId,
    },
    services::{tickets::access_in, workspaces::Acting},
};

/// Durasi sprint bawaan (hari kalender).
pub const DEFAULT_DAYS: i64 = 14;
/// Jumlah sprint selesai terakhir untuk rata-rata velocity.
pub const VELOCITY_WINDOW: usize = 3;

/// Isian sprint mentah dari form.
#[derive(Debug, Clone, Default)]
pub struct SprintInput {
    pub name: String,
    pub goal: String,
    /// `YYYY-MM-DD`.
    pub starts_on: String,
    pub ends_on: String,
    /// Angka atau kosong (= dari velocity).
    pub capacity: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidSprint {
    pub name: String,
    pub goal: String,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub capacity: Option<i32>,
}

/// Validasi isian sprint.
///
/// # Errors
///
/// Kode error per field.
pub fn validate(input: &SprintInput) -> Result<ValidSprint, Failure> {
    let name = input.name.trim().to_string();
    if name.is_empty() || name.chars().count() > NAME_MAX_LEN {
        return Err(Failure::field("name", NAME_INVALID));
    }
    let goal = input.goal.trim().to_string();
    if goal.chars().count() > GOAL_MAX_LEN {
        return Err(Failure::field("goal", TEXT_TOO_LONG));
    }
    let date = |s: &str| NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok();
    let (Some(starts_on), Some(ends_on)) = (date(&input.starts_on), date(&input.ends_on)) else {
        return Err(Failure::field("starts_on", DATES_INVALID));
    };
    if ends_on < starts_on || (ends_on - starts_on).num_days() > 180 {
        return Err(Failure::field("ends_on", DATES_INVALID));
    }
    let capacity = match input.capacity.trim() {
        "" => None,
        raw => match raw.parse::<i32>() {
            Ok(n) if (0..=999).contains(&n) => Some(n),
            _ => return Err(Failure::field("capacity", CAPACITY_INVALID)),
        },
    };
    Ok(ValidSprint {
        name,
        goal,
        starts_on,
        ends_on,
        capacity,
    })
}

/// Isian bawaan sprint baru: "Sprint N" mulai hari kerja setelah sprint terakhir, 2 minggu.
///
/// # Errors
///
/// Error database.
pub async fn suggestion<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project_id: Uuid,
) -> Result<SprintInput, DbErr> {
    let all = sprints::Model::of_project(db, wid, project_id).await?;
    let start = all.iter().map(|s| s.ends_on).max().map_or_else(
        || super::add_workdays(super::today(), 0),
        |d| super::add_workdays(d.succ_opt().unwrap_or(d), 0),
    );
    let end = start + chrono::Duration::days(DEFAULT_DAYS - 1);
    Ok(SprintInput {
        name: format!("Sprint {}", all.len() + 1),
        goal: String::new(),
        starts_on: start.to_string(),
        ends_on: end.to_string(),
        capacity: String::new(),
    })
}

async fn project_for(
    db: &impl ConnectionTrait,
    acting: &Acting<'_>,
    project_id: Uuid,
    permission: Permission,
) -> Result<projects::Model, Failure> {
    let project = projects::Model::find_in(db, acting.workspace_id(), project_id)
        .await?
        .ok_or(Failure::NotFound)?;
    access_in(db, acting, project.id, permission).await?;
    Ok(project)
}

/// Sprint yang boleh dibuka pelaku (`ViewProject`).
///
/// # Errors
///
/// `NotFound`, error database.
pub async fn find_visible<C: ConnectionTrait>(
    db: &C,
    acting: &Acting<'_>,
    sprint_id: Uuid,
) -> Result<sprints::Model, Failure> {
    let sprint = sprints::Model::find_in(db, acting.workspace_id(), sprint_id)
        .await?
        .ok_or(Failure::NotFound)?;
    access_in(db, acting, sprint.project_id, Permission::ViewProject).await?;
    Ok(sprint)
}

async fn editable<C: ConnectionTrait>(
    db: &C,
    acting: &Acting<'_>,
    sprint_id: Uuid,
) -> Result<sprints::Model, Failure> {
    let sprint = sprints::Model::find_in(db, acting.workspace_id(), sprint_id)
        .await?
        .ok_or(Failure::NotFound)?;
    access_in(db, acting, sprint.project_id, Permission::EditProject).await?;
    Ok(sprint)
}

/// Buat sprint (keadaan Rencana).
///
/// # Errors
///
/// Validasi, `NotFound`, error database.
pub async fn create(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    project_id: Uuid,
    input: &SprintInput,
) -> Result<sprints::Model, Failure> {
    let valid = validate(input)?;
    let txn = db.begin().await?;
    let project = project_for(&txn, acting, project_id, Permission::EditProject).await?;
    let sprint = sprints::ActiveModel {
        workspace_id: Set(acting.workspace_id().get()),
        project_id: Set(project.id),
        name: Set(valid.name),
        goal: Set(valid.goal),
        starts_on: Set(valid.starts_on),
        ends_on: Set(valid.ends_on),
        capacity: Set(valid.capacity),
        state: Set(SprintState::Planned.to_value()),
        report: Set(serde_json::json!({})),
        created_by: Set(Some(acting.user.id)),
        ..Default::default()
    }
    .insert(&txn)
    .await?;
    txn.commit().await?;
    Ok(sprint)
}

/// Ubah nama, tujuan, tanggal, kapasitas.
///
/// # Errors
///
/// Validasi, `NotFound`, `planning-sprint-state` (sprint selesai), error database.
pub async fn update(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    sprint_id: Uuid,
    input: &SprintInput,
) -> Result<sprints::Model, Failure> {
    let valid = validate(input)?;
    let sprint = editable(db, acting, sprint_id).await?;
    if sprint.state() == SprintState::Completed {
        return Err(Failure::field("sprint", SPRINT_STATE));
    }
    let mut m = sprint.into_active_model();
    m.name = Set(valid.name);
    m.goal = Set(valid.goal);
    m.starts_on = Set(valid.starts_on);
    m.ends_on = Set(valid.ends_on);
    m.capacity = Set(valid.capacity);
    Ok(m.update(db).await?)
}

/// Hapus sprint yang belum selesai; tiketnya kembali ke backlog.
///
/// # Errors
///
/// `NotFound`, `planning-sprint-state`, error database.
pub async fn delete(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    sprint_id: Uuid,
) -> Result<(), Failure> {
    let txn = db.begin().await?;
    let sprint = editable(&txn, acting, sprint_id).await?;
    if sprint.state() == SprintState::Completed {
        return Err(Failure::field("sprint", SPRINT_STATE));
    }
    let wid = acting.workspace_id();
    for t in tickets_of(&txn, wid, sprint.id).await? {
        record_move(&txn, wid, acting, &t, Some(&sprint), None).await?;
    }
    sprints::Entity::delete_by_id(sprint.id).exec(&txn).await?;
    txn.commit().await?;
    Ok(())
}

/// Tiket aktif (bukan di sampah) di sprint, urut backlog.
///
/// # Errors
///
/// Error database.
pub async fn tickets_of<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    sprint_id: Uuid,
) -> Result<Vec<tickets::Model>, DbErr> {
    tickets::Entity::find()
        .filter(tickets::Column::WorkspaceId.eq(wid.get()))
        .filter(tickets::Column::SprintId.eq(sprint_id))
        .filter(tickets::Column::DeletedAt.is_null())
        .order_by_asc(tickets::Column::BacklogRank)
        .order_by_asc(tickets::Column::Id)
        .all(db)
        .await
}

fn points(ts: &[tickets::Model]) -> i32 {
    ts.iter().filter_map(|t| t.story_points).sum()
}

/// Mulai sprint (boleh berjalan paralel dengan sprint aktif lain). Isian opsional menimpa nama,
/// tujuan, dan tanggal.
///
/// # Errors
///
/// Validasi, `NotFound`, `planning-sprint-state`, error database.
pub async fn start(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    sprint_id: Uuid,
    input: Option<&SprintInput>,
) -> Result<sprints::Model, Failure> {
    let valid = input.map(validate).transpose()?;
    let txn = db.begin().await?;
    let sprint = editable(&txn, acting, sprint_id).await?;
    if sprint.state() != SprintState::Planned {
        return Err(Failure::field("sprint", SPRINT_STATE));
    }
    let committed = points(&tickets_of(&txn, acting.workspace_id(), sprint.id).await?);
    let mut m = sprint.into_active_model();
    if let Some(v) = valid {
        m.name = Set(v.name);
        m.goal = Set(v.goal);
        m.starts_on = Set(v.starts_on);
        m.ends_on = Set(v.ends_on);
        m.capacity = Set(v.capacity);
    }
    m.state = Set(SprintState::Active.to_value());
    m.started_at = Set(Some(chrono::Utc::now().into()));
    m.committed_points = Set(committed);
    let sprint = m.update(&txn).await?;
    txn.commit().await?;
    Ok(sprint)
}

/// Tujuan tiket belum selesai saat sprint diselesaikan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompleteTarget {
    Sprint(Uuid),
    NewSprint,
    Backlog,
}

impl CompleteTarget {
    /// `"backlog"`, `"new"`, atau id sprint.
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim() {
            "backlog" | "" => Some(Self::Backlog),
            "new" => Some(Self::NewSprint),
            id => Uuid::parse_str(id).ok().map(Self::Sprint),
        }
    }
}

fn report_ticket(t: &tickets::Model, types: &[ticket_types::Model]) -> ReportTicket {
    ReportTicket {
        id: t.id,
        code: t.code.clone(),
        title: t.title.clone(),
        type_key: types
            .iter()
            .find(|ty| ty.id == t.type_id)
            .map(|ty| ty.key.clone())
            .unwrap_or_default(),
        points: t.story_points,
        estimate_hours: None,
        actual_hours: None,
    }
}

/// Selesaikan sprint aktif: simpan laporan beku, pindahkan tiket belum selesai ke `target`.
///
/// # Errors
///
/// `NotFound`, `planning-sprint-state`, `planning-target-invalid`, error database.
pub async fn complete(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    sprint_id: Uuid,
    target: CompleteTarget,
) -> Result<sprints::Model, Failure> {
    let wid = acting.workspace_id();
    let txn = db.begin().await?;
    let sprint = editable(&txn, acting, sprint_id).await?;
    if sprint.state() != SprintState::Active {
        return Err(Failure::field("sprint", SPRINT_STATE));
    }
    let to = match target {
        CompleteTarget::Backlog => None,
        CompleteTarget::Sprint(id) => {
            let s = sprints::Model::find_in(&txn, wid, id)
                .await?
                .filter(|s| {
                    s.project_id == sprint.project_id
                        && s.id != sprint.id
                        && s.state() != SprintState::Completed
                })
                .ok_or_else(|| Failure::field("target", TARGET_INVALID))?;
            Some(s)
        }
        CompleteTarget::NewSprint => {
            let start = super::add_workdays(sprint.ends_on.succ_opt().unwrap_or(sprint.ends_on), 0);
            let all = sprints::Model::of_project(&txn, wid, sprint.project_id).await?;
            Some(
                sprints::ActiveModel {
                    workspace_id: Set(wid.get()),
                    project_id: Set(sprint.project_id),
                    name: Set(format!("Sprint {}", all.len() + 1)),
                    goal: Set(String::new()),
                    starts_on: Set(start),
                    ends_on: Set(start + chrono::Duration::days(DEFAULT_DAYS - 1)),
                    state: Set(SprintState::Planned.to_value()),
                    report: Set(serde_json::json!({})),
                    created_by: Set(Some(acting.user.id)),
                    ..Default::default()
                }
                .insert(&txn)
                .await?,
            )
        }
    };
    let types = ticket_types::Model::all_in(&txn, wid).await?;
    let inside = tickets_of(&txn, wid, sprint.id).await?;
    let (done, not_done): (Vec<_>, Vec<_>) = inside.into_iter().partition(|t| t.status == "done");
    let events: Vec<scope::ScopeEvent> =
        crate::models::sprint_scope_events::Model::of_sprint(&txn, wid, sprint.id)
            .await?
            .iter()
            .map(scope::ScopeEvent::of)
            .collect();
    let added = sprint.started_at.map_or(0, |s| {
        scope::added_after(&events, s.with_timezone(&chrono::Utc)).0
    });
    let ids: Vec<Uuid> = done.iter().map(|t| t.id).collect();
    let est = super::estimates::latest(&txn, wid, &ids).await?;
    let act = super::estimates::actuals(&txn, wid, &done).await?;
    let round = |v: f64| (v * 10.0).round() / 10.0;
    let report = SprintReport {
        committed: sprint.committed_points,
        completed: points(&done),
        added,
        done: done
            .iter()
            .map(|t| {
                let mut r = report_ticket(t, &types);
                r.estimate_hours = est
                    .get(&t.id)
                    .and_then(super::estimates::Latest::hours)
                    .map(|(a, h)| round(a + h));
                r.actual_hours = act
                    .get(&t.id)
                    .map(|m| round(m.values().map(|(a, h)| a + h).sum()));
                r
            })
            .collect(),
        not_done: not_done.iter().map(|t| report_ticket(t, &types)).collect(),
        moved_to: to.as_ref().map(|s| s.name.clone()).unwrap_or_default(),
    };
    for t in &not_done {
        record_move(&txn, wid, acting, t, Some(&sprint), to.as_ref()).await?;
    }
    let mut m = sprint.into_active_model();
    m.state = Set(SprintState::Completed.to_value());
    m.completed_at = Set(Some(chrono::Utc::now().into()));
    m.completed_points = Set(report.completed);
    m.report = Set(serde_json::to_value(&report).unwrap_or_default());
    let sprint = m.update(&txn).await?;
    txn.commit().await?;
    Ok(sprint)
}

/// Pindahkan `ticket` dari sprint `from` ke `to` (None = backlog): kolom, log scope, riwayat.
async fn record_move<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    acting: &Acting<'_>,
    ticket: &tickets::Model,
    from: Option<&sprints::Model>,
    to: Option<&sprints::Model>,
) -> Result<tickets::Model, DbErr> {
    let mut m = ticket.clone().into_active_model();
    m.sprint_id = Set(to.map(|s| s.id));
    let updated = m.update(db).await?;
    scope::sprint_changed(db, wid, &updated, from.map(|s| s.id), to.map(|s| s.id)).await?;
    ticket_events::ActiveModel::record(
        db,
        wid,
        ticket.id,
        Some(acting.user.id),
        EventKind::Updated,
        serde_json::json!({
            "sprint": {
                "from": from.map(|s| s.name.clone()),
                "to": to.map(|s| s.name.clone()),
            }
        }),
    )
    .await?;
    Ok(updated)
}

/// Letak tiket setelah dipindah.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    Top,
    Bottom,
    /// Tetap di urutannya sekarang.
    Keep,
}

/// Ganti sprint satu tiket (dipakai juga ubah tiket dan bulk edit) di transaksi pemanggil.
/// `to` harus sprint project yang sama dan belum selesai.
///
/// # Errors
///
/// `planning-target-invalid`, error database.
pub async fn set_sprint_in<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    acting: &Acting<'_>,
    ticket: &tickets::Model,
    to: Option<Uuid>,
) -> Result<tickets::Model, Failure> {
    if ticket.sprint_id == to {
        return Ok(ticket.clone());
    }
    let target = match to {
        Some(id) => Some(
            sprints::Model::find_in(db, wid, id)
                .await?
                .filter(|s| {
                    s.project_id == ticket.project_id && s.state() != SprintState::Completed
                })
                .ok_or_else(|| Failure::field("sprint", TARGET_INVALID))?,
        ),
        None => None,
    };
    let from = match ticket.sprint_id {
        Some(id) => sprints::Model::find_in(db, wid, id).await?,
        None => None,
    };
    Ok(record_move(db, wid, acting, ticket, from.as_ref(), target.as_ref()).await?)
}

/// Pindahkan tiket ke sprint `to` (None = backlog) dan/atau ubah urutannya.
///
/// # Errors
///
/// `NotFound`, `planning-target-invalid`, error database.
pub async fn move_ticket(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    ticket_id: Uuid,
    to: Option<Uuid>,
    place: Place,
) -> Result<tickets::Model, Failure> {
    let wid = acting.workspace_id();
    let txn = db.begin().await?;
    let ticket = tickets::Model::lock_in(&txn, wid, ticket_id)
        .await?
        .filter(|t| !t.is_trashed())
        .ok_or(Failure::NotFound)?;
    access_in(&txn, acting, ticket.project_id, Permission::EditProject).await?;
    let mut ticket = set_sprint_in(&txn, wid, acting, &ticket, to).await?;
    if place != Place::Keep {
        let rank = edge_rank(&txn, wid, ticket.project_id, place == Place::Top).await?;
        let mut m = ticket.into_active_model();
        m.backlog_rank = Set(rank);
        ticket = m.update(&txn).await?;
    }
    txn.commit().await?;
    Ok(ticket)
}

/// Rank di atas semua (`top`) atau di bawah semua tiket project.
async fn edge_rank<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project_id: Uuid,
    top: bool,
) -> Result<i64, DbErr> {
    let q = tickets::Entity::find()
        .filter(tickets::Column::WorkspaceId.eq(wid.get()))
        .filter(tickets::Column::ProjectId.eq(project_id))
        .select_only()
        .column(tickets::Column::BacklogRank)
        .limit(1);
    let q = if top {
        q.order_by_asc(tickets::Column::BacklogRank)
    } else {
        q.order_by_desc(tickets::Column::BacklogRank)
    };
    let edge: Option<i64> = q.into_tuple().one(db).await?;
    Ok(match (edge, top) {
        (Some(r), true) => r - 1024,
        (Some(r), false) => r + 1024,
        (None, _) => 0,
    })
}

/// Rank tiket baru (paling bawah backlog).
///
/// # Errors
///
/// Error database.
pub async fn bottom_rank<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project_id: Uuid,
) -> Result<i64, DbErr> {
    edge_rank(db, wid, project_id, false).await
}

/// Velocity sprint selesai terakhir (lama → baru, maks. `n`): (nama, komitmen, selesai).
///
/// # Errors
///
/// Error database.
pub async fn velocity<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project_id: Uuid,
    n: usize,
) -> Result<Vec<sprints::Model>, DbErr> {
    let mut done: Vec<sprints::Model> = sprints::Model::of_project(db, wid, project_id)
        .await?
        .into_iter()
        .filter(|s| s.state() == SprintState::Completed)
        .collect();
    done.sort_by_key(|s| (s.completed_at, s.id));
    let skip = done.len().saturating_sub(n);
    Ok(done.into_iter().skip(skip).collect())
}

/// Rata-rata velocity `VELOCITY_WINDOW` sprint selesai terakhir (`None` bila belum ada).
#[must_use]
pub fn average_velocity(done: &[sprints::Model]) -> Option<f64> {
    let last: Vec<&sprints::Model> = done.iter().rev().take(VELOCITY_WINDOW).collect();
    if last.is_empty() {
        return None;
    }
    #[allow(clippy::cast_precision_loss)]
    let avg = last
        .iter()
        .map(|s| f64::from(s.completed_points))
        .sum::<f64>()
        / last.len() as f64;
    Some(avg)
}

/// Kapasitas sprint: isian manual, atau rata-rata velocity dibulatkan.
#[must_use]
pub fn capacity(sprint: &sprints::Model, avg_velocity: Option<f64>) -> Option<i32> {
    #[allow(clippy::cast_possible_truncation)]
    sprint
        .capacity
        .or_else(|| avg_velocity.map(|v| v.round() as i32))
}
