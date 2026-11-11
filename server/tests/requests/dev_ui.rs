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

#[tokio::test]
async fn component_catalog_renders_field_and_flash() {
    request::<App, _, _>(|request, _ctx| async move {
        let html = request.get("/_dev/ui").await.text();
        // field: label terhubung ke input, error diterjemahkan dari kode dan ditautkan aria.
        assert!(html.contains("for=\"catalog-email\""), "{html}");
        assert!(html.contains("aria-invalid=\"true\""));
        assert!(html.contains("aria-describedby=\"catalog-password-error\""));
        assert!(html.contains("Password minimal 12 karakter."));
        for kind in ["info", "success", "error"] {
            assert!(html.contains(&format!("data-flash=\"{kind}\"")), "{kind}");
        }
    })
    .await;
}
