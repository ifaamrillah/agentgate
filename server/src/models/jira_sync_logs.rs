//! Log sinkron Jira (M11): satu baris per perubahan yang dikirim/ditarik, ditolak gate, atau
//! gagal. Pesan disimpan sebagai kunci Fluent (`message`) + argumen (`args`: `detail`, `status`,
//! `stage`, `size`, `email`), diterjemahkan saat dirender, bukan kalimat jadi.

use chrono::{DateTime, Duration, Utc};
use sea_orm::{
    ActiveValue::Set,
    Condition, PaginatorTrait, QueryOrder, QuerySelect,
    entity::prelude::*,
    sea_query::{Expr, ExprTrait, Func},
};
use serde::{Deserialize, Serialize};

pub use super::_entities::jira_sync_logs::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

standard_behavior!(ActiveModel);

/// Lama baris log disimpan (hari); lebih tua dihapus job pembersihan.
pub const RETENTION_DAYS: i64 = 30;

/// Arah perubahan (CHECK `jira_sync_logs.direction`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// Jira → Agent Gate.
    #[sea_orm(string_value = "in")]
    In,
    /// Agent Gate → Jira.
    #[sea_orm(string_value = "out")]
    Out,
}

/// Hasil (CHECK `jira_sync_logs.result`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum SyncResult {
    #[sea_orm(string_value = "ok")]
    Ok,
    /// Ditolak gate (status Jira dikembalikan) atau stage tujuan butuh gate.
    #[sea_orm(string_value = "gate")]
    Gate,
    #[sea_orm(string_value = "fail")]
    Fail,
}

/// Argumen pesan log (nama tetap supaya template bisa meneruskannya ke `t()`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogArgs {
    #[serde(default)]
    pub detail: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub stage: String,
    #[serde(default)]
    pub size: String,
    #[serde(default)]
    pub email: String,
}

/// Baris log baru.
#[derive(Debug, Clone)]
pub struct NewLog {
    pub direction: Direction,
    pub result: SyncResult,
    pub ticket_id: Option<Uuid>,
    pub ticket_code: String,
    pub issue_key: String,
    /// Kunci Fluent (`jira-log-…`).
    pub message: &'static str,
    pub args: LogArgs,
}

impl NewLog {
    #[must_use]
    pub fn new(direction: Direction, result: SyncResult, message: &'static str) -> Self {
        Self {
            direction,
            result,
            ticket_id: None,
            ticket_code: String::new(),
            issue_key: String::new(),
            message,
            args: LogArgs::default(),
        }
    }

    #[must_use]
    pub fn ticket(mut self, id: Uuid, code: &str) -> Self {
        self.ticket_id = Some(id);
        self.ticket_code = code.to_string();
        self
    }

    #[must_use]
    pub fn issue(mut self, key: &str) -> Self {
        self.issue_key = key.to_string();
        self
    }

    #[must_use]
    pub fn args(mut self, args: LogArgs) -> Self {
        self.args = args;
        self
    }

    #[must_use]
    pub fn detail(mut self, detail: &str) -> Self {
        self.args.detail = detail.to_string();
        self
    }
}

/// Filter halaman Log sinkron.
#[derive(Debug, Clone, Default)]
pub struct LogQuery {
    pub results: Vec<String>,
    pub directions: Vec<String>,
    pub q: Option<String>,
    pub offset: u64,
    pub limit: u64,
}

/// Hitungan 24 jam terakhir untuk Ringkasan.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct DayCounts {
    pub incoming: u64,
    pub outgoing: u64,
    pub gate: u64,
}

impl Model {
    #[must_use]
    pub fn args(&self) -> LogArgs {
        serde_json::from_value(self.args.clone()).unwrap_or_default()
    }

    /// Halaman log koneksi, terbaru dulu.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn page<C: ConnectionTrait>(
        db: &C,
        connection_id: Uuid,
        query: &LogQuery,
    ) -> Result<(Vec<Self>, u64), DbErr> {
        let mut cond = Condition::all().add(Column::ConnectionId.eq(connection_id));
        let results: Vec<String> = query
            .results
            .iter()
            .filter(|r| SyncResult::try_from_value(r).is_ok())
            .cloned()
            .collect();
        if !results.is_empty() {
            cond = cond.add(Column::Result.is_in(results));
        }
        let dirs: Vec<String> = query
            .directions
            .iter()
            .filter(|r| Direction::try_from_value(r).is_ok())
            .cloned()
            .collect();
        if !dirs.is_empty() {
            cond = cond.add(Column::Direction.is_in(dirs));
        }
        if let Some(q) = &query.q {
            let pattern = crate::common::text::contains_pattern(q);
            cond = cond.add(
                Condition::any()
                    .add(Expr::expr(Func::lower(Expr::col(Column::TicketCode))).like(pattern.clone()))
                    .add(Expr::expr(Func::lower(Expr::col(Column::IssueKey))).like(pattern.clone()))
                    .add(Expr::expr(Func::lower(Expr::cust("args::text"))).like(pattern)),
            );
        }
        let base = Entity::find().filter(cond);
        let total = base.clone().count(db).await?;
        let rows = base
            .order_by_desc(Column::CreatedAt)
            .order_by_desc(Column::Id)
            .offset(query.offset)
            .limit(query.limit)
            .all(db)
            .await?;
        Ok((rows, total))
    }

    /// Semua log koneksi (unduh JSONL), terbaru dulu, maksimal `limit` baris.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn all_of<C: ConnectionTrait>(
        db: &C,
        connection_id: Uuid,
        limit: u64,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::ConnectionId.eq(connection_id))
            .order_by_desc(Column::CreatedAt)
            .order_by_desc(Column::Id)
            .limit(limit)
            .all(db)
            .await
    }

    /// Hitungan 24 jam sebelum `now`.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn day_counts<C: ConnectionTrait>(
        db: &C,
        connection_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<DayCounts, DbErr> {
        let since = DateTimeWithTimeZone::from(now - Duration::hours(24));
        let base = || {
            Entity::find()
                .filter(Column::ConnectionId.eq(connection_id))
                .filter(Column::CreatedAt.gte(since))
        };
        Ok(DayCounts {
            incoming: base()
                .filter(Column::Direction.eq(Direction::In.to_value()))
                .filter(Column::Result.eq(SyncResult::Ok.to_value()))
                .count(db)
                .await?,
            outgoing: base()
                .filter(Column::Direction.eq(Direction::Out.to_value()))
                .filter(Column::Result.eq(SyncResult::Ok.to_value()))
                .count(db)
                .await?,
            gate: base()
                .filter(Column::Result.eq(SyncResult::Gate.to_value()))
                .count(db)
                .await?,
        })
    }

    /// Jumlah semua baris log koneksi.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn count_of<C: ConnectionTrait>(db: &C, connection_id: Uuid) -> Result<u64, DbErr> {
        Entity::find()
            .filter(Column::ConnectionId.eq(connection_id))
            .count(db)
            .await
    }
}

impl ActiveModel {
    /// Tulis satu baris log.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn write<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        connection_id: Uuid,
        log: NewLog,
    ) -> Result<Model, DbErr> {
        Self {
            workspace_id: Set(wid.get()),
            connection_id: Set(connection_id),
            direction: Set(log.direction.to_value()),
            result: Set(log.result.to_value()),
            ticket_id: Set(log.ticket_id),
            ticket_code: Set(log.ticket_code),
            issue_key: Set(log.issue_key),
            message: Set(log.message.to_string()),
            args: Set(serde_json::to_value(&log.args).unwrap_or_default()),
            ..Default::default()
        }
        .insert(db)
        .await
    }
}

/// Hapus log lebih tua dari [`RETENTION_DAYS`] hari (job pembersihan).
///
/// # Errors
///
/// Error database.
pub async fn delete_expired<C: ConnectionTrait>(
    db: &C,
    now: DateTime<Utc>,
    dry_run: bool,
) -> Result<u64, DbErr> {
    let cutoff = DateTimeWithTimeZone::from(now - Duration::days(RETENTION_DAYS));
    let filter = Column::CreatedAt.lt(cutoff);
    if dry_run {
        return Entity::find().filter(filter).count(db).await;
    }
    Ok(Entity::delete_many()
        .filter(filter)
        .exec(db)
        .await?
        .rows_affected)
}
