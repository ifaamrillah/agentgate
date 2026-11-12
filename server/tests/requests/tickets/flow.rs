//! Alur dasar tiket: buat, lihat, ubah sebagian, pindah status (board), board dan daftar, riwayat.

use agentgate_server::{
    app::App,
    models::{
        ticket_events::{self, EventKind},
        tickets::TicketStatus,
    },
};
use loco_rs::testing::prelude::*;

use super::{create_ticket, fixture, ticket};

#[tokio::test]
async fn create_show_update_and_move_ticket() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let wid = f.workspace.workspace_id();

        // Form baru tersedia; isian salah → 422 dengan error di field.
        let res = request
            .get("/w/acme/p/KSR/tickets/new")
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 200);
        let res = request
            .post("/w/acme/p/KSR/tickets/new")
            .add_cookie(f.cookie.clone())
            .form(&[("title", ""), ("story_points", "-3")])
            .await;
        assert_eq!(res.status_code(), 422);
        assert_css_exists(&res.text(), "#ticket-title-error");

        let code = create_ticket(
            &request,
            &f.cookie,
            &[
                ("title", "Bayar dengan QRIS"),
                ("description", "Integrasi **QRIS** di kasir"),
                ("priority", "high"),
                ("labels", "Kasir, Pembayaran"),
                ("story_points", "5"),
            ],
        )
        .await;
        assert_eq!(code, "KSR-1");
        let second = create_ticket(&request, &f.cookie, &[("title", "Struk")]).await;
        assert_eq!(second, "KSR-2");

        let t = ticket(&ctx, &f.workspace, &code).await;
        assert_eq!(t.priority, "high");
        assert_eq!(
            t.labels,
            vec!["Kasir".to_string(), "Pembayaran".to_string()]
        );
        assert_eq!(t.story_points, Some(5));
        assert_eq!(t.reporter_id, Some(f.owner.id));

        let page = request
            .get("/w/acme/t/KSR-1")
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(page.status_code(), 200);
        let html = page.text();
        assert!(html.contains("Bayar dengan QRIS"));
        assert_css_exists(&html, "[data-ticket-code]");
        // Kode tidak membedakan huruf besar.
        let page = request
            .get("/w/acme/t/ksr-1")
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(page.status_code(), 200);

        // Ubah sebagian: field yang tidak dikirim tetap.
        let res = request
            .post("/w/acme/t/KSR-1")
            .add_cookie(f.cookie.clone())
            .form(&[
                ("priority", "urgent"),
                ("assignee", &f.owner.id.to_string()),
            ])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let t = ticket(&ctx, &f.workspace, &code).await;
        assert_eq!(t.priority, "urgent");
        assert_eq!(t.assignee_id, Some(f.owner.id));
        assert_eq!(t.title, "Bayar dengan QRIS");
        assert_eq!(t.story_points, Some(5));

        // Pindah status lewat board.
        let res = request
            .post("/w/acme/t/KSR-1/status")
            .add_cookie(f.cookie.clone())
            .form(&[
                ("status", "in_progress"),
                ("return_to", "/w/acme/p/KSR/board"),
            ])
            .await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(res.header("location"), "/w/acme/p/KSR/board");
        let t = ticket(&ctx, &f.workspace, &code).await;
        assert_eq!(t.status().unwrap(), TicketStatus::InProgress);
        // return_to di luar workspace diabaikan.
        let res = request
            .post("/w/acme/t/KSR-1/status")
            .add_cookie(f.cookie.clone())
            .form(&[("status", "done"), ("return_to", "https://evil.example")])
            .await;
        assert_eq!(res.header("location"), "/w/acme/t/KSR-1");
        assert!(ticket(&ctx, &f.workspace, &code).await.done_at.is_some());

        let events = ticket_events::Model::of_ticket(&ctx.db, wid, t.id)
            .await
            .unwrap();
        let kinds: Vec<_> = events.iter().map(|e| e.kind().unwrap()).collect();
        assert_eq!(
            kinds,
            [
                EventKind::Created,
                EventKind::Updated,
                EventKind::StageChanged,
                EventKind::StageChanged
            ]
        );

        // Board: kartu ada di kolom statusnya; daftar memuat keduanya.
        let board = request
            .get("/w/acme/p/KSR/board")
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(board.status_code(), 200);
        let html = board.text();
        assert_css_exists(&html, "[data-column='done'] [data-ticket='KSR-1']");
        assert_css_exists(&html, "[data-column='todo'] [data-ticket='KSR-2']");
        let list = request
            .get("/w/acme/p/KSR/list?q=struk")
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(list.status_code(), 200);
        let html = list.text();
        assert_css_exists(&html, "[data-ticket-row='KSR-2']");
        assert_css_not_exists(&html, "[data-ticket-row='KSR-1']");

        // Riwayat.
        let activity = request
            .get("/w/acme/t/KSR-1/activity")
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(activity.status_code(), 200);
        assert_css_exists(&activity.text(), "[data-event='stage_changed']");

        // Ringkasan project menghitung tiket per status.
        let home = request
            .get("/w/acme/p/KSR")
            .add_cookie(f.cookie.clone())
            .await;
        assert_css_exists(&home.text(), "[data-ticket-stats]");
    })
    .await;
}

#[tokio::test]
async fn my_tickets_lists_assigned_reported_and_watched() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        create_ticket(
            &request,
            &f.cookie,
            &[("title", "Punyaku"), ("assignee", &f.owner.id.to_string())],
        )
        .await;
        let page = request.get("/w/acme/my").add_cookie(f.cookie.clone()).await;
        assert_eq!(page.status_code(), 200);
        assert_css_exists(&page.text(), "[data-ticket-row='KSR-1']");
        let page = request
            .get("/w/acme/my?tab=reported")
            .add_cookie(f.cookie.clone())
            .await;
        assert_css_exists(&page.text(), "[data-ticket-row='KSR-1']");
    })
    .await;
}
