use agentgate_server::{
    app::App,
    mailers::auth::{AuthMailer, TEMPLATES},
};
use loco_rs::testing::prelude::*;

use crate::support::{a_user, sent_mail};

/// Tera 2 gagal pada variabel yang tidak didefinisikan, dan untuk email kegagalan itu baru
/// muncul saat job mengirim (rules §8): setiap template dirender lewat jalur kirim aslinya.
#[tokio::test]
async fn every_auth_email_template_renders() {
    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    let user = a_user().name("Eka Surel").create(ctx).await;

    AuthMailer::send_verification(ctx, &user, "tok-verify")
        .await
        .unwrap();
    AuthMailer::send_already_registered(ctx, &user, "tok-registered")
        .await
        .unwrap();
    AuthMailer::send_password_reset(ctx, &user, "tok-reset")
        .await
        .unwrap();

    let expected = [
        "/verify-email?token=tok-verify",
        "/password/reset?token=tok-registered",
        "/password/reset?token=tok-reset",
    ];
    assert_eq!(TEMPLATES.len(), expected.len());
    let mail = sent_mail(ctx);
    assert_eq!(mail.len(), expected.len());
    for (message, link) in mail.iter().zip(expected) {
        assert!(
            message.contains(&format!("To: {}", user.email)),
            "{message}"
        );
        assert!(message.contains("Subject: "), "{message}");
        // Versi teks dan HTML sama-sama membawa link lengkap dari public_base_url.
        assert!(message.matches(link).count() >= 2, "{link}\n{message}");
        assert!(message.contains("text/plain") && message.contains("text/html"));
        assert!(message.contains("Eka Surel"), "{message}");
    }
}

/// Setiap folder template di `assets/views/mailers/auth` terdaftar (dan dengan begitu dites).
#[test]
fn every_auth_template_folder_is_listed() {
    let mut dirs: Vec<String> = std::fs::read_dir("assets/views/mailers/auth")
        .unwrap()
        .map(|e| format!("auth/{}", e.unwrap().file_name().to_string_lossy()))
        .collect();
    dirs.sort();
    let mut listed: Vec<String> = TEMPLATES.iter().map(ToString::to_string).collect();
    listed.sort();
    assert_eq!(dirs, listed);
}
