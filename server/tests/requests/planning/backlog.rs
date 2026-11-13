//! Backlog & sprint: satu sprint direncanakan, dijalankan, dan dilaporkan.

use agentgate_server::{
    app::App,
    models::{project_grants::GrantSubject, roles::RoleKey, sprints::SprintState},
};
use loco_rs::testing::prelude::*;

use super::{create_ticket, fixture, ksr, sprint, ticket};
use crate::support::{a_user, add_member, builtin_role, grant, sign_in_as};

#[tokio::test]
async fn sprint_is_planned_started_and_completed() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let a = create_ticket(&request, &f.cookie, &[("title", "Bayar QRIS"), ("story_points", "5")]).await;
        let b = create_ticket(&request, &f.cookie, &[("title", "Struk email"), ("story_points", "3")]).await;
        let c = create_ticket(&request, &f.cookie, &[("title", "Diskon item")]).await;

        let page = request.get("/w/acme/p/KSR/backlog").add_cookie(f.cookie.clone()).await;
        assert_eq!(page.status_code(), 200, "{}", page.text());
        let html = page.text();
        assert_css_exists(&html, "[data-sprint=backlog] [data-ticket-row='KSR-1']");
        assert_css_exists(&html, "dialog#sprint-new");
        assert_css_exists(&html, "a[href='/w/acme/p/KSR/backlog'][aria-current]");

        // Validasi: tanggal terbalik → 422, dialog terbuka.
        let res = request
            .post("/w/acme/p/KSR/backlog/sprints")
            .add_cookie(f.cookie.clone())
            .form(&[("name", "Sprint 1"), ("starts_on", "2026-11-20"), ("ends_on", "2026-11-10")])
            .await;
        assert_eq!(res.status_code(), 422);
        assert_css_exists(&res.text(), "dialog#sprint-new[data-open-on-load]");

        let res = request
            .post("/w/acme/p/KSR/backlog/sprints")
            .add_cookie(f.cookie.clone())
            .form(&[
                ("name", "Sprint 1"),
                ("goal", "Pembayaran"),
                ("starts_on", "2026-11-16"),
                ("ends_on", "2026-11-27"),
                ("capacity", "6"),
            ])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let s1 = sprint(&ctx, &f, "Sprint 1").await;
        assert_eq!(s1.state(), SprintState::Planned);

        // Rencanakan: A dan B masuk sprint, A ke paling atas.
        for (code, place) in [(&a, "top"), (&b, "keep")] {
            let t = ticket(&ctx, &f.workspace, code).await;
            let res = request
                .post(&format!("/w/acme/p/KSR/backlog/move/{}", t.id))
                .add_cookie(f.cookie.clone())
                .form(&[("to", s1.id.to_string().as_str()), ("place", place)])
                .await;
            assert_eq!(res.status_code(), 303, "{}", res.text());
        }
        let html = request.get("/w/acme/p/KSR/backlog").add_cookie(f.cookie.clone()).await.text();
        let sel = format!("[data-sprint='{}'] [data-ticket-row='KSR-2']", s1.id);
        assert_css_exists(&html, &sel);
        assert_css_exists(&html, "[data-sprint=backlog] [data-ticket-row='KSR-3']");
        // 8 poin > kapasitas 6.
        assert!(html.contains("Melebihi kapasitas 2 poin"), "{html}");
        assert_css_exists(&html, &format!("dialog#sprint-start-{}", s1.id));
        let t3 = ticket(&ctx, &f.workspace, &c).await;
        assert!(t3.sprint_id.is_none());

        // Mulai sprint: komitmen = 8 poin.
        let res = request
            .post(&format!("/w/acme/p/KSR/backlog/sprints/{}/start", s1.id))
            .add_cookie(f.cookie.clone())
            .form(&[("name", "")])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let s1 = sprint(&ctx, &f, "Sprint 1").await;
        assert_eq!(s1.state(), SprintState::Active);
        assert_eq!(s1.committed_points, 8);
        // Mulai dua kali → 422.
        let res = request
            .post(&format!("/w/acme/p/KSR/backlog/sprints/{}/start", s1.id))
            .add_cookie(f.cookie.clone())
            .form(&[("name", "")])
            .await;
        assert_eq!(res.status_code(), 422);

        // Tengah sprint: C ditambahkan, A selesai.
        let t3 = ticket(&ctx, &f.workspace, &c).await;
        let res = request
            .post(&format!("/w/acme/p/KSR/backlog/move/{}", t3.id))
            .add_cookie(f.cookie.clone())
            .form(&[("to", s1.id.to_string().as_str())])
            .await;
        assert_eq!(res.status_code(), 303);
        let res = request
            .post(&format!("/w/acme/t/{a}/status"))
            .add_cookie(f.cookie.clone())
            .form(&[("status", "done")])
            .await;
        assert_eq!(res.status_code(), 303);
        let html = request.get("/w/acme/p/KSR/backlog").add_cookie(f.cookie.clone()).await.text();
        assert_css_exists(&html, &format!("dialog#sprint-complete-{}", s1.id));
        assert_css_exists(&html, "[data-ticket-row='KSR-1'][data-done]");

        // Selesaikan: sisa kembali ke backlog, laporan beku.
        let res = request
            .post(&format!("/w/acme/p/KSR/backlog/sprints/{}/complete", s1.id))
            .add_cookie(f.cookie.clone())
            .form(&[("target", "backlog")])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let s1 = sprint(&ctx, &f, "Sprint 1").await;
        assert_eq!(s1.state(), SprintState::Completed);
        let report = s1.report();
        assert_eq!(report.committed, 8);
        assert_eq!(report.completed, 5);
        assert_eq!(report.done.len(), 1);
        assert_eq!(report.not_done.len(), 2);
        assert!(ticket(&ctx, &f.workspace, &b).await.sprint_id.is_none());
        assert_eq!(ticket(&ctx, &f.workspace, &a).await.sprint_id, Some(s1.id));
        let html = request.get("/w/acme/p/KSR/backlog").add_cookie(f.cookie.clone()).await.text();
        assert_css_exists(&html, "[data-sprint=backlog] [data-ticket-row='KSR-2']");
        // Velocity muncul setelah sprint pertama selesai.
        assert_css_exists(&html, "[data-stat='backlog-stat-velocity']");
        assert!(html.contains(">5<"), "velocity 5: {html}");
    })
    .await;
}

#[tokio::test]
async fn viewer_sees_backlog_but_cannot_plan() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        create_ticket(&request, &f.cookie, &[("title", "Bayar QRIS")]).await;
        let viewer = a_user().create(&ctx).await;
        add_member(&ctx, &f.workspace, &viewer, RoleKey::Member).await;
        let viewer_role = builtin_role(&ctx, &f.workspace, RoleKey::Viewer).await;
        let p = ksr(&ctx, &f).await;
        grant(&ctx, &f.workspace, &p, GrantSubject::User(viewer.id), &viewer_role).await;
        let cookie = sign_in_as(&request, &viewer).await;
        let page = request.get("/w/acme/p/KSR/backlog").add_cookie(cookie.clone()).await;
        assert_eq!(page.status_code(), 200, "{}", page.text());
        let html = page.text();
        assert_css_exists(&html, "[data-ticket-row='KSR-1']");
        assert_css_not_exists(&html, "[data-sprint-new]");
        assert_css_not_exists(&html, "[data-move-action]");
        let res = request
            .post("/w/acme/p/KSR/backlog/sprints")
            .add_cookie(cookie)
            .form(&[("name", "S"), ("starts_on", "2026-11-16"), ("ends_on", "2026-11-27")])
            .await;
        assert!(matches!(res.status_code().as_u16(), 403 | 404));
    })
    .await;
}
