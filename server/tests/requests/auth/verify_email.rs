use agentgate_server::{
    app::App,
    models::{
        user_tokens::{self, TokenPurpose},
        users,
    },
};
use chrono::Duration;
use loco_rs::{TestServer, auth::cookies::Cookie, testing::prelude::*};

use crate::support::{
    TEST_PASSWORD, a_user, mail_to, sent_mail, session_cookie, sign_in_as, token_in,
};

const DAY: Duration = Duration::hours(24);

#[tokio::test]
async fn verification_link_get_does_not_consume_token() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().unverified().create(&ctx).await;
        let raw = user_tokens::issue(&ctx.db, user.id, TokenPurpose::EmailVerification, DAY)
            .await
            .unwrap();

        for _ in 0..2 {
            let res = request.get(&format!("/verify-email?token={raw}")).await;
            assert_eq!(res.status_code(), 200, "{}", res.text());
            assert_eq!(res.header("referrer-policy"), "no-referrer");
            let html = res.text();
            // Halaman hanya menampilkan tombol POST yang membawa token.
            assert_css_exists(&html, r#"form[action="/verify-email"][method="post"]"#);
            assert_css_exists(
                &html,
                &format!(r#"input[type="hidden"][name="token"][value="{raw}"]"#),
            );
        }
        assert!(
            user_tokens::peek(&ctx.db, &raw, TokenPurpose::EmailVerification)
                .await
                .unwrap()
        );
        let user = users::Model::find_by_id(&ctx.db, user.id)
            .await
            .unwrap()
            .unwrap();
        assert!(user.email_verified_at.is_none());
    })
    .await;
}

#[tokio::test]
async fn verified_user_can_sign_in() {
    request::<App, _, _>(|request, ctx| async move {
        request
            .post("/register")
            .form(&[
                ("name", "Citra"),
                ("email", "citra@example.test"),
                ("password", TEST_PASSWORD),
            ])
            .await;
        let mail = mail_to(&ctx, "citra@example.test");
        let raw = token_in(&mail[0], "/verify-email").unwrap();

        let res = request
            .post("/verify-email")
            .form(&[("token", raw.as_str())])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(res.header("location"), "/login");
        // Tidak login otomatis: sesi hanya membawa flash.
        let cookie = session_cookie(&res).expect("flash disimpan di sesi");
        let home = request.get("/account").add_cookie(cookie.clone()).await;
        assert_eq!(home.status_code(), 303);

        // Flash tampil sekali di halaman login.
        let login = request.get("/login").add_cookie(cookie.clone()).await;
        assert_eq!(login.status_code(), 200);
        assert!(
            login.text().contains("Email terverifikasi. Silakan masuk."),
            "{}",
            login.text()
        );
        assert_css_exists(&login.text(), r#"[data-flash="success"]"#);
        let again = request
            .get("/login")
            .add_cookie(session_cookie(&login).unwrap_or(cookie))
            .await;
        assert!(!again.text().contains("Email terverifikasi"));

        let user = users::Model::find_by_email(&ctx.db, "citra@example.test")
            .await
            .unwrap()
            .unwrap();
        assert!(user.email_verified_at.is_some());
        sign_in_as(&request, &user).await;
    })
    .await;
}

#[tokio::test]
async fn verification_token_cannot_be_used_twice() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().unverified().create(&ctx).await;
        let raw = user_tokens::issue(&ctx.db, user.id, TokenPurpose::EmailVerification, DAY)
            .await
            .unwrap();
        let first = request
            .post("/verify-email")
            .form(&[("token", raw.as_str())])
            .await;
        assert_eq!(first.status_code(), 303);
        let second = request
            .post("/verify-email")
            .form(&[("token", raw.as_str())])
            .await;
        assert_eq!(second.status_code(), 422);
        assert_css_exists(&second.text(), r#"[data-page="token-invalid"]"#);

        let res = request.get("/verify-email?token=tidak-ada").await;
        assert_eq!(res.status_code(), 200);
        assert_css_exists(&res.text(), r#"[data-page="token-invalid"]"#);
    })
    .await;
}

/// Login dengan password benar sebagai user belum terverifikasi; kembalikan cookie sesi
/// yang mengingat user itu untuk tombol kirim ulang.
async fn pending_cookie(request: &TestServer, user: &users::Model) -> Cookie<'static> {
    let res = request
        .post("/login")
        .form(&[("email", user.email.as_str()), ("password", TEST_PASSWORD)])
        .await;
    assert_eq!(res.status_code(), 200);
    assert_css_exists(
        &res.text(),
        r#"form[action="/verify-email/resend"][method="post"]"#,
    );
    session_cookie(&res).expect("sesi menunggu verifikasi")
}

#[tokio::test]
async fn resend_from_pending_page_sends_a_new_link() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().unverified().create(&ctx).await;
        let cookie = pending_cookie(&request, &user).await;

        let res = request
            .post("/verify-email/resend")
            .add_cookie(cookie)
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(res.header("location"), "/check-email");
        let mail = mail_to(&ctx, &user.email);
        assert_eq!(mail.len(), 1);
        let raw = token_in(&mail[0], "/verify-email").unwrap();
        assert!(
            user_tokens::peek(&ctx.db, &raw, TokenPurpose::EmailVerification)
                .await
                .unwrap()
        );

        // Tanpa sesi menunggu verifikasi: tidak ada email, kembali ke login.
        let anonymous = request.post("/verify-email/resend").await;
        assert_eq!(anonymous.status_code(), 303);
        assert_eq!(anonymous.header("location"), "/login");
        assert_eq!(sent_mail(&ctx).len(), 1);
    })
    .await;
}

#[tokio::test]
async fn resend_is_throttled() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().unverified().create(&ctx).await;
        let cookie = pending_cookie(&request, &user).await;
        let mut statuses = Vec::new();
        for _ in 0..6 {
            statuses.push(
                request
                    .post("/verify-email/resend")
                    .add_cookie(cookie.clone())
                    .await
                    .status_code()
                    .as_u16(),
            );
        }
        assert_eq!(statuses, [303, 303, 303, 303, 303, 429]);
        assert_eq!(mail_to(&ctx, &user.email).len(), 5);
    })
    .await;
}
