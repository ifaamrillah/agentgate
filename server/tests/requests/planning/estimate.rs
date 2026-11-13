//! Tiket: panel planning & estimasi (sprint/rilis di detail, ubah estimasi manusia, minta
//! estimasi AI lewat komentar @agent-analyst) dan field sprint/Fix version di form tiket.

use agentgate_server::{
    app::App,
    models::{ticket_comments, ticket_estimates},
    services::planning::releases::ReleaseInput,
};
use loco_rs::testing::prelude::*;

use super::{create_ticket, fixture, ksr, sprint, ticket};

#[tokio::test]
async fn ticket_shows_planning_and_saves_estimates() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let wid = f.workspace.workspace_id();
        let code = create_ticket(&request, &f.cookie, &[("title", "Struk digital")]).await;
        request
            .post("/w/acme/p/KSR/backlog/sprints")
            .add_cookie(f.cookie.clone())
            .form(&[
                ("name", "Sprint 1"),
                ("starts_on", "2026-11-16"),
                ("ends_on", "2026-11-27"),
            ])
            .await;
        let s1 = sprint(&ctx, &f, "Sprint 1").await;
        let p = ksr(&ctx, &f).await;
        let (_, role) = agentgate_server::models::workspace_members::Model::find_with_role(
            &ctx.db, wid, f.owner.id,
        )
        .await
        .unwrap()
        .unwrap();
        let acting = agentgate_server::services::workspaces::Acting {
            workspace: &f.workspace,
            user: &f.owner,
            role: &role,
            request_id: None,
        };
        let rel = agentgate_server::services::planning::releases::create(
            &ctx.db,
            &acting,
            p.id,
            &ReleaseInput {
                name: "v1.4".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let page = request
            .get(&format!("/w/acme/t/{code}"))
            .add_cookie(f.cookie.clone())
            .await
            .text();
        assert_css_exists(&page, "[data-ticket-estimate] [data-estimate-none]");
        assert_css_exists(&page, "#ticket-edit select[name=sprint]");
        assert_css_exists(&page, "#ticket-edit select[name=release]");

        // Ubah detail: masuk Sprint 1 dan v1.4.
        let res = request
            .post(&format!("/w/acme/t/{code}"))
            .add_cookie(f.cookie.clone())
            .form(&[
                ("title", "Struk digital"),
                ("sprint", s1.id.to_string().as_str()),
                ("release", rel.id.to_string().as_str()),
            ])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let t = ticket(&ctx, &f.workspace, &code).await;
        assert_eq!(t.sprint_id, Some(s1.id));
        assert_eq!(t.release_id, Some(rel.id));
        let page = request
            .get(&format!("/w/acme/t/{code}"))
            .add_cookie(f.cookie.clone())
            .await
            .text();
        assert_css_exists(&page, "[data-ticket-sprint]");
        assert_css_exists(&page, "[data-ticket-release]");
        let activity = request
            .get(&format!("/w/acme/t/{code}/activity"))
            .add_cookie(f.cookie.clone())
            .await
            .text();
        assert!(activity.contains("Sprint 1"), "riwayat sprint");

        // Estimasi manusia.
        let res = request
            .post(&format!("/w/acme/t/{code}/estimate"))
            .add_cookie(f.cookie.clone())
            .form(&[
                ("points", "3"),
                ("ai_development", "2,5"),
                ("human_development", "1"),
                ("ai_done", ""),
                ("human_done", ""),
                ("basis", "Mirip KSR-9"),
            ])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let t = ticket(&ctx, &f.workspace, &code).await;
        assert_eq!(t.story_points, Some(3));
        let rows = ticket_estimates::Model::of_tickets(&ctx.db, wid, &[t.id])
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].totals(), (2.5, 1.0));
        let page = request
            .get(&format!("/w/acme/t/{code}"))
            .add_cookie(f.cookie.clone())
            .await
            .text();
        assert_css_exists(&page, "[data-estimate-stages]");
        // Jam tidak valid → flash error, tidak tersimpan.
        request
            .post(&format!("/w/acme/t/{code}/estimate"))
            .add_cookie(f.cookie.clone())
            .form(&[
                ("points", "3"),
                ("ai_development", "abc"),
                ("human_development", "1"),
            ])
            .await;
        assert_eq!(
            ticket_estimates::Model::of_tickets(&ctx.db, wid, &[t.id])
                .await
                .unwrap()
                .len(),
            1
        );

        // Minta estimasi AI → komentar @agent-analyst.
        let res = request
            .post(&format!("/w/acme/t/{code}/estimate/ai"))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303);
        let comments = ticket_comments::Model::of_ticket(&ctx.db, wid, t.id)
            .await
            .unwrap();
        assert!(
            comments
                .iter()
                .any(|c| c.body.starts_with("@agent-analyst"))
        );
    })
    .await;
}
