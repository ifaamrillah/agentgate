use agentgate_server::app::App;
use loco_rs::testing::prelude::*;

#[tokio::test]
async fn home_page_renders_with_layout_and_translations() {
    request::<App, _, _>(|request, _ctx| async move {
        let res = request.get("/").await;
        assert_eq!(res.status_code(), 200);
        let html = res.text();
        assert!(html.contains("<title>Beranda · Agent Gate</title>"), "{html}");
        assert!(html.contains("hx-boost:inherited=\"true\""));
        assert!(html.contains("/static/vendor/htmx-4.0.1.min.js"));
    })
    .await;
}
