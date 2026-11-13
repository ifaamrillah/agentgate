//! Kirim delivery webhook keluar (M11): tipis, logika di `services::webhooks::dispatch`.
//! Args hanya id delivery (tanpa secret/payload; dibaca ulang saat job jalan). Idempoten: baris
//! yang sudah dikirim/di-claim dilewati.
use loco_rs::{app::AppContext, bgworker::BackgroundWorker};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{common::settings::Settings, services::webhooks::dispatch};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct WebhookDeliveryArgs {
    pub delivery_ids: Vec<Uuid>,
}

pub struct WebhookDeliveryWorker {
    ctx: AppContext,
}

#[async_trait::async_trait]
impl BackgroundWorker<WebhookDeliveryArgs> for WebhookDeliveryWorker {
    fn build(ctx: &AppContext) -> Self {
        Self { ctx: ctx.clone() }
    }

    async fn perform(&self, args: WebhookDeliveryArgs) -> loco_rs::Result<()> {
        let settings = Settings::from_ctx(&self.ctx)?;
        dispatch::send_many(&self.ctx.db, &settings, &args.delivery_ids, chrono::Utc::now())
            .await?;
        Ok(())
    }
}

/// Enqueue pengiriman `ids` (best-effort, rules §5): gagal enqueue hanya dicatat; sapuan
/// scheduler (task `webhooks`) mengirimnya nanti.
pub async fn enqueue(ctx: &AppContext, ids: Vec<Uuid>) {
    if ids.is_empty() {
        return;
    }
    let n = ids.len();
    if let Err(e) =
        WebhookDeliveryWorker::perform_later(ctx, WebhookDeliveryArgs { delivery_ids: ids }).await
    {
        tracing::warn!(error = %e, deliveries = n, "enqueue webhook delivery failed; sweep will retry");
    }
}
