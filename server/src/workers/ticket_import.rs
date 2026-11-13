//! Impor CSV tiket (M11): tipis, logika di `services::imports::run`. Idempoten: baris impor
//! di-claim dengan update bersyarat dan setiap tiket dibuat bersama kenaikan `processed`, jadi
//! job yang diulang (retry, sapuan `export_sweep`) melanjutkan, tidak menggandakan tiket.
use loco_rs::{app::AppContext, bgworker::BackgroundWorker};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicketImportArgs {
    pub import_id: Uuid,
}

pub struct TicketImportWorker {
    ctx: AppContext,
}

#[async_trait::async_trait]
impl BackgroundWorker<TicketImportArgs> for TicketImportWorker {
    fn build(ctx: &AppContext) -> Self {
        Self { ctx: ctx.clone() }
    }

    async fn perform(&self, args: TicketImportArgs) -> loco_rs::Result<()> {
        crate::services::imports::run(&self.ctx, args.import_id).await?;
        Ok(())
    }
}
