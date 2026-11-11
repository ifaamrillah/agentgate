use agentgate_server::{
    app::App, common::settings::Settings, models::users, services::auth::password,
};
use loco_rs::testing::prelude::*;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, IntoActiveModel};
use secrecy::SecretString;

use crate::support::{TEST_PASSWORD, a_user, session_cookie, sign_in_as};

const INVALID: &str = "Email atau password salah.";
const CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; object-src 'none'; base-uri 'self'; form-action 'self' https://accounts.google.com https://github.com; frame-ancestors 'none'";

#[tokio::test]
async fn login_page_has_csp_header() {
    request::<App, _, _>(|request, _ctx| async move {
        let res = request.get("/login").await;
        assert_eq!(res.status_code(), 200, "{}", res.text());
        assert_eq!(res.header("content-security-policy").to_str().unwrap(), CSP);
        let html = res.text();
        assert!(html.contains("<title>Masuk · Agent Gate</title>"), "{html}");
        assert!(html.contains("action=\"/login\""));
        // Tanpa provider OAuth di config, tombol Google/GitHub tidak dirender.
        assert!(!html.contains("/auth/google"));
        assert!(!html.contains("/auth/github"));
    })
    .await;
}

#[tokio::test]
async fn unknown_email_and_wrong_password_get_same_422_message() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().create(&ctx).await;

        let unknown = request
            .post("/login")
            .form(&[
                ("email", "tidak-ada@example.test"),
                ("password", TEST_PASSWORD),
            ])
            .await;
        let wrong = request
            .post("/login")
            .form(&[
                ("email", user.email.as_str()),
                ("password", "password yang salah"),
            ])
            .await;

        for res in [&unknown, &wrong] {
            assert_eq!(res.status_code(), 422);
            let html = res.text();
            assert!(html.contains(INVALID), "{html}");
            // Password tidak pernah dirender ulang.
            assert!(!html.contains(TEST_PASSWORD));
            assert!(!html.contains("password yang salah"));
            assert!(session_cookie(res).is_none());
        }
        // Email tetap terisi.
        assert!(unknown.text().contains("value=\"tidak-ada@example.test\""));
        assert!(wrong.text().contains(&format!("value=\"{}\"", user.email)));
        // Selain email yang terisi, kedua halaman identik.
        assert_eq!(
            unknown.text().replace("tidak-ada@example.test", "EMAIL"),
            wrong.text().replace(&user.email, "EMAIL")
        );
    })
    .await;
}

#[tokio::test]
async fn disabled_user_cannot_sign_in() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().disabled().create(&ctx).await;
        let res = request
            .post("/login")
            .form(&[("email", user.email.as_str()), ("password", TEST_PASSWORD)])
            .await;
        assert_eq!(res.status_code(), 422);
        assert!(res.text().contains(INVALID));
        assert!(session_cookie(&res).is_none());
    })
    .await;
}

#[tokio::test]
async fn unverified_user_gets_verify_page_not_session() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().unverified().create(&ctx).await;
        let res = request
            .post("/login")
            .form(&[("email", user.email.as_str()), ("password", TEST_PASSWORD)])
            .await;
        assert_eq!(res.status_code(), 200);
        assert!(
            res.text().contains("Cek email untuk verifikasi"),
            "{}",
            res.text()
        );
        // Sesi hanya mengingat user untuk tombol kirim ulang verifikasi, bukan sesi login.
        let cookie = session_cookie(&res).expect("sesi menunggu verifikasi");
        let home = request.get("/").add_cookie(cookie).await;
        assert_eq!(home.status_code(), 303);
        assert_eq!(home.header("location"), "/login?return_to=%2F");
    })
    .await;
}

#[tokio::test]
async fn external_return_to_is_dropped() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().create(&ctx).await;
        for target in [
            "//evil.test/x",
            "https://evil.test/x",
            "/\\evil.test",
            "/\t/evil.test",
            "evil",
        ] {
            let res = request
                .post("/login")
                .form(&[
                    ("email", user.email.as_str()),
                    ("password", TEST_PASSWORD),
                    ("return_to", target),
                ])
                .await;
            assert_eq!(res.status_code(), 303, "{target:?}");
            assert_eq!(res.header("location"), "/", "{target:?}");
        }
        // Halaman login juga tidak menaruh return_to eksternal di form.
        let page = request.get("/login?return_to=%2F%2Fevil.test").await;
        assert!(!page.text().contains("evil.test"));
    })
    .await;
}

#[tokio::test]
async fn return_to_internal_path_is_followed() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().create(&ctx).await;
        let page = request.get("/login?return_to=%2Fprojects%3Fx%3D1").await;
        assert!(
            page.text()
                .contains("name=\"return_to\" value=\"/projects?x=1\""),
            "{}",
            page.text()
        );
        let res = request
            .post("/login")
            .form(&[
                ("email", user.email.as_str()),
                ("password", TEST_PASSWORD),
                ("return_to", "/projects?x=1"),
            ])
            .await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(res.header("location"), "/projects?x=1");
    })
    .await;
}

#[tokio::test]
async fn signed_in_user_visiting_login_is_redirected() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().create(&ctx).await;
        let cookie = sign_in_as(&request, &user).await;
        let res = request.get("/login").add_cookie(cookie).await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(res.header("location"), "/");
    })
    .await;
}

#[tokio::test]
async fn two_factor_user_gets_pending_session_not_sign_in() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().with_totp().create(&ctx).await;
        let res = request
            .post("/login")
            .form(&[("email", user.email.as_str()), ("password", TEST_PASSWORD)])
            .await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(res.header("location"), "/login/2fa");
        let pending = session_cookie(&res).unwrap();
        // Sesi pending bukan sesi login.
        let home = request.get("/").add_cookie(pending).await;
        assert_eq!(home.status_code(), 303);
    })
    .await;
}

#[tokio::test]
async fn malformed_form_is_a_400_html_page() {
    request::<App, _, _>(|request, _ctx| async move {
        let res = request.post("/login").form(&[("email", "a@b.test")]).await;
        assert_eq!(res.status_code(), 400);
        assert!(res.text().starts_with("<!doctype html>"));
    })
    .await;
}

#[tokio::test]
async fn login_rehashes_password_hashed_with_old_params() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().create(&ctx).await;
        let mut old = Settings::from_ctx(&ctx).unwrap().auth;
        old.argon2_memory_kib = 16;
        let old_hash = password::hash(&old, SecretString::from(TEST_PASSWORD))
            .await
            .unwrap();
        let mut active = user.clone().into_active_model();
        active.password_hash = Set(Some(old_hash.clone()));
        active.update(&ctx.db).await.unwrap();

        sign_in_as(&request, &user).await;

        let stored = users::Model::find_by_id(&ctx.db, user.id)
            .await
            .unwrap()
            .unwrap();
        let new_hash = stored.password_hash.unwrap();
        assert_ne!(new_hash, old_hash);
        assert!(new_hash.contains("m=8,"), "{new_hash}");
        // Rehash bukan penggantian password oleh user.
        assert_eq!(stored.password_changed_at, None);
    })
    .await;
}
