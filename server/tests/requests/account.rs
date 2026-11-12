use agentgate_server::{app::App, models::users};
use loco_rs::testing::prelude::*;

use crate::support::{a_user, a_workspace, sign_in_as};

#[tokio::test]
async fn account_page_requires_sign_in() {
    request::<App, _, _>(|request, _ctx| async move {
        let res = request.get("/account").await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(res.header("location"), "/login?return_to=%2Faccount");
        let post = request
            .post("/account/profile")
            .form(&[("name", "Baru")])
            .await;
        assert_eq!(post.status_code(), 303);
        assert_eq!(post.header("location"), "/login");
    })
    .await;
}

#[tokio::test]
async fn account_page_shows_profile_and_links() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().name("Dewi Akun").create(&ctx).await;
        let cookie = sign_in_as(&request, &user).await;
        let res = request.get("/account").add_cookie(cookie).await;
        assert_eq!(res.status_code(), 200, "{}", res.text());
        let html = res.text();
        assert!(html.contains(&user.email), "{html}");
        assert_css_exists(&html, r#"input[name="name"][value="Dewi Akun"]"#);
        assert_css_exists(&html, r#"form[action="/account/profile"][method="post"]"#);
        assert_css_exists(&html, r#"a[href="/account/password"]"#);
        assert!(html.contains("Autentikasi dua langkah"), "{html}");
    })
    .await;
}

#[tokio::test]
async fn profile_name_can_be_changed() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().create(&ctx).await;
        let cookie = sign_in_as(&request, &user).await;

        let res = request
            .post("/account/profile")
            .add_cookie(cookie.clone())
            .form(&[("name", "  Nama Baru  ")])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(res.header("location"), "/account");
        let user = users::Model::find_by_id(&ctx.db, user.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(user.name, "Nama Baru");

        let page = request.get("/account").add_cookie(cookie.clone()).await;
        assert!(page.text().contains("Profil disimpan."), "{}", page.text());

        let invalid = request
            .post("/account/profile")
            .add_cookie(cookie)
            .form(&[("name", "   ")])
            .await;
        assert_eq!(invalid.status_code(), 422);
        assert!(invalid.text().contains("Nama wajib diisi."));
    })
    .await;
}

#[tokio::test]
async fn project_views_saved_in_order_and_shown_as_tabs() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        a_workspace(&owner).slug("acme").create(&ctx).await;
        let cookie = sign_in_as(&request, &owner).await;
        request
            .post("/w/acme/projects/new")
            .add_cookie(cookie.clone())
            .form(&[("key", "KSR"), ("name", "Kasir"), ("description", "")])
            .await;

        // Urutan dari `order`, hanya yang dicentang; kunci asing dan duplikat dibuang.
        let res = request
            .post("/account/project-views")
            .add_cookie(cookie.clone())
            .form(&[
                ("order", "list"),
                ("order", "bogus"),
                ("order", "overview"),
                ("order", "board"),
                ("view", "overview"),
                ("view", "list"),
                ("view", "bogus"),
                ("view", "list"),
                ("return_to", "/w/acme/p/KSR"),
            ])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(res.header("location"), "/w/acme/p/KSR");
        let saved = users::Model::find_by_id(&ctx.db, owner.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(saved.project_views, vec!["list", "overview"]);

        let html = request
            .get("/w/acme/p/KSR")
            .add_cookie(cookie.clone())
            .await
            .text();
        assert_css_exists(
            &html,
            "nav[data-tabs=\"project-views\"] a[data-nav=\"list\"]",
        );
        assert_css_exists(
            &html,
            "nav[data-tabs=\"project-views\"] a[data-nav=\"overview\"][aria-current=\"page\"]",
        );
        assert_css_not_exists(
            &html,
            "nav[data-tabs=\"project-views\"] a[data-nav=\"board\"]",
        );

        // Tanpa satu pun yang disematkan: tidak disimpan. `return_to` asing → `/`.
        let res = request
            .post("/account/project-views")
            .add_cookie(cookie)
            .form(&[("order", "board"), ("return_to", "//evil.example")])
            .await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(res.header("location"), "/");
        let saved = users::Model::find_by_id(&ctx.db, owner.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(saved.project_views, vec!["list", "overview"]);
    })
    .await;
}
