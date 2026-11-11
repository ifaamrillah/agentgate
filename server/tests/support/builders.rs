//! Builder data tes. Default-nya user yang bisa login: email unik, password `TEST_PASSWORD`,
//! email terverifikasi, aktif, tanpa 2FA. Setiap method mengubah satu hal dari default itu.

use agentgate_server::{
    common::{crypto, settings::Settings},
    models::users::{self, NewUser},
    services::auth::password,
};
use loco_rs::app::AppContext;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, IntoActiveModel};
use secrecy::SecretString;

use super::TEST_PASSWORD;

/// Secret TOTP contoh (base32). Task 6 menggantinya dengan secret yang benar-benar dipakai.
const DUMMY_TOTP_SECRET: &[u8] = b"JBSWY3DPEHPK3PXP";

pub struct UserBuilder {
    email: Option<String>,
    name: String,
    password: Option<String>,
    verified: bool,
    disabled: bool,
    totp: bool,
}

#[must_use]
pub fn a_user() -> UserBuilder {
    UserBuilder {
        email: None,
        name: "Ana Tes".to_string(),
        password: Some(TEST_PASSWORD.to_string()),
        verified: true,
        disabled: false,
        totp: false,
    }
}

impl UserBuilder {
    #[must_use]
    pub fn email(mut self, email: &str) -> Self {
        self.email = Some(email.to_string());
        self
    }

    #[must_use]
    pub fn name(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }

    #[must_use]
    pub fn password(mut self, password: &str) -> Self {
        self.password = Some(password.to_string());
        self
    }

    /// `email_verified_at` NULL.
    #[must_use]
    pub fn unverified(mut self) -> Self {
        self.verified = false;
        self
    }

    /// `disabled_at` terisi.
    #[must_use]
    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }

    /// `totp_enabled_at` terisi dengan secret dummy terenkripsi.
    #[must_use]
    pub fn with_totp(mut self) -> Self {
        self.totp = true;
        self
    }

    pub async fn create(self, ctx: &AppContext) -> users::Model {
        let settings = Settings::from_ctx(ctx).unwrap().auth;
        let password_hash = match self.password {
            Some(p) => Some(
                password::hash(&settings, SecretString::from(p))
                    .await
                    .unwrap(),
            ),
            None => None,
        };
        let email = self
            .email
            .unwrap_or_else(|| format!("user-{}@example.test", uuid::Uuid::now_v7().simple()));
        let user = users::ActiveModel::create(
            &ctx.db,
            NewUser {
                email,
                name: self.name,
                password_hash,
            },
        )
        .await
        .unwrap();

        let now: sea_orm::prelude::DateTimeWithTimeZone = chrono::Utc::now().into();
        let mut active = user.into_active_model();
        if self.verified {
            active.email_verified_at = Set(Some(now));
        }
        if self.disabled {
            active.disabled_at = Set(Some(now));
        }
        if self.totp {
            let key = settings.encryption_key_bytes().unwrap();
            active.totp_secret_encrypted =
                Set(Some(crypto::encrypt(&key, DUMMY_TOTP_SECRET).unwrap()));
            active.totp_enabled_at = Set(Some(now));
        }
        active.update(&ctx.db).await.unwrap()
    }
}
