//! Penghemat token (M10): tool MCP graph mencatat penghematan (`token_savings`), halaman
//! Pengaturan project → Penghemat token (4 panel, Admin project), toggle fitur opsional disimpan +
//! diaudit, dan pengguna lain workspace tidak bisa membukanya.

use agentgate_server::{
    app::App,
    models::{projects, token_savings},
    services::tokens::TokenSaver,
};
use loco_rs::testing::prelude::*;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;

use super::{
    graph::{add_repo, index},
    mcp::call,
    tickets::{fixture, stages::member},
};
use crate::support::{
    a_user, a_workspace, newest_audit_events, oauth_flow::device_login, sign_in_as,
};

/// File besar dengan satu fungsi kecil: membaca potongan jauh lebih hemat daripada file utuh.
fn big_file() -> String {
    let filler =
        "// catatan panjang yang tidak relevan untuk fungsi kecil di bawah ini.\n".repeat(400);
    format!("{filler}export function tiny(id: string) {{\n  return id;\n}}\n")
}

#[tokio::test]
async fn graph_tools_record_savings_and_page_shows_them() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let token = device_login(&request, &f.cookie, &f.workspace.id.to_string()).await;
        add_repo(&request, &f, "kasir-api").await;
        index(
            &request,
            &token,
            "kasir-api",
            &[("src/big.ts", &big_file())],
        )
        .await;
        let (_, mcp, _) =
            crate::support::oauth_flow::connect(&request, &f.cookie, &f.workspace.id.to_string())
                .await;
        let (err, code) = call(
            &request,
            &mcp,
            "ag_snippet",
            json!({ "project": "KSR", "symbol": "tiny" }),
        )
        .await;
        assert!(!err, "{code}");

        let rows = token_savings::Entity::find()
            .filter(token_savings::Column::WorkspaceId.eq(f.workspace.id))
            .all(&ctx.db)
            .await
            .unwrap();
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert_eq!(rows[0].feature, "code_graph");
        assert_eq!(rows[0].tool, "ag_snippet");
        assert!(rows[0].tokens > 5000, "{}", rows[0].tokens);

        let res = request
            .get("/w/acme/p/KSR/settings/tokens")
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303);
        for panel in ["ringkasan", "fitur", "tiket", "job"] {
            let res = request
                .get(&format!("/w/acme/p/KSR/settings/tokens/{panel}"))
                .add_cookie(f.cookie.clone())
                .await;
            assert_eq!(res.status_code(), 200, "{panel}");
            let html = res.text();
            assert!(
                html.contains(&format!("data-tokens-page=\"{panel}\"")),
                "{panel}"
            );
            assert!(
                html.contains("data-nav=\"tokens\""),
                "menu Pengaturan project"
            );
        }
        let html = request
            .get("/w/acme/p/KSR/settings/tokens/ringkasan")
            .add_cookie(f.cookie.clone())
            .await
            .text();
        assert!(html.contains("Code graph built-in menyumbang"), "{html}");
        assert!(html.contains("class=\"app-chart\""), "grafik minggu");
        let html = request
            .get("/w/acme/p/KSR/settings/tokens/fitur")
            .add_cookie(f.cookie.clone())
            .await
            .text();
        assert!(html.contains("data-feature=\"rtk\""));
        assert!(html.contains("tidak terukur"));
        assert_eq!(html.matches("name=\"concise\"").count(), 1);

        let res = request
            .get("/w/acme/p/KSR/settings/tokens/lain")
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}

#[tokio::test]
async fn optional_toggles_are_saved_audited_and_admin_only() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let res = request
            .post("/w/acme/p/KSR/settings/tokens")
            .add_cookie(f.cookie.clone())
            .form(&[("persona_models", "1")])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(
            res.header("location").to_str().unwrap(),
            "/w/acme/p/KSR/settings/tokens/fitur"
        );
        let project = projects::Entity::find()
            .filter(projects::Column::Key.eq("KSR"))
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            TokenSaver::of(&project),
            TokenSaver {
                concise: false,
                persona_models: true
            }
        );
        let actions: Vec<String> = newest_audit_events(&ctx.db, f.workspace.workspace_id(), 2)
            .await
            .unwrap()
            .into_iter()
            .map(|e| e.action)
            .collect();
        assert!(
            actions.iter().any(|a| a == "project.token_saver_updated"),
            "{actions:?}"
        );
        let html = request
            .get("/w/acme/p/KSR/settings/tokens/fitur")
            .add_cookie(f.cookie.clone())
            .await
            .text();
        assert!(html.contains("Pengaturan penghemat token disimpan"));

        // Anggota biasa (bukan Admin project) dan workspace lain: 404.
        let (_, cookie) = member(&request, &ctx, &f, "Mira Member").await;
        let res = request
            .get("/w/acme/p/KSR/settings/tokens/ringkasan")
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(res.status_code(), 404);
        let res = request
            .post("/w/acme/p/KSR/settings/tokens")
            .add_cookie(cookie)
            .form(&[("concise", "1")])
            .await;
        assert_eq!(res.status_code(), 404);
        let stranger = a_user().name("Sam Stranger").create(&ctx).await;
        a_workspace(&stranger).slug("globex").create(&ctx).await;
        let cookie = sign_in_as(&request, &stranger).await;
        let res = request
            .get("/w/acme/p/KSR/settings/tokens/ringkasan")
            .add_cookie(cookie)
            .await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}
