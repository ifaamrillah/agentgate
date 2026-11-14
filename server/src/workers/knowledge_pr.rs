//! PR knowledge (R2): tipis, logika di `services::knowledge::pr`. Gagal provider dicatat di
//! baris `knowledge_prs` (tidak di-retry; approve berikutnya membuat percobaan baru).
use loco_rs::{app::AppContext, bgworker::BackgroundWorker};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{common::settings::Settings, models::workspaces, services::knowledge::pr};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgePrArgs {
    pub workspace_id: Uuid,
    pub id: Uuid,
}

pub struct KnowledgePrWorker {
    ctx: AppContext,
}

#[async_trait::async_trait]
impl BackgroundWorker<KnowledgePrArgs> for KnowledgePrWorker {
    fn build(ctx: &AppContext) -> Self {
        Self { ctx: ctx.clone() }
    }

    async fn perform(&self, args: KnowledgePrArgs) -> loco_rs::Result<()> {
        let db = &self.ctx.db;
        let Some(ws) = workspaces::Model::find_by_id(db, args.workspace_id).await? else {
            return Ok(());
        };
        let settings = Settings::from_ctx(&self.ctx)?;
        pr::run(db, &settings, ws.workspace_id(), args.id).await?;
        Ok(())
    }
}
