//! `GET /` (pilih workspace) dan `GET /w/{slug}` (beranda workspace).

use agentgate_server::{app::App, models::roles::RoleKey};
use loco_rs::testing::prelude::*;

use crate::support::{a_user, a_workspace, add_member, sign_in_as};

#[tokio::test]
async fn root_without_workspace_redirects_to_new() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().create(&ctx).await;
        let cookie = sign_in_as(&request, &user).await;
        let res = request.get("/").add_cookie(cookie).await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(res.header("location"), "/w/new");
    })
    .await;
}

#[tokio::test]
async fn root_redirects_to_only_workspace() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().create(&ctx).await;
        a_workspace(&user).slug("satu-satunya").create(&ctx).await;
        let cookie = sign_in_as(&request, &user).await;
        let res = request.get("/").add_cookie(cookie).await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(res.header("location"), "/w/satu-satunya");
    })
    .await;
}

#[tokio::test]
async fn root_lists_workspaces_when_several() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().create(&ctx).await;
        let other = a_user().create(&ctx).await;
        a_workspace(&user)
            .name("Beta")
            .slug("beta")
            .create(&ctx)
            .await;
        let joined = a_workspace(&other)
            .name("Alfa")
            .slug("alfa")
            .create(&ctx)
            .await;
        add_member(&ctx, &joined, &user, RoleKey::Viewer).await;
        // Workspace orang lain tanpa keanggotaan tidak ikut.
        a_workspace(&other)
            .name("Rahasia")
            .slug("rahasia")
            .create(&ctx)
            .await;

        let cookie = sign_in_as(&request, &user).await;
        let res = request.get("/").add_cookie(cookie).await;
        assert_eq!(res.status_code(), 200);
        let html = res.text();
        assert_css_exists(&html, "[data-workspace-choice][href=\"/w/alfa\"]");
        assert_css_exists(&html, "[data-workspace-choice][href=\"/w/beta\"]");
        assert_css_not_exists(&html, "[href=\"/w/rahasia\"]");
        // Switcher sidebar juga berisi keduanya, tanpa workspace aktif.
        assert_css_exists(&html, "[data-workspace-item][href=\"/w/alfa\"]");
        assert_css_not_exists(&html, "[data-workspace-item][aria-current]");
    })
    .await;
}

#[tokio::test]
async fn uppercase_slug_in_url_resolves() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().create(&ctx).await;
        a_workspace(&user)
            .name("Acme")
            .slug("acme")
            .create(&ctx)
            .await;
        let cookie = sign_in_as(&request, &user).await;
        let res = request.get("/w/ACME").add_cookie(cookie.clone()).await;
        assert_eq!(res.status_code(), 200);
        // Link di halaman memakai slug kanonis.
        assert_css_exists(
            &res.text(),
            "[data-workspace-item][href=\"/w/acme\"][aria-current=\"true\"]",
        );
        let res = request.get("/w/AcMe/settings").add_cookie(cookie).await;
        assert_eq!(res.status_code(), 200);
    })
    .await;
}

#[tokio::test]
async fn workspace_home_shows_empty_projects_and_settings_only_for_managers() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let member = a_user().create(&ctx).await;
        let admin = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner)
            .name("Acme")
            .slug("acme")
            .create(&ctx)
            .await;
        add_member(&ctx, &workspace, &member, RoleKey::Member).await;
        add_member(&ctx, &workspace, &admin, RoleKey::Admin).await;

        let cookie = sign_in_as(&request, &owner).await;
        let res = request.get("/w/acme").add_cookie(cookie).await;
        assert_eq!(res.status_code(), 200);
        let html = res.text();
        assert!(html.contains("Belum ada project"), "{html}");
        // Owner memegang CreateProjects: tombol buat project menuju form.
        assert_css_exists(
            &html,
            "a[data-create-project][href=\"/w/acme/projects/new\"]",
        );
        assert_css_eq(&html, "[data-workspace-name]", "Acme");
        assert_css_exists(&html, "a[href=\"/w/acme/settings\"]");

        let cookie = sign_in_as(&request, &member).await;
        let html = request.get("/w/acme").add_cookie(cookie).await.text();
        assert_css_not_exists(&html, "a[href=\"/w/acme/settings\"]");

        // Admin punya halaman pengaturan lain (anggota, role, tim), jadi tautannya tampil.
        let cookie = sign_in_as(&request, &admin).await;
        let html = request.get("/w/acme").add_cookie(cookie).await.text();
        assert_css_exists(&html, "a[href=\"/w/acme/settings\"]");
    })
    .await;
}
