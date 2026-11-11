use agentgate_server::{app::App, services::auth::dev_login};
use loco_rs::testing::prelude::*;

use crate::support::{a_user, request_as_development, session_cookie};

#[tokio::test]
async fn dev_login_route_is_404_in_test_env() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().create(&ctx).await;
        let secret = ctx
            .config
            .auth
            .as_ref()
            .and_then(|a| a.session.as_ref())
            .and_then(|s| s.secret.clone())
            .unwrap();
        let token = dev_login::sign(&secret, user.id, chrono::Utc::now().timestamp());
        let res = request.get(&format!("/_dev/login?token={token}")).await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}

fn token_for(ctx: &loco_rs::app::AppContext, user_id: sea_orm::prelude::Uuid, now: i64) -> String {
    let secret = ctx
        .config
        .auth
        .as_ref()
        .and_then(|a| a.session.as_ref())
        .and_then(|s| s.secret.clone())
        .unwrap();
    dev_login::sign(&secret, user_id, now)
}

#[tokio::test]
async fn valid_token_signs_in_and_opens_protected_pages() {
    request_as_development(|request, ctx| async move {
        let user = a_user().create(&ctx).await;
        let token = token_for(&ctx, user.id, chrono::Utc::now().timestamp());
        let res = request.get(&format!("/_dev/login?token={token}")).await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(res.header("location"), "/");
        let cookie = session_cookie(&res).expect("cookie sesi");
        let home = request.get("/account").add_cookie(cookie).await;
        assert_eq!(home.status_code(), 200);
    })
    .await;
}

#[tokio::test]
async fn token_of_user_with_totp_goes_to_second_factor_without_login() {
    request_as_development(|request, ctx| async move {
        let user = a_user().with_totp().create(&ctx).await;
        let token = token_for(&ctx, user.id, chrono::Utc::now().timestamp());
        let res = request.get(&format!("/_dev/login?token={token}")).await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(res.header("location"), "/login/2fa");
        let cookie = session_cookie(&res).expect("cookie sesi pending");
        // Sesi pending bukan sesi login: halaman terlindungi masih mengarah ke login.
        let home = request.get("/account").add_cookie(cookie).await;
        assert_eq!(home.status_code(), 303);
        assert!(
            home.header("location")
                .to_str()
                .unwrap()
                .starts_with("/login")
        );
    })
    .await;
}

#[tokio::test]
async fn expired_unknown_and_disabled_tokens_are_404() {
    request_as_development(|request, ctx| async move {
        let user = a_user().create(&ctx).await;
        let disabled = a_user().disabled().create(&ctx).await;
        let now = chrono::Utc::now().timestamp();
        let tokens = [
            token_for(&ctx, user.id, now - 3_600),
            token_for(&ctx, sea_orm::prelude::Uuid::now_v7(), now),
            token_for(&ctx, disabled.id, now),
            "garbage".to_string(),
        ];
        for token in tokens {
            let res = request.get(&format!("/_dev/login?token={token}")).await;
            assert_eq!(res.status_code(), 404, "{token}");
            assert!(session_cookie(&res).is_none());
        }
    })
    .await;
}
