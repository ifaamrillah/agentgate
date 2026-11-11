use agentgate_server::app::App;
use axum_test::{TestResponse, TestServer};
use loco_rs::testing::prelude::*;

use crate::support::{TEST_PASSWORD, a_user, session_cookie};

const TOO_MANY: &str = "Terlalu banyak percobaan login";

async fn attempt(request: &TestServer, email: &str, password: &str) -> TestResponse {
    request
        .post("/login")
        .form(&[("email", email), ("password", password)])
        .await
}

#[tokio::test]
async fn email_is_locked_after_5_failures_even_if_unregistered() {
    request::<App, _, _>(|request, ctx| async move {
        for _ in 0..5 {
            let res = attempt(&request, "Hantu@Example.test", "password yang salah").await;
            assert_eq!(res.status_code(), 422);
        }
        // Email tak terdaftar terkunci, dengan pesan yang sama seperti email terdaftar.
        let locked = attempt(&request, "hantu@example.test", "password yang salah").await;
        assert_eq!(locked.status_code(), 429);
        assert!(locked.text().contains(TOO_MANY), "{}", locked.text());

        // Email terdaftar yang terkunci tidak bisa login walau password benar.
        let user = a_user().email("ana@example.test").create(&ctx).await;
        for _ in 0..5 {
            attempt(&request, &user.email, "password yang salah").await;
        }
        let res = attempt(&request, &user.email, TEST_PASSWORD).await;
        assert_eq!(res.status_code(), 429);
        assert!(session_cookie(&res).is_none());
    })
    .await;
}

#[tokio::test]
async fn successful_login_resets_email_failures() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().create(&ctx).await;
        for _ in 0..4 {
            assert_eq!(
                attempt(&request, &user.email, "password yang salah")
                    .await
                    .status_code(),
                422
            );
        }
        assert_eq!(
            attempt(&request, &user.email, TEST_PASSWORD)
                .await
                .status_code(),
            303
        );
        // Tanpa reset, kegagalan ke-5 dan seterusnya akan mengunci.
        for _ in 0..4 {
            assert_eq!(
                attempt(&request, &user.email, "password yang salah")
                    .await
                    .status_code(),
                422
            );
        }
        assert_eq!(
            attempt(&request, &user.email, TEST_PASSWORD)
                .await
                .status_code(),
            303
        );
    })
    .await;
}

#[tokio::test]
async fn ip_is_locked_after_20_failures_across_emails() {
    request::<App, _, _>(|request, _ctx| async move {
        for i in 0..20 {
            let res = attempt(
                &request,
                &format!("acak-{i}@example.test"),
                "salah salah salah",
            )
            .await;
            assert_eq!(res.status_code(), 422, "percobaan {i}");
        }
        let res = attempt(&request, "baru@example.test", "salah salah salah").await;
        assert_eq!(res.status_code(), 429);
    })
    .await;
}
