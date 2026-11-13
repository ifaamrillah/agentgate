//! Helper bersama tes (rules §9): builder data dan helper login ditambahkan di sini, bukan
//! disalin per file tes. Builder datang bersama model-nya (M1 Task 3 dan seterusnya).
#![allow(dead_code)] // helper dipakai sebagian tes saja; yang belum dipakai tidak boleh memicu warning.

mod audit;
mod builders;
pub mod fake_codeload;
pub mod fake_github;
pub mod fake_hook;
pub mod fake_jira;
pub mod fake_oauth;
mod mail;
pub mod oauth_flow;
mod session;

pub use audit::newest_audit_events;
pub use builders::{
    a_project, a_team, a_user, a_workspace, add_member, builtin_role, custom_role, grant,
};
pub use mail::{invitation_token_in, mail_to, sent_mail, token_in};
pub use session::{
    request_as_development, request_with_config, request_with_settings, session_cookie, sign_in_as,
};

/// Password tes yang memenuhi aturan panjang (12–128 karakter). Satu nilai untuk semua tes
/// supaya `sign_in_as` dan builder user tidak berbeda pendapat.
pub const TEST_PASSWORD: &str = "correct horse battery staple";

/// Secret TOTP (base32, 160 bit) untuk `a_user().with_totp()`. Tes menghitung kodenya lewat
/// `totp::code_at`.
pub const TEST_TOTP_SECRET: &str = "JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP";

use agentgate_server::models::users::{self, NewUser};
use sea_orm::DatabaseConnection;

/// User tanpa password dengan email tertentu, untuk tes model yang hanya butuh baris `users`.
/// Tes yang butuh user siap login memakai [`a_user`].
pub async fn insert_user(db: &DatabaseConnection, email: &str) -> users::Model {
    users::ActiveModel::create(
        db,
        NewUser {
            email: email.to_string(),
            name: "Ana Tes".to_string(),
            password_hash: None,
        },
    )
    .await
    .unwrap()
}
