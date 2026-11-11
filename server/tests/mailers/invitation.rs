use agentgate_server::{
    app::App,
    mailers::invitation::{InvitationMail, InvitationMailer, TEMPLATES},
};
use loco_rs::testing::prelude::*;

use crate::support::sent_mail;

/// Setiap template email undangan dirender lewat jalur kirim aslinya (rules §8).
#[tokio::test]
async fn every_invitation_email_template_renders() {
    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    InvitationMailer::send_invitation(
        ctx,
        &InvitationMail {
            to: "tamu@example.test".to_string(),
            workspace_name: "<Acme> & Co".to_string(),
            inviter_name: "Olivia".to_string(),
            role_name: "Member".to_string(),
        },
        "tok-invite",
    )
    .await
    .unwrap();

    assert_eq!(TEMPLATES.len(), 1);
    let mail = sent_mail(ctx);
    assert_eq!(mail.len(), 1);
    let message = &mail[0];
    assert!(message.contains("To: tamu@example.test"), "{message}");
    assert!(message.contains("Subject: "), "{message}");
    assert!(
        message.matches("/invitations/tok-invite").count() >= 2,
        "{message}"
    );
    assert!(message.contains("text/plain") && message.contains("text/html"));
    assert!(message.contains("Olivia") && message.contains("Member"));
    // Nama workspace di versi HTML lewat autoescape.
    let html = &message[message.find("Content-Type: text/html").unwrap()..];
    assert!(html.contains("&lt;Acme&gt; &amp; Co"), "{html}");
}

/// Setiap folder template di `assets/views/mailers/invitation` terdaftar (dan dengan begitu dites).
#[test]
fn every_invitation_template_folder_is_listed() {
    let mut dirs: Vec<String> = std::fs::read_dir("assets/views/mailers/invitation")
        .unwrap()
        .map(|e| format!("invitation/{}", e.unwrap().file_name().to_string_lossy()))
        .collect();
    dirs.sort();
    let mut listed: Vec<String> = TEMPLATES.iter().map(ToString::to_string).collect();
    listed.sort();
    assert_eq!(dirs, listed);
}
