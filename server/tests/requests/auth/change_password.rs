use agentgate_server::{app::App, models::users};
use loco_rs::testing::prelude::*;

use crate::support::{TEST_PASSWORD, a_user, session_cookie, sign_in_as};

const NEW_PASSWORD: &str = "kuda baterai staples benar";

#[tokio::test]
async fn change_password_page_requires_sign_in() {
    request::<App, _, _>(|request, _ctx| async move {
        let res = request.get("/account/password").await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(
            res.header("location"),
            "/login?return_to=%2Faccount%2Fpassword"
        );
    })
    .await;
}

#[tokio::test]
async fn change_password_requires_current_password() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().create(&ctx).await;
        let cookie = sign_in_as(&request, &user).await;

        let page = request
            .get("/account/password")
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(page.status_code(), 200);
        assert_css_exists(
            &page.text(),
            r#"form[action="/account/password"][method="post"]"#,
        );
        assert_css_exists(
            &page.text(),
            r#"input[name="current_password"][type="password"]"#,
        );

        let res = request
            .post("/account/password")
            .add_cookie(cookie.clone())
            .form(&[
                ("current_password", "password yang salah"),
                ("password", NEW_PASSWORD),
            ])
            .await;
        assert_eq!(res.status_code(), 422);
        let html = res.text();
        assert!(html.contains("Password saat ini salah."), "{html}");
        assert!(!html.contains(NEW_PASSWORD));
        assert!(!html.contains("password yang salah"));

        let missing = request
            .post("/account/password")
            .add_cookie(cookie.clone())
            .form(&[("current_password", ""), ("password", NEW_PASSWORD)])
            .await;
        assert_eq!(missing.status_code(), 422);

        let still = users::Model::find_by_id(&ctx.db, user.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(still.password_hash, user.password_hash);
        // Sesi tetap berlaku.
        assert_eq!(request.get("/").add_cookie(cookie).await.status_code(), 200);
    })
    .await;
}

#[tokio::test]
async fn change_password_rejects_a_short_new_password() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().create(&ctx).await;
        let cookie = sign_in_as(&request, &user).await;
        let res = request
            .post("/account/password")
            .add_cookie(cookie)
            .form(&[("current_password", TEST_PASSWORD), ("password", "pendek")])
            .await;
        assert_eq!(res.status_code(), 422);
        assert!(res.text().contains("Password minimal 12 karakter."));
    })
    .await;
}

#[tokio::test]
async fn change_password_keeps_only_current_device_signed_in() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().create(&ctx).await;
        let this_device = sign_in_as(&request, &user).await;
        let other_device = sign_in_as(&request, &user).await;

        let res = request
            .post("/account/password")
            .add_cookie(this_device.clone())
            .form(&[
                ("current_password", TEST_PASSWORD),
                ("password", NEW_PASSWORD),
            ])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(res.header("location"), "/account");
        let renewed = session_cookie(&res).expect("sesi baru untuk perangkat ini");
        assert_ne!(renewed.value(), this_device.value());

        // Perangkat ini tetap login dengan sesi baru, dan melihat flash.
        let account = request.get("/account").add_cookie(renewed).await;
        assert_eq!(account.status_code(), 200);
        assert!(
            account.text().contains("Password berhasil diganti."),
            "{}",
            account.text()
        );
        // Sesi lama perangkat ini dan sesi perangkat lain berakhir.
        for cookie in [this_device, other_device] {
            assert_eq!(request.get("/").add_cookie(cookie).await.status_code(), 303);
        }

        let changed = users::Model::find_by_id(&ctx.db, user.id)
            .await
            .unwrap()
            .unwrap();
        assert!(changed.password_changed_at.is_some());
        let login = request
            .post("/login")
            .form(&[("email", user.email.as_str()), ("password", NEW_PASSWORD)])
            .await;
        assert_eq!(login.status_code(), 303);
    })
    .await;
}
