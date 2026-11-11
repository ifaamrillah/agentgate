use agentgate_server::app::App;
use loco_rs::testing::prelude::*;

use crate::support::{a_user, sign_in_as};

#[tokio::test]
async fn home_page_renders_with_layout_and_translations() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().create(&ctx).await;
        let cookie = sign_in_as(&request, &user).await;
        let res = request.get("/").add_cookie(cookie).await;
        assert_eq!(res.status_code(), 200);
        let html = res.text();
        assert!(
            html.contains("<title>Beranda · Agent Gate</title>"),
            "{html}"
        );
        assert!(html.contains("hx-boost:inherited=\"true\""));
        assert!(html.contains("/static/vendor/htmx-4.0.1.min.js"));
        // Logout adalah form POST, bukan link GET.
        assert!(html.contains("action=\"/logout\""));
    })
    .await;
}

#[tokio::test]
async fn home_requires_sign_in() {
    request::<App, _, _>(|request, _ctx| async move {
        let res = request.get("/").await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(res.header("location"), "/login?return_to=%2F");
    })
    .await;
}
