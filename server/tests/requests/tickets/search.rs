//! Pencarian dasar: kata di judul/deskripsi (full-text), potongan kode/judul, hanya project yang
//! boleh dibuka, tanpa tiket di sampah.

use agentgate_server::app::App;
use loco_rs::testing::prelude::*;

use super::{create_ticket, fixture};

#[tokio::test]
async fn search_finds_by_words_and_code() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        create_ticket(
            &request,
            &f.cookie,
            &[
                ("title", "Pembayaran QRIS"),
                ("description", "Hubungkan gateway pembayaran"),
            ],
        )
        .await;
        create_ticket(&request, &f.cookie, &[("title", "Laporan harian")]).await;
        let gone = create_ticket(&request, &f.cookie, &[("title", "Gateway lama")]).await;
        request
            .post(&format!("/w/acme/t/{gone}/delete"))
            .add_cookie(f.cookie.clone())
            .await;

        let search = |q: &str| {
            let request = &request;
            let cookie = f.cookie.clone();
            let path = format!("/w/acme/search?q={q}");
            async move {
                let res = request.get(&path).add_cookie(cookie).await;
                assert_eq!(res.status_code(), 200);
                res.text()
            }
        };
        let html = search("gateway").await;
        assert_css_exists(&html, "[data-ticket-row='KSR-1']");
        assert_css_not_exists(&html, "[data-ticket-row='KSR-2']");
        assert_css_not_exists(&html, "[data-ticket-row='KSR-3']");
        let html = search("ksr-2").await;
        assert_css_exists(&html, "[data-ticket-row='KSR-2']");
        let html = search("lapor").await;
        assert_css_exists(&html, "[data-ticket-row='KSR-2']");
        let html = search("100%25").await;
        assert_css_not_exists(&html, "[data-ticket-row]");
        // Tanpa kata kunci: halaman kosong yang jujur.
        let html = search("").await;
        assert_css_not_exists(&html, "[data-ticket-row]");
    })
    .await;
}
