//! Pencarian semantik code graph (R2): embedding per simbol (provider `fake` di tes) dibuat
//! setelah index, inkremental menurut hash teks, pencarian hibrid di UI dan MCP `ag_search`,
//! serta jatuh ke leksikal bila embedding dimatikan.

use agentgate_server::{app::App, models::graph_repos, services::graph::embed};
use loco_rs::testing::prelude::*;
use sea_orm::{ConnectionTrait, EntityTrait};
use serde_json::{Value, json};

use super::{
    graph::{API_TS, STOCK_TS, add_repo, index},
    mcp::call,
    tickets::fixture,
};
use crate::support::{oauth_flow::device_login, request_with_settings};

async fn embeddings(ctx: &loco_rs::app::AppContext) -> i64 {
    ctx.db
        .query_one_raw(sea_orm::Statement::from_string(
            sea_orm::DbBackend::Postgres,
            "SELECT count(*) AS n FROM graph_embeddings",
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<i64>("", "n")
        .unwrap()
}

#[tokio::test]
async fn symbols_are_embedded_incrementally_and_searched_hybrid() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let token = device_login(&request, &f.cookie, &f.workspace.id.to_string()).await;
        add_repo(&request, &f, "kasir-api").await;
        index(
            &request,
            &token,
            "kasir-api",
            &[("src/api.ts", API_TS), ("src/stock.ts", STOCK_TS)],
        )
        .await;

        let row = graph_repos::Entity::find()
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.embed_error, "");
        assert!(row.embedded_at.is_some());
        let first = embeddings(&ctx).await;
        assert!(first >= 4, "{first}");
        assert_eq!(i64::from(row.embed_symbols), first);

        // Tanpa perubahan → tidak ada yang di-embed ulang.
        let again = embed::index_repo(
            &ctx.db,
            &embed::configured(),
            f.workspace.workspace_id(),
            row.repo_id,
        )
        .await
        .unwrap();
        assert_eq!(again.embedded, 0);
        assert_eq!(i64::try_from(again.symbols).unwrap(), first);

        // Satu simbol baru → hanya itu yang di-embed; total bertambah satu.
        let stock =
            format!("{STOCK_TS}export function refundOrder(id: string) {{\n  return id;\n}}\n");
        index(
            &request,
            &token,
            "kasir-api",
            &[("src/api.ts", API_TS), ("src/stock.ts", &stock)],
        )
        .await;
        assert_eq!(embeddings(&ctx).await, first + 1);

        // MCP: mode semantik, hibrid (default), dan mode tidak valid.
        let (_, mcp, _) =
            crate::support::oauth_flow::connect(&request, &f.cookie, &f.workspace.id.to_string())
                .await;
        let (err, sem) = call(
            &request,
            &mcp,
            "ag_search",
            json!({ "project": "KSR", "query": "refund order", "mode": "semantic" }),
        )
        .await;
        assert!(!err, "{sem}");
        assert_eq!(sem["mode"], "semantic");
        assert_eq!(sem["fallback"], Value::Null);
        assert_eq!(sem["results"][0]["name"], "refundOrder", "{sem}");
        let (err, hyb) = call(
            &request,
            &mcp,
            "ag_search",
            json!({ "project": "KSR", "query": "reserve" }),
        )
        .await;
        assert!(!err, "{hyb}");
        assert_eq!(hyb["mode"], "hybrid");
        assert_eq!(
            hyb["results"][0]["name"], "reserve",
            "nama persis tetap pertama"
        );
        let (err, bad) = call(
            &request,
            &mcp,
            "ag_search",
            json!({ "project": "KSR", "query": "x", "mode": "vektor" }),
        )
        .await;
        assert!(err, "{bad}");

        // UI Cari simbol: pilihan mode.
        let page = request
            .get("/w/acme/p/KSR/graph?q=refund+order&mode=semantic")
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(page.status_code(), 200);
        let html = page.text();
        assert_css_exists(
            &html,
            "select[data-search-mode] option[value=semantic][selected]",
        );
        assert!(html.contains("data-symbol=\"refundOrder\""), "{html}");
        assert_css_not_exists(&html, "[data-search-fallback]");
    })
    .await;
}

#[tokio::test]
async fn disabled_embedding_falls_back_to_lexical() {
    request_with_settings(
        |s| {
            s["graph"]["embedding"] = json!({ "enabled": false, "provider": "fake" });
        },
        |request, ctx| async move {
            let f = fixture(&request, &ctx).await;
            let token = device_login(&request, &f.cookie, &f.workspace.id.to_string()).await;
            add_repo(&request, &f, "kasir-api").await;
            index(&request, &token, "kasir-api", &[("src/stock.ts", STOCK_TS)]).await;
            assert_eq!(embeddings(&ctx).await, 0);
            let html = request
                .get("/w/acme/p/KSR/graph?q=reserve")
                .add_cookie(f.cookie.clone())
                .await
                .text();
            assert_css_exists(&html, "[data-search-fallback=graph-semantic-off]");
            assert!(html.contains("data-symbol=\"reserve\""));
        },
    )
    .await;
}
