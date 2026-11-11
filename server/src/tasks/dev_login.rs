//! `bin/loco task dev_login email:<email>`: cetak link login sekali pakai untuk development
//! (rules §7). Tidak menulis apa pun ke database. Hanya dikompilasi di build debug.
use async_trait::async_trait;
use loco_rs::{
    Error, Result,
    app::AppContext,
    environment::Environment,
    task::{Task, TaskInfo, Vars},
};
use sea_orm::ConnectionTrait;

use super::{banner, fail, ok, required, skip, summary};
use crate::{models::users, services::auth::dev_login};

pub struct DevLogin;

/// Link `<host>/_dev/login?token=...` untuk user `email` yang ada dan aktif.
///
/// # Errors
///
/// User tidak ada atau dinonaktifkan, atau error database.
pub async fn issue_link<C: ConnectionTrait>(
    db: &C,
    host: &str,
    secret: &str,
    email: &str,
    now: i64,
) -> Result<String> {
    let user = users::Model::find_by_email(db, email)
        .await?
        .ok_or_else(|| Error::string("user not found"))?;
    if !user.is_active() {
        return Err(Error::string("user is disabled"));
    }
    let token = dev_login::sign(secret, user.id, now);
    Ok(format!(
        "{}/_dev/login?token={token}",
        host.trim_end_matches('/')
    ))
}

#[async_trait]
impl Task for DevLogin {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "dev_login".to_string(),
            detail: "Cetak link login development (10 menit): dev_login email:<email>".to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, vars: &Vars) -> Result<()> {
        banner("dev_login");
        if ctx.environment != Environment::Development {
            skip("dev_login hanya berjalan di environment development");
            summary(0, 0, 1);
            return Err(Error::string(
                "dev_login only runs in the development environment",
            ));
        }
        let email = required(vars, "email")?;
        let secret = ctx
            .config
            .auth
            .as_ref()
            .and_then(|a| a.session.as_ref())
            .and_then(|s| s.secret.as_deref())
            .ok_or_else(|| Error::string("auth.session.secret is not configured"))?;
        let now = chrono::Utc::now().timestamp();
        match issue_link(&ctx.db, &ctx.config.server.host, secret, email, now).await {
            Ok(link) => {
                ok(&format!(
                    "link untuk {email} (berlaku 10 menit, jangan dibagikan)"
                ));
                println!("{link}");
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
