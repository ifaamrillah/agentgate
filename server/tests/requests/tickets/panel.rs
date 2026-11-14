//! Panel samping detail tiket (R3): permintaan berheader `X-Ticket-Panel: 1` mendapat partial panel
//! (isi Ringkasan yang sama, tanpa layout); URL yang sama tanpa header = halaman penuh; board,
//! daftar, dan backlog memuat dialog panel + tautan pembuka; orang luar tetap 404.

use agentgate_server::app::App;
use loco_rs::testing::prelude::*;

use super::{create_ticket, fixture};
use crate::support::{a_user, a_workspace, sign_in_as};

#[tokio::test]
async fn panel_partial_shares_summary_with_full_page() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let parent = create_ticket(&request, &f.cookie, &[("title", "Rilis kasir")]).await;
        let code = create_ticket(
            &request,
            &f.cookie,
            &[("title", "Cetak <struk>"), ("parent", &parent)],
        )
        .await;
        let path = format!("/w/acme/t/{code}");

        let panel = request
            .get(&path)
            .add_header("x-ticket-panel", "1")
            .add_header("hx-request", "true")
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(panel.status_code(), 200);
        assert_eq!(panel.header("vary").to_str().unwrap(), "X-Ticket-Panel");
        let html = panel.text();
        assert!(!html.contains("<html"), "panel bukan halaman lengkap");
        assert_css_exists(&html, &format!("[data-ticket-panel-content='{code}']"));
        assert_css_exists(&html, "#ticket-panel-title");
        assert_css_exists(&html, &format!("a[data-ticket-panel-full][href='{path}']"));
        assert_css_exists(&html, "button[data-ticket-panel-close][aria-label]");
        // Isi Ringkasan yang sama: deskripsi, komentar, kartu stage, dialog ubah detail.
        assert_css_exists(&html, "#description");
        assert_css_exists(&html, "#comments");
        assert_css_exists(&html, "[data-ticket-details]");
        assert_css_exists(&html, "dialog#ticket-edit");
        // Induk dibuka di panel juga.
        assert_css_exists(
            &html,
            &format!("a[data-ticket-peek][href='/w/acme/t/{parent}']"),
        );
        assert!(!html.contains("<struk>"), "judul di-escape");

        // URL sama tanpa header: halaman penuh (dibuka langsung/refresh).
        let full = request.get(&path).add_cookie(f.cookie.clone()).await.text();
        assert!(full.contains("<html"));
        assert_css_exists(&full, "[data-ticket-page]");
        assert_css_exists(&full, "#description");
        assert_css_not_exists(&full, "dialog#ticket-panel");

        // Board, daftar, backlog: dialog panel + tautan pembuka.
        for view in ["board", "list", "backlog"] {
            let html = request
                .get(&format!("/w/acme/p/KSR/{view}"))
                .add_cookie(f.cookie.clone())
                .await
                .text();
            assert_css_exists(
                &html,
                "dialog#ticket-panel[aria-labelledby='ticket-panel-title']",
            );
            assert_css_exists(&html, &format!("a[data-ticket-peek][href='{path}']"));
        }

        // Orang luar: 404, juga untuk panel.
        let bob = a_user().create(&ctx).await;
        a_workspace(&bob).slug("milik-bob").create(&ctx).await;
        let cookie = sign_in_as(&request, &bob).await;
        let res = request
            .get(&path)
            .add_header("x-ticket-panel", "1")
            .add_cookie(cookie)
            .await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}
