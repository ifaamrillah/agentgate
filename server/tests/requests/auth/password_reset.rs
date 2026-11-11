use agentgate_server::{
    app::App,
    models::{
        user_tokens::{self, TokenPurpose},
        users,
    },
};
use chrono::Duration;
use loco_rs::testing::prelude::*;

use crate::support::{TEST_PASSWORD, a_user, mail_to, sent_mail, sign_in_as, token_in};

const NEW_PASSWORD: &str = "kuda baterai staples benar";
const TTL: Duration = Duration::minutes(30);

async fn reload(ctx: &loco_rs::app::AppContext, user: &users::Model) -> users::Model {
    users::Model::find_by_id(&ctx.db, user.id)
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn forgot_password_responds_same_for_unknown_email() {
    request::<App, _, _>(|request, ctx| async move {
        let page = request.get("/password/forgot").await;
        assert_eq!(page.status_code(), 200);
        assert_css_exists(
            &page.text(),
            r#"form[action="/password/forgot"][method="post"]"#,
        );

        let user = a_user().create(&ctx).await;
        let known = request
            .post("/password/forgot")
            .form(&[("email", user.email.as_str())])
            .await;
        let unknown = request
            .post("/password/forgot")
            .form(&[("email", "tidak-ada@example.test")])
            .await;
        for res in [&known, &unknown] {
            assert_eq!(res.status_code(), 303, "{}", res.text());
            assert_eq!(res.header("location"), "/check-email");
        }
        assert_eq!(known.text(), unknown.text());

        let all = sent_mail(&ctx);
        assert_eq!(all.len(), 1, "{all:?}");
        let mail = mail_to(&ctx, &user.email);
        assert!(
            token_in(&mail[0], "/password/reset").is_some(),
            "{}",
            mail[0]
        );
    })
    .await;
}

#[tokio::test]
async fn forgot_password_is_throttled() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().create(&ctx).await;
        let mut statuses = Vec::new();
        for _ in 0..6 {
            statuses.push(
                request
                    .post("/password/forgot")
                    .form(&[("email", user.email.as_str())])
                    .await
                    .status_code()
                    .as_u16(),
            );
        }
        assert_eq!(statuses, [303, 303, 303, 303, 303, 429]);
        assert_eq!(mail_to(&ctx, &user.email).len(), 5);
        // Kunci permintaan email terpisah dari kunci login: user tetap bisa masuk.
        sign_in_as(&request, &user).await;
    })
    .await;
}

#[tokio::test]
async fn reset_get_does_not_consume_token() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().create(&ctx).await;
        let raw = user_tokens::issue(&ctx.db, user.id, TokenPurpose::PasswordReset, TTL)
            .await
            .unwrap();
        for _ in 0..2 {
            let res = request.get(&format!("/password/reset?token={raw}")).await;
            assert_eq!(res.status_code(), 200, "{}", res.text());
            assert_eq!(res.header("referrer-policy"), "no-referrer");
            let html = res.text();
            assert_css_exists(&html, r#"form[action="/password/reset"][method="post"]"#);
            assert_css_exists(
                &html,
                &format!(r#"input[type="hidden"][name="token"][value="{raw}"]"#),
            );
        }
        assert!(
            user_tokens::peek(&ctx.db, &raw, TokenPurpose::PasswordReset)
                .await
                .unwrap()
        );
    })
    .await;
}

#[tokio::test]
async fn reset_revokes_other_sessions() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().create(&ctx).await;
        let other_device = sign_in_as(&request, &user).await;

        request
            .post("/password/forgot")
            .form(&[("email", user.email.as_str())])
            .await;
        let raw = token_in(&mail_to(&ctx, &user.email)[0], "/password/reset").unwrap();
        let res = request
            .post("/password/reset")
            .form(&[("token", raw.as_str()), ("password", NEW_PASSWORD)])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(res.header("location"), "/login");

        // Sesi lama berakhir.
        let home = request.get("/").add_cookie(other_device).await;
        assert_eq!(home.status_code(), 303);

        // Password lama ditolak, yang baru berlaku; password_changed_at terisi.
        let old = request
            .post("/login")
            .form(&[("email", user.email.as_str()), ("password", TEST_PASSWORD)])
            .await;
        assert_eq!(old.status_code(), 422);
        let new = request
            .post("/login")
            .form(&[("email", user.email.as_str()), ("password", NEW_PASSWORD)])
            .await;
        assert_eq!(new.status_code(), 303);
        assert!(reload(&ctx, &user).await.password_changed_at.is_some());

        // Token sekali pakai.
        let again = request
            .post("/password/reset")
            .form(&[("token", raw.as_str()), ("password", NEW_PASSWORD)])
            .await;
        assert_eq!(again.status_code(), 422);
        assert_css_exists(&again.text(), r#"[data-page="token-invalid"]"#);
    })
    .await;
}

#[tokio::test]
async fn reset_sets_email_verified_at() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().unverified().create(&ctx).await;
        let raw = user_tokens::issue(&ctx.db, user.id, TokenPurpose::PasswordReset, TTL)
            .await
            .unwrap();
        let res = request
            .post("/password/reset")
            .form(&[("token", raw.as_str()), ("password", NEW_PASSWORD)])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert!(reload(&ctx, &user).await.email_verified_at.is_some());
    })
    .await;
}

#[tokio::test]
async fn expired_reset_token_shows_invalid_page() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().create(&ctx).await;
        let raw = user_tokens::issue(
            &ctx.db,
            user.id,
            TokenPurpose::PasswordReset,
            Duration::minutes(-1),
        )
        .await
        .unwrap();

        let page = request.get(&format!("/password/reset?token={raw}")).await;
        assert_eq!(page.status_code(), 200);
        assert_css_exists(&page.text(), r#"[data-page="token-invalid"]"#);
        assert_css_not_exists(&page.text(), r#"input[name="password"]"#);

        let res = request
            .post("/password/reset")
            .form(&[("token", raw.as_str()), ("password", NEW_PASSWORD)])
            .await;
        assert_eq!(res.status_code(), 422);
        assert_css_exists(&res.text(), r#"[data-page="token-invalid"]"#);
        assert_eq!(reload(&ctx, &user).await.password_hash, user.password_hash);
    })
    .await;
}

#[tokio::test]
async fn old_reset_token_is_rejected_after_a_new_request() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().create(&ctx).await;
        for _ in 0..2 {
            request
                .post("/password/forgot")
                .form(&[("email", user.email.as_str())])
                .await;
        }
        let mail = mail_to(&ctx, &user.email);
        let old = token_in(&mail[0], "/password/reset").unwrap();
        let res = request
            .post("/password/reset")
            .form(&[("token", old.as_str()), ("password", NEW_PASSWORD)])
            .await;
        assert_eq!(res.status_code(), 422);
        assert_css_exists(&res.text(), r#"[data-page="token-invalid"]"#);
    })
    .await;
}

#[tokio::test]
async fn reset_with_short_password_keeps_the_token() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().create(&ctx).await;
        let raw = user_tokens::issue(&ctx.db, user.id, TokenPurpose::PasswordReset, TTL)
            .await
            .unwrap();
        let res = request
            .post("/password/reset")
            .form(&[("token", raw.as_str()), ("password", "pendek")])
            .await;
        assert_eq!(res.status_code(), 422);
        let html = res.text();
        assert!(html.contains("Password minimal 12 karakter."), "{html}");
        assert!(!html.contains("pendek"));
        assert_css_exists(
            &html,
            &format!(r#"input[type="hidden"][name="token"][value="{raw}"]"#),
        );
        assert!(
            user_tokens::peek(&ctx.db, &raw, TokenPurpose::PasswordReset)
                .await
                .unwrap()
        );
    })
    .await;
}
