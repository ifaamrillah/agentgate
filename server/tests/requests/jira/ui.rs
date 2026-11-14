//! Pengaturan → Jira: panel (ManageIntegrations; selain itu 404, juga lintas workspace), uji
//! koneksi, simpan (token terenkripsi, audit), isian ditolak (422), pemetaan, jeda/lanjut,
//! putus (tiket tetap ada), log sinkron + unduhan JSONL.

use agentgate_server::{
    app::App,
    models::{jira_connections, jira_links, jira_sync_logs},
};
use loco_rs::testing::prelude::*;
use sea_orm::{EntityTrait, PaginatorTrait};

use super::{connect, connection, fixture, set_mode};
use crate::{
    requests::tickets::{create_ticket, stages::member},
    support::{a_user, a_workspace, fake_jira, newest_audit_events, sign_in_as},
};

const PANELS: [&str; 6] = ["overview", "conn", "mode", "status", "fields", "log"];

#[tokio::test]
async fn pages_need_manage_integrations() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let res = request
            .get("/w/acme/settings/jira")
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(res.header("location"), "/w/acme/settings/jira/overview");
        for panel in PANELS {
            let res = request
                .get(&format!("/w/acme/settings/jira/{panel}"))
                .add_cookie(f.cookie.clone())
                .await;
            assert_eq!(res.status_code(), 200, "{panel}: {}", res.text());
            let html = res.text();
            assert_css_exists(&html, &format!("[data-jira-page='{panel}']"));
            assert_css_exists(&html, "[data-nav='jira']");
            if panel != "conn" {
                assert_css_exists(&html, "[data-jira-empty]");
            }
        }
        let res = request
            .get("/w/acme/settings/jira/bogus")
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 404);

        let (_, member_cookie) = member(&request, &ctx, &f, "Mira Member").await;
        for path in ["/w/acme/settings/jira", "/w/acme/settings/jira/overview"] {
            let res = request.get(path).add_cookie(member_cookie.clone()).await;
            assert_eq!(res.status_code(), 404, "{path}");
        }
        for path in [
            "/w/acme/settings/jira/sync",
            "/w/acme/settings/jira/disconnect",
        ] {
            let res = request.post(path).add_cookie(member_cookie.clone()).await;
            assert_eq!(res.status_code(), 404, "{path}");
        }
        let res = request
            .get("/w/acme/settings")
            .add_cookie(member_cookie.clone())
            .await;
        assert!(!res.text().contains("/w/acme/settings/jira"));
        let res = request
            .get("/w/acme/settings")
            .add_cookie(f.cookie.clone())
            .await;
        assert!(res.text().contains("/w/acme/settings/jira"));
    })
    .await;
}

#[tokio::test]
async fn other_workspace_is_404() {
    request::<App, _, _>(|request, ctx| async move {
        let fake = fake_jira::start().await;
        let f = fixture(&request, &ctx).await;
        connect(&request, &f.cookie, &fake).await;
        // Owner workspace lain tidak bisa melihat atau mengubah koneksi acme.
        let other = a_user().name("Budi Lain").create(&ctx).await;
        a_workspace(&other).slug("beta").create(&ctx).await;
        let cookie = sign_in_as(&request, &other).await;
        for panel in PANELS {
            let res = request
                .get(&format!("/w/acme/settings/jira/{panel}"))
                .add_cookie(cookie.clone())
                .await;
            assert_eq!(res.status_code(), 404, "{panel}");
        }
        for path in ["sync", "pause", "disconnect", "resync"] {
            let res = request
                .post(&format!("/w/acme/settings/jira/{path}"))
                .add_cookie(cookie.clone())
                .await;
            assert_eq!(res.status_code(), 404, "{path}");
        }
        let res = request
            .get("/w/acme/settings/jira/log/download")
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(res.status_code(), 404);
        // Workspace beta tidak punya koneksi: panelnya kosong, bukan koneksi acme.
        let res = request
            .get("/w/beta/settings/jira/overview")
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(res.status_code(), 200);
        assert_css_exists(&res.text(), "[data-jira-empty]");
        assert!(!res.text().contains(&fake.url));
        assert_eq!(connection(&ctx).await.paused_at, None);
    })
    .await;
}

#[tokio::test]
async fn connection_is_tested_saved_and_audited() {
    request::<App, _, _>(|request, ctx| async move {
        let fake = fake_jira::start().await;
        let f = fixture(&request, &ctx).await;
        let form = |token: &'static str| {
            vec![
                ("env", "cloud"),
                ("base_url", fake.url.as_str()),
                ("email", fake_jira::EMAIL),
                ("token", token),
                ("jira_project", "SR"),
                ("project", "KSR"),
            ]
        };
        // Uji koneksi: nama akun + jumlah project, tanpa menyimpan.
        let res = request
            .post("/w/acme/settings/jira/conn/test")
            .add_cookie(f.cookie.clone())
            .form(&form(fake_jira::TOKEN))
            .await;
        assert_eq!(res.status_code(), 200, "{}", res.text());
        let html = res.text();
        assert_css_exists(&html, "[data-jira-tested]");
        assert!(html.contains("Olivia Owner"), "{html}");
        assert!(html.contains("2 project"), "{html}");
        assert!(html.contains("SR · Sevima Retail POS"));
        assert_eq!(
            jira_connections::Entity::find()
                .count(&ctx.db)
                .await
                .unwrap(),
            0
        );

        // Token salah → 422 dengan pesan di field token.
        let res = request
            .post("/w/acme/settings/jira/conn/test")
            .add_cookie(f.cookie.clone())
            .form(&form("salah"))
            .await;
        assert_eq!(res.status_code(), 422);
        assert!(res.text().contains("Token ditolak Jira"));
        let res = request
            .post("/w/acme/settings/jira/conn")
            .add_cookie(f.cookie.clone())
            .form(&form("salah"))
            .await;
        assert_eq!(res.status_code(), 422);
        assert_eq!(
            jira_connections::Entity::find()
                .count(&ctx.db)
                .await
                .unwrap(),
            0
        );

        // URL bukan https (selain loopback) ditolak.
        let res = request
            .post("/w/acme/settings/jira/conn")
            .add_cookie(f.cookie.clone())
            .form(&[
                ("env", "cloud"),
                ("base_url", "http://jira.example.com"),
                ("email", fake_jira::EMAIL),
                ("token", fake_jira::TOKEN),
            ])
            .await;
        assert_eq!(res.status_code(), 422);

        connect(&request, &f.cookie, &fake).await;
        let conn = connection(&ctx).await;
        assert_eq!(conn.jira_project_key, "SR");
        assert_eq!(conn.account_name, "Olivia Owner");
        assert_eq!(conn.owner_id, Some(f.owner.id));
        assert!(
            !String::from_utf8_lossy(&conn.token_encrypted).contains(fake_jira::TOKEN),
            "token terenkripsi"
        );
        // Peta dimuat dari Jira: status disarankan dari workflow Kanban, user lewat email.
        let statuses = conn.status_map();
        let stage = |id: &str| statuses.stage_of(id).map(str::to_string);
        assert_eq!(stage(fake_jira::TODO).as_deref(), Some("todo"));
        assert_eq!(
            stage(fake_jira::IN_PROGRESS).as_deref(),
            Some("in_progress")
        );
        assert_eq!(stage(fake_jira::DONE).as_deref(), Some("done"));
        assert_eq!(
            conn.user_map().user_of(fake_jira::ACCOUNT),
            Some(f.owner.id)
        );
        let audit = newest_audit_events(&ctx.db, f.workspace.workspace_id(), 1)
            .await
            .unwrap();
        assert_eq!(audit[0].action, "jira.connected");
        // Token tidak pernah dirender; menyimpan ulang tanpa token memakai token tersimpan.
        let res = request
            .get("/w/acme/settings/jira/conn")
            .add_cookie(f.cookie.clone())
            .await;
        assert!(!res.text().contains(fake_jira::TOKEN));
        let res = request
            .post("/w/acme/settings/jira/conn")
            .add_cookie(f.cookie.clone())
            .form(&form(""))
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let audit = newest_audit_events(&ctx.db, f.workspace.workspace_id(), 1)
            .await
            .unwrap();
        assert_eq!(audit[0].action, "jira.updated");
        assert_eq!(connection(&ctx).await.jira_project_key, "SR");

        // Ringkasan menampilkan koneksi; panel Status menampilkan status Jira.
        let res = request
            .get("/w/acme/settings/jira/overview")
            .add_cookie(f.cookie.clone())
            .await;
        let html = res.text();
        assert_css_exists(&html, "[data-jira-overview]");
        assert!(html.contains("127.0.0.1"));
        let res = request
            .get("/w/acme/settings/jira/status")
            .add_cookie(f.cookie.clone())
            .await;
        assert!(res.text().contains("In Progress"));
    })
    .await;
}

#[tokio::test]
async fn mappings_are_saved_and_rejected_values_are_422() {
    request::<App, _, _>(|request, ctx| async move {
        let fake = fake_jira::start().await;
        fake.add_status("10003", "Blocked", "indeterminate");
        let f = fixture(&request, &ctx).await;
        connect(&request, &f.cookie, &fake).await;
        set_mode(&request, &f.cookie, "once").await;
        // Status yang tidak cocok dengan nama stage memakai kategori → in_progress.
        let conn = connection(&ctx).await;
        assert_eq!(conn.status_map().stage_of("10003"), Some("in_progress"));
        let res = request
            .post("/w/acme/settings/jira/status")
            .add_cookie(f.cookie.clone())
            .form(&[("st:10003", ""), ("st:10000", "backlog")])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let conn = connection(&ctx).await;
        assert_eq!(conn.status_map().stage_of("10003"), None);
        assert_eq!(conn.status_map().stage_of("10000"), Some("backlog"));
        let res = request
            .get("/w/acme/settings/jira/overview")
            .add_cookie(f.cookie.clone())
            .await;
        assert_css_exists(&res.text(), "[data-jira-unmapped]");
        assert!(res.text().contains("Blocked"));

        let res = request
            .post("/w/acme/settings/jira/status")
            .add_cookie(f.cookie.clone())
            .form(&[("st:10003", "bukan-stage")])
            .await;
        assert_eq!(res.status_code(), 422);
        assert_css_exists(&res.text(), "[data-jira-error]");

        let res = request
            .post("/w/acme/settings/jira/fields")
            .add_cookie(f.cookie.clone())
            .form(&[
                ("f:labels", "none"),
                ("f:customfield_10016", "story_points"),
            ])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let fm = connection(&ctx).await.field_map();
        assert_eq!(fm.jira_field(jira_connections::FieldTarget::Labels), None);
        let res = request
            .post("/w/acme/settings/jira/fields")
            .add_cookie(f.cookie.clone())
            .form(&[("f:labels", "custom:00000000-0000-0000-0000-000000000000")])
            .await;
        assert_eq!(res.status_code(), 422);
        let res = request
            .post("/w/acme/settings/jira/fields")
            .add_cookie(f.cookie.clone())
            .form(&[(
                format!("u:{}", fake_jira::ACCOUNT),
                uuid::Uuid::now_v7().to_string(),
            )])
            .await;
        assert_eq!(res.status_code(), 422);

        let res = request
            .post("/w/acme/settings/jira/mode")
            .add_cookie(f.cookie.clone())
            .form(&[("mode", "sideways")])
            .await;
        assert_eq!(res.status_code(), 422);
        let res = request
            .get("/w/acme/settings/jira/fields")
            .add_cookie(f.cookie.clone())
            .await;
        assert!(res.text().contains("Story Points"));
    })
    .await;
}

#[tokio::test]
async fn pause_resume_log_and_disconnect() {
    request::<App, _, _>(|request, ctx| async move {
        let fake = fake_jira::start().await;
        let f = fixture(&request, &ctx).await;
        create_ticket(&request, &f.cookie, &[("title", "Checkout QRIS")]).await;
        connect(&request, &f.cookie, &fake).await;
        set_mode(&request, &f.cookie, "mirror").await;
        assert_eq!(fake.issues().len(), 1);

        // Dijeda: perubahan tidak didorong.
        let res = request
            .post("/w/acme/settings/jira/pause")
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303);
        assert!(connection(&ctx).await.paused_at.is_some());
        create_ticket(&request, &f.cookie, &[("title", "Struk digital")]).await;
        assert_eq!(fake.issues().len(), 1);
        // Dilanjutkan: tertinggal ikut terdorong.
        let res = request
            .post("/w/acme/settings/jira/resume")
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(fake.issues().len(), 2);

        // Log sinkron: datatable + filter + unduhan JSONL.
        let res = request
            .get("/w/acme/settings/jira/log")
            .add_cookie(f.cookie.clone())
            .await;
        let html = res.text();
        assert_css_exists(&html, "[data-jira-log] tr[data-result='ok']");
        assert!(html.contains("Issue baru dibuat di Jira"), "{html}");
        let res = request
            .get("/w/acme/settings/jira/log?result=fail")
            .add_cookie(f.cookie.clone())
            .await;
        assert_css_not_exists(&res.text(), "[data-jira-log] tr[data-result='ok']");
        let res = request
            .get("/w/acme/settings/jira/log/download")
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 200);
        assert!(
            res.header("content-disposition")
                .to_str()
                .unwrap()
                .contains("jira-sync-log.jsonl")
        );
        let lines: Vec<serde_json::Value> = res
            .text()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert!(
            lines
                .iter()
                .any(|l| l["message"] == "jira-log-issue-created")
        );
        assert!(
            lines
                .iter()
                .all(|l| l["direction"] == "out" || l["direction"] == "in")
        );

        // Putus: koneksi, tautan, dan log hilang; tiket tetap ada.
        let res = request
            .post("/w/acme/settings/jira/disconnect")
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(
            jira_connections::Entity::find()
                .count(&ctx.db)
                .await
                .unwrap(),
            0
        );
        assert_eq!(jira_links::Entity::find().count(&ctx.db).await.unwrap(), 0);
        assert_eq!(
            jira_sync_logs::Entity::find().count(&ctx.db).await.unwrap(),
            0
        );
        let res = request
            .get("/w/acme/t/KSR-2")
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 200);
        let audit = newest_audit_events(&ctx.db, f.workspace.workspace_id(), 1)
            .await
            .unwrap();
        assert_eq!(audit[0].action, "jira.disconnected");
        // Ticket baru setelah putus tidak memicu apa pun ke Jira.
        create_ticket(&request, &f.cookie, &[("title", "Diskon")]).await;
        assert_eq!(fake.issues().len(), 2);
    })
    .await;
}
