//! Embedding simbol code graph (R2) setelah index selesai: tipis, logika di
//! `services::graph::embed`. Galat mesin dicatat di `graph_repos.embed_error`; job tetap
//! selesai (pencarian tetap leksikal).
use loco_rs::{app::AppContext, bgworker::BackgroundWorker};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{common::settings::Settings, models::workspaces, services::graph::embed};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEmbedArgs {
    pub workspace_id: Uuid,
    pub repo_id: Uuid,
}

pub struct GraphEmbedWorker {
    ctx: AppContext,
}

#[async_trait::async_trait]
impl BackgroundWorker<GraphEmbedArgs> for GraphEmbedWorker {
    fn build(ctx: &AppContext) -> Self {
        Self { ctx: ctx.clone() }
    }

    async fn perform(&self, args: GraphEmbedArgs) -> loco_rs::Result<()> {
        let db = &self.ctx.db;
        let Some(ws) = workspaces::Model::find_by_id(db, args.workspace_id).await? else {
            return Ok(());
        };
        let settings = Settings::from_ctx(&self.ctx)?;
        embed::index_repo(
            db,
            &settings.graph.embedding,
            ws.workspace_id(),
            args.repo_id,
        )
        .await?;
        Ok(())
    }
}
