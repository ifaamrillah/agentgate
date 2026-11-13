//! Pengiriman webhook (M11): satu baris per event × webhook, sekaligus antrean kirim (outbox)
//! dan log pengiriman. `pending` + `next_attempt_at` = menunggu dikirim; pengirim meng-claim
//! baris dengan update bersyarat ([`Model::claim`], rules §5 race guard) sebelum mengirim,
//! sehingga worker dan sapuan scheduler tidak mengirim baris yang sama dua kali.
//!
//! Percobaan gagal dijadwalkan ulang dengan jeda bertahap ([`RETRY_DELAYS`]); setelah
//! [`MAX_ATTEMPTS`] percobaan baris menjadi `failed`. Baris lebih tua dari [`RETENTION_DAYS`]
//! dihapus job pembersihan.

use chrono::{DateTime, Duration, Utc};
use sea_orm::{
    ActiveValue::Set,
    Condition, PaginatorTrait, QueryOrder, QuerySelect,
    entity::prelude::*,
    sea_query::{Expr, ExprTrait, Func},
};

pub use super::_entities::webhook_deliveries::{ActiveModel, Column, Entity, Model};
use super::{outbox_events::WebhookEvent, workspaces::WorkspaceId};

standard_behavior!(ActiveModel);

/// Percobaan kirim per delivery (termasuk yang pertama).
pub const MAX_ATTEMPTS: i32 = 5;
/// Jeda sebelum percobaan ke-2, ke-3, … (menit).
pub const RETRY_DELAYS: [i64; 4] = [1, 5, 30, 120];
/// Lama baris disimpan (log pengiriman 30 hari terakhir).
pub const RETENTION_DAYS: i64 = 30;
/// Batas potongan respons yang disimpan (byte).
pub const RESPONSE_MAX: usize = 2_000;
/// Lama claim sebelum baris boleh di-claim ulang (pengirim mati di tengah jalan).
pub const CLAIM_SECONDS: i64 = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, serde::Serialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum DeliveryStatus {
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "succeeded")]
    Succeeded,
    #[sea_orm(string_value = "failed")]
    Failed,
}

/// Hasil satu percobaan kirim.
#[derive(Debug, Clone, Default)]
pub struct Attempt {
    /// Status HTTP; `None` = gagal sebelum ada respons (DNS, koneksi, timeout, URL ditolak).
    pub status: Option<u16>,
    pub body: String,
    pub error: String,
    pub duration_ms: i64,
}

impl Attempt {
    #[must_use]
    pub fn succeeded(&self) -> bool {
        self.status.is_some_and(|s| (200..300).contains(&s))
    }
}

/// Filter log pengiriman.
#[derive(Debug, Clone, Default)]
pub struct LogQuery {
    pub webhook_ids: Vec<Uuid>,
    pub events: Vec<String>,
    /// `ok` / `fail` / `pending`.
    pub results: Vec<String>,
    pub q: Option<String>,
    pub offset: u64,
    pub limit: u64,
}

impl Model {
    /// # Errors
    ///
    /// Nilai kolom bukan varian [`DeliveryStatus`].
    pub fn status(&self) -> Result<DeliveryStatus, DbErr> {
        DeliveryStatus::try_from_value(&self.status)
    }

    /// # Errors
    ///
    /// Nilai kolom bukan varian [`WebhookEvent`].
    pub fn event(&self) -> Result<WebhookEvent, DbErr> {
        WebhookEvent::try_from_value(&self.event)
    }

    /// Delivery `id` milik workspace.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn find_in<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        id: Uuid,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(id)
            .filter(Column::WorkspaceId.eq(wid.get()))
            .one(db)
            .await
    }

    /// Claim baris untuk dikirim sekarang: hanya bila masih `pending` dan jatuh tempo pada
    /// `now`. Claim memundurkan `next_attempt_at` sejauh [`CLAIM_SECONDS`] supaya pengirim lain
    /// tidak mengambilnya; `true` bila pemanggil yang mendapatkannya.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn claim<C: ConnectionTrait>(
        db: &C,
        id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<bool, DbErr> {
        let res = Entity::update_many()
            .col_expr(
                Column::NextAttemptAt,
                Expr::value(DateTimeWithTimeZone::from(
                    now + Duration::seconds(CLAIM_SECONDS),
                )),
            )
            .filter(Column::Id.eq(id))
            .filter(Column::Status.eq(DeliveryStatus::Pending.to_value()))
            .filter(Column::NextAttemptAt.lte(DateTimeWithTimeZone::from(now)))
            .exec(db)
            .await?;
        Ok(res.rows_affected == 1)
    }

    /// Id delivery pending yang jatuh tempo pada `now` (terlama dulu), untuk sapuan.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn due<C: ConnectionTrait>(
        db: &C,
        now: DateTime<Utc>,
        limit: u64,
    ) -> Result<Vec<Uuid>, DbErr> {
        Entity::find()
            .select_only()
            .column(Column::Id)
            .filter(Column::Status.eq(DeliveryStatus::Pending.to_value()))
            .filter(Column::NextAttemptAt.lte(DateTimeWithTimeZone::from(now)))
            .order_by_asc(Column::NextAttemptAt)
            .limit(limit)
            .into_tuple()
            .all(db)
            .await
    }

    /// Satu halaman log pengiriman workspace (terbaru dulu) dan totalnya.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn log_page<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        query: &LogQuery,
    ) -> Result<(Vec<Self>, u64), DbErr> {
        let mut cond = Condition::all().add(Column::WorkspaceId.eq(wid.get()));
        if !query.webhook_ids.is_empty() {
            cond = cond.add(Column::WebhookId.is_in(query.webhook_ids.clone()));
        }
        if !query.events.is_empty() {
            cond = cond.add(Column::Event.is_in(query.events.clone()));
        }
        if !query.results.is_empty() {
            let mut any = Condition::any();
            for r in &query.results {
                any = match r.as_str() {
                    "ok" => any.add(Column::Status.eq(DeliveryStatus::Succeeded.to_value())),
                    "fail" => any.add(Column::Status.eq(DeliveryStatus::Failed.to_value())),
                    "pending" => any.add(Column::Status.eq(DeliveryStatus::Pending.to_value())),
                    _ => any,
                };
            }
            cond = cond.add(any);
        }
        if let Some(q) = &query.q {
            let pattern = crate::common::text::contains_pattern(q);
            cond = cond.add(
                Condition::any()
                    .add(Expr::expr(Func::lower(Expr::col(Column::Event))).like(pattern.clone()))
                    .add(Expr::expr(Func::lower(Expr::col(Column::Body))).like(pattern)),
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
}

impl ActiveModel {
    /// Delivery pending baru (jatuh tempo sekarang).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn enqueue<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        webhook_id: Uuid,
        event_id: Option<Uuid>,
        event: WebhookEvent,
        body: String,
    ) -> Result<Model, DbErr> {
        let d = Self {
            workspace_id: Set(wid.get()),
            webhook_id: Set(webhook_id),
            event_id: Set(event_id),
            event: Set(event.to_value()),
            body: Set(body),
            status: Set(DeliveryStatus::Pending.to_value()),
            next_attempt_at: Set(Some(Utc::now().into())),
            ..Default::default()
        }
        .insert(db)
        .await?;
        // Dikirim worker setelah request (dan transaksinya) selesai; di luar request, sapuan.
        super::outbox_events::note_pending(&[d.id]);
        Ok(d)
    }

    /// Catat hasil percobaan pada baris yang sudah di-claim: berhasil → `succeeded`; gagal →
    /// dijadwalkan ulang, atau `failed` setelah [`MAX_ATTEMPTS`] (atau bila `give_up`).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn record_attempt<C: ConnectionTrait>(
        db: &C,
        delivery: Model,
        attempt: &Attempt,
        now: DateTime<Utc>,
        give_up: bool,
    ) -> Result<Model, DbErr> {
        let attempts = delivery.attempts + 1;
        let mut m: Self = delivery.into();
        m.attempts = Set(attempts);
        m.response_status = Set(attempt.status.map(i32::from));
        m.response_body = Set(clip(&attempt.body, RESPONSE_MAX));
        m.error = Set(clip(&attempt.error, 500));
        m.duration_ms = Set(Some(i32::try_from(attempt.duration_ms).unwrap_or(i32::MAX)));
        if attempt.succeeded() {
            m.status = Set(DeliveryStatus::Succeeded.to_value());
            m.next_attempt_at = Set(None);
            m.delivered_at = Set(Some(now.into()));
        } else if give_up || attempts >= MAX_ATTEMPTS {
            m.status = Set(DeliveryStatus::Failed.to_value());
            m.next_attempt_at = Set(None);
        } else {
            let idx = usize::try_from(attempts - 1).unwrap_or(0);
            let delay = RETRY_DELAYS.get(idx).copied().unwrap_or(120);
            m.next_attempt_at = Set(Some((now + Duration::minutes(delay)).into()));
        }
        m.update(db).await
    }
}

/// Tandai semua delivery pending milik webhook sebagai gagal (webhook dinonaktifkan/dihapus).
///
/// # Errors
///
/// Error database.
pub async fn fail_pending_of<C: ConnectionTrait>(
    db: &C,
    webhook_id: Uuid,
    error: &str,
) -> Result<u64, DbErr> {
    Ok(Entity::update_many()
        .col_expr(
            Column::Status,
            Expr::value(DeliveryStatus::Failed.to_value()),
        )
        .col_expr(
            Column::NextAttemptAt,
            Expr::value(Option::<DateTimeWithTimeZone>::None),
        )
        .col_expr(Column::Error, Expr::value(error.to_string()))
        .col_expr(
            Column::UpdatedAt,
            Expr::value(DateTimeWithTimeZone::from(Utc::now())),
        )
        .filter(Column::WebhookId.eq(webhook_id))
        .filter(Column::Status.eq(DeliveryStatus::Pending.to_value()))
        .exec(db)
        .await?
        .rows_affected)
}

/// Hapus delivery lebih tua dari [`RETENTION_DAYS`] (job pembersihan).
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
    let filter = Condition::all()
        .add(Column::CreatedAt.lt(cutoff))
        .add(Column::Status.ne(DeliveryStatus::Pending.to_value()));
    if dry_run {
        return Entity::find().filter(filter).count(db).await;
    }
    Ok(Entity::delete_many()
        .filter(filter)
        .exec(db)
        .await?
        .rows_affected)
}

/// Potong `s` sampai `max` byte di batas karakter.
fn clip(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}
