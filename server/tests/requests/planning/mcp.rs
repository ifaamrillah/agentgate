//! `ag_planning` lewat MCP: estimasi AI (poin dipakai bila kosong), sprint, release notes
//! (baca → simpan versi AI → isi sama ditolak).

use agentgate_server::{
    app::App,
    models::{releases, ticket_estimates},
    services::planning::releases::ReleaseInput,
};
use loco_rs::testing::prelude::*;
use serde_json::json;

use super::{create_ticket, fixture, ksr, ticket};
use crate::{requests::mcp::call, support::oauth_flow};

#[tokio::test]
async fn ag_planning_estimates_and_writes_release_notes() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let wid = f.workspace.workspace_id();
        let code = create_ticket(&request, &f.cookie, &[("title", "Struk digital")]).await;
        let (_, token, _) =
            oauth_flow::connect(&request, &f.cookie, &f.workspace.id.to_string()).await;

        let (err, out) = call(
            &request,
            &token,
            "ag_planning",
            json!({
                "action": "estimate", "ticket": code, "points": 5, "basis": "Mirip KSR-9",
                "stages": [{ "stage": "development", "ai_hours": 3, "human_hours": 1.5 }],
                "persona": "analyst",
            }),
        )
        .await;
        assert!(!err, "{out}");
        let t = ticket(&ctx, &f.workspace, &code).await;
        assert_eq!(t.story_points, Some(5));
        let rows = ticket_estimates::Model::of_tickets(&ctx.db, wid, &[t.id]).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].persona, "analyst");
        // Usulan AI kedua tidak menimpa poin yang sudah ada.
        let (err, _) = call(&request, &token, "ag_planning", json!({ "action": "estimate", "ticket": code, "points": 8 })).await;
        assert!(!err);
        assert_eq!(ticket(&ctx, &f.workspace, &code).await.story_points, Some(5));
        let (err, out) = call(&request, &token, "ag_planning", json!({ "action": "estimate", "ticket": code, "points": 5000 })).await;
        assert!(err, "{out}");

        let (err, out) = call(&request, &token, "ag_planning", json!({ "action": "sprint", "project": "KSR" })).await;
        assert!(!err, "{out}");
        assert_eq!(out["project"], "KSR");

        // Release notes.
        let p = ksr(&ctx, &f).await;
        let (_, role) = agentgate_server::models::workspace_members::Model::find_with_role(&ctx.db, wid, f.owner.id)
            .await
            .unwrap()
            .unwrap();
        let acting = agentgate_server::services::workspaces::Acting {
            workspace: &f.workspace,
            user: &f.owner,
            role: &role,
            request_id: None,
        };
        let rel = agentgate_server::services::planning::releases::create(
            &ctx.db,
            &acting,
            p.id,
            &ReleaseInput { name: "v1.4".into(), ..Default::default() },
        )
        .await
        .unwrap();
        let (err, out) = call(&request, &token, "ag_planning", json!({ "action": "release_notes", "project": "KSR", "release": "V1.4" })).await;
        assert!(!err, "{out}");
        assert_eq!(out["release"], "v1.4");
        let (err, out) = call(&request, &token, "ag_planning", json!({ "action": "release_notes", "project": "KSR", "release": "v1.4", "body": "## Fitur baru\n- Struk digital (KSR-1)" })).await;
        assert!(!err, "{out}");
        assert_eq!(out["version"], 1);
        let (err, _) = call(&request, &token, "ag_planning", json!({ "action": "release_notes", "project": "KSR", "release": "v1.4", "body": "## Fitur baru\n- Struk digital (KSR-1)" })).await;
        assert!(err, "isi sama ditolak");
        let saved = releases::Model::find_in(&ctx.db, wid, rel.id).await.unwrap().unwrap();
        assert_eq!(saved.notes_source, "ai");
        assert!(saved.notes.contains("Struk digital"));
    })
    .await;
}
