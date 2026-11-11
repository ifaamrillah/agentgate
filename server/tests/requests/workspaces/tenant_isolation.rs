//! Extractor `Tenant` dan `Require<P>`: bukan anggota, workspace lain, tanpa izin, dan
//! workspace yang dijadwalkan hapus semuanya 404 (bukan 403); 2FA wajib → setup 2FA.

use agentgate_server::{app::App, models::roles::RoleKey};
use loco_rs::testing::prelude::*;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, IntoActiveModel};

use crate::support::{a_user, a_workspace, add_member, sign_in_as};

#[tokio::test]
async fn non_member_gets_404_for_workspace() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        a_workspace(&owner).slug("acme").create(&ctx).await;
        let stranger = a_user().create(&ctx).await;
        let cookie = sign_in_as(&request, &stranger).await;
        for path in [
            "/w/acme",
            "/w/acme/settings",
            "/w/tidak-ada",
            "/w/tidak-ada/settings",
        ] {
            let res = request.get(path).add_cookie(cookie.clone()).await;
            assert_eq!(res.status_code(), 404, "{path}");
        }
        let res = request
            .post("/w/acme/settings")
            .add_cookie(cookie)
            .form(&[("name", "Dibajak"), ("slug", "acme")])
            .await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}

#[tokio::test]
async fn signed_out_user_is_sent_to_login() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        a_workspace(&owner).slug("acme").create(&ctx).await;
        let res = request.get("/w/acme").await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(res.header("location"), "/login?return_to=%2Fw%2Facme");
    })
    .await;
}

#[tokio::test]
async fn other_workspace_member_gets_404_for_settings() {
    request::<App, _, _>(|request, ctx| async move {
        let alice = a_user().create(&ctx).await;
        let bob = a_user().create(&ctx).await;
        a_workspace(&alice).slug("milik-alice").create(&ctx).await;
        a_workspace(&bob).slug("milik-bob").create(&ctx).await;
        // Bob Owner di workspace-nya sendiri, tetapi bukan anggota workspace Alice.
        let cookie = sign_in_as(&request, &bob).await;
        assert_eq!(
            request
                .get("/w/milik-bob/settings")
                .add_cookie(cookie.clone())
                .await
                .status_code(),
            200
        );
        let res = request
            .get("/w/milik-alice/settings")
            .add_cookie(cookie)
            .await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}

#[tokio::test]
async fn viewer_cannot_open_settings_gets_404() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner)
            .name("Acme")
            .slug("acme")
            .create(&ctx)
            .await;
        for key in [RoleKey::Viewer, RoleKey::Member, RoleKey::Admin] {
            let user = a_user().create(&ctx).await;
            add_member(&ctx, &workspace, &user, key).await;
            let cookie = sign_in_as(&request, &user).await;
            assert_eq!(
                request
                    .get("/w/acme")
                    .add_cookie(cookie.clone())
                    .await
                    .status_code(),
                200,
                "{key:?}"
            );
            let res = request
                .get("/w/acme/settings")
                .add_cookie(cookie.clone())
                .await;
            assert_eq!(res.status_code(), 404, "{key:?}");
            // Izin dicek sebelum body: form tidak valid pun tetap 404, bukan 422.
            let res = request
                .post("/w/acme/settings")
                .add_cookie(cookie)
                .form(&[("name", ""), ("slug", "x")])
                .await;
            assert_eq!(res.status_code(), 404, "{key:?}");
        }
        let unchanged = agentgate_server::models::workspaces::Model::find_by_slug(&ctx.db, "acme")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(unchanged.name, "Acme");
    })
    .await;
}

#[tokio::test]
async fn scheduled_deletion_hides_workspace_from_non_owner() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let member = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        add_member(&ctx, &workspace, &member, RoleKey::Admin).await;
        let mut active = workspace.into_active_model();
        active.deletion_scheduled_at = Set(Some(chrono::Utc::now().into()));
        active.update(&ctx.db).await.unwrap();

        let cookie = sign_in_as(&request, &member).await;
        assert_eq!(
            request
                .get("/w/acme")
                .add_cookie(cookie.clone())
                .await
                .status_code(),
            404
        );
        // Juga hilang dari `/`: tanpa workspace lain, member diarahkan membuat workspace.
        let res = request.get("/").add_cookie(cookie).await;
        assert_eq!(res.header("location"), "/w/new");

        let cookie = sign_in_as(&request, &owner).await;
        assert_eq!(
            request
                .get("/w/acme")
                .add_cookie(cookie)
                .await
                .status_code(),
            200
        );
    })
    .await;
}

#[tokio::test]
async fn require_2fa_redirects_member_without_totp_to_setup() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let member = a_user().create(&ctx).await;
        let stranger = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        add_member(&ctx, &workspace, &member, RoleKey::Member).await;
        let mut active = workspace.into_active_model();
        active.require_2fa_at = Set(Some(chrono::Utc::now().into()));
        active.update(&ctx.db).await.unwrap();

        let cookie = sign_in_as(&request, &member).await;
        let res = request.get("/w/acme").add_cookie(cookie.clone()).await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(res.header("location"), "/account/2fa?required=acme");

        let page = request
            .get("/account/2fa?required=acme")
            .add_cookie(cookie)
            .await;
        assert_eq!(page.status_code(), 200);
        assert_css_exists(&page.text(), "[data-two-factor-required]");

        // Bukan anggota tetap 404: redirect 2FA tidak boleh membocorkan keberadaan workspace.
        let cookie = sign_in_as(&request, &stranger).await;
        assert_eq!(
            request
                .get("/w/acme")
                .add_cookie(cookie)
                .await
                .status_code(),
            404
        );
    })
    .await;
}
