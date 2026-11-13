//! Code graph (M7): CLI meng-index repo lewat manifest → upload → finish (inkremental), file
//! rahasia tidak pernah disimpan isinya, build menghasilkan simbol + edge + relasi lintas repo,
//! query (cari, trace, snippet, dampak, Cypher) dibatasi project.

use agentgate_server::{
    app::App,
    models::{graph_files, graph_repos, projects, repos},
    services::graph::query::{self, Direction, SearchFilter},
};
use loco_rs::{TestServer, app::AppContext, testing::prelude::*};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::{Value, json};

use super::{
    runner::{get_json, post_json},
    tickets::{Fixture, fixture},
};
use crate::support::oauth_flow::device_login;

const API_TS: &str = "import { reserve } from './stock';\n\
export function checkout(id: string) {\n  return reserve(id);\n}\n\
app.post('/v1/orders/:id', checkout);\n";
const STOCK_TS: &str = "export function reserve(id: string) {\n  return lock(id);\n}\n\
function lock(id: string) {\n  return id;\n}\nexport function unusedHelper() {}\n";
const WEB_TS: &str = "export async function placeOrder(id: string) {\n\
  return fetch(`/v1/orders/${id}`, { method: 'POST' });\n}\n";

pub async fn add_repo(request: &TestServer, f: &Fixture, name: &str) {
    let res = request
        .post("/w/acme/p/KSR/settings/repos")
        .add_cookie(f.cookie.clone())
        .form(&[
            ("name", name),
            ("provider", "github"),
            ("url", &format!("https://github.com/pt-contoh/{name}")),
            ("default_branch", "main"),
        ])
        .await;
    assert_eq!(res.status_code(), 303, "{}", res.text());
}

fn sha(s: &str) -> String {
    agentgate_graph::walk::sha256(s.as_bytes())
}

/// Index penuh lewat API CLI → ringkasan `finish`.
pub async fn index(request: &TestServer, token: &str, repo: &str, files: &[(&str, &str)]) -> Value {
    let manifest: Vec<Value> = files
        .iter()
        .map(|(p, c)| json!({ "path": p, "sha": sha(c) }))
        .collect();
    let (status, need) = post_json(
        request,
        token,
        None,
        "/api/cli/graph/manifest",
        &json!({ "project": "KSR", "repo": repo, "commit": "abc123", "branch": "main", "files": manifest }),
    )
    .await;
    assert_eq!(status, 200, "{need}");
    let need: Vec<String> = serde_json::from_value(need["need"].clone()).unwrap();
    let upload: Vec<Value> = files
        .iter()
        .filter(|(p, _)| need.iter().any(|n| n == p))
        .map(|(p, c)| json!({ "path": p, "sha": sha(c), "bytes": c.len(), "content": c }))
        .collect();
    let (status, body) = post_json(
        request,
        token,
        None,
        "/api/cli/graph/upload",
        &json!({ "project": "KSR", "repo": repo, "files": upload }),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let (status, summary) = post_json(
        request,
        token,
        None,
        "/api/cli/graph/finish",
        &json!({ "project": "KSR", "repo": repo }),
    )
    .await;
    assert_eq!(status, 200, "{summary}");
    summary
}

async fn ksr(ctx: &AppContext, f: &Fixture) -> projects::Model {
    projects::Model::find_by_key(&ctx.db, f.workspace.workspace_id(), "KSR")
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn cli_indexes_incrementally_and_never_stores_secrets() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let token = device_login(&request, &f.cookie, &f.workspace.id.to_string()).await;
        add_repo(&request, &f, "kasir-api").await;

        let secret = "export const key = 'sk_live_123';\n";
        let summary = index(
            &request,
            &token,
            "kasir-api",
            &[
                ("src/api.ts", API_TS),
                ("src/stock.ts", STOCK_TS),
                ("src/credentials.ts", secret),
            ],
        )
        .await;
        assert_eq!(summary["status"], "ready");
        assert_eq!(summary["files_indexed"], 2);
        assert_eq!(summary["files_skipped"], 1);
        assert!(summary["symbols"].as_i64().unwrap() >= 4, "{summary}");
        assert!(summary["edges_certain"].as_i64().unwrap() >= 2, "{summary}");

        let stored = graph_files::Entity::find()
            .filter(graph_files::Column::Path.eq("src/credentials.ts"))
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.skip_reason, "secret");
        assert!(stored.content.is_empty());

        // Inkremental: api.ts sama, stock.ts berubah, credentials.ts dihapus.
        let changed = format!("{STOCK_TS}export function extra() {{}}\n");
        let (status, diff) = post_json(
            &request,
            &token,
            None,
            "/api/cli/graph/manifest",
            &json!({ "project": "KSR", "repo": "kasir-api", "files": [
                { "path": "src/api.ts", "sha": sha(API_TS) },
                { "path": "src/stock.ts", "sha": sha(&changed) },
            ]}),
        )
        .await;
        assert_eq!(status, 200, "{diff}");
        assert_eq!(diff["need"], json!(["src/stock.ts"]));
        assert_eq!(diff["removed"], 1);
        assert_eq!(diff["unchanged"], 1);

        // Path berbahaya ditolak, repo tak dikenal 404.
        let (status, err) = post_json(
            &request,
            &token,
            None,
            "/api/cli/graph/upload",
            &json!({ "project": "KSR", "repo": "kasir-api", "files": [{ "path": "../etc/passwd", "content": "x" }] }),
        )
        .await;
        assert_eq!((status, err["error"].as_str()), (422, Some("invalid-path")));
        let (status, _) = post_json(
            &request,
            &token,
            None,
            "/api/cli/graph/finish",
            &json!({ "project": "KSR", "repo": "tidak-ada" }),
        )
        .await;
        assert_eq!(status, 404);

        let (status, list) = get_json(&request, &token, "/api/cli/graph/status?project=KSR").await;
        assert_eq!(status, 200, "{list}");
        assert_eq!(list[0]["repo"], "kasir-api");
        assert_eq!(list[0]["status"], "running");
    })
    .await;
}

#[tokio::test]
async fn graph_queries_cover_search_trace_snippet_impact_and_cross_repo() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let token = device_login(&request, &f.cookie, &f.workspace.id.to_string()).await;
        add_repo(&request, &f, "kasir-api").await;
        add_repo(&request, &f, "kasir-web").await;
        index(
            &request,
            &token,
            "kasir-api",
            &[("src/api.ts", API_TS), ("src/stock.ts", STOCK_TS)],
        )
        .await;
        let web = index(&request, &token, "kasir-web", &[("src/order.ts", WEB_TS)]).await;
        assert_eq!(web["links"], 1, "{web}");

        let wid = f.workspace.workspace_id();
        let project = ksr(&ctx, &f).await.id;
        let hits = query::search(&ctx.db, wid, project, "reserve", &SearchFilter::default(), 10)
            .await
            .unwrap();
        assert_eq!(hits[0].name, "reserve", "{hits:?}");
        let lock = query::search(&ctx.db, wid, project, "lock", &SearchFilter::default(), 10)
            .await
            .unwrap()
            .remove(0);

        // Pemanggil `lock`: reserve → checkout → route → placeOrder (repo web, lewat HTTP).
        let hops = query::trace(&ctx.db, wid, project, &[lock.id], Direction::Callers, 5, 50)
            .await
            .unwrap();
        let names: Vec<&str> = hops.iter().map(|h| h.symbol.name.as_str()).collect();
        assert!(names.contains(&"reserve"), "{names:?}");
        assert!(names.contains(&"checkout"), "{names:?}");
        let order = hops.iter().find(|h| h.symbol.name == "placeOrder").expect("lintas repo");
        assert_eq!(order.symbol.repo, "kasir-web");
        assert_eq!(order.via, "http");

        let snip = query::snippet(&ctx.db, wid, project, &lock, 0).await.unwrap().unwrap();
        assert_eq!(snip.path, "src/stock.ts");
        assert!(snip.code.starts_with("function lock"), "{}", snip.code);

        let diff = "--- a/src/stock.ts\n+++ b/src/stock.ts\n@@ -5,1 +5,1 @@\n-  return id;\n+  return id.trim();\n";
        let changed = query::changed_by_diff(&ctx.db, wid, project, diff).await.unwrap();
        assert_eq!(changed.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), vec!["lock"]);
        let impact = query::impact(&ctx.db, wid, project, changed, 5).await.unwrap();
        assert_eq!(impact.repos, vec!["kasir-web".to_string()]);
        assert_eq!(impact.risk, "tinggi");

        let dead = query::dead_code(&ctx.db, wid, project, None, 50).await.unwrap();
        assert!(dead.iter().any(|d| d.name == "unusedHelper"), "{dead:?}");

        let res = query::run_cypher(
            &ctx.db,
            wid,
            project,
            "MATCH (a {name: 'lock'})<-[:CALLS*1..3]-(b) RETURN b.name",
        )
        .await
        .unwrap();
        let rows: Vec<&str> = res.rows.iter().map(|r| r[0].as_str()).collect();
        assert!(rows.contains(&"reserve") && rows.contains(&"checkout"), "{rows:?}");

        let links = query::links(&ctx.db, wid, project, None).await.unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].from_repo, "kasir-web");
        assert_eq!(links[0].to_repo.as_deref(), Some("kasir-api"));

        let row = graph_repos::Entity::find()
            .filter(
                graph_repos::Column::RepoId.in_subquery(
                    sea_orm::sea_query::Query::select()
                        .column(repos::Column::Id)
                        .from(repos::Entity)
                        .and_where(repos::Column::Name.eq("kasir-api"))
                        .to_owned(),
                ),
            )
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.languages()[0].0, "typescript");
        assert_eq!(row.surface().routes.len(), 1);
    })
    .await;
}
