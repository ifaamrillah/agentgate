//! `bin/loco task user:create email:<email> name:<nama>`: user terverifikasi tanpa password,
//! lewat fungsi model yang sama dengan aplikasi. Password diberi lewat `user:set_password`.
use async_trait::async_trait;
use loco_rs::{
    Result,
    app::AppContext,
    task::{Task, TaskInfo, Vars},
};
use sea_orm::DatabaseConnection;

use super::{banner, fail, ok, required, summary};
use crate::models::users::{self, NewUser};

pub struct UserCreate;

/// Buat user terverifikasi tanpa password.
///
/// # Errors
///
/// Email/nama tidak valid, email sudah dipakai, atau error database.
pub async fn create_verified(db: &DatabaseConnection, email: &str, name: &str) -> Result<()> {
    let user = users::ActiveModel::create(
        db,
        NewUser {
            email: email.to_string(),
            name: name.to_string(),
            password_hash: None,
        },
    )
    .await?;
    user.mark_email_verified(db).await?;
    Ok(())
}

#[async_trait]
impl Task for UserCreate {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "user:create".to_string(),
            detail: "Buat user terverifikasi tanpa password: user:create email:<email> name:<nama>"
                .to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, vars: &Vars) -> Result<()> {
        banner("user:create");
        let (email, name) = (required(vars, "email")?, required(vars, "name")?);
        match create_verified(&ctx.db, email, name).await {
            Ok(()) => {
                ok(&format!("{email} dibuat (terverifikasi, tanpa password)"));
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
