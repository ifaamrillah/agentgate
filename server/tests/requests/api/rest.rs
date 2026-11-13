//! REST `/api/v1`: bearer token (`ag_pat_…`), cakupan, akses lintas workspace 404, token dicabut
//! / user nonaktif / keluar workspace tidak berlaku, rate limit 429, gate saat transisi, webhook
//! API, dan dokumen OpenAPI memuat semua route.

use agentgate_server::{
    app::App,
    models::{api_tokens, outbox_events, tickets},
};
use axum::http::Method;
use loco_rs::{app::Hooks, testing::prelude::*};
use sea_orm::{ColumnTrait, ConnectionTrait, DbBackend, EntityTrait, QueryFilter, Statement};
use serde_json::json;

use super::{get, json, new_token, send};
use crate::{
    requests::tickets::{
        create_ticket, fixture,
        stages::{FLOW_YAML, member, use_workflow},
        ticket,
    },
    support::{a_project, a_user, a_workspace, request_with_settings, sign_in_as},
};

#[tokio::test]
async fn bearer_token_is_required_and_acts_as_its_creator() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let res = request.get("/api/v1/projects").await;
        assert_eq!(res.status_code(), 401);
        assert_eq!(json(&res)["error"], "invalid_token");
        assert_eq!(get(&request, "ag_pat_salah", "/api/v1/projects").await.status_code(), 401);

        let token = new_token(&request, &f.cookie, &["tiket:baca"], "all").await;
        assert!(token.starts_with("ag_pat_"));
        // Hanya digest yang disimpan.
        let row = api_tokens::Entity::find().one(&ctx.db).await.unwrap().unwrap();
        assert_ne!(row.digest, token);
        assert!(!row.digest.contains("ag_pat_"));
        assert!(token.starts_with(&row.prefix));

        let res = get(&request, &token, "/api/v1/projects").await;
        assert_eq!(res.status_code(), 200, "{}", res.text());
        assert_eq!(res.header("x-ratelimit-remaining"), "599");
        assert_eq!(json(&res)["projects"][0]["key"], "KSR");
        let row = api_tokens::Entity::find().one(&ctx.db).await.unwrap().unwrap();
        assert!(row.last_used_at.is_some());

        // Tanpa cookie sesi pun POST tidak kena CSRF (bearer saja).
        let res = request
            .post("/api/v1/tickets")
            .add_header("sec-fetch-site", "cross-site")
            .add_header("authorization", format!("Bearer {token}"))
            .json(&json!({ "project": "KSR", "title": "x" }))
            .await;
        assert_eq!(res.status_code(), 403);
        assert_eq!(json(&res)["error"], "insufficient_scope");
    })
    .await;
}

#[tokio::test]
async fn tickets_can_be_listed_created_updated_and_commented() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let token = new_token(
            &request,
            &f.cookie,
            &["tiket:baca", "tiket:tulis", "komentar:tulis"],
            "KSR",
        )
        .await;
        let res = send(
            &request,
            &token,
            Method::POST,
            "/api/v1/tickets",
            &json!({ "project": "KSR", "title": "Checkout QRIS", "priority": "high", "labels": ["kasir"] }),
        )
        .await;
        assert_eq!(res.status_code(), 201, "{}", res.text());
        let body = json(&res);
        assert_eq!(body["ticket"]["code"], "KSR-1");
        assert_eq!(body["ticket"]["priority"], "high");

        // Validasi memakai kode error Fluent yang sama dengan form.
        let res = send(
            &request,
            &token,
            Method::POST,
            "/api/v1/tickets",
            &json!({ "project": "KSR", "title": "" }),
        )
        .await;
        assert_eq!(res.status_code(), 422);
        assert_eq!(json(&res)["field"], "title");

        for i in 2..=4 {
            create_ticket(&request, &f.cookie, &[("title", &format!("Tiket {i}"))]).await;
        }
        let res = get(&request, &token, "/api/v1/tickets?project=KSR&limit=3").await;
        let page = json(&res);
        assert_eq!(page["total"], 4);
        assert_eq!(page["tickets"].as_array().unwrap().len(), 3);
        let cursor = page["next_cursor"].as_str().unwrap().to_string();
        let res = get(&request, &token, &format!("/api/v1/tickets?project=KSR&limit=3&cursor={cursor}")).await;
        let page2 = json(&res);
        assert_eq!(page2["tickets"].as_array().unwrap().len(), 1);
        assert!(page2["next_cursor"].is_null());
        let res = get(&request, &token, "/api/v1/tickets?q=QRIS").await;
        assert_eq!(json(&res)["total"], 1);
        let res = get(&request, &token, "/api/v1/tickets?stage=bogus").await;
        assert_eq!(json(&res)["total"], 0);
        assert_eq!(get(&request, &token, "/api/v1/tickets?cursor=%%%").await.status_code(), 400);

        let res = send(
            &request,
            &token,
            Method::PATCH,
            "/api/v1/tickets/KSR-1",
            &json!({ "title": "Checkout QRIS v2" }),
        )
        .await;
        assert_eq!(res.status_code(), 200, "{}", res.text());
        assert_eq!(ticket(&ctx, &f.workspace, "KSR-1").await.title, "Checkout QRIS v2");
        // Field yang tidak dikirim tidak berubah.
        assert_eq!(json(&res)["ticket"]["priority"], "high");

        let res = send(
            &request,
            &token,
            Method::POST,
            "/api/v1/tickets/KSR-1/comments",
            &json!({ "body": "Dari integrasi" }),
        )
        .await;
        assert_eq!(res.status_code(), 201, "{}", res.text());
        let res = get(&request, &token, "/api/v1/tickets/KSR-1/comments").await;
        let comments = json(&res);
        assert_eq!(comments["comments"][0]["body"], "Dari integrasi");
        assert_eq!(comments["comments"][0]["author"], "Olivia Owner");

        // Setiap perubahan domain menulis event outbox (feed generik).
        let events: Vec<String> = outbox_events::Entity::find()
            .filter(outbox_events::Column::WorkspaceId.eq(f.workspace.id))
            .all(&ctx.db)
            .await
            .unwrap()
            .into_iter()
            .map(|e| e.event)
            .collect();
        assert!(events.contains(&"ticket.created".to_string()), "{events:?}");
        assert!(events.contains(&"ticket.updated".to_string()), "{events:?}");
        assert!(events.contains(&"comment.created".to_string()), "{events:?}");
        let feed = outbox_events::Model::since(&ctx.db, f.workspace.workspace_id(), None, 100)
            .await
            .unwrap();
        assert_eq!(feed.len(), events.len());
        let last = feed.last().unwrap();
        let after = outbox_events::Model::since(
            &ctx.db,
            f.workspace.workspace_id(),
            Some((last.created_at, last.id)),
            100,
        )
        .await
        .unwrap();
        assert!(after.is_empty());

        // Sprint dan knowledge butuh cakupannya sendiri.
        let res = get(&request, &token, "/api/v1/sprints?project=KSR").await;
        assert_eq!(res.status_code(), 403);
    })
    .await;
}

#[tokio::test]
async fn scopes_and_project_limits_are_enforced() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        a_project(&ctx, &f.workspace, "GDG").await;
        let reports = new_token(&request, &f.cookie, &["laporan:baca", "knowledge:baca"], "GDG").await;
        let res = get(&request, &reports, "/api/v1/tickets").await;
        assert_eq!(res.status_code(), 403);
        assert_eq!(json(&res)["error"], "insufficient_scope");
        let res = get(&request, &reports, "/api/v1/sprints?project=GDG").await;
        assert_eq!(res.status_code(), 200, "{}", res.text());
        assert!(json(&res)["sprints"].as_array().unwrap().is_empty());
        let res = get(&request, &reports, "/api/v1/knowledge?project=GDG").await;
        assert_eq!(res.status_code(), 200, "{}", res.text());
        assert_eq!(get(&request, &reports, "/api/v1/knowledge/tidak-ada?project=GDG").await.status_code(), 404);
        assert_eq!(get(&request, &reports, "/api/v1/sprints").await.status_code(), 400);
        // Token dibatasi ke GDG: project lain 404.
        let res = get(&request, &reports, "/api/v1/sprints?project=KSR").await;
        assert_eq!(res.status_code(), 404);
        assert_eq!(json(&res)["error"], "not_found");
        let res = get(&request, &reports, "/api/v1/projects").await;
        let keys: Vec<_> = json(&res)["projects"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["key"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(keys, ["GDG"]);

        // Tanpa webhook:kelola.
        assert_eq!(get(&request, &reports, "/api/v1/webhooks").await.status_code(), 403);
    })
    .await;
}

#[tokio::test]
async fn other_workspaces_and_inaccessible_projects_are_not_found() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        create_ticket(&request, &f.cookie, &[("title", "Milik acme")]).await;
        let other = a_user().name("Lain").create(&ctx).await;
        let ws = a_workspace(&other).slug("lain").create(&ctx).await;
        a_project(&ctx, &ws, "LAIN").await;
        let other_cookie = sign_in_as(&request, &other).await;
        let res = request
            .post("/w/lain/p/LAIN/tickets/new")
            .add_cookie(other_cookie.clone())
            .form(&[("title", "Rahasia")])
            .await;
        assert_eq!(res.status_code(), 303);

        let token = new_token(&request, &f.cookie, &["tiket:baca", "tiket:tulis"], "all").await;
        assert_eq!(get(&request, &token, "/api/v1/tickets/LAIN-1").await.status_code(), 404);
        assert_eq!(get(&request, &token, "/api/v1/tickets?project=LAIN").await.status_code(), 404);
        let res = send(&request, &token, Method::POST, "/api/v1/tickets", &json!({ "project": "LAIN", "title": "x" })).await;
        assert_eq!(res.status_code(), 404);

        // Member tanpa akses project: tiket project itu 404 lewat tokennya.
        let user = a_user().name("Mira Member").create(&ctx).await;
        crate::support::add_member(&ctx, &f.workspace, &user, agentgate_server::models::roles::RoleKey::Member).await;
        let member_cookie = sign_in_as(&request, &user).await;
        // Member biasa tidak memegang ManageIntegrations: Pengaturan API 404.
        let res = request
            .post("/w/acme/settings/api/tokens")
            .add_cookie(member_cookie.clone())
            .form(&[("name", "x"), ("scopes", "tiket:baca"), ("expiry", "30"), ("project", "all")])
            .await;
        assert_eq!(res.status_code(), 404);
        let member_token = token_for(&ctx, &f.workspace, &user, &["tiket:baca"]).await;
        assert_eq!(get(&request, &member_token, "/api/v1/tickets/KSR-1").await.status_code(), 404);
        let res = get(&request, &member_token, "/api/v1/tickets").await;
        assert_eq!(json(&res)["total"], 0);
    })
    .await;
}

#[tokio::test]
async fn revoked_disabled_and_departed_tokens_stop_working() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let token = new_token(&request, &f.cookie, &["tiket:baca"], "all").await;
        assert_eq!(get(&request, &token, "/api/v1/projects").await.status_code(), 200);
        let row = api_tokens::Entity::find().one(&ctx.db).await.unwrap().unwrap();
        let res = request
            .post(&format!("/w/acme/settings/api/tokens/{}/revoke", row.id))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(get(&request, &token, "/api/v1/projects").await.status_code(), 401);

        // User dinonaktifkan.
        let token = new_token(&request, &f.cookie, &["tiket:baca"], "all").await;
        exec(&ctx, "UPDATE users SET disabled_at = now() WHERE id = $1", f.owner.id).await;
        assert_eq!(get(&request, &token, "/api/v1/projects").await.status_code(), 401);
        exec(&ctx, "UPDATE users SET disabled_at = NULL WHERE id = $1", f.owner.id).await;
        assert_eq!(get(&request, &token, "/api/v1/projects").await.status_code(), 200);

        // Kedaluwarsa.
        exec(&ctx, "UPDATE api_tokens SET expires_at = now() - interval '1 minute' WHERE user_id = $1", f.owner.id).await;
        assert_eq!(get(&request, &token, "/api/v1/projects").await.status_code(), 401);
    })
    .await;
}

#[tokio::test]
async fn member_tokens_follow_their_role_and_die_when_they_leave() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let (member_id, _) = member(&request, &ctx, &f, "Mira Member").await;
        let member_row = agentgate_server::models::users::Entity::find_by_id(member_id)
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        let raw = token_for(&ctx, &f.workspace, &member_row, &["tiket:baca", "webhook:kelola"]).await;
        create_ticket(&request, &f.cookie, &[("title", "Tiket KSR")]).await;
        assert_eq!(get(&request, &raw, "/api/v1/tickets/KSR-1").await.status_code(), 200);
        // Member tidak memegang ManageIntegrations: webhook API 404 walau cakupannya ada.
        assert_eq!(get(&request, &raw, "/api/v1/webhooks").await.status_code(), 404);
        exec(
            &ctx,
            "DELETE FROM workspace_members WHERE user_id = $1",
            member_id,
        )
        .await;
        assert_eq!(get(&request, &raw, "/api/v1/tickets/KSR-1").await.status_code(), 401);
    })
    .await;
}

/// Token langsung lewat model (user tanpa ManageIntegrations tidak bisa memakai UI).
async fn token_for(
    ctx: &loco_rs::app::AppContext,
    workspace: &agentgate_server::models::workspaces::Model,
    user: &agentgate_server::models::users::Model,
    scopes: &[&str],
) -> String {
    let (_, raw) = api_tokens::ActiveModel::create(
        &ctx.db,
        api_tokens::NewApiToken {
            workspace_id: workspace.workspace_id(),
            user_id: user.id,
            project_id: None,
            name: "Token member".into(),
            scopes: scopes.iter().filter_map(|s| api_tokens::ApiScope::parse(s)).collect(),
            expires_at: None,
        },
    )
    .await
    .unwrap();
    raw
}

async fn exec(ctx: &loco_rs::app::AppContext, sql: &str, id: uuid::Uuid) {
    ctx.db
        .execute_raw(Statement::from_sql_and_values(DbBackend::Postgres, sql, [id.into()]))
        .await
        .unwrap();
}

#[tokio::test]
async fn rate_limit_answers_429_json() {
    request_with_settings(
        |s| s["api"]["rate_limit_per_minute"] = json!(2),
        |request, ctx| async move {
            let f = fixture(&request, &ctx).await;
            let token = new_token(&request, &f.cookie, &["tiket:baca"], "all").await;
            let first = get(&request, &token, "/api/v1/projects").await;
            assert_eq!(first.header("x-ratelimit-remaining"), "1");
            let second = get(&request, &token, "/api/v1/projects").await;
            assert_eq!(second.header("x-ratelimit-remaining"), "0");
            let third = get(&request, &token, "/api/v1/projects").await;
            assert_eq!(third.status_code(), 429);
            assert_eq!(json(&third)["error"], "rate_limited");
            assert_eq!(third.header("x-ratelimit-remaining"), "0");
            // Token lain punya jatahnya sendiri.
            let other = new_token(&request, &f.cookie, &["tiket:baca"], "all").await;
            assert_eq!(get(&request, &other, "/api/v1/projects").await.status_code(), 200);
        },
    )
    .await;
}

#[tokio::test]
async fn transition_goes_through_the_gate() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        use_workflow(&request, &ctx, &f, FLOW_YAML).await;
        let code = create_ticket(&request, &f.cookie, &[("title", "Checkout QRIS")]).await;
        let token = new_token(&request, &f.cookie, &["tiket:baca", "tiket:tulis"], "all").await;
        let path = format!("/api/v1/tickets/{code}/transition");
        let res = send(&request, &token, Method::POST, &path, &json!({ "to": "prd" })).await;
        assert_eq!(res.status_code(), 200, "{}", res.text());
        assert_eq!(json(&res)["ticket"]["stage"], "prd");
        // PRD butuh artifact + approval: lompat ke Dev ditolak gate.
        let res = send(&request, &token, Method::POST, &path, &json!({ "to": "dev" })).await;
        assert_eq!(res.status_code(), 422, "{}", res.text());
        assert!(json(&res)["error"].as_str().unwrap().starts_with("ticket-"), "{}", res.text());
        assert_eq!(ticket(&ctx, &f.workspace, &code).await.stage_key, "prd");
        let res = send(&request, &token, Method::POST, &path, &json!({ "to": "" })).await;
        assert_eq!(res.status_code(), 422);
        let t = tickets::Entity::find().one(&ctx.db).await.unwrap().unwrap();
        assert_eq!(t.stage_key, "prd");
    })
    .await;
}

#[tokio::test]
async fn webhooks_can_be_managed_over_the_api() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let token = new_token(&request, &f.cookie, &["webhook:kelola"], "all").await;
        let res = send(
            &request,
            &token,
            Method::POST,
            "/api/v1/webhooks",
            &json!({ "url": "https://contoh.id/hooks", "events": ["ticket.created"], "projects": ["KSR"] }),
        )
        .await;
        assert_eq!(res.status_code(), 201, "{}", res.text());
        let hook = json(&res)["webhook"].clone();
        assert!(hook["secret"].as_str().unwrap().starts_with("whsec_"));
        let res = send(
            &request,
            &token,
            Method::POST,
            "/api/v1/webhooks",
            &json!({ "url": "http://contoh.id/hooks", "events": ["ticket.created"] }),
        )
        .await;
        assert_eq!(res.status_code(), 422);
        assert_eq!(json(&res)["error"], "webhook-url-invalid");
        let res = send(&request, &token, Method::POST, "/api/v1/webhooks", &json!({ "url": "https://contoh.id/h", "events": [] })).await;
        assert_eq!(json(&res)["error"], "webhook-events-required");

        let res = get(&request, &token, "/api/v1/webhooks").await;
        let list = json(&res);
        assert_eq!(list["webhooks"].as_array().unwrap().len(), 1);
        assert!(list["webhooks"][0].get("secret").is_none());

        let id = hook["id"].as_str().unwrap();
        let res = send(&request, &token, Method::DELETE, &format!("/api/v1/webhooks/{id}"), &json!({})).await;
        assert_eq!(res.status_code(), 204);
        let res = send(&request, &token, Method::DELETE, &format!("/api/v1/webhooks/{id}"), &json!({})).await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}

#[tokio::test]
async fn openapi_document_lists_every_route() {
    request::<App, _, _>(|request, ctx| async move {
        let res = request.get("/api/v1/openapi.yaml").await;
        assert_eq!(res.status_code(), 200);
        let doc = res.text();
        assert!(doc.starts_with("openapi: 3"));
        // Peta path → metode dari YAML (path di indentasi 2, metode di indentasi 4).
        let mut documented: Vec<(String, String)> = Vec::new();
        let mut current = String::new();
        for line in doc.lines() {
            if let Some(p) = line.strip_prefix("  /").and_then(|l| l.strip_suffix(':')) {
                current = format!("/{p}");
            } else if let Some(m) = line.strip_prefix("    ").and_then(|l| l.strip_suffix(':')) {
                if ["get", "post", "patch", "put", "delete"].contains(&m) {
                    documented.push((current.clone(), m.to_string()));
                }
            }
        }
        let mut missing = Vec::new();
        let mut count = 0;
        for route in App::routes(&ctx).collect() {
            let Some(path) = route.uri.strip_prefix("/api/v1") else {
                continue;
            };
            for action in &route.actions {
                let m = action.as_str().to_lowercase();
                if m == "head" {
                    continue;
                }
                count += 1;
                if !documented.contains(&(path.to_string(), m.clone())) {
                    missing.push(format!("{m} {path}"));
                }
            }
        }
        assert!(count >= 15, "{count}");
        assert!(missing.is_empty(), "route tanpa dokumentasi OpenAPI: {missing:?}");
    })
    .await;
}
