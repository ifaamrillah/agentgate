//! Impor tiket dari CSV (M11): satu baris per berkas yang diunggah, sekaligus state wizard
//! (unggah → pemetaan → pratinjau → impor). `draft` = masih di wizard; `queued`/`running` =
//! diproses `workers::ticket_import` mulai baris `processed` (dilanjutkan bila terhenti);
//! `done`/`failed`/`cancelled` = riwayat. `skipped_rows` = laporan baris dilewati, dikosongkan
//! setelah `report_expires_at` ([`REPORT_RETENTION_DAYS`]).
use chrono::{DateTime, Duration, Utc};
use loco_rs::model::{TenantEntity, TenantQueryExt};
use sea_orm::{
    ActiveEnum, ColumnTrait, ConnectionTrait, DbErr, DeriveActiveEnum, EntityTrait, EnumIter,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, sea_query::Expr,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use super::_entities::ticket_imports::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

standard_behavior!(ActiveModel);

/// Laporan baris dilewati disimpan 30 hari.
pub const REPORT_RETENTION_DAYS: i64 = 30;
/// Ukuran berkas CSV maksimal.
pub const MAX_BYTES: usize = 20 * 1024 * 1024;
/// Baris data maksimal per berkas.
pub const MAX_ROWS: usize = 5_000;
/// `queued`/`running` tanpa kemajuan selama ini disapu (enqueue ulang).
pub const STALE_MINUTES: i64 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum ImportStatus {
    #[sea_orm(string_value = "draft")]
    Draft,
    #[sea_orm(string_value = "queued")]
    Queued,
    #[sea_orm(string_value = "running")]
    Running,
    #[sea_orm(string_value = "done")]
    Done,
    #[sea_orm(string_value = "failed")]
    Failed,
    #[sea_orm(string_value = "cancelled")]
    Cancelled,
}

impl ImportStatus {
    /// Masih bisa diubah di wizard.
    #[must_use]
    pub fn is_draft(self) -> bool {
        self == Self::Draft
    }

    /// Sedang/akan diproses worker.
    #[must_use]
    pub fn is_pending(self) -> bool {
        matches!(self, Self::Queued | Self::Running)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum Delimiter {
    #[sea_orm(string_value = "comma")]
    Comma,
    #[sea_orm(string_value = "semicolon")]
    Semicolon,
}

impl Delimiter {
    #[must_use]
    pub fn byte(self) -> u8 {
        match self {
            Self::Comma => b',',
            Self::Semicolon => b';',
        }
    }
}

/// Field Agent Gate tujuan satu kolom CSV. Disimpan di `mapping` (jsonb) per kolom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ImportField {
    /// Kode tiket lama → alias.
    Alias,
    Title,
    Description,
    Type,
    Stage,
    Assignee,
    Priority,
    StoryPoints,
    Labels,
    Sprint,
    DueOn,
    CreatedAt,
    /// Kolom tidak diimpor.
    #[default]
    Skip,
}

impl ImportField {
    /// Nilai form/query (`snake_case`).
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::Alias => "alias",
            Self::Title => "title",
            Self::Description => "description",
            Self::Type => "type",
            Self::Stage => "stage",
            Self::Assignee => "assignee",
            Self::Priority => "priority",
            Self::StoryPoints => "story_points",
            Self::Labels => "labels",
            Self::Sprint => "sprint",
            Self::DueOn => "due_on",
            Self::CreatedAt => "created_at",
            Self::Skip => "skip",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        use sea_orm::Iterable;
        Self::iter().find(|f| f.key() == value.trim())
    }

    /// Satu kolom saja yang boleh memetakan field ini (Label boleh banyak kolom: ekspor Jira
    /// menulis satu kolom `Labels` per label).
    #[must_use]
    pub fn single(self) -> bool {
        !matches!(self, Self::Labels | Self::Skip)
    }
}

/// Satu baris laporan dilewati.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkippedRow {
    /// Nomor baris di berkas (header = baris 1).
    pub row: u32,
    /// Kode alasan (kunci Fluent) dan argumennya.
    pub reason: String,
    #[serde(default)]
    pub arg: String,
    /// Nilai asli baris.
    pub values: Vec<String>,
}

impl TenantEntity for Entity {
    type TenantId = WorkspaceId;

    fn tenant_column() -> Column {
        Column::WorkspaceId
    }
}

impl Model {
    #[must_use]
    pub fn status(&self) -> ImportStatus {
        ImportStatus::try_from_value(&self.status).unwrap_or(ImportStatus::Failed)
    }

    #[must_use]
    pub fn delimiter(&self) -> Delimiter {
        Delimiter::try_from_value(&self.delimiter).unwrap_or(Delimiter::Comma)
    }

    #[must_use]
    pub fn headers(&self) -> Vec<String> {
        serde_json::from_value(self.headers.clone()).unwrap_or_default()
    }

    /// Pemetaan per kolom (panjang selalu sama dengan header; kurang = Lewati).
    #[must_use]
    pub fn mapping(&self) -> Vec<ImportField> {
        let mut m: Vec<ImportField> =
            serde_json::from_value(self.mapping.clone()).unwrap_or_default();
        m.resize(self.headers().len(), ImportField::Skip);
        m
    }

    #[must_use]
    pub fn skipped_rows(&self) -> Vec<SkippedRow> {
        serde_json::from_value(self.skipped_rows.clone()).unwrap_or_default()
    }

    /// Laporan baris dilewati masih tersedia pada `now`.
    #[must_use]
    pub fn report_available(&self, now: DateTime<Utc>) -> bool {
        self.skipped > 0 && self.report_expires_at.is_some_and(|e| e > now)
    }

    /// # Errors
    ///
    /// Error database.
    pub async fn find_in<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        id: Uuid,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(id).in_tenant(wid).one(db).await
    }

    /// Riwayat (bukan draf), terbaru dulu.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn history<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        q: Option<&str>,
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<Self>, u64), DbErr> {
        let mut select = Entity::find()
            .in_tenant(wid)
            .filter(Column::Status.ne(ImportStatus::Draft.to_value()));
        if let Some(q) = q {
            select = select.filter(Column::Filename.contains(q));
        }
        let total = select.clone().count(db).await?;
        let rows = select
            .order_by_desc(Column::CreatedAt)
            .order_by_desc(Column::Id)
            .offset(offset)
            .limit(limit)
            .all(db)
            .await?;
        Ok((rows, total))
    }

    /// Jumlah riwayat (badge menu).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn history_count<C: ConnectionTrait>(db: &C, wid: WorkspaceId) -> Result<u64, DbErr> {
        Entity::find()
            .in_tenant(wid)
            .filter(Column::Status.ne(ImportStatus::Draft.to_value()))
            .count(db)
            .await
    }

    /// `queued`/`running` tanpa kemajuan sejak [`STALE_MINUTES`] (sapuan).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn stuck<C: ConnectionTrait>(db: &C, now: DateTime<Utc>) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::Status.is_in([
                ImportStatus::Queued.to_value(),
                ImportStatus::Running.to_value(),
            ]))
            .filter(Column::UpdatedAt.lt(now - Duration::minutes(STALE_MINUTES)))
            .all(db)
            .await
    }

    /// Draf yang ditinggalkan (tidak disentuh sejak `before`): berkasnya dibersihkan.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn abandoned_drafts<C: ConnectionTrait>(
        db: &C,
        before: DateTime<Utc>,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::Status.eq(ImportStatus::Draft.to_value()))
            .filter(Column::UpdatedAt.lt(before))
            .all(db)
            .await
    }
}

impl ActiveModel {
    /// Kosongkan laporan yang lewat `report_expires_at` pada `now`; kembalikan jumlahnya.
    /// `apply` false = hanya menghitung.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn purge_reports<C: ConnectionTrait>(
        db: &C,
        now: DateTime<Utc>,
        apply: bool,
    ) -> Result<u64, DbErr> {
        let due = Column::ReportExpiresAt.lte(now);
        if !apply {
            return Entity::find().filter(due).count(db).await;
        }
        let res = Entity::update_many()
            .col_expr(Column::SkippedRows, Expr::value(serde_json::json!([])))
            .col_expr(
                Column::ReportExpiresAt,
                Expr::value(Option::<chrono::DateTime<chrono::FixedOffset>>::None),
            )
            .filter(due)
            .exec(db)
            .await?;
        Ok(res.rows_affected)
    }

    /// Pindahkan `id` dari status `from` ke `to` (update bersyarat). `false` = status berbeda.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn transition<C: ConnectionTrait>(
        db: &C,
        id: Uuid,
        from: &[ImportStatus],
        to: ImportStatus,
    ) -> Result<bool, DbErr> {
        let now: chrono::DateTime<chrono::FixedOffset> = Utc::now().into();
        let mut update = Entity::update_many()
            .col_expr(Column::Status, Expr::value(to.to_value()))
            .col_expr(Column::UpdatedAt, Expr::value(now));
        if to == ImportStatus::Running {
            update = update.col_expr(Column::StartedAt, Expr::cust("COALESCE(started_at, now())"));
        }
        if matches!(
            to,
            ImportStatus::Done | ImportStatus::Failed | ImportStatus::Cancelled
        ) {
            update = update.col_expr(Column::FinishedAt, Expr::value(now));
        }
        let res = update
            .filter(Column::Id.eq(id))
            .filter(Column::Status.is_in(from.iter().map(ActiveEnum::to_value)))
            .exec(db)
            .await?;
        Ok(res.rows_affected == 1)
    }
}
