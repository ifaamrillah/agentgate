//! Pencarian lanjutan (query ala JQL), saved filter (simpan, tampil, hapus hanya pemilik), dan
//! edit massal (prioritas, sprint, label; field kosong tidak mengubah; pilihan kosong ditolak).

use agentgate_server::{app::App, models::saved_filters, services::planning};
use chrono::Duration;
use loco_rs::testing::prelude::*;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait, IntoActiveModel};

use super::{create_ticket, fixture, sprint, ticket};
use crate::support::{a_user, add_member, sign_in_as};
use agentgate_server::models::roles::RoleKey;

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn advanced_search_filters_and_bulk_edit() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let today = planning::today();
        let start = today.to_string();
        let end = (today + Duration::days(10)).to_string();
        request
            .post("/w/acme/p/KSR/backlog/sprints")
            .add_cookie(f.cookie.clone())
            .form(&[
                ("name", "Sprint 1"),
                ("starts_on", start.as_str()),
                ("ends_on", end.as_str()),
            ])
            .await;
        let s1 = sprint(&ctx, &f, "Sprint 1").await;
        let a = create_ticket(
            &request,
            &f.cookie,
            &[("title", "Bayar QRIS"), ("priority", "high")],
        )
        .await;
        let b = create_ticket(
            &request,
            &f.cookie,
            &[("title", "Struk digital"), ("priority", "low")],
        )
        .await;
        let c = create_ticket(
            &request,
            &f.cookie,
            &[("title", "Laporan harian"), ("priority", "low")],
        )
        .await;
        let mut m = ticket(&ctx, &f.workspace, &a).await.into_active_model();
        m.sprint_id = Set(Some(s1.id));
        m.story_points = Set(Some(5));
        m.update(&ctx.db).await.unwrap();

        // Halaman awal + tautan dari pencarian dasar.
        let page = request
            .get("/w/acme/search")
            .add_cookie(f.cookie.clone())
            .await;
        assert!(page.text().contains("/w/acme/search/advanced"));
        let page = request
            .get("/w/acme/search/advanced")
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(page.status_code(), 200);
        assert!(page.text().contains("data-advanced-page"));

        // Query: sprint terbuka.
        let page = request
            .get("/w/acme/search/advanced")
            .add_query_param("q", "sprint in openSprints() AND points >= 3")
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(page.status_code(), 200);
        let html = page.text();
        assert!(html.contains(&format!("data-ticket-row=\"{a}\"")), "{html}");
        assert!(!html.contains(&format!("data-ticket-row=\"{b}\"")));

        // Teks + prioritas, ORDER BY.
        let page = request
            .get("/w/acme/search/advanced")
            .add_query_param("q", "priority = low ORDER BY created DESC")
            .add_cookie(f.cookie.clone())
            .await;
        let html = page.text();
        let (pb, pc) = (
            html.find(&format!("data-ticket-row=\"{b}\"")).expect("b"),
            html.find(&format!("data-ticket-row=\"{c}\"")).expect("c"),
        );
        assert!(pc < pb, "terbaru dulu");
        assert!(!html.contains(&format!("data-ticket-row=\"{a}\"")));

        // ORDER BY priority: paling mendesak dulu.
        let html = request
            .get("/w/acme/search/advanced")
            .add_query_param("q", "project = KSR ORDER BY priority")
            .add_cookie(f.cookie.clone())
            .await
            .text();
        let pos = |code: &str| {
            html.find(&format!("data-ticket-row=\"{code}\""))
                .expect("baris")
        };
        assert!(pos(&a) < pos(&b), "high sebelum low");

        // OR, NOT, dan kurung (R3).
        let html = request
            .get("/w/acme/search/advanced")
            .add_query_param("q", "(text ~ qris OR text ~ struk) AND NOT priority = high")
            .add_cookie(f.cookie.clone())
            .await
            .text();
        assert!(html.contains(&format!("data-ticket-row=\"{b}\"")), "{html}");
        assert!(!html.contains(&format!("data-ticket-row=\"{a}\"")));
        assert!(!html.contains(&format!("data-ticket-row=\"{c}\"")));
        let html = request
            .get("/w/acme/search/advanced")
            .add_query_param("q", "text ~ qris OR text ~ laporan")
            .add_cookie(f.cookie.clone())
            .await
            .text();
        assert!(html.contains(&format!("data-ticket-row=\"{a}\"")));
        assert!(html.contains(&format!("data-ticket-row=\"{c}\"")));
        assert!(!html.contains(&format!("data-ticket-row=\"{b}\"")));

        // Query salah → pesan Fluent, tanpa hasil.
        let page = request
            .get("/w/acme/search/advanced")
            .add_query_param("q", "warna = merah")
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(page.status_code(), 200);
        let html = page.text();
        assert!(html.contains("data-jql-error"));
        assert!(html.contains("Field “warna” tidak dikenal"), "{html}");
        let html = request
            .get("/w/acme/search/advanced")
            .add_query_param("q", "(priority = low")
            .add_cookie(f.cookie.clone())
            .await
            .text();
        assert!(html.contains("Kurung belum ditutup."), "{html}");

        // Saved filter: nama kosong 422, lalu tersimpan.
        let res = request
            .post("/w/acme/search/advanced/filters")
            .add_cookie(f.cookie.clone())
            .form(&[
                ("name", ""),
                ("query", "priority = low"),
                ("visibility", "private"),
            ])
            .await;
        assert_eq!(res.status_code(), 422);
        assert!(res.text().contains("data-open-on-load"));
        let res = request
            .post("/w/acme/search/advanced/filters")
            .add_cookie(f.cookie.clone())
            .form(&[
                ("name", "Prioritas rendah"),
                ("query", "OR"),
                ("visibility", "private"),
            ])
            .await;
        assert_eq!(res.status_code(), 422);
        let res = request
            .post("/w/acme/search/advanced/filters")
            .add_cookie(f.cookie.clone())
            .form(&[
                ("name", "Prioritas rendah"),
                ("query", "priority = low"),
                ("visibility", "workspace"),
            ])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let filters = saved_filters::Entity::find().all(&ctx.db).await.unwrap();
        assert_eq!(filters.len(), 1);
        let page = request
            .get("/w/acme/search/advanced")
            .add_cookie(f.cookie.clone())
            .await;
        assert!(page.text().contains("data-filter=\"Prioritas rendah\""));

        // Anggota lain melihat filter workspace tetapi tidak bisa menghapusnya.
        let member = a_user().create(&ctx).await;
        add_member(&ctx, &f.workspace, &member, RoleKey::Member).await;
        let mc = sign_in_as(&request, &member).await;
        let page = request
            .get("/w/acme/search/advanced")
            .add_cookie(mc.clone())
            .await;
        let html = page.text();
        assert!(html.contains("data-filter=\"Prioritas rendah\""));
        assert!(!html.contains("data-filter-delete"));
        let res = request
            .post(&format!(
                "/w/acme/search/advanced/filters/{}/delete",
                filters[0].id
            ))
            .add_cookie(mc.clone())
            .form(&[("x", "1")])
            .await;
        assert_eq!(res.status_code(), 404);

        // Edit massal: tanpa pilihan → ditolak tanpa perubahan.
        let tb = ticket(&ctx, &f.workspace, &b).await;
        let tc = ticket(&ctx, &f.workspace, &c).await;
        let res = request
            .post("/w/acme/search/advanced/bulk")
            .add_cookie(f.cookie.clone())
            .form(&[("priority", "urgent")])
            .await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(ticket(&ctx, &f.workspace, &b).await.priority, "low");

        // Prioritas + sprint + label untuk b dan c; penerima tidak diubah.
        let sid = s1.id.to_string();
        let (idb, idc) = (tb.id.to_string(), tc.id.to_string());
        let res = request
            .post("/w/acme/search/advanced/bulk")
            .add_cookie(f.cookie.clone())
            .form(&[
                ("ids", idb.as_str()),
                ("ids", idc.as_str()),
                ("priority", "urgent"),
                ("assignee", ""),
                ("sprint", sid.as_str()),
                ("release", ""),
                ("label", "kasir"),
                ("return_to", "/w/acme/search/advanced?q=priority+%3D+low"),
            ])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(
            res.headers().get("location").unwrap().to_str().unwrap(),
            "/w/acme/search/advanced?q=priority+%3D+low"
        );
        for code in [&b, &c] {
            let t = ticket(&ctx, &f.workspace, code).await;
            assert_eq!(t.priority, "urgent");
            assert_eq!(t.sprint_id, Some(s1.id));
            assert!(t.labels.contains(&"kasir".to_string()));
            assert_eq!(t.assignee_id, tb.assignee_id);
        }

        // Kembalikan ke backlog.
        request
            .post("/w/acme/search/advanced/bulk")
            .add_cookie(f.cookie.clone())
            .form(&[("ids", idb.as_str()), ("sprint", "backlog")])
            .await;
        assert_eq!(ticket(&ctx, &f.workspace, &b).await.sprint_id, None);

        // Pemilik menghapus filternya.
        let res = request
            .post(&format!(
                "/w/acme/search/advanced/filters/{}/delete",
                filters[0].id
            ))
            .add_cookie(f.cookie.clone())
            .form(&[("x", "1")])
            .await;
        assert_eq!(res.status_code(), 303);
        assert!(
            saved_filters::Entity::find()
                .all(&ctx.db)
                .await
                .unwrap()
                .is_empty()
        );
    })
    .await;
}

#[tokio::test]
async fn query_autocomplete_suggests_fields_operators_and_values() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let today = planning::today();
        request
            .post("/w/acme/p/KSR/backlog/sprints")
            .add_cookie(f.cookie.clone())
            .form(&[
                ("name", "Sprint QRIS"),
                ("starts_on", today.to_string().as_str()),
                ("ends_on", (today + Duration::days(7)).to_string().as_str()),
            ])
            .await;
        create_ticket(
            &request,
            &f.cookie,
            &[("title", "Bayar"), ("labels", "pembayaran, kasir")],
        )
        .await;
        let suggest = |q: &'static str| {
            let request = &request;
            let cookie = f.cookie.clone();
            async move {
                let res = request
                    .get("/w/acme/search/advanced/suggest")
                    .add_query_param("q", q)
                    .add_cookie(cookie)
                    .await;
                assert_eq!(res.status_code(), 200, "{}", res.text());
                res.json::<serde_json::Value>()
            }
        };
        let inserts = |v: &serde_json::Value| -> Vec<String> {
            v["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|i| i["insert"].as_str().unwrap().to_string())
                .collect()
        };

        // Field yang diketik sebagian (awalan dulu) + keterangan dari Fluent.
        let v = suggest("assig").await;
        assert_eq!(v["partial"], "assig");
        assert_eq!(inserts(&v)[0], "assignee ");
        assert!(v["items"][0]["hint"].as_str().unwrap().contains("currentUser()"));
        // Operator per field: label tanpa <, points dengan <.
        let ops = inserts(&suggest("label ").await);
        assert!(ops.contains(&"in (".to_string()) && !ops.iter().any(|o| o.starts_with('<')));
        assert!(inserts(&suggest("points ").await).contains(&"<= ".to_string()));
        // Nilai dari data: label, sprint (dikutip), project, fungsi.
        let labels = inserts(&suggest("label = ").await);
        assert!(labels.contains(&"pembayaran ".to_string()), "{labels:?}");
        let sprints = inserts(&suggest("sprint = \"Spr").await);
        assert_eq!(sprints, vec!["\"Sprint QRIS\" ".to_string()]);
        assert!(inserts(&suggest("sprint in ").await).contains(&"openSprints() ".to_string()));
        assert!(inserts(&suggest("project in (").await).contains(&"KSR".to_string()));
        assert!(inserts(&suggest("assignee = ").await).contains(&"currentUser() ".to_string()));
        // Sambungan setelah syarat lengkap; `)` hanya di dalam kurung.
        let joins = inserts(&suggest("(status = done ").await);
        assert!(joins.contains(&"OR ".to_string()) && joins.contains(&") ".to_string()));
        assert!(inserts(&suggest("status = done ").await).contains(&"ORDER BY ".to_string()));
        // Query yang tidak terbaca: tanpa saran.
        assert!(inserts(&suggest("warna ").await).is_empty());
    })
    .await;
}
