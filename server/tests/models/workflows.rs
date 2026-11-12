//! Workflow bawaan workspace dan data backfill migrasi M3.

use agentgate_core::workflow::{Definition, templates};
use agentgate_server::{
    app::App,
    models::{ticket_types, workflows},
};
use loco_rs::testing::prelude::*;

use crate::support::{a_user, a_workspace};

/// Definisi yang ditulis migrasi untuk workspace lama = definisi template Kanban/Subtask.
#[test]
fn workflow_backfill_matches_templates() {
    let kanban: Definition =
        serde_json::from_str(migration::m20261114_000001_create_workflows::KANBAN_JSON).unwrap();
    assert_eq!(kanban, templates::kanban().document().definition);
    let subtask: Definition =
        serde_json::from_str(migration::m20261114_000001_create_workflows::SUBTASK_JSON).unwrap();
    assert_eq!(subtask, templates::SUBTASK.document().definition);
}

/// Workspace baru: Kanban (semua tipe) dan Subtask ringkas (tipe Subtask).
#[tokio::test]
async fn new_workspace_gets_builtin_workflows() {
    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    let owner = a_user().create(ctx).await;
    let workspace = a_workspace(&owner).create(ctx).await;
    let wid = workspace.workspace_id();
    let all = workflows::Model::all_in(&ctx.db, wid).await.unwrap();
    let keys: Vec<&str> = all.iter().map(|w| w.key.as_str()).collect();
    assert_eq!(keys.len(), 2, "{keys:?}");
    let kanban = all.iter().find(|w| w.key == "kanban").unwrap();
    let subtask = all.iter().find(|w| w.key != "kanban").unwrap();
    assert_eq!(kanban.version, 1);
    assert_eq!(
        kanban.definition(&ctx.db).await.unwrap(),
        templates::kanban().document().definition
    );
    for t in ticket_types::Model::all_in(&ctx.db, wid).await.unwrap() {
        let expected = if t.key == "subtask" {
            subtask.id
        } else {
            kanban.id
        };
        assert_eq!(t.workflow_id, expected, "{}", t.key);
    }
}
