use agentgate_server::app::App;
use loco_rs::testing::prelude::*;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, IntoActiveModel};

use crate::support::{TEST_PASSWORD, a_user, request_with_settings, session_cookie, sign_in_as};

#[tokio::test]
async fn disabled_user_existing_session_is_rejected() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().create(&ctx).await;
        let cookie = sign_in_as(&request, &user).await;
        assert_eq!(
            request
                .get("/")
                .add_cookie(cookie.clone())
                .await
                .status_code(),
            200
        );

        let mut active = user.into_active_model();
        active.disabled_at = Set(Some(chrono::Utc::now().into()));
        active.update(&ctx.db).await.unwrap();

        let res = request.get("/").add_cookie(cookie.clone()).await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(res.header("location"), "/login?return_to=%2F");
        // Sesinya ikut dihapus: cookie dihapus di browser.
        let removed = session_cookie(&res).unwrap();
        assert_eq!(removed.value(), "");
    })
    .await;
}

#[tokio::test]
async fn session_cookie_from_before_login_is_not_valid_after() {
    request::<App, _, _>(|request, ctx| async move {
        // Sesi sebelum login: sesi pending 2FA milik user lain (punya id di server).
        let two_factor = a_user().with_totp().create(&ctx).await;
        let pending = request
            .post("/login")
            .form(&[
                ("email", two_factor.email.as_str()),
                ("password", TEST_PASSWORD),
            ])
            .await;
        let before = session_cookie(&pending).unwrap();

        let user = a_user().create(&ctx).await;
        let res = request
            .post("/login")
            .add_cookie(before.clone())
            .form(&[("email", user.email.as_str()), ("password", TEST_PASSWORD)])
            .await;
        assert_eq!(res.status_code(), 303);
        let after = session_cookie(&res).unwrap();
        assert_ne!(after.value(), before.value());

        // Cookie lama tidak ikut menjadi sesi login.
        assert_eq!(request.get("/").add_cookie(before).await.status_code(), 303);
        assert_eq!(request.get("/").add_cookie(after).await.status_code(), 200);
    })
    .await;
}

#[tokio::test]
async fn session_older_than_absolute_lifetime_is_rejected() {
    // Umur mutlak 0 jam: setiap sesi sudah melewati batasnya saat dipakai berikutnya.
    request_with_settings(
        |s| s["auth"]["absolute_lifetime_hours"] = 0.into(),
        |request, ctx| async move {
            let user = a_user().create(&ctx).await;
            let cookie = sign_in_as(&request, &user).await;
            let res = request.get("/").add_cookie(cookie).await;
            assert_eq!(res.status_code(), 303);
            assert_eq!(res.header("location"), "/login?return_to=%2F");
        },
    )
    .await;
}
