//! Ekspor data workspace (M11, spec "Ekspor data"): satu baris per permintaan. Status
//! `queued → running → ready | failed`, lalu `ready → expired` setelah [`RETENTION_DAYS`].
//!
//! Efek samping "wajib terjadi" (rules §5): status disimpan di baris ini, worker
//! (`workers::data_export`) meng-claim `queued` dengan update bersyarat, dan task
//! `export_sweep` meng-enqueue ulang baris yang tertahan ([`Model::stuck`]).
use chrono::{DateTime, Duration, Utc};
use loco_rs::model::{TenantEntity, TenantQueryExt};
use sea_orm::{
    ActiveEnum,
    ActiveValue::Set,
    ColumnTrait, ConnectionTrait, DbErr, DeriveActiveEnum, EntityTrait, EnumIter, Iterable,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
    sea_query::{Expr, ExprTrait},
};
use serde::Serialize;
use uuid::Uuid;

pub use super::_entities::data_exports::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

standard_behavior!(ActiveModel);

/// Berkas ekspor disimpan 7 hari setelah selesai.
pub const RETENTION_DAYS: i64 = 7;
/// Percobaan worker paling banyak sebelum ekspor dinyatakan gagal oleh sapuan.
pub const MAX_ATTEMPTS: i32 = 3;
/// Baris `queued` lebih lama dari ini dianggap enqueue-nya hilang; `running` tanpa kemajuan
/// selama [`STALE_RUNNING_MINUTES`] dianggap worker-nya mati.
pub const STALE_QUEUED_MINUTES: i64 = 10;
pub const STALE_RUNNING_MINUTES: i64 = 30;

/// Kode error (kunci Fluent).
pub const PARTS_REQUIRED: &str = "export-parts-required";
pub const RECIPIENT_INVALID: &str = "export-recipient-invalid";
pub const PROJECTS_INVALID: &str = "export-projects-invalid";
pub const FAILED_STORAGE: &str = "export-error-storage";
pub const FAILED_BUILD: &str = "export-error-build";
pub const FAILED_ATTEMPTS: &str = "export-error-attempts";

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum ExportStatus {
    #[sea_orm(string_value = "queued")]
    Queued,
    #[sea_orm(string_value = "running")]
    Running,
    #[sea_orm(string_value = "ready")]
    Ready,
    #[sea_orm(string_value = "failed")]
    Failed,
    #[sea_orm(string_value = "expired")]
    Expired,
}

/// Isi ekspor yang bisa dipilih (urutan tampil = urutan dikerjakan worker).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, DeriveActiveEnum, Serialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum ExportPart {
    /// Tiket, komentar, riwayat (JSON per project).
    #[sea_orm(string_value = "tickets")]
    Tickets,
    /// Artifact beserta versinya (Markdown/HTML).
    #[sea_orm(string_value = "artifacts")]
    Artifacts,
    /// Knowledge base (`.agentgate/knowledge/` per project).
    #[sea_orm(string_value = "knowledge")]
    Knowledge,
    /// Lampiran tiket dan evidence job (bisa besar; default mati).
    #[sea_orm(string_value = "attachments")]
    Attachments,
    /// Workflow, persona, skill pack (YAML).
    #[sea_orm(string_value = "config")]
    Config,
    /// Audit log 12 bulan (JSON Lines; default mati).
    #[sea_orm(string_value = "audit")]
    Audit,
}

impl ExportPart {
    /// Tercentang di form Ekspor baru.
    #[must_use]
    pub fn default_on(self) -> bool {
        !matches!(self, Self::Attachments | Self::Audit)
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::try_from_value(&value.trim().to_string()).ok()
    }

    /// Bagian Fluent `export-part-<key>`.
    #[must_use]
    pub fn key(self) -> String {
        self.to_value()
    }
}

/// Permintaan ekspor baru (sudah divalidasi service).
#[derive(Debug, Clone)]
pub struct NewExport {
    pub requested_by: Uuid,
    pub recipient_id: Uuid,
    pub parts: Vec<ExportPart>,
    pub project_ids: Vec<Uuid>,
}

impl TenantEntity for Entity {
    type TenantId = WorkspaceId;

    fn tenant_column() -> Column {
        Column::WorkspaceId
    }
}

impl Model {
    #[must_use]
    pub fn status(&self) -> ExportStatus {
        ExportStatus::try_from_value(&self.status).unwrap_or(ExportStatus::Failed)
    }

    /// Isi yang dipilih (nilai tak dikenal tidak mungkin: CHECK).
    #[must_use]
    pub fn parts(&self) -> Vec<ExportPart> {
        ExportPart::iter()
            .filter(|p| self.parts.contains(&p.to_value()))
            .collect()
    }

    #[must_use]
    pub fn phase(&self) -> Option<ExportPart> {
        self.phase.as_deref().and_then(ExportPart::parse)
    }

    /// Siap diunduh pada `now` (status `ready`, belum lewat `expires_at`, berkas ada).
    #[must_use]
    pub fn downloadable(&self, now: DateTime<Utc>) -> bool {
        self.status() == ExportStatus::Ready
            && self.storage_key.is_some()
            && self.expires_at.is_some_and(|e| e > now)
    }

    /// Nama berkas unduhan: `agentgate-<slug>-<YYYY-MM-DD>.zip`.
    #[must_use]
    pub fn filename(&self, slug: &str) -> String {
        format!(
            "agentgate-{slug}-{}.zip",
            self.created_at.with_timezone(&Utc).format("%Y-%m-%d")
        )
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

    /// Riwayat workspace, terbaru dulu.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn newest_first<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        statuses: &[ExportStatus],
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<Self>, u64), DbErr> {
        let mut select = Entity::find().in_tenant(wid);
        if !statuses.is_empty() {
            select = select.filter(Column::Status.is_in(statuses.iter().map(ActiveEnum::to_value)));
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
    pub async fn count_in<C: ConnectionTrait>(db: &C, wid: WorkspaceId) -> Result<u64, DbErr> {
        Entity::find().in_tenant(wid).count(db).await
    }

    /// Baris tertahan pada `now`: `queued` tanpa diproses atau `running` tanpa kemajuan.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn stuck<C: ConnectionTrait>(db: &C, now: DateTime<Utc>) -> Result<Vec<Self>, DbErr> {
        let queued = now - Duration::minutes(STALE_QUEUED_MINUTES);
        let running = now - Duration::minutes(STALE_RUNNING_MINUTES);
        Entity::find()
            .filter(
                sea_orm::Condition::any()
                    .add(
                        Column::Status
                            .eq(ExportStatus::Queued.to_value())
                            .and(Column::UpdatedAt.lt(queued)),
                    )
                    .add(
                        Column::Status
                            .eq(ExportStatus::Running.to_value())
                            .and(Column::UpdatedAt.lt(running)),
                    ),
            )
            .order_by_asc(Column::UpdatedAt)
            .all(db)
            .await
    }

    /// Ekspor `ready` yang `expires_at`-nya lewat.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn expired_due<C: ConnectionTrait>(
        db: &C,
        now: DateTime<Utc>,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::Status.eq(ExportStatus::Ready.to_value()))
            .filter(Column::ExpiresAt.lte(now))
            .all(db)
            .await
    }
}

impl ActiveModel {
    /// # Errors
    ///
    /// Error database.
    pub async fn create<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        new: NewExport,
    ) -> Result<Model, DbErr> {
        use sea_orm::ActiveModelTrait;
        Self {
            workspace_id: Set(wid.get()),
            requested_by: Set(Some(new.requested_by)),
            recipient_id: Set(Some(new.recipient_id)),
            parts: Set(new.parts.iter().map(ActiveEnum::to_value).collect()),
            project_ids: Set(new.project_ids),
            status: Set(ExportStatus::Queued.to_value()),
            ..Default::default()
        }
        .insert(db)
        .await
    }

    /// Ambil ekspor `queued` untuk diproses. `false` = sudah diambil worker lain (atau bukan
    /// `queued`): job ini selesai tanpa melakukan apa pun.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn claim<C: ConnectionTrait>(db: &C, id: Uuid) -> Result<bool, DbErr> {
        let now: chrono::DateTime<chrono::FixedOffset> = Utc::now().into();
        let res = Entity::update_many()
            .col_expr(
                Column::Status,
                Expr::value(ExportStatus::Running.to_value()),
            )
            .col_expr(Column::StartedAt, Expr::value(now))
            .col_expr(Column::UpdatedAt, Expr::value(now))
            .col_expr(Column::Progress, Expr::value(0))
            .col_expr(Column::Attempts, Expr::col(Column::Attempts).add(1))
            .filter(Column::Id.eq(id))
            .filter(Column::Status.eq(ExportStatus::Queued.to_value()))
            .exec(db)
            .await?;
        Ok(res.rows_affected == 1)
    }

    /// Kemajuan worker (juga menyegarkan `updated_at`, penanda hidup untuk sapuan).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn progress<C: ConnectionTrait>(
        db: &C,
        id: Uuid,
        phase: ExportPart,
        percent: i32,
    ) -> Result<(), DbErr> {
        let now: chrono::DateTime<chrono::FixedOffset> = Utc::now().into();
        Entity::update_many()
            .col_expr(Column::Phase, Expr::value(phase.to_value()))
            .col_expr(Column::Progress, Expr::value(percent.clamp(0, 99)))
            .col_expr(Column::UpdatedAt, Expr::value(now))
            .filter(Column::Id.eq(id))
            .filter(Column::Status.eq(ExportStatus::Running.to_value()))
            .exec(db)
            .await?;
        Ok(())
    }

    /// Selesai: berkas di `key`, berlaku [`RETENTION_DAYS`] hari. `false` = baris tidak lagi
    /// `running` (pemanggil menghapus berkasnya).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn finish<C: ConnectionTrait>(
        db: &C,
        id: Uuid,
        key: &str,
        size: i64,
    ) -> Result<bool, DbErr> {
        let now = Utc::now();
        let at: chrono::DateTime<chrono::FixedOffset> = now.into();
        let expires: chrono::DateTime<chrono::FixedOffset> =
            (now + Duration::days(RETENTION_DAYS)).into();
        let res = Entity::update_many()
            .col_expr(Column::Status, Expr::value(ExportStatus::Ready.to_value()))
            .col_expr(Column::Progress, Expr::value(100))
            .col_expr(Column::Phase, Expr::value(Option::<String>::None))
            .col_expr(Column::StorageKey, Expr::value(key))
            .col_expr(Column::SizeBytes, Expr::value(size))
            .col_expr(Column::FinishedAt, Expr::value(at))
            .col_expr(Column::ExpiresAt, Expr::value(expires))
            .col_expr(Column::UpdatedAt, Expr::value(at))
            .filter(Column::Id.eq(id))
            .filter(Column::Status.eq(ExportStatus::Running.to_value()))
            .exec(db)
            .await?;
        Ok(res.rows_affected == 1)
    }

    /// Gagal dengan kode `code` (dari `queued` atau `running`). `false` = sudah selesai.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn fail<C: ConnectionTrait>(db: &C, id: Uuid, code: &str) -> Result<bool, DbErr> {
        let now: chrono::DateTime<chrono::FixedOffset> = Utc::now().into();
        let res = Entity::update_many()
            .col_expr(Column::Status, Expr::value(ExportStatus::Failed.to_value()))
            .col_expr(Column::Error, Expr::value(code))
            .col_expr(Column::FinishedAt, Expr::value(now))
            .col_expr(Column::UpdatedAt, Expr::value(now))
            .filter(Column::Id.eq(id))
            .filter(Column::Status.is_in([
                ExportStatus::Queued.to_value(),
                ExportStatus::Running.to_value(),
            ]))
            .exec(db)
            .await?;
        Ok(res.rows_affected == 1)
    }

    /// Ulangi ekspor `failed`: kembali `queued` dari awal. `false` = bukan `failed`.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn requeue_failed<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        id: Uuid,
    ) -> Result<bool, DbErr> {
        Self::requeue(db, wid.get(), id, ExportStatus::Failed, true).await
    }

    /// Kembalikan baris tertahan (`queued`/`running`) ke `queued` (sapuan).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn requeue_stuck<C: ConnectionTrait>(db: &C, row: &Model) -> Result<bool, DbErr> {
        Self::requeue(db, row.workspace_id, row.id, row.status(), false).await
    }

    async fn requeue<C: ConnectionTrait>(
        db: &C,
        workspace_id: Uuid,
        id: Uuid,
        from: ExportStatus,
        reset_attempts: bool,
    ) -> Result<bool, DbErr> {
        let now: chrono::DateTime<chrono::FixedOffset> = Utc::now().into();
        let mut update = Entity::update_many()
            .col_expr(Column::Status, Expr::value(ExportStatus::Queued.to_value()))
            .col_expr(Column::Progress, Expr::value(0))
            .col_expr(Column::Phase, Expr::value(Option::<String>::None))
            .col_expr(Column::Error, Expr::value(""))
            .col_expr(
                Column::FinishedAt,
                Expr::value(Option::<chrono::DateTime<chrono::FixedOffset>>::None),
            )
            .col_expr(Column::UpdatedAt, Expr::value(now));
        if reset_attempts {
            update = update.col_expr(Column::Attempts, Expr::value(0));
        }
        let res = update
            .filter(Column::Id.eq(id))
            .filter(Column::WorkspaceId.eq(workspace_id))
            .filter(Column::Status.eq(from.to_value()))
            .exec(db)
            .await?;
        Ok(res.rows_affected == 1)
    }

    /// Tandai `expired` dan lepas key berkas (berkasnya dihapus pemanggil setelah ini).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn expire<C: ConnectionTrait>(db: &C, id: Uuid) -> Result<bool, DbErr> {
        let now: chrono::DateTime<chrono::FixedOffset> = Utc::now().into();
        let res = Entity::update_many()
            .col_expr(
                Column::Status,
                Expr::value(ExportStatus::Expired.to_value()),
            )
            .col_expr(Column::StorageKey, Expr::value(Option::<String>::None))
            .col_expr(Column::UpdatedAt, Expr::value(now))
            .filter(Column::Id.eq(id))
            .filter(Column::Status.eq(ExportStatus::Ready.to_value()))
            .exec(db)
            .await?;
        Ok(res.rows_affected == 1)
    }
}
