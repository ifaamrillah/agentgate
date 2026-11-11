use agentgate_server::{
    app::App,
    common::settings::Settings,
    models::users,
    services::auth::password::{self, Verify},
    tasks::user_set_password::set_password,
};
use loco_rs::testing::prelude::*;
use secrecy::SecretString;

use crate::support::{a_user, sign_in_as};

fn secret(s: &str) -> SecretString {
    SecretString::from(s.to_string())
}

#[tokio::test]
async fn set_password_hashes_it_and_signs_the_user_out_everywhere() {
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

        set_password(&ctx, &user.email, &secret("a brand new password 1"))
            .await
            .unwrap();

        let reloaded = users::Model::find_by_id(&ctx.db, user.id)
            .await
            .unwrap()
            .unwrap();
        assert!(reloaded.password_changed_at.is_some());
        let settings = Settings::from_ctx(&ctx).unwrap().auth;
        let verdict = password::verify(
            &settings,
            &secret("a brand new password 1"),
            reloaded.password_hash.as_deref().unwrap(),
        )
        .await
        .unwrap();
        assert_ne!(verdict, Verify::Mismatch);
        assert_eq!(
            request.get("/").add_cookie(cookie).await.status_code(),
            303,
            "sesi lama harus dicabut"
        );
    })
    .await;
}

#[tokio::test]
async fn set_password_rejects_short_passwords_and_unknown_users() {
    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    let user = a_user().create(ctx).await;
    assert!(
        set_password(ctx, &user.email, &secret("short"))
            .await
            .is_err()
    );
    assert!(
        set_password(ctx, "nobody@example.com", &secret("a long enough password"))
            .await
            .is_err()
    );
}
