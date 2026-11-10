use agentgate_server::app::App;
use loco_rs::testing::prelude::*;

#[tokio::test]
async fn component_catalog_renders_every_button_variant() {
    request::<App, _, _>(|request, _ctx| async move {
        let res = request.get("/_dev/ui").await;
        assert_eq!(res.status_code(), 200, "{}", res.text());
        let html = res.text();
        for variant in [
            "primary",
            "secondary",
            "outline",
            "ghost",
            "destructive",
            "link",
        ] {
            assert!(
                html.contains(&format!("data-variant=\"{variant}\"")),
                "{variant}"
            );
        }
        assert!(html.contains("<svg"), "ikon tidak dirender");
    })
    .await;
}
