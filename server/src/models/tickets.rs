//! Tiket: unit kerja di project (spec "Tiket, board & manajemen proyek").
//!
//! - `code` = kode sekarang (dari template, `ticket_code`); kode lama tetap di `ticket_codes`
//!   sebagai alias ([`Model::find_by_code`] menjawab keduanya).
//! - `status` = kolom board M2 dan kategori stage di M3 ([`TicketStatus`]). "Mulai" berarti
//!   masuk `in_progress` atau `done` dari `backlog`/`todo`; relasi *blocks* menahan langkah itu
//!   (`ticket_links::open_blockers`, dicek `services::tickets::transition`).
//! - `deleted_at` = di sampah. Semua query halaman memakai [`active`] (tanpa yang di sampah);
//!   hanya halaman Sampah dan purge membaca yang terhapus.
//! - `custom_values` = nilai field custom per id field ([`CustomValues`]).
//!
//! Validasi isian di [`validate_input`] dipakai buat dan ubah.

use std::collections::BTreeMap;

use loco_rs::model::{TenantEntity, TenantQueryExt};
use sea_orm::{
    ActiveValue::Set,
    Condition, IntoActiveModel, Order, QueryOrder, QuerySelect,
    entity::prelude::*,
    sea_query::{ExprTrait, Func, SimpleExpr},
};
use serde::{Deserialize, Serialize};
use validator::{ValidationError, ValidationErrors};

pub use super::_entities::tickets::{ActiveModel, Column, Entity, Model};
use super::{ticket_codes, workspaces::WorkspaceId};
use crate::common::text;

pub const TITLE_REQUIRED: &str = "ticket-title-required";
pub const TITLE_TOO_LONG: &str = "ticket-title-too-long";
pub const DESCRIPTION_TOO_LONG: &str = "ticket-description-too-long";
pub const STORY_POINTS_INVALID: &str = "ticket-story-points-invalid";
pub const DUE_ON_INVALID: &str = "ticket-due-on-invalid";
pub const LABEL_INVALID: &str = "ticket-label-invalid";
pub const STATUS_INVALID: &str = "ticket-status-invalid";
pub const PRIORITY_INVALID: &str = "ticket-priority-invalid";
pub const TYPE_INVALID: &str = "ticket-type-invalid";
pub const ASSIGNEE_INVALID: &str = "ticket-assignee-invalid";
pub const PARENT_INVALID: &str = "ticket-parent-invalid";
/// Tiket diblokir tiket lain yang belum selesai, jadi belum bisa mulai.
pub const BLOCKED: &str = "ticket-blocked";
pub const ERROR_CODES: &[&str] = &[
    TITLE_REQUIRED,
    TITLE_TOO_LONG,
    DESCRIPTION_TOO_LONG,
    STORY_POINTS_INVALID,
    DUE_ON_INVALID,
    LABEL_INVALID,
    STATUS_INVALID,
    PRIORITY_INVALID,
    TYPE_INVALID,
    ASSIGNEE_INVALID,
    PARENT_INVALID,
    BLOCKED,
];

pub const TITLE_MAX_LEN: usize = 200;
pub const DESCRIPTION_MAX_LEN: usize = 50_000;
pub const LABEL_MAX_LEN: usize = 30;
pub const LABELS_MAX: usize = 10;
pub const STORY_POINTS_MAX: i32 = 1000;

/// Kolom board M2 = kategori stage M3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, DeriveActiveEnum, Serialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum TicketStatus {
    #[sea_orm(string_value = "backlog")]
    Backlog,
    #[sea_orm(string_value = "todo")]
    Todo,
    #[sea_orm(string_value = "in_progress")]
    InProgress,
    #[sea_orm(string_value = "done")]
    Done,
}

impl TicketStatus {
    /// Pekerjaan sudah dimulai (atau selesai).
    #[must_use]
    pub fn is_started(self) -> bool {
        matches!(self, Self::InProgress | Self::Done)
    }

    /// Status dari isian form; nilai tak dikenal → `None`.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::try_from_value(&value.trim().to_string()).ok()
    }
}

/// Keadaan tiket di stage sekarang (M3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, DeriveActiveEnum, Serialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum StageState {
    /// Sedang dikerjakan.
    #[sea_orm(string_value = "active")]
    Active,
    /// Menunggu review approver stage.
    #[sea_orm(string_value = "review")]
    Review,
    /// Putaran gagal mencapai batas: Butuh keputusan manusia.
    #[sea_orm(string_value = "decision")]
    Decision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, DeriveActiveEnum, Serialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum TicketPriority {
    #[sea_orm(string_value = "urgent")]
    Urgent,
    #[sea_orm(string_value = "high")]
    High,
    #[sea_orm(string_value = "medium")]
    Medium,
    #[sea_orm(string_value = "low")]
    Low,
}

impl TicketPriority {
    /// Label prioritas C (P1–P4).
    #[must_use]
    pub fn rank(self) -> u8 {
        match self {
            Self::Urgent => 1,
            Self::High => 2,
            Self::Medium => 3,
            Self::Low => 4,
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::try_from_value(&value.trim().to_string()).ok()
    }
}

/// Nilai field custom per id field (string yang sudah dinormalisasi `custom_fields::parse_value`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomValues(pub BTreeMap<Uuid, String>);

impl CustomValues {
    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_else(|_| serde_json::json!({}))
    }
}

/// Isian form tiket (buat dan ubah), mentah dari user.
#[derive(Debug, Clone, Default)]
pub struct TicketInput {
    pub title: String,
    pub description: String,
    pub status: String,
    pub priority: String,
    /// Label dipisah koma.
    pub labels: String,
    /// `YYYY-MM-DD` atau kosong.
    pub due_on: String,
    /// Angka atau kosong.
    pub story_points: String,
}

/// Isian yang lolos validasi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidTicket {
    pub title: String,
    pub description: String,
    pub status: TicketStatus,
    pub priority: TicketPriority,
    pub labels: Vec<String>,
    pub due_on: Option<chrono::NaiveDate>,
    pub story_points: Option<i32>,
}

/// Validasi isian tiket (semua field sekaligus).
///
/// # Errors
///
/// Error per field dengan kode di [`ERROR_CODES`].
pub fn validate_input(input: &TicketInput) -> Result<ValidTicket, ValidationErrors> {
    let mut errors = ValidationErrors::new();
    let title = input.title.trim().to_string();
    if title.is_empty() {
        errors.add("title", ValidationError::new(TITLE_REQUIRED));
    } else if title.chars().count() > TITLE_MAX_LEN {
        let mut e = ValidationError::new(TITLE_TOO_LONG);
        e.add_param("max".into(), &TITLE_MAX_LEN);
        errors.add("title", e);
    }
    let description = input.description.trim().to_string();
    if description.chars().count() > DESCRIPTION_MAX_LEN {
        let mut e = ValidationError::new(DESCRIPTION_TOO_LONG);
        e.add_param("max".into(), &DESCRIPTION_MAX_LEN);
        errors.add("description", e);
    }
    let status = if input.status.trim().is_empty() {
        Some(TicketStatus::Todo)
    } else {
        TicketStatus::parse(&input.status)
    };
    if status.is_none() {
        errors.add("status", ValidationError::new(STATUS_INVALID));
    }
    let priority = if input.priority.trim().is_empty() {
        Some(TicketPriority::Medium)
    } else {
        TicketPriority::parse(&input.priority)
    };
    if priority.is_none() {
        errors.add("priority", ValidationError::new(PRIORITY_INVALID));
    }
    let labels = match parse_labels(&input.labels) {
        Ok(l) => l,
        Err(e) => {
            errors.add("labels", e);
            Vec::new()
        }
    };
    let due_on = match input.due_on.trim() {
        "" => None,
        raw => match chrono::NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
            Ok(d) => Some(d),
            Err(_) => {
                errors.add("due_on", ValidationError::new(DUE_ON_INVALID));
                None
            }
        },
    };
    let story_points = match input.story_points.trim() {
        "" => None,
        raw => match raw.parse::<i32>() {
            Ok(n) if (0..=STORY_POINTS_MAX).contains(&n) => Some(n),
            _ => {
                let mut e = ValidationError::new(STORY_POINTS_INVALID);
                e.add_param("max".into(), &STORY_POINTS_MAX);
                errors.add("story_points", e);
                None
            }
        },
    };
    match (status, priority) {
        (Some(status), Some(priority)) if errors.is_empty() => Ok(ValidTicket {
            title,
            description,
            status,
            priority,
            labels,
            due_on,
            story_points,
        }),
        _ => Err(errors),
    }
}

/// Label dari isian dipisah koma: di-trim, unik tanpa membedakan huruf besar, maksimal
/// [`LABELS_MAX`], masing-masing 1–[`LABEL_MAX_LEN`] karakter tanpa koma.
///
/// # Errors
///
/// [`LABEL_INVALID`].
pub fn parse_labels(raw: &str) -> Result<Vec<String>, ValidationError> {
    let mut out: Vec<String> = Vec::new();
    for label in raw.split(',').map(str::trim).filter(|l| !l.is_empty()) {
        if label.chars().count() > LABEL_MAX_LEN || label.chars().any(char::is_control) {
            return Err(label_error());
        }
        if !out.iter().any(|l| l.eq_ignore_ascii_case(label)) {
            out.push(label.to_string());
        }
    }
    if out.len() > LABELS_MAX {
        return Err(label_error());
    }
    Ok(out)
}

fn label_error() -> ValidationError {
    let mut e = ValidationError::new(LABEL_INVALID);
    e.add_param("max".into(), &LABEL_MAX_LEN);
    e.add_param("count".into(), &LABELS_MAX);
    e
}

impl TenantEntity for Entity {
    type TenantId = WorkspaceId;

    fn tenant_column() -> Column {
        Column::WorkspaceId
    }
}

standard_behavior!(ActiveModel);

/// Filter "siapa" di board, daftar, dan Tiket saya.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersonFilter {
    AssignedTo(Uuid),
    ReportedBy(Uuid),
    WatchedBy(Uuid),
    Unassigned,
}

/// Urutan daftar tiket (kolom datatable).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TicketSort {
    Code,
    Title,
    Status,
    Priority,
    DueOn,
    Updated,
}

/// Query daftar/board: semua filter dikerjakan database.
#[derive(Debug, Clone, Default)]
pub struct TicketQuery {
    /// `None` = semua project yang boleh dibuka (`project_ids`).
    pub project_ids: Vec<Uuid>,
    pub q: Option<String>,
    pub statuses: Vec<TicketStatus>,
    pub priorities: Vec<TicketPriority>,
    pub type_ids: Vec<Uuid>,
    pub people: Vec<PersonFilter>,
    /// Kunci stage workflow (API publik `?stage=`); kosong = semua.
    pub stage_keys: Vec<String>,
    pub sort: Option<(TicketSort, bool)>,
    pub offset: u64,
    pub limit: u64,
}

/// Tiket yang tidak di sampah.
#[must_use]
pub fn active() -> SimpleExpr {
    Column::DeletedAt.is_null()
}

/// Ekspresi full-text yang sama dengan index `tickets_search_idx`.
fn search_condition(q: &str) -> Condition {
    let pattern = text::contains_pattern(q);
    Condition::any()
        .add(Expr::cust_with_values(
            "to_tsvector('simple', tickets.code || ' ' || tickets.title || ' ' || \
             tickets.description) @@ plainto_tsquery('simple', $1)",
            [q.trim().to_string()],
        ))
        .add(Expr::expr(Func::lower(Expr::col((Entity, Column::Code)))).like(pattern.clone()))
        .add(Expr::expr(Func::lower(Expr::col((Entity, Column::Title)))).like(pattern))
}

impl Model {
    /// # Errors
    ///
    /// Nilai kolom bukan varian [`TicketStatus`].
    pub fn status(&self) -> Result<TicketStatus, DbErr> {
        TicketStatus::try_from_value(&self.status)
    }

    /// # Errors
    ///
    /// Nilai kolom bukan varian [`TicketPriority`].
    pub fn priority(&self) -> Result<TicketPriority, DbErr> {
        TicketPriority::try_from_value(&self.priority)
    }

    /// # Errors
    ///
    /// Nilai kolom bukan varian [`StageState`].
    pub fn stage_state(&self) -> Result<StageState, DbErr> {
        StageState::try_from_value(&self.stage_state)
    }

    #[must_use]
    pub fn custom_values(&self) -> CustomValues {
        serde_json::from_value(self.custom_values.clone()).unwrap_or_default()
    }

    #[must_use]
    pub fn is_trashed(&self) -> bool {
        self.deleted_at.is_some()
    }

    /// Tiket dengan kode (sekarang atau alias) `code` di workspace ini, termasuk yang di
    /// sampah. `bool` = `code` adalah kode sekarang (bukan alias lama).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn find_by_code<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        code: &str,
    ) -> Result<Option<(Self, bool)>, DbErr> {
        let Some(alias) = ticket_codes::Model::find(db, workspace_id, code).await? else {
            return Ok(None);
        };
        let ticket = Entity::find_by_id(alias.ticket_id)
            .in_tenant(workspace_id)
            .one(db)
            .await?;
        Ok(ticket.map(|t| {
            let current = t.code.eq_ignore_ascii_case(code.trim());
            (t, current)
        }))
    }

    /// Tiket `id` di workspace ini (termasuk di sampah).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn find_in<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        id: Uuid,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(id).in_tenant(workspace_id).one(db).await
    }

    /// Seperti [`Self::find_in`] dengan baris dikunci sampai transaksi selesai.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn lock_in<C: ConnectionTrait>(
        txn: &C,
        workspace_id: WorkspaceId,
        id: Uuid,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(id)
            .in_tenant(workspace_id)
            .lock_exclusive()
            .one(txn)
            .await
    }

    /// Satu halaman tiket aktif yang cocok `query`, beserta total.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn page<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        query: &TicketQuery,
    ) -> Result<(Vec<Self>, u64), DbErr> {
        let select = filtered(workspace_id, query);
        let total = select.clone().count(db).await?;
        let (sort, desc) = query.sort.unwrap_or((TicketSort::Updated, true));
        let order = if desc { Order::Desc } else { Order::Asc };
        let select = match sort {
            TicketSort::Code => select
                .order_by(Column::Code, order.clone())
                .order_by(Column::Number, order.clone()),
            TicketSort::Title => select.order_by(Column::Title, order.clone()),
            TicketSort::Status => select.order_by(
                Expr::cust(
                    "CASE tickets.status WHEN 'backlog' THEN 0 WHEN 'todo' THEN 1 \
                     WHEN 'in_progress' THEN 2 ELSE 3 END",
                ),
                order.clone(),
            ),
            TicketSort::Priority => select.order_by(
                Expr::cust(
                    "CASE tickets.priority WHEN 'urgent' THEN 0 WHEN 'high' THEN 1 \
                     WHEN 'medium' THEN 2 ELSE 3 END",
                ),
                order.clone(),
            ),
            TicketSort::DueOn => select.order_by(Column::DueOn, order.clone()),
            TicketSort::Updated => select.order_by(Column::UpdatedAt, order.clone()),
        };
        let rows = select
            .order_by(Column::Id, order)
            .offset(query.offset)
            .limit(query.limit)
            .all(db)
            .await?;
        Ok((rows, total))
    }

    /// Semua tiket aktif project untuk board (filter `query`), urut kolom lalu posisi.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn board<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        query: &TicketQuery,
    ) -> Result<Vec<Self>, DbErr> {
        filtered(workspace_id, query)
            .order_by_asc(Column::Position)
            .order_by_asc(Column::Id)
            .limit(std::cmp::max(query.limit, 1))
            .all(db)
            .await
    }

    /// Tiket di sampah project ini, terbaru dihapus dulu.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn trashed_in<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        project_id: Uuid,
        q: Option<&str>,
    ) -> Result<Vec<Self>, DbErr> {
        let mut select = Entity::find()
            .in_tenant(workspace_id)
            .filter(Column::ProjectId.eq(project_id))
            .filter(Column::DeletedAt.is_not_null());
        if let Some(q) = q.filter(|q| !q.trim().is_empty()) {
            select = select.filter(search_condition(q));
        }
        select
            .order_by_desc(Column::DeletedAt)
            .order_by_desc(Column::Id)
            .all(db)
            .await
    }

    /// Jumlah tiket aktif per status di project ini.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn count_by_status<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        project_id: Uuid,
    ) -> Result<Vec<(String, i64)>, DbErr> {
        Entity::find()
            .select_only()
            .column(Column::Status)
            .column_as(Column::Id.count(), "n")
            .in_tenant(workspace_id)
            .filter(Column::ProjectId.eq(project_id))
            .filter(active())
            .group_by(Column::Status)
            .into_tuple()
            .all(db)
            .await
    }

    /// Ubah status (posisi baru di kolom). `done_at` diisi saat masuk `done`, dikosongkan saat
    /// keluar.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn set_status<C: ConnectionTrait>(
        self,
        db: &C,
        status: TicketStatus,
        position: i64,
    ) -> Result<Self, DbErr> {
        let was_done = self.status == TicketStatus::Done.to_value();
        let mut model = self.into_active_model();
        model.status = Set(status.to_value());
        model.position = Set(position);
        if status == TicketStatus::Done && !was_done {
            model.done_at = Set(Some(chrono::Utc::now().into()));
        } else if status != TicketStatus::Done {
            model.done_at = Set(None);
        }
        model.update(db).await
    }
}

fn filtered(workspace_id: WorkspaceId, query: &TicketQuery) -> sea_orm::Select<Entity> {
    let mut select = Entity::find()
        .in_tenant(workspace_id)
        .filter(active())
        .filter(Column::ProjectId.is_in(query.project_ids.iter().copied()));
    if let Some(q) = query.q.as_deref().filter(|q| !q.trim().is_empty()) {
        select = select.filter(search_condition(q));
    }
    if !query.statuses.is_empty() {
        select = select.filter(Column::Status.is_in(query.statuses.iter().map(|s| s.to_value())));
    }
    if !query.priorities.is_empty() {
        select =
            select.filter(Column::Priority.is_in(query.priorities.iter().map(|s| s.to_value())));
    }
    if !query.type_ids.is_empty() {
        select = select.filter(Column::TypeId.is_in(query.type_ids.iter().copied()));
    }
    if !query.stage_keys.is_empty() {
        select = select.filter(Column::StageKey.is_in(query.stage_keys.iter().cloned()));
    }
    if !query.people.is_empty() {
        let mut any = Condition::any();
        for person in &query.people {
            any = any.add(match person {
                PersonFilter::AssignedTo(id) => Column::AssigneeId.eq(*id),
                PersonFilter::ReportedBy(id) => Column::ReporterId.eq(*id),
                PersonFilter::Unassigned => Column::AssigneeId.is_null(),
                PersonFilter::WatchedBy(id) => {
                    Column::Id.in_subquery(sea_orm::QueryTrait::into_query(
                        super::ticket_watchers::Entity::find()
                            .select_only()
                            .column(super::ticket_watchers::Column::TicketId)
                            .filter(super::ticket_watchers::Column::UserId.eq(*id)),
                    ))
                }
            });
        }
        select = select.filter(any);
    }
    select
}

/// Nilai `position` untuk kartu di bawah kolom: waktu sekarang (milidetik).
#[must_use]
pub fn bottom_position() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_are_trimmed_and_unique() {
        assert_eq!(
            parse_labels(" Kasir, kasir ,Promo,,").unwrap(),
            vec!["Kasir".to_string(), "Promo".to_string()]
        );
        assert!(parse_labels(&"a".repeat(31)).is_err());
        let many: Vec<String> = (0..11).map(|i| format!("l{i}")).collect();
        assert!(parse_labels(&many.join(",")).is_err());
    }

    #[test]
    fn input_defaults_and_errors() {
        let ok = validate_input(&TicketInput {
            title: " Bayar QRIS ".into(),
            ..TicketInput::default()
        })
        .unwrap();
        assert_eq!(ok.title, "Bayar QRIS");
        assert_eq!(ok.status, TicketStatus::Todo);
        assert_eq!(ok.priority, TicketPriority::Medium);
        let err = validate_input(&TicketInput {
            title: String::new(),
            status: "nope".into(),
            story_points: "-1".into(),
            due_on: "2026-13-01".into(),
            ..TicketInput::default()
        })
        .unwrap_err();
        let fields = err.field_errors();
        for f in ["title", "status", "story_points", "due_on"] {
            assert!(fields.contains_key(f), "{f}");
        }
    }

    #[test]
    fn started_means_in_progress_or_done() {
        assert!(!TicketStatus::Backlog.is_started());
        assert!(!TicketStatus::Todo.is_started());
        assert!(TicketStatus::InProgress.is_started());
        assert!(TicketStatus::Done.is_started());
    }
}
