use agentgate_server::app::App;
use loco_rs::testing::prelude::*;

use crate::support::{a_user, sign_in_as};

#[tokio::test]
async fn app_page_renders_with_layout_and_translations() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().create(&ctx).await;
        let cookie = sign_in_as(&request, &user).await;
        let res = request.get("/w/new").add_cookie(cookie).await;
        assert_eq!(res.status_code(), 200);
        let html = res.text();
        assert!(
            html.contains("<title>Buat workspace · Agent Gate</title>"),
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

#[tokio::test]
async fn app_layout_renders_with_minimal_shell() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().name("Budi Santoso").create(&ctx).await;
        let cookie = sign_in_as(&request, &user).await;
        // User tanpa workspace: `/` mengarah ke `/w/new`, yang memakai shell tanpa workspace.
        let html = request.get("/w/new").add_cookie(cookie).await.text();
        assert_css_exists(
            &html,
            "aside.sidebar#app-sidebar > nav[hx-preload\\:inherited=\"mouseover\"]",
        );
        // Tombol menu mobile menunjuk ke sidebar.
        assert_css_exists(
            &html,
            "button[data-sidebar-toggle=\"app-sidebar\"][aria-controls=\"app-sidebar\"]",
        );
        // Switcher tanpa workspace: hanya "Buat workspace", tanpa daftar.
        assert_css_exists(
            &html,
            "[role=\"menu\"] a[role=\"menuitem\"][href=\"/w/new\"]",
        );
        assert_css_not_exists(&html, "[data-workspace-item]");
        assert_css_not_exists(&html, "[data-project-list]");
        // Tanpa workspace tidak ada Pengaturan.
        assert_css_not_exists(&html, "a[href$=\"/settings\"]");
        // Menu profil: Akun (link) dan Keluar (form POST).
        assert_css_exists(&html, "a[role=\"menuitem\"][href=\"/account\"]");
        assert_css_exists(
            &html,
            "form[method=\"post\"][action=\"/logout\"] button[type=\"submit\"]",
        );
        assert_css_not_exists(&html, "a[href=\"/logout\"]");
        assert_css_eq(&html, "#profile-menu-trigger .avatar > span", "BS");
        assert!(html.contains("Budi Santoso"));
    })
    .await;
}
