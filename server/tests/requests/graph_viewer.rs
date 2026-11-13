//! Graph viewer (M10): level Project → Repo → Modul → File → Fungsi dari index nyata, cari membuka
//! simbol, analisis dampak (penutupan pemanggil), sorot tiket dari `jobs.changed_files`, filter
//! layer, dan isolasi project/workspace.

use agentgate_server::{app::App, models::projects};
use loco_rs::{TestServer, testing::prelude::*};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use super::{
    graph::{API_TS, STOCK_TS, add_repo, index},
    tickets::{Fixture, create_ticket, fixture, stages::member, ticket},
};
use crate::support::{a_user, a_workspace, oauth_flow::device_login, sign_in_as};

const BASE: &str = "/w/acme/p/KSR/graph/viewer";

async fn get(request: &TestServer, f: &Fixture, q: &str) -> (u16, String) {
    let url = if q.is_empty() {
        BASE.to_string()
    } else {
        format!("{BASE}?{q}")
    };
    let res = request.get(&url).add_cookie(f.cookie.clone()).await;
    (res.status_code().as_u16(), res.text())
}

fn node_id(html: &str, label: &str) -> String {
    // `<a class="app-gv-n" href=… data-node="s:<uuid>" … aria-label="lock() · …">`
    let at = html.find(&format!("aria-label=\"{label} ·")).expect(label);
    let start = html[..at].rfind("data-node=\"").unwrap() + 11;
    html[start..start + html[start..].find('"').unwrap()].to_string()
}

#[tokio::test]
async fn viewer_zooms_from_project_to_function() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let token = device_login(&request, &f.cookie, &f.workspace.id.to_string()).await;
        add_repo(&request, &f, "kasir-api").await;

        // Belum di-index: ajakan CLI, bukan error.
        let (status, html) = get(&request, &f, "").await;
        assert_eq!(status, 200);
        assert!(html.contains("data-graph-not-indexed"));

        index(
            &request,
            &token,
            "kasir-api",
            &[("src/api.ts", API_TS), ("src/stock.ts", STOCK_TS)],
        )
        .await;

        let (status, html) = get(&request, &f, "").await;
        assert_eq!(status, 200);
        assert!(html.contains("data-graph-viewer=\"project\""));
        assert!(html.contains("data-node=\"r:kasir-api\""), "{html}");
        assert!(html.contains("data-drill=\"/w/acme/p/KSR/graph/viewer?repo=kasir-api\""));
        assert!(html.contains("aria-current=\"page\""), "tab Viewer aktif");

        let (_, html) = get(&request, &f, "repo=kasir-api").await;
        assert!(html.contains("data-graph-viewer=\"repo\""));
        assert!(html.contains("data-node=\"m:src\""), "{html}");

        let (_, html) = get(&request, &f, "repo=kasir-api&module=src").await;
        assert!(html.contains("data-graph-viewer=\"module\""));
        assert!(html.contains("data-node=\"f:src/api.ts\""));
        assert!(html.contains("data-node=\"f:src/stock.ts\""));
        assert!(
            html.contains("class=\"app-gv-e\""),
            "relasi api.ts → stock.ts"
        );

        let file = "repo=kasir-api&module=src&file=src%2Fstock.ts";
        let (_, html) = get(&request, &f, file).await;
        assert!(html.contains("data-graph-viewer=\"file\""));
        let lock = node_id(&html, "lock()");
        let reserve = node_id(&html, "reserve()");

        // Pilih lock + analisis dampak: reserve (dan checkout di luar file) terdampak.
        let (_, html) = get(&request, &f, &format!("{file}&sel={lock}&imp=1")).await;
        assert!(html.contains(&format!("data-gv-panel=\"{lock}\"")));
        let at = html.find(&format!("data-node=\"{reserve}\"")).unwrap();
        let tag = &html[at..at + html[at..].find('>').unwrap()];
        assert!(tag.contains("data-impact"), "{tag}");
        assert!(html.contains("data-gv-impact-list"));

        // Cari → level Fungsi simbol teratas.
        let res = request
            .get(&format!("{BASE}?q=reserve"))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303);
        let to = res.header("location").to_str().unwrap().to_string();
        assert!(to.contains("sym="), "{to}");
        let res = request.get(&to).add_cookie(f.cookie.clone()).await;
        let html = res.text();
        assert!(html.contains("data-graph-viewer=\"symbol\""), "{html}");
        assert!(
            html.contains("aria-label=\"checkout() ·"),
            "pemanggil tampil"
        );
        assert!(
            html.contains("aria-label=\"lock() ·"),
            "yang dipanggil tampil"
        );

        // Filter layer: sembunyikan Logika → node simbol hilang.
        let (_, html) = get(&request, &f, &format!("{file}&hide=logic")).await;
        assert!(!html.contains(&format!("data-node=\"{lock}\"")));
        assert!(
            html.contains("data-layer=\"logic\" ")
                || html.contains("aria-pressed=\"false\" data-layer=\"logic\"")
        );

        // Lingkup tak dikenal → 404.
        let (status, _) = get(&request, &f, "repo=tidak-ada").await;
        assert_eq!(status, 404);
        let (status, _) = get(&request, &f, "repo=kasir-api&module=src&file=src%2Fnope.ts").await;
        assert_eq!(status, 404);
    })
    .await;
}

#[tokio::test]
async fn ticket_highlight_uses_changed_files_and_access_is_isolated() {
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
        let code = create_ticket(&request, &f.cookie, &[("title", "Kunci stok")]).await;
        let t = ticket(&ctx, &f.workspace, &code).await;
        let project = projects::Entity::find()
            .filter(projects::Column::Key.eq("KSR"))
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        ctx.db
            .execute_raw(sea_orm::Statement::from_sql_and_values(
                sea_orm::DbBackend::Postgres,
                "INSERT INTO jobs (workspace_id, project_id, ticket_id, kind, mode, stage_key, \
                 persona, status, finished_at, changed_files) VALUES ($1, $2, $3, 'stage', \
                 'auto', 'dev', 'dev', 'succeeded', now(), $4)",
                [
                    f.workspace.id.into(),
                    project.id.into(),
                    t.id.into(),
                    serde_json::json!([{ "repo": "kasir-api", "path": "src/stock.ts" }]).into(),
                ],
            ))
            .await
            .unwrap();

        let (_, html) = get(
            &request,
            &f,
            &format!("repo=kasir-api&module=src&tk={code}"),
        )
        .await;
        assert!(html.contains("data-gv-banner=\"ticket\""), "{html}");
        assert!(html.contains(&format!("data-gv-ticket=\"{code}\"")));
        let at = html.find("data-node=\"f:src/stock.ts\"").unwrap();
        let tag = &html[at..at + html[at..].find('>').unwrap()];
        assert!(tag.contains("data-tk"), "{tag}");
        let at = html.find("data-node=\"f:src/api.ts\"").unwrap();
        let tag = &html[at..at + html[at..].find('>').unwrap()];
        assert!(tag.contains("data-dim"), "{tag}");

        // Panel file menampilkan tiket yang menyentuhnya.
        let (_, html) = get(
            &request,
            &f,
            "repo=kasir-api&module=src&sel=f%3Asrc%2Fstock.ts",
        )
        .await;
        assert!(html.contains("Kunci stok"));

        // Anggota workspace boleh melihat; workspace lain: 404.
        let (_, cookie) = member(&request, &ctx, &f, "Mira Member").await;
        let res = request.get(BASE).add_cookie(cookie).await;
        assert_eq!(res.status_code(), 200);
        let stranger = a_user().name("Sam Stranger").create(&ctx).await;
        a_workspace(&stranger).slug("globex").create(&ctx).await;
        let cookie = sign_in_as(&request, &stranger).await;
        let res = request.get(BASE).add_cookie(cookie).await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}
