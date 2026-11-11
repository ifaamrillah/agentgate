//! `bin/loco task user:set_password email:<email>`: beri password ke user (mis. user seed)
//! untuk mencoba form login. Password dibaca dua kali dari prompt tanpa echo, tidak dari
//! argumen CLI (argumen tercatat di history shell dan daftar proses).
use async_trait::async_trait;
use loco_rs::{
    Error, Result,
    app::AppContext,
    auth::session_store,
    task::{Task, TaskInfo, Vars},
};
use secrecy::{ExposeSecret, SecretString};

use super::{banner, fail, ok, required, summary};
use crate::{common::settings::Settings, models::users, services::auth::password};

pub struct UserSetPassword;

/// Validasi, hash, dan simpan `password` untuk `email` (mengisi `password_changed_at`), lalu
/// cabut semua sesi user itu.
///
/// # Errors
///
/// User tidak ada, password di luar batas panjang, atau error database.
pub async fn set_password(ctx: &AppContext, email: &str, password: &SecretString) -> Result<()> {
    let user = users::Model::find_by_email(&ctx.db, email)
        .await?
        .ok_or_else(|| Error::string("user not found"))?;
    password::validate(password)?;
    let settings = Settings::from_ctx(ctx)?;
    let hash = password::hash(&settings.auth, password.clone()).await?;
    let user = user.change_password(&ctx.db, hash).await?;
    session_store::sign_out_everywhere(ctx, &user.id.to_string()).await?;
    Ok(())
}

/// Baca password dua kali dari terminal tanpa echo; keduanya harus sama.
fn prompt() -> Result<SecretString> {
    let read = |label: &str| {
        rpassword::prompt_password(label)
            .map(SecretString::from)
            .map_err(|e| Error::string(&format!("cannot read password: {e}")))
    };
    let first = read("Password baru: ")?;
    let second = read("Ulangi password: ")?;
    if first.expose_secret() != second.expose_secret() {
        return Err(Error::string("passwords do not match"));
    }
    Ok(first)
}

#[async_trait]
impl Task for UserSetPassword {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "user:set_password".to_string(),
            detail: "Set password user lewat prompt tanpa echo: user:set_password email:<email>"
                .to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, vars: &Vars) -> Result<()> {
        banner("user:set_password");
        let email = required(vars, "email")?;
        let result = match prompt() {
            Ok(password) => set_password(ctx, email, &password).await,
            Err(err) => Err(err),
        };
        match result {
            Ok(()) => {
                ok(&format!("password {email} diganti; semua sesinya dicabut"));
                summary(1, 0, 0);
                Ok(())
            }
            Err(err) => {
                fail(&format!("{email}: {err}"));
                summary(0, 1, 0);
                Err(err)
            }
        }
    }
}
