//! Roadmap dan kalender: tiket bertanggal (sprint, tenggat) tampil, tiket tanpa tanggal masuk
//! "Tanpa tanggal", target rilis, filter jenis, zoom, navigasi bulan, panah blocks.

use agentgate_server::{
    app::App,
    models::ticket_links,
    services::planning,
};
use chrono::{Datelike, Duration};
use loco_rs::testing::prelude::*;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, IntoActiveModel};

use super::{create_ticket, fixture, sprint, ticket};

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn roadmap_and_calendar_show_dated_tickets() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let wid = f.workspace.workspace_id();
        let today = planning::today();
        let start = today.to_string();
        let end = (today + Duration::days(10)).to_string();
        request
            .post("/w/acme/p/KSR/backlog/sprints")
            .add_cookie(f.cookie.clone())
            .form(&[("name", "Sprint 1"), ("starts_on", start.as_str()), ("ends_on", end.as_str())])
            .await;
        let s1 = sprint(&ctx, &f, "Sprint 1").await;
        let a = create_ticket(&request, &f.cookie, &[("title", "Bayar QRIS")]).await;
        let b = create_ticket(&request, &f.cookie, &[("title", "Struk <b>digital</b>")]).await;
        let c = create_ticket(&request, &f.cookie, &[("title", "Tanpa tanggal")]).await;
        let ta = ticket(&ctx, &f.workspace, &a).await;
        let mut m = ta.clone().into_active_model();
        m.sprint_id = Set(Some(s1.id));
        m.status = Set("in_progress".into());
        m.update(&ctx.db).await.unwrap();
        // Tenggat di bulan ini (hari ini) agar ada di kalender bulan berjalan.
        let tb = ticket(&ctx, &f.workspace, &b).await;
        let mut m = tb.clone().into_active_model();
        m.due_on = Set(Some(today));
        m.update(&ctx.db).await.unwrap();
        ticket_links::ActiveModel {
            workspace_id: Set(wid.get()),
            from_id: Set(ta.id),
            to_id: Set(tb.id),
            kind: Set("blocks".into()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();
        let target = today.to_string();
        request
            .post("/w/acme/p/KSR/releases")
            .add_cookie(f.cookie.clone())
            .form(&[("name", "v1.4"), ("target_on", target.as_str())])
            .await;

        // Roadmap.
        let page = request.get("/w/acme/p/KSR/roadmap").add_cookie(f.cookie.clone()).await;
        assert_eq!(page.status_code(), 200);
        let page = page.text();
        assert_css_exists(&page, "[data-roadmap-page='week']");
        assert_css_exists(&page, &format!("[data-tl-row='{a}'] .app-tl-bar:not([data-est])"));
        assert_css_exists(&page, &format!("[data-tl-row='{b}'] .app-tl-bar[data-est]"));
        assert_css_not_exists(&page, &format!("[data-tl-row='{c}']"));
        assert_css_exists(&page, ".app-tl-mkl[data-release]");
        assert_css_exists(&page, ".app-tl-ov svg path[marker-end]");
        assert!(!page.contains("<b>digital</b>"));
        for zoom in ["month", "quarter"] {
            let page = request.get(&format!("/w/acme/p/KSR/roadmap?zoom={zoom}")).add_cookie(f.cookie.clone()).await;
            assert_eq!(page.status_code(), 200);
            assert_css_exists(&page.text(), &format!("[data-roadmap-page='{zoom}']"));
        }
        let page = request.get("/w/acme/p/KSR/roadmap?q=QRIS").add_cookie(f.cookie.clone()).await.text();
        assert_css_exists(&page, &format!("[data-tl-row='{a}']"));
        assert_css_not_exists(&page, &format!("[data-tl-row='{b}']"));

        // Kalender bulan berjalan.
        let page = request.get("/w/acme/p/KSR/calendar").add_cookie(f.cookie.clone()).await;
        assert_eq!(page.status_code(), 200);
        let page = page.text();
        let day = format!("[data-date='{today}']");
        assert_css_exists(&page, &format!("{day}[data-today]"));
        assert_css_exists(&page, &format!("{day} [data-event='due']"));
        assert_css_exists(&page, &format!("{day} [data-event='release']"));
        assert_css_exists(&page, "[data-calendar-undated]");
        assert!(page.contains(&c));
        // Filter jenis: hanya rilis.
        let month = format!("{:04}-{:02}", today.year(), today.month());
        let page = request
            .get(&format!("/w/acme/p/KSR/calendar?month={month}&kind=release"))
            .add_cookie(f.cookie.clone())
            .await
            .text();
        assert_css_not_exists(&page, "[data-event='due']");
        assert_css_exists(&page, "[data-event='release']");
        assert_css_exists(&page, "[data-kind='release'][aria-pressed='true']");
        // Bulan lain tanpa acara; bulan tidak valid → bulan ini.
        let page = request.get("/w/acme/p/KSR/calendar?month=2020-01").add_cookie(f.cookie.clone()).await.text();
        assert_css_exists(&page, "[data-calendar-page='2020-01'] [data-cal-empty]");
        let page = request.get("/w/acme/p/KSR/calendar?month=xx").add_cookie(f.cookie.clone()).await;
        assert_eq!(page.status_code(), 200);
    })
    .await;
}
