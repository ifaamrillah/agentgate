//! Langkah kedua login (`/login/2fa`) dan pengaturan 2FA di `/account/2fa`.

use agentgate_server::{app::App, models::users, services::auth::totp};
use loco_rs::{TestServer, auth::cookies::Cookie, testing::prelude::*};

use crate::support::{TEST_PASSWORD, TEST_TOTP_SECRET, a_user, session_cookie, sign_in_as};

fn now() -> u64 {
    u64::try_from(chrono::Utc::now().timestamp()).unwrap()
}

fn current_code() -> String {
    totp::code_at(TEST_TOTP_SECRET, now()).unwrap()
}

/// Kode 6 digit yang pasti salah untuk saat ini (dan langkah tetangganya).
fn wrong_code() -> String {
    let t = now();
    let near: Vec<String> = [t - 30, t, t + 30]
        .iter()
        .map(|t| totp::code_at(TEST_TOTP_SECRET, *t).unwrap())
        .collect();
    (0..)
        .map(|n| format!("{n:06}"))
        .find(|c| !near.contains(c))
        .unwrap()
}

/// Login password untuk user ber-2FA: kembalikan cookie sesi pending.
async fn pending_login(request: &TestServer, user: &users::Model) -> Cookie<'static> {
    let res = request
        .post("/login")
        .form(&[
            ("email", user.email.as_str()),
            ("password", TEST_PASSWORD),
            ("return_to", "/account"),
        ])
        .await;
    assert_eq!(res.status_code(), 303, "{}", res.text());
    assert_eq!(res.header("location"), "/login/2fa");
    session_cookie(&res).unwrap()
}

async fn submit_code(
    request: &TestServer,
    cookie: &Cookie<'static>,
    code: &str,
) -> axum_test::TestResponse {
    request
        .post("/login/2fa")
        .add_cookie(cookie.clone())
        .form(&[("code", code)])
        .await
}

async fn reload(ctx: &loco_rs::app::AppContext, user: &users::Model) -> users::Model {
    users::Model::find_by_id(&ctx.db, user.id)
        .await
        .unwrap()
        .unwrap()
}

/// Isi elemen pertama setelah `marker` sampai `<`.
fn text_after(html: &str, marker: &str) -> Vec<String> {
    html.split(marker)
        .skip(1)
        .map(|part| {
            let start = part.find('>').unwrap() + 1;
            let end = part[start..].find('<').unwrap();
            part[start..start + end].trim().to_string()
        })
        .collect()
}

#[tokio::test]
async fn user_with_totp_needs_second_step_before_session() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().with_totp().create(&ctx).await;
        let pending = pending_login(&request, &user).await;

        let page = request.get("/login/2fa").add_cookie(pending.clone()).await;
        assert_eq!(page.status_code(), 200);
        assert_css_exists(&page.text(), r#"form[action="/login/2fa"][method="post"]"#);
        assert_css_exists(
            &page.text(),
            r#"input[name="code"][autocomplete="one-time-code"]"#,
        );

        let res = submit_code(&request, &pending, &current_code()).await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        // `return_to` dari form login dibawa melewati langkah kedua.
        assert_eq!(res.header("location"), "/account");
        let signed_in = session_cookie(&res).unwrap();
        assert_ne!(
            signed_in.value(),
            pending.value(),
            "sesi baru setelah langkah kedua"
        );
        assert_eq!(
            request
                .get("/account")
                .add_cookie(signed_in)
                .await
                .status_code(),
            200
        );
        // Sesi pending lama tidak menjadi sesi login.
        assert_eq!(
            request.get("/").add_cookie(pending).await.status_code(),
            303
        );
    })
    .await;
}

#[tokio::test]
async fn second_step_without_pending_session_goes_to_login() {
    request::<App, _, _>(|request, _ctx| async move {
        let page = request.get("/login/2fa").await;
        assert_eq!(page.status_code(), 303);
        assert_eq!(page.header("location"), "/login");
        let res = request.post("/login/2fa").form(&[("code", "123456")]).await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(res.header("location"), "/login");
    })
    .await;
}

#[tokio::test]
async fn pending_2fa_session_cannot_open_protected_pages() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().with_totp().create(&ctx).await;
        let pending = pending_login(&request, &user).await;
        for path in ["/", "/account", "/account/2fa"] {
            let res = request.get(path).add_cookie(pending.clone()).await;
            assert_eq!(res.status_code(), 303, "{path}");
            assert!(
                res.header("location")
                    .to_str()
                    .unwrap()
                    .starts_with("/login")
            );
        }
        for path in [
            "/account/2fa",
            "/account/2fa/disable",
            "/account/2fa/recovery-codes",
        ] {
            let res = request
                .post(path)
                .add_cookie(pending.clone())
                .form(&[("code", "123456"), ("current_password", TEST_PASSWORD)])
                .await;
            assert_eq!(res.status_code(), 303, "{path}");
            assert_eq!(res.header("location"), "/login", "{path}");
        }
        assert!(reload(&ctx, &user).await.totp_enabled_at.is_some());
    })
    .await;
}

#[tokio::test]
async fn wrong_totp_five_times_drops_pending_login() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().with_totp().create(&ctx).await;
        let pending = pending_login(&request, &user).await;
        let wrong = wrong_code();
        for attempt in 1..=4 {
            let res = submit_code(&request, &pending, &wrong).await;
            assert_eq!(res.status_code(), 422, "percobaan {attempt}");
            assert!(res.text().contains("Kode salah"), "{}", res.text());
        }
        let fifth = submit_code(&request, &pending, &wrong).await;
        assert_eq!(fifth.status_code(), 303);
        assert_eq!(fifth.header("location"), "/login");
        let flash_cookie = session_cookie(&fifth).unwrap();
        let login = request.get("/login").add_cookie(flash_cookie).await;
        assert!(
            login.text().contains("Terlalu banyak kode salah"),
            "{}",
            login.text()
        );

        // Sesi pending sudah dibuang: kode benar pun tidak lagi membuat sesi.
        let late = submit_code(&request, &pending, &current_code()).await;
        assert_eq!(late.status_code(), 303);
        assert_eq!(late.header("location"), "/login");

        // Login ulang dengan password tidak membuka kunci langkah kedua user ini.
        let again = pending_login(&request, &user).await;
        let res = submit_code(&request, &again, &current_code()).await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(res.header("location"), "/login");
        assert!(reload(&ctx, &user).await.totp_last_step.is_none());
    })
    .await;
}

#[tokio::test]
async fn totp_code_cannot_be_replayed_in_same_step() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().with_totp().create(&ctx).await;
        let code = current_code();
        let first = pending_login(&request, &user).await;
        assert_eq!(
            submit_code(&request, &first, &code).await.status_code(),
            303
        );

        let second = pending_login(&request, &user).await;
        let res = submit_code(&request, &second, &code).await;
        assert_eq!(res.status_code(), 422);
        // Masih sesi pending: belum login.
        assert_eq!(request.get("/").add_cookie(second).await.status_code(), 303);
    })
    .await;
}

#[tokio::test]
async fn recovery_code_works_once() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().with_totp().create(&ctx).await;
        let codes = totp::regenerate_recovery_codes(&ctx, &user)
            .await
            .unwrap()
            .unwrap();

        let first = pending_login(&request, &user).await;
        let res = submit_code(&request, &first, &codes[0]).await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(res.header("location"), "/account");

        let second = pending_login(&request, &user).await;
        let res = submit_code(&request, &second, &codes[0]).await;
        assert_eq!(res.status_code(), 422);
    })
    .await;
}

#[tokio::test]
async fn account_page_links_to_two_factor_settings() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().create(&ctx).await;
        let cookie = sign_in_as(&request, &user).await;
        let page = request.get("/account").add_cookie(cookie).await;
        assert_css_exists(&page.text(), r#"a[href="/account/2fa"]"#);
    })
    .await;
}

#[tokio::test]
async fn setup_shows_qr_and_confirms_with_a_code() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().create(&ctx).await;
        let cookie = sign_in_as(&request, &user).await;

        let page = request.get("/account/2fa").add_cookie(cookie.clone()).await;
        assert_eq!(page.status_code(), 200);
        let html = page.text();
        // QR dirender sebagai markup SVG inline (tidak di-escape), bukan teks.
        assert_css_exists(
            &html,
            r#"[role="img"] > svg[shape-rendering="crispEdges"] > path"#,
        );
        assert!(!html.contains("&lt;svg"));
        assert_css_exists(&html, r#"form[action="/account/2fa"][method="post"]"#);
        let secret = text_after(&html, r#"id="totp-secret""#).remove(0);
        assert_eq!(secret.len(), 32, "{html}");
        // Secret pending belum ada di database.
        assert!(reload(&ctx, &user).await.totp_secret_encrypted.is_none());

        // Membuka halaman lagi (mis. preload) tidak mengganti secret yang sudah dipindai.
        let again = request.get("/account/2fa").add_cookie(cookie.clone()).await;
        assert_eq!(text_after(&again.text(), r#"id="totp-secret""#)[0], secret);
        // ... dan tidak menulis sesi lagi (tidak ada Set-Cookie sesi).
        assert!(session_cookie(&again).is_none());

        let wrong = request
            .post("/account/2fa")
            .add_cookie(cookie.clone())
            .form(&[("code", "abc")])
            .await;
        assert_eq!(wrong.status_code(), 422);
        assert!(reload(&ctx, &user).await.totp_enabled_at.is_none());

        let code = totp::code_at(&secret, now()).unwrap();
        let res = request
            .post("/account/2fa")
            .add_cookie(cookie.clone())
            .form(&[("code", code.as_str())])
            .await;
        assert_eq!(res.status_code(), 200, "{}", res.text());
        assert_eq!(res.header("cache-control"), "no-store");
        let codes = text_after(&res.text(), "data-recovery-code");
        assert_eq!(codes.len(), 10, "{}", res.text());
        assert!(reload(&ctx, &user).await.totp_enabled_at.is_some());

        // Sudah aktif: halaman menampilkan status, bukan QR baru.
        let status = request.get("/account/2fa").add_cookie(cookie.clone()).await;
        assert_eq!(status.status_code(), 200);
        assert_css_not_exists(&status.text(), r#"[id="totp-secret"]"#);
        assert_css_exists(&status.text(), r#"form[action="/account/2fa/disable"]"#);

        // Kode pemulihan dari halaman setup berlaku untuk login.
        request.post("/logout").add_cookie(cookie).await;
        let pending = pending_login(&request, &user).await;
        let res = submit_code(&request, &pending, &codes[3]).await;
        assert_eq!(res.status_code(), 303);
    })
    .await;
}

#[tokio::test]
async fn disable_2fa_requires_password() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().with_totp().create(&ctx).await;
        let pending = pending_login(&request, &user).await;
        let res = submit_code(&request, &pending, &current_code()).await;
        let cookie = session_cookie(&res).unwrap();

        let wrong = request
            .post("/account/2fa/disable")
            .add_cookie(cookie.clone())
            .form(&[("current_password", "password yang salah")])
            .await;
        assert_eq!(wrong.status_code(), 422);
        assert!(wrong.text().contains("Password saat ini salah."));
        assert!(!wrong.text().contains("password yang salah"));
        assert!(reload(&ctx, &user).await.totp_enabled_at.is_some());

        let res = request
            .post("/account/2fa/disable")
            .add_cookie(cookie.clone())
            .form(&[("current_password", TEST_PASSWORD)])
            .await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(res.header("location"), "/account");
        let user = reload(&ctx, &user).await;
        assert!(user.totp_enabled_at.is_none());
        assert!(user.totp_secret_encrypted.is_none());

        // Login berikutnya tanpa langkah kedua.
        request.post("/logout").add_cookie(cookie).await;
        sign_in_as(&request, &user).await;
    })
    .await;
}

#[tokio::test]
async fn regenerate_recovery_codes_shows_new_codes_once() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().with_totp().create(&ctx).await;
        let old = totp::regenerate_recovery_codes(&ctx, &user)
            .await
            .unwrap()
            .unwrap();
        let pending = pending_login(&request, &user).await;
        let res = submit_code(&request, &pending, &current_code()).await;
        let cookie = session_cookie(&res).unwrap();

        let res = request
            .post("/account/2fa/recovery-codes")
            .add_cookie(cookie)
            .await;
        assert_eq!(res.status_code(), 200, "{}", res.text());
        assert_eq!(res.header("cache-control"), "no-store");
        let new = text_after(&res.text(), "data-recovery-code");
        assert_eq!(new.len(), 10);
        assert!(!totp::use_recovery_code(&ctx, &user, &old[0]).await.unwrap());
        assert!(totp::use_recovery_code(&ctx, &user, &new[0]).await.unwrap());
    })
    .await;
}
