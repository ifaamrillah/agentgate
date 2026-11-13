//! Sinkron Jira (M11): tipis, logika di `services::jira::sync`. Args hanya id koneksi + jenis
//! putaran; koneksi dibaca ulang saat job jalan (dijeda/diputus → dilewati). Putaran yang
//! bersamaan untuk koneksi yang sama ditolak lewat claim di database.
use loco_rs::{app::AppContext, bgworker::BackgroundWorker};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    common::settings::Settings,
    models::workspaces::WorkspaceId,
    services::jira::sync::{self, RunKind},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraSyncArgs {
    pub connection_id: Uuid,
    pub kind: RunKind,
}

pub struct JiraSyncWorker {
    ctx: AppContext,
}

#[async_trait::async_trait]
impl BackgroundWorker<JiraSyncArgs> for JiraSyncWorker {
    fn build(ctx: &AppContext) -> Self {
        Self { ctx: ctx.clone() }
    }

    async fn perform(&self, args: JiraSyncArgs) -> loco_rs::Result<()> {
        let settings = Settings::from_ctx(&self.ctx)?;
        sync::run(&self.ctx.db, &self.ctx.storage, &settings, args.connection_id, args.kind).await?;
        Ok(())
    }
}

/// Enqueue satu putaran (best-effort: gagal hanya dicatat; scheduler mengulang).
pub async fn enqueue(ctx: &AppContext, connection_id: Uuid, kind: RunKind) {
    if let Err(e) =
        JiraSyncWorker::perform_later(ctx, JiraSyncArgs { connection_id, kind }).await
    {
        tracing::warn!(error = %e, %connection_id, "enqueue jira sync failed; scheduler will retry");
    }
}

/// Dorong outbox bila workspace `wid` punya koneksi mirror/dua arah yang aktif.
pub async fn enqueue_push(ctx: &AppContext, wid: WorkspaceId) {
    match sync::pushable(&ctx.db, wid).await {
        Ok(Some(id)) => enqueue(ctx, id, RunKind::Push).await,
        Ok(None) => {}
        Err(e) => tracing::warn!(error = %e, "jira push lookup failed"),
    }
}
