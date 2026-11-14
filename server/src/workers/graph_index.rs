//! Index code graph oleh server (tombol Index ulang, PR di-merge ke branch utama): tipis,
//! logika di `services::graph::fetch`. Gagal → status repo `failed` beserta pesannya; job
//! tetap selesai (tidak di-retry: user bisa menekan Index ulang).
use loco_rs::{app::AppContext, bgworker::BackgroundWorker};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    common::settings::Settings,
    models::{repos, workspaces},
    services::graph::{embed, fetch, ingest},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphIndexArgs {
    pub workspace_id: Uuid,
    pub repo_id: Uuid,
    #[serde(default)]
    pub requested_by: Option<Uuid>,
    /// Index penuh (semua file diproses ulang).
    #[serde(default)]
    pub full: bool,
}

pub struct GraphIndexWorker {
    ctx: AppContext,
}

#[async_trait::async_trait]
impl BackgroundWorker<GraphIndexArgs> for GraphIndexWorker {
    fn build(ctx: &AppContext) -> Self {
        Self { ctx: ctx.clone() }
    }

    async fn perform(&self, args: GraphIndexArgs) -> loco_rs::Result<()> {
        let db = &self.ctx.db;
        let Some(ws) = workspaces::Model::find_by_id(db, args.workspace_id).await? else {
            return Ok(());
        };
        let wid = ws.workspace_id();
        let Some(repo) = repos::Model::find_in(db, wid, args.repo_id).await? else {
            return Ok(());
        };
        let settings = Settings::from_ctx(&self.ctx)?;
        match fetch::fetch_and_index(db, &settings, wid, &repo, args.requested_by, args.full).await
        {
            Ok(_) => embed::enqueue(&self.ctx, repo.workspace_id, repo.id).await?,
            Err(e) => {
                tracing::warn!(error = %e, repo = %repo.id, "graph index failed");
                ingest::fail(db, wid, &repo, &e.message()).await?;
            }
        }
        Ok(())
    }
}
