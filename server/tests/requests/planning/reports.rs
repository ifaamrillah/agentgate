//! Laporan: semua tab terbuka tanpa data (empty state) dan dengan sprint selesai (grafik SVG,
//! sprint report beku).

use agentgate_server::app::App;
use loco_rs::testing::prelude::*;

use super::{create_ticket, fixture, sprint, ticket};

const TABS: &[&str] = &["burndown", "burnup", "velocity", "sprint", "cfd", "calibration", "cycle"];

#[tokio::test]
async fn reports_render_empty_and_after_a_sprint() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        for tab in TABS {
            let res = request
                .get(&format!("/w/acme/p/KSR/reports?tab={tab}"))
                .add_cookie(f.cookie.clone())
                .await;
            assert_eq!(res.status_code(), 200, "{tab}: {}", res.text());
            assert_css_exists(&res.text(), &format!("[data-reports-page='{tab}']"));
        }

        let a = create_ticket(&request, &f.cookie, &[("title", "Bayar QRIS"), ("story_points", "5")]).await;
        let b = create_ticket(&request, &f.cookie, &[("title", "Struk <b>email</b>"), ("story_points", "3")]).await;
        let res = request
            .post("/w/acme/p/KSR/backlog/sprints")
            .add_cookie(f.cookie.clone())
            .form(&[("name", "Sprint <1>"), ("starts_on", "2026-11-02"), ("ends_on", "2026-11-13")])
            .await;
        assert_eq!(res.status_code(), 303);
        let s = sprint(&ctx, &f, "Sprint <1>").await;
        for code in [&a, &b] {
            let t = ticket(&ctx, &f.workspace, code).await;
            request
                .post(&format!("/w/acme/p/KSR/backlog/move/{}", t.id))
                .add_cookie(f.cookie.clone())
                .form(&[("to", s.id.to_string().as_str())])
                .await;
        }
        request
            .post(&format!("/w/acme/p/KSR/backlog/sprints/{}/start", s.id))
            .add_cookie(f.cookie.clone())
            .form(&[("name", "")])
            .await;
        let html = request
            .get("/w/acme/p/KSR/reports")
            .add_cookie(f.cookie.clone())
            .await
            .text();
        assert_css_exists(&html, "[data-report-chart] svg.app-chart");
        assert!(html.contains("Sprint &lt;1&gt;"), "nama sprint di-escape");
        assert!(!html.contains("Sprint <1>"));

        request
            .post(&format!("/w/acme/t/{a}/status"))
            .add_cookie(f.cookie.clone())
            .form(&[("status", "done")])
            .await;
        request
            .post(&format!("/w/acme/p/KSR/backlog/sprints/{}/complete", s.id))
            .add_cookie(f.cookie.clone())
            .form(&[("target", "new")])
            .await;

        let html = request
            .get(&format!("/w/acme/p/KSR/reports?tab=sprint&sprint={}", s.id))
            .add_cookie(f.cookie.clone())
            .await
            .text();
        assert_css_exists(&html, "[data-report-done] [data-report-row='KSR-1']");
        assert_css_exists(&html, "[data-report-left] [data-report-row='KSR-2']");
        assert!(html.contains("dipindah ke Sprint 2"), "{html}");
        for tab in TABS {
            let res = request
                .get(&format!("/w/acme/p/KSR/reports?tab={tab}"))
                .add_cookie(f.cookie.clone())
                .await;
            assert_eq!(res.status_code(), 200, "{tab}: {}", res.text());
            assert!(!res.text().contains("<b>email</b>"), "{tab}");
        }
        let velocity = request
            .get("/w/acme/p/KSR/reports?tab=velocity")
            .add_cookie(f.cookie.clone())
            .await
            .text();
        assert_css_exists(&velocity, "svg.app-chart rect.bx");
        let cfd = request
            .get("/w/acme/p/KSR/reports?tab=cfd")
            .add_cookie(f.cookie.clone())
            .await
            .text();
        assert_css_exists(&cfd, "svg.app-chart path.ar");
    })
    .await;
}
