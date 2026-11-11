use agentgate_server::app::App;
use axum_test::{TestResponse, TestServer};
use loco_rs::testing::prelude::*;
use std::future::IntoFuture;

use crate::support::{TEST_PASSWORD, a_user, session_cookie};

const TOO_MANY: &str = "Terlalu banyak percobaan";

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

#[tokio::test]
async fn parallel_attempts_cannot_exceed_the_email_limit() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().create(&ctx).await;
        // Satu burst: semua request berjalan bersamaan, tidak ada yang menunggu yang lain.
        let burst = (0..12).map(|_| {
            request
                .post("/login")
                .form(&[
                    ("email", user.email.as_str()),
                    ("password", "password yang salah"),
                ])
                .into_future()
        });
        let codes: Vec<u16> = futures::future::join_all(burst)
            .await
            .iter()
            .map(|r| r.status_code().as_u16())
            .collect();
        // Hanya 5 percobaan sampai ke verifikasi password (422); sisanya ditolak sebelum itu.
        assert_eq!(codes.iter().filter(|c| **c == 422).count(), 5, "{codes:?}");
        assert_eq!(codes.iter().filter(|c| **c == 429).count(), 7, "{codes:?}");
        // Password benar pun ditolak selama terkunci.
        let res = attempt(&request, &user.email, TEST_PASSWORD).await;
        assert_eq!(res.status_code(), 429);
    })
    .await;
}

#[tokio::test]
async fn successful_logins_do_not_count_toward_the_ip_limit() {
    request::<App, _, _>(|request, ctx| async move {
        // Satu NAT kantor: banyak login berhasil dari IP yang sama tidak mengunci IP itu.
        for _ in 0..25 {
            let user = a_user().create(&ctx).await;
            let res = attempt(&request, &user.email, TEST_PASSWORD).await;
            assert_eq!(res.status_code(), 303);
        }
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_service_calls_verify_at_most_the_email_limit() {
    use agentgate_server::{
        common::settings::Settings,
        services::auth::sign_in::{self, PasswordSignIn},
    };
    use secrecy::SecretString;

    request::<App, _, _>(|_request, ctx| async move {
        let settings = Settings::from_ctx(&ctx).unwrap().auth;
        let password = SecretString::from("password yang salah");
        // Email tidak terdaftar: jalur kunci sama untuk email terdaftar dan tidak.
        let calls = (0..12u8).map(|i| {
            let ip = std::net::IpAddr::from([10, 0, 0, i]);
            sign_in::with_password(&ctx.db, &settings, "hantu@example.test", &password, ip)
        });
        let results = futures::future::join_all(calls).await;
        let rejected = results
            .iter()
            .filter(|r| matches!(r, Ok(PasswordSignIn::Rejected)))
            .count();
        let locked = results
            .iter()
            .filter(|r| matches!(r, Ok(PasswordSignIn::Locked)))
            .count();
        assert_eq!((rejected, locked), (5, 7), "{results:?}");
    })
    .await;
}
