//! Email undangan ke workspace. Link dibentuk dari `settings.public_base_url`
//! (`{public_base_url}/invitations/{token}`), tidak dari header `Host` request.
//!
//! **Pengecualian rules §8 (secret di job args):** seperti email autentikasi, token mentah harus
//! sampai ke inbox, jadi link berisi token ikut di isi email yang disimpan sebagai args job
//! `MailerWorker` (queue menghapus job yang selesai). Umur token `invitation_days`, dan token
//! tidak pernah di-log: fungsi di sini tidak mencatat link maupun isi email.

use loco_rs::{Result, app::AppContext};
use serde_json::json;

use super::deliver;
use crate::common::settings::Settings;

pub const INVITE: &str = "invitation/invite";
/// Semua email modul ini (dijaga tes: setiap folder template terdaftar dan dirender).
pub const TEMPLATES: &[&str] = &[INVITE];

/// Isi email undangan selain tokennya: alamat tujuan dan teks yang ditampilkan.
#[derive(Debug, Clone)]
pub struct InvitationMail {
    pub to: String,
    pub workspace_name: String,
    pub inviter_name: String,
    pub role_name: String,
}

pub struct InvitationMailer;

impl InvitationMailer {
    /// Link undangan (`GET /invitations/{token}`), berlaku `invitation_days`.
    ///
    /// # Errors
    ///
    /// Settings, render, atau antrean job gagal.
    pub async fn send_invitation(ctx: &AppContext, mail: &InvitationMail, raw: &str) -> Result<()> {
        let settings = Settings::from_ctx(ctx)?;
        let locals = json!({
            // Penerima belum tentu punya akun: sapaan memakai alamat email-nya.
            "name": mail.to,
            "workspace_name": mail.workspace_name,
            "inviter_name": mail.inviter_name,
            "role_name": mail.role_name,
            "link": format!("{}/invitations/{raw}", settings.public_base_url),
            "days": settings.auth.invitation_days,
        });
        deliver(ctx, &settings, &mail.to, INVITE, &locals).await
    }
}
