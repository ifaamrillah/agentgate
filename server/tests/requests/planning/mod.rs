//! Planning (M9): backlog & sprint (rencana → mulai → selesai + laporan beku), laporan, roadmap,
//! kalender, rilis + release notes, components, pencarian lanjutan, saved filter, bulk edit.

mod backlog;
mod components;
mod estimate;
mod mcp;
mod releases;
mod reports;
mod timeline;

use agentgate_server::models::{projects, sprints};
use loco_rs::app::AppContext;

pub use super::tickets::{Fixture, create_ticket, fixture, ticket};

/// Project KSR di workspace fixture.
pub async fn ksr(ctx: &AppContext, f: &Fixture) -> projects::Model {
    projects::Model::find_by_key(&ctx.db, f.workspace.workspace_id(), "KSR")
        .await
        .unwrap()
        .unwrap()
}

/// Sprint project KSR bernama `name`.
pub async fn sprint(ctx: &AppContext, f: &Fixture, name: &str) -> sprints::Model {
    let p = ksr(ctx, f).await;
    sprints::Model::of_project(&ctx.db, f.workspace.workspace_id(), p.id)
        .await
        .unwrap()
        .into_iter()
        .find(|s| s.name == name)
        .expect("sprint ada")
}
