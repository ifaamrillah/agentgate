use agentgate_server::{app::App, models::users};
use loco_rs::testing::prelude::*;

use crate::support::{
    TEST_PASSWORD, a_user, mail_to, request_with_settings, sent_mail, sign_in_as, token_in,
};

fn form<'a>(name: &'a str, email: &'a str, password: &'a str) -> [(&'a str, &'a str); 3] {
    [("name", name), ("email", email), ("password", password)]
}

#[tokio::test]
async fn register_page_renders_form() {
    request::<App, _, _>(|request, _ctx| async move {
        let res = request.get("/register").await;
        assert_eq!(res.status_code(), 200, "{}", res.text());
        assert_css_exists(&res.text(), r#"form[action="/register"][method="post"]"#);
        assert_css_exists(&res.text(), r#"input[name="name"]"#);
        assert_css_exists(&res.text(), r#"input[name="email"][type="email"]"#);
        assert_css_exists(&res.text(), r#"input[name="password"][type="password"]"#);
    })
    .await;
}

#[tokio::test]
async fn register_sends_verification_email() {
    request::<App, _, _>(|request, ctx| async move {
        let res = request
            .post("/register")
            .form(&form("Budi Baru", "  Budi@Example.test ", TEST_PASSWORD))
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(res.header("location"), "/check-email");

        let user = users::Model::find_by_email(&ctx.db, "budi@example.test")
            .await
            .unwrap()
            .expect("user dibuat");
        assert_eq!(user.name, "Budi Baru");
        assert!(user.email_verified_at.is_none());
        assert!(user.password_hash.is_some());

        let mail = mail_to(&ctx, "budi@example.test");
        assert_eq!(mail.len(), 1, "{:?}", sent_mail(&ctx));
        assert!(token_in(&mail[0], "/verify-email").is_some(), "{}", mail[0]);
        // Link dibentuk dari settings.public_base_url, bukan header Host.
        assert!(mail[0].contains("http://localhost:"), "{}", mail[0]);

        // Belum terverifikasi: password benar tidak menghasilkan sesi login.
        let login = request
            .post("/login")
            .form(&[("email", "budi@example.test"), ("password", TEST_PASSWORD)])
            .await;
        assert_eq!(login.status_code(), 200);
        assert!(login.text().contains("Cek email untuk verifikasi"));
    })
    .await;
}

#[tokio::test]
async fn register_with_existing_email_responds_same_as_new() {
    request::<App, _, _>(|request, ctx| async move {
        let existing = a_user().email("ada@example.test").create(&ctx).await;

        let fresh = request
            .post("/register")
            .form(&form("Orang Baru", "baru@example.test", TEST_PASSWORD))
            .await;
        let taken = request
            .post("/register")
            .form(&form(
                "Penyerang",
                "ADA@example.test",
                "password lain yang panjang",
            ))
            .await;

        // Status, tujuan redirect, dan isi respons sama persis.
        for res in [&fresh, &taken] {
            assert_eq!(res.status_code(), 303, "{}", res.text());
            assert_eq!(res.header("location"), "/check-email");
        }
        assert_eq!(fresh.text(), taken.text());

        let page = request.get("/check-email").await;
        assert_eq!(page.status_code(), 200);
        assert_css_exists(&page.text(), r#"[data-page="check-email"]"#);

        // Email terdaftar: satu email pemberitahuan berisi link reset, tanpa link verifikasi.
        let notice = mail_to(&ctx, "ada@example.test");
        assert_eq!(notice.len(), 1, "{:?}", sent_mail(&ctx));
        assert!(
            token_in(&notice[0], "/password/reset").is_some(),
            "{}",
            notice[0]
        );
        assert!(token_in(&notice[0], "/verify-email").is_none());
        assert_eq!(mail_to(&ctx, "baru@example.test").len(), 1);

        // Akun lama tidak berubah: nama dan password tetap.
        let still = users::Model::find_by_id(&ctx.db, existing.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(still.name, existing.name);
        assert_eq!(still.password_hash, existing.password_hash);
        sign_in_as(&request, &still).await;
    })
    .await;
}

#[tokio::test]
async fn register_with_invalid_input_rerenders_form_with_422() {
    request::<App, _, _>(|request, ctx| async move {
        let res = request
            .post("/register")
            .form(&form("Ana", "bukan-email", "pendek"))
            .await;
        assert_eq!(res.status_code(), 422);
        let html = res.text();
        assert!(html.contains("Alamat email tidak valid."), "{html}");
        assert!(html.contains("Password minimal 12 karakter."), "{html}");
        // Nilai terisi dipertahankan, password tidak pernah dirender ulang.
        assert_css_exists(&res.text(), r#"input[name="name"][value="Ana"]"#);
        assert_css_exists(&res.text(), r#"input[name="email"][value="bukan-email"]"#);
        assert!(!html.contains("pendek"));
        assert!(sent_mail(&ctx).is_empty());
    })
    .await;
}

#[tokio::test]
async fn registration_is_throttled_per_email() {
    request::<App, _, _>(|request, ctx| async move {
        let mut last = 0;
        for _ in 0..6 {
            last = request
                .post("/register")
                .form(&form("Ana", "banjir@example.test", TEST_PASSWORD))
                .await
                .status_code()
                .as_u16();
        }
        assert_eq!(last, 429);
        // Hanya 5 permintaan yang diproses (satu buat akun + empat pemberitahuan).
        assert_eq!(mail_to(&ctx, "banjir@example.test").len(), 5);
    })
    .await;
}

#[tokio::test]
async fn registration_closed_hides_form_returns_404() {
    request_with_settings(
        |s| s["auth"]["registration_open"] = serde_json::Value::Bool(false),
        |request, ctx| async move {
            assert_eq!(request.get("/register").await.status_code(), 404);
            let res = request
                .post("/register")
                .form(&form("Ana", "tutup@example.test", TEST_PASSWORD))
                .await;
            assert_eq!(res.status_code(), 404);
            assert!(
                users::Model::find_by_email(&ctx.db, "tutup@example.test")
                    .await
                    .unwrap()
                    .is_none()
            );
            assert!(sent_mail(&ctx).is_empty());
            // Link "Daftar" di halaman login ikut hilang.
            let login = request.get("/login").await;
            assert_css_not_exists(&login.text(), r#"a[href="/register"]"#);
        },
    )
    .await;
}
