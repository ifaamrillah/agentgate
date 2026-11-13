//! Board Scrum: bawaan sprint aktif, chip "Semua tiket", swimlane penerima/epic, batas WIP dari
//! workflow (disimpan lewat builder) ditandai saat terlampaui.

use agentgate_server::app::App;
use loco_rs::testing::prelude::*;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, IntoActiveModel};

use super::{create_ticket, fixture, sprint, ticket};

#[tokio::test]
async fn scrum_board_filters_active_sprint_and_swimlanes() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let a = create_ticket(&request, &f.cookie, &[("title", "Di sprint")]).await;
        let b = create_ticket(&request, &f.cookie, &[("title", "Di backlog")]).await;

        // Tanpa sprint: semua tiket, tanpa chip sprint.
        let page = request.get("/w/acme/p/KSR/board").add_cookie(f.cookie.clone()).await.text();
        assert_css_not_exists(&page, "[data-board-sprints]");
        assert!(page.contains(&a) && page.contains(&b));

        request
            .post("/w/acme/p/KSR/backlog/sprints")
            .add_cookie(f.cookie.clone())
            .form(&[("name", "Sprint 1"), ("goal", "QRIS jalan"), ("starts_on", "2026-11-16"), ("ends_on", "2026-11-27")])
            .await;
        let s1 = sprint(&ctx, &f, "Sprint 1").await;
        let t = ticket(&ctx, &f.workspace, &a).await;
        let mut m = t.into_active_model();
        m.sprint_id = Set(Some(s1.id));
        m.update(&ctx.db).await.unwrap();
        // Sprint rencana belum aktif → bawaan tetap semua tiket; pilih sprint lewat chip.
        let page = request.get("/w/acme/p/KSR/board").add_cookie(f.cookie.clone()).await.text();
        assert_css_exists(&page, "[data-board-sprints]");
        assert!(page.contains(&b));
        let page = request.get(&format!("/w/acme/p/KSR/board?sprint={}", s1.id)).add_cookie(f.cookie.clone()).await.text();
        assert!(page.contains(&a) && !page.contains(&b));
        // Mulai sprint → board bawaan sprint aktif.
        request
            .post(&format!("/w/acme/p/KSR/backlog/sprints/{}/start", s1.id))
            .add_cookie(f.cookie.clone())
            .form(&[("name", "")])
            .await;
        let page = request.get("/w/acme/p/KSR/board").add_cookie(f.cookie.clone()).await.text();
        assert!(page.contains(&a) && !page.contains(&b), "bawaan sprint aktif");
        assert_css_exists(&page, "[data-board-goal]");
        let page = request.get("/w/acme/p/KSR/board?sprint=all").add_cookie(f.cookie.clone()).await.text();
        assert!(page.contains(&b));

        // Swimlane.
        let page = request.get("/w/acme/p/KSR/board?sprint=all&swim=assignee").add_cookie(f.cookie.clone()).await.text();
        assert_css_exists(&page, "[data-lane]");
        assert_css_exists(&page, "[data-lane-board][data-board]");
        let page = request.get("/w/acme/p/KSR/board?sprint=all&swim=epic").add_cookie(f.cookie.clone()).await;
        assert_eq!(page.status_code(), 200);
        assert!(page.text().contains("Tanpa epic"));
    })
    .await;
}
