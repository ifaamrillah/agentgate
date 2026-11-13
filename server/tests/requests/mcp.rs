//! `/mcp` lewat HTTP: 401 + `WWW-Authenticate`, initialize, tools/list, setiap tool (izin
//! Viewer, lintas workspace, gate/stage ditolak), prompts/list + prompts/get, grant dicabut, dan
//! alur end-to-end OAuth → `ag_draft_tickets`.

use agentgate_server::{
    app::App,
    models::{
        artifact_versions, notifications, project_grants::GrantSubject, roles::RoleKey,
        ticket_artifacts, ticket_comments, ticket_drafts,
    },
    services::oauth,
};
use loco_rs::{TestServer, testing::prelude::*};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::{Value, json};

use super::tickets::{create_ticket, fixture, ticket};
use crate::support::{
    a_project, a_user, a_workspace, add_member, builtin_role, grant, oauth_flow, sign_in_as,
};

const PROTOCOL: &str = "2025-06-18";

/// Satu panggilan JSON-RPC ke `/mcp`; kembalikan (status, body JSON).
async fn rpc(request: &TestServer, token: &str, method: &str, params: Value) -> (u16, Value) {
    let res = request
        .post("/mcp")
        .add_header("authorization", format!("Bearer {token}"))
        .add_header("accept", "application/json, text/event-stream")
        .add_header("mcp-protocol-version", PROTOCOL)
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }))
        .await;
    let status = res.status_code().as_u16();
    let text = res.text();
    let body = serde_json::from_str(&text).unwrap_or_else(|_| {
        // Fallback SSE: ambil baris `data:` terakhir yang berisi JSON.
        text.lines()
            .filter_map(|l| l.strip_prefix("data:"))
            .filter_map(|d| serde_json::from_str::<Value>(d.trim()).ok())
            .next_back()
            .unwrap_or(Value::Null)
    });
    (status, body)
}

/// `tools/call`; kembalikan (isError, structuredContent atau teks).
pub(super) async fn call(
    request: &TestServer,
    token: &str,
    tool: &str,
    args: Value,
) -> (bool, Value) {
    let (status, body) = rpc(
        request,
        token,
        "tools/call",
        json!({ "name": tool, "arguments": args }),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let result = &body["result"];
    assert!(!result.is_null(), "{body}");
    let is_error = result["isError"].as_bool().unwrap_or(false);
    let value = if result["structuredContent"].is_null() {
        result["content"][0]["text"].clone()
    } else {
        result["structuredContent"].clone()
    };
    (is_error, value)
}

#[tokio::test]
async fn requires_bearer_token() {
    request::<App, _, _>(|request, _ctx| async move {
        let res = request
            .post("/mcp")
            .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }))
            .await;
        assert_eq!(res.status_code(), 401);
        let www = res.header("www-authenticate").to_str().unwrap().to_string();
        assert!(
            www.contains("resource_metadata=")
                && www.contains("/.well-known/oauth-protected-resource/mcp"),
            "{www}"
        );
        let (status, _) = rpc(&request, "salah", "tools/list", json!({})).await;
        assert_eq!(status, 401);
    })
    .await;
}

#[tokio::test]
async fn end_to_end_oauth_tools_and_prompts() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let code = create_ticket(
            &request,
            &f.cookie,
            &[("title", "Struk digital"), ("description", "Kirim struk lewat email")],
        )
        .await;
        let wid = f.workspace.workspace_id();
        let (_, token, _) =
            oauth_flow::connect(&request, &f.cookie, &f.workspace.id.to_string()).await;

        let (status, body) = rpc(
            &request,
            &token,
            "initialize",
            json!({
                "protocolVersion": PROTOCOL,
                "capabilities": {},
                "clientInfo": { "name": "test", "version": "1.0" },
            }),
        )
        .await;
        assert_eq!(status, 200, "{body}");
        assert_eq!(body["result"]["serverInfo"]["name"], "agentgate");
        assert!(body["result"]["capabilities"]["tools"].is_object(), "{body}");
        assert!(body["result"]["capabilities"]["prompts"].is_object(), "{body}");

        let (_, body) = rpc(&request, &token, "tools/list", json!({})).await;
        let names: Vec<&str> = body["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert_eq!(
            names,
            agentgate_core::persona::MCP_TOOLS.to_vec(),
            "{body}"
        );

        // Konteks workspace dan tiket.
        let (err, value) = call(&request, &token, "ag_context", json!({})).await;
        assert!(!err, "{value}");
        assert_eq!(value["projects"][0]["key"], "KSR");
        let (err, value) = call(&request, &token, "ag_context", json!({ "ticket": code })).await;
        assert!(!err, "{value}");
        assert_eq!(value["ticket"]["code"], code.as_str());
        assert!(value["stage"].is_object(), "{value}");

        // Tiket: get, search, create.
        let (err, value) =
            call(&request, &token, "ag_ticket", json!({ "action": "get", "ticket": code })).await;
        assert!(!err, "{value}");
        assert_eq!(value["description"], "Kirim struk lewat email");
        let (err, value) =
            call(&request, &token, "ag_ticket", json!({ "action": "search", "query": "struk" }))
                .await;
        assert!(!err, "{value}");
        assert_eq!(value["total"], 1);
        let (err, value) = call(
            &request,
            &token,
            "ag_ticket",
            json!({ "action": "create", "project": "KSR", "title": "Dibuat agent", "type": "bug" }),
        )
        .await;
        assert!(!err, "{value}");
        let created = value["created"]["code"].as_str().unwrap().to_string();
        assert!(created.starts_with("KSR-"));

        // Komentar dan artifact.
        let t = ticket(&ctx, &f.workspace, &code).await;
        let (err, _) =
            call(&request, &token, "ag_comment", json!({ "ticket": code, "body": "Catatan agent" }))
                .await;
        assert!(!err);
        let comments = ticket_comments::Model::of_ticket(&ctx.db, wid, t.id).await.unwrap();
        assert_eq!(comments.last().unwrap().body, "Catatan agent");
        assert_eq!(comments.last().unwrap().author_id, Some(f.owner.id));

        let (err, value) = call(
            &request,
            &token,
            "ag_artifact",
            json!({ "action": "save", "ticket": code, "name": "prd.md", "body": "# PRD\n\nIsi" }),
        )
        .await;
        assert!(!err, "{value}");
        assert!(value["version"].as_i64().unwrap() >= 1, "{value}");
        let art = ticket_artifacts::Model::find_named(&ctx.db, wid, t.id, "prd.md")
            .await
            .unwrap()
            .unwrap();
        let v = artifact_versions::Model::latest(&ctx.db, wid, art.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(v.body, "# PRD\n\nIsi");
        let (err, value) = call(
            &request,
            &token,
            "ag_artifact",
            json!({ "action": "get", "ticket": code, "name": "prd.md" }),
        )
        .await;
        assert!(!err);
        assert_eq!(value["body"], "# PRD\n\nIsi");

        // Pindah ke stage yang tidak ada → ditolak dengan alasan.
        let (err, value) =
            call(&request, &token, "ag_transition", json!({ "ticket": code, "to": "nope" })).await;
        assert!(err, "{value}");

        // ag_ask: komentar + notifikasi ke penerima tugas.
        let helper = a_user().create(&ctx).await;
        add_member(&ctx, &f.workspace, &helper, RoleKey::Admin).await;
        let mut model: agentgate_server::models::tickets::ActiveModel =
            sea_orm::IntoActiveModel::into_active_model(t.clone());
        model.assignee_id = sea_orm::ActiveValue::Set(Some(helper.id));
        sea_orm::ActiveModelTrait::update(model, &ctx.db).await.unwrap();
        let (err, value) = call(
            &request,
            &token,
            "ag_ask",
            json!({ "ticket": code, "question": "Struk perlu PDF?" }),
        )
        .await;
        assert!(!err, "{value}");
        assert_eq!(value["notified"], 1);
        let (inbox, _) = notifications::Model::inbox(
            &ctx.db,
            wid,
            helper.id,
            &[notifications::NotificationKind::AgentQuestion],
            false,
            0,
            10,
        )
        .await
        .unwrap();
        assert!(
            inbox
                .iter()
                .any(|n| n.kind().unwrap() == notifications::NotificationKind::AgentQuestion),
            "{inbox:?}"
        );

        // Draft brainstorming dengan hierarki.
        let (err, value) = call(
            &request,
            &token,
            "ag_draft_tickets",
            json!({
                "project": "KSR",
                "source_ticket": code,
                "drafts": [
                    { "ref": "e1", "type": "epic", "title": "Struk digital" },
                    { "ref": "s1", "parent": "e1", "type": "story", "title": "Kirim via email", "story_points": 3 },
                    { "ref": "s2", "parent": "e1", "type": "story", "title": "Kirim via WhatsApp" },
                ],
            }),
        )
        .await;
        assert!(!err, "{value}");
        assert_eq!(value["saved"], 3);
        let ksr = agentgate_server::models::projects::Model::find_by_key(&ctx.db, wid, "KSR")
            .await
            .unwrap()
            .unwrap();
        let drafts = ticket_drafts::Model::of_project(&ctx.db, wid, ksr.id).await.unwrap();
        assert_eq!(drafts.len(), 3);
        assert_eq!(drafts[1].parent_id, Some(drafts[0].id));
        assert_eq!(drafts[1].story_points, Some(3));
        assert_eq!(drafts[0].source_ticket_id, Some(t.id));
        // Induk tidak dikenal → ditolak, tanpa draft tersimpan.
        let (err, _) = call(
            &request,
            &token,
            "ag_draft_tickets",
            json!({ "project": "KSR", "drafts": [{ "parent": "x", "title": "Yatim" }] }),
        )
        .await;
        assert!(err);
        assert_eq!(
            ticket_drafts::Model::of_project(&ctx.db, wid, ksr.id).await.unwrap().len(),
            3
        );

        // Prompts: workflow Superpowers → slash command per stage ber-persona.
        let res = request
            .post("/w/acme/settings/workflows")
            .add_cookie(f.cookie.clone())
            .form(&[("source", "template:superpowers"), ("name", "")])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let (_, body) = rpc(&request, &token, "prompts/list", json!({})).await;
        let brainstorm = body["result"]["prompts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["name"] == "ag-brainstorm")
            .unwrap_or_else(|| panic!("{body}"))
            .clone();
        assert_eq!(brainstorm["arguments"][0]["name"], "ticket");
        let (_, body) = rpc(
            &request,
            &token,
            "prompts/get",
            json!({ "name": "ag-brainstorm", "arguments": { "ticket": code } }),
        )
        .await;
        let text = body["result"]["messages"][0]["content"]["text"].as_str().unwrap();
        assert!(text.contains(&code) && text.contains("ag_context"), "{text}");
        let page = request
            .get("/account/connect?section=slash")
            .add_cookie(f.cookie.clone())
            .await;
        assert_css_exists(&page.text(), "[data-prompt='ag-brainstorm']");

        let (_, body) = rpc(&request, &token, "prompts/list", json!({})).await;
        let prompts: Vec<&str> = body["result"]["prompts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["name"].as_str().unwrap())
            .collect();
        assert!(prompts.contains(&"ag-status"), "{body}");
        assert!(prompts.contains(&"ag-ticket"), "{body}");
        let (_, body) = rpc(
            &request,
            &token,
            "prompts/get",
            json!({ "name": "ag-ticket", "arguments": { "ticket": code } }),
        )
        .await;
        let text = body["result"]["messages"][0]["content"]["text"].as_str().unwrap();
        assert!(text.contains(&code), "{text}");
        let (_, body) = rpc(&request, &token, "prompts/get", json!({ "name": "ag-nope" })).await;
        assert!(body["error"].is_object(), "{body}");

        // Grant dicabut → token ditolak.
        let grants = oauth::grants_of(&ctx.db, f.owner.id).await.unwrap();
        assert!(oauth::revoke_own(&ctx.db, f.owner.id, grants[0].0.id, None).await.unwrap());
        let (status, _) = rpc(&request, &token, "tools/list", json!({})).await;
        assert_eq!(status, 401);
    })
    .await;
}

#[tokio::test]
async fn viewer_cannot_write_and_workspaces_are_isolated() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let code = create_ticket(&request, &f.cookie, &[("title", "Struk digital")]).await;

        // Viewer di KSR.
        let viewer = a_user().create(&ctx).await;
        add_member(&ctx, &f.workspace, &viewer, RoleKey::Member).await;
        let ksr = agentgate_server::models::projects::Model::find_by_key(
            &ctx.db,
            f.workspace.workspace_id(),
            "KSR",
        )
        .await
        .unwrap()
        .unwrap();
        let viewer_role = builtin_role(&ctx, &f.workspace, RoleKey::Viewer).await;
        grant(
            &ctx,
            &f.workspace,
            &ksr,
            GrantSubject::User(viewer.id),
            &viewer_role,
        )
        .await;
        let vc = sign_in_as(&request, &viewer).await;
        let (_, token, _) = oauth_flow::connect(&request, &vc, &f.workspace.id.to_string()).await;

        let (err, value) = call(
            &request,
            &token,
            "ag_ticket",
            json!({ "action": "get", "ticket": code }),
        )
        .await;
        assert!(!err, "{value}");
        for (tool, args) in [
            ("ag_comment", json!({ "ticket": code, "body": "x" })),
            (
                "ag_artifact",
                json!({ "action": "save", "ticket": code, "name": "a.md", "body": "x" }),
            ),
            ("ag_transition", json!({ "ticket": code, "to": "done" })),
            ("ag_ask", json!({ "ticket": code, "question": "x" })),
            (
                "ag_draft_tickets",
                json!({ "project": "KSR", "drafts": [{ "title": "x" }] }),
            ),
            (
                "ag_ticket",
                json!({ "action": "create", "project": "KSR", "title": "x" }),
            ),
        ] {
            let (err, value) = call(&request, &token, tool, args).await;
            assert!(err, "{tool}: {value}");
        }

        // Workspace lain: token tidak melihat tiket/project di sana.
        let other = a_user().create(&ctx).await;
        let ws2 = a_workspace(&other).slug("lain").create(&ctx).await;
        a_project(&ctx, &ws2, "LN").await;
        let oc = sign_in_as(&request, &other).await;
        let (_, token2, _) = oauth_flow::connect(&request, &oc, &ws2.id.to_string()).await;
        let (err, _) = call(
            &request,
            &token2,
            "ag_ticket",
            json!({ "action": "get", "ticket": code }),
        )
        .await;
        assert!(err);
        let (_, value) = call(&request, &token2, "ag_context", json!({})).await;
        assert_eq!(value["projects"].as_array().unwrap().len(), 1);
        assert_eq!(value["projects"][0]["key"], "LN");

        // Viewer dikeluarkan dari workspace → token ditolak di level tool.
        agentgate_server::models::workspace_members::Entity::delete_many()
            .filter(agentgate_server::models::workspace_members::Column::UserId.eq(viewer.id))
            .exec(&ctx.db)
            .await
            .unwrap();
        let (_, body) = rpc(&request, &token, "tools/list", json!({})).await;
        assert!(body["error"].is_object(), "{body}");
    })
    .await;
}
