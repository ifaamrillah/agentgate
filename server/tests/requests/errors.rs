use agentgate_server::app::App;
use loco_rs::testing::prelude::*;

#[tokio::test]
async fn unknown_path_renders_full_html_404_page() {
    request::<App, _, _>(|request, _ctx| async move {
        let res = request.get("/tidak-ada").await;
        assert_eq!(res.status_code(), 404);
        let html = res.text();
        assert!(html.starts_with("<!doctype html>"), "{html}");
        assert!(html.contains("Halaman tidak ditemukan"));
        assert!(html.contains("Kode error"));
    })
    .await;
}

#[tokio::test]
async fn boosted_htmx_request_to_unknown_path_also_gets_full_page() {
    request::<App, _, _>(|request, _ctx| async move {
        let res = request
            .get("/tidak-ada")
            .add_header("HX-Request", "true")
            .add_header("HX-Request-Type", "full")
            .await;
        assert_eq!(res.status_code(), 404);
        assert!(res.text().contains("<html lang=\"id\">"));
        assert_eq!(
            res.header("content-type").to_str().unwrap(),
            "text/html; charset=utf-8"
        );
    })
    .await;
}

#[tokio::test]
async fn missing_static_file_is_a_404_not_a_200_fallback() {
    request::<App, _, _>(|request, _ctx| async move {
        let res = request.get("/static/tidak-ada.js").await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}
