//! Ekspor data workspace (M11): tipis, logika di `services::exports::run`. Idempoten: ekspor
//! di-claim (`queued → running`) dengan update bersyarat; job kedua untuk ekspor yang sama
//! tidak melakukan apa pun. Baris yang tertahan disapu task `export_sweep`.
use loco_rs::{app::AppContext, bgworker::BackgroundWorker};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataExportArgs {
    pub export_id: Uuid,
}

pub struct DataExportWorker {
    ctx: AppContext,
}

#[async_trait::async_trait]
impl BackgroundWorker<DataExportArgs> for DataExportWorker {
    fn build(ctx: &AppContext) -> Self {
        Self { ctx: ctx.clone() }
    }

    async fn perform(&self, args: DataExportArgs) -> loco_rs::Result<()> {
        crate::services::exports::run(&self.ctx, args.export_id).await?;
        Ok(())
    }
}
