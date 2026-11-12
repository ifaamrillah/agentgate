//! Builder data tes. Default-nya user yang bisa login: email unik, password `TEST_PASSWORD`,
//! email terverifikasi, aktif, tanpa 2FA. Setiap method mengubah satu hal dari default itu.
//! Workspace dibangun lewat [`a_workspace`] dan anggotanya lewat [`add_member`].

use agentgate_server::{
    common::{crypto, settings::Settings},
    models::{
        permission::Permission,
        project_grants::{self, GrantSubject},
        projects::{self, NewProject},
        roles::{self, RoleKey},
        team_members, teams,
        users::{self, NewUser},
        workspace_members, workspaces,
    },
    services::{
        auth::password,
        workspaces::create::{self, NewWorkspace},
    },
};
use loco_rs::app::AppContext;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, IntoActiveModel};
use secrecy::SecretString;

use super::{TEST_PASSWORD, TEST_TOTP_SECRET};

pub struct UserBuilder {
    email: Option<String>,
    name: String,
    password: Option<String>,
    verified: bool,
    disabled: bool,
    /// Secret TOTP (base32) bila 2FA aktif.
    totp: Option<String>,
}

#[must_use]
pub fn a_user() -> UserBuilder {
    UserBuilder {
        email: None,
        name: "Ana Tes".to_string(),
        password: Some(TEST_PASSWORD.to_string()),
        verified: true,
        disabled: false,
        totp: None,
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

    /// 2FA aktif dengan secret [`TEST_TOTP_SECRET`]: tes menghitung kode yang benar lewat
    /// `totp::code_at(TEST_TOTP_SECRET, waktu)`.
    #[must_use]
    pub fn with_totp(self) -> Self {
        self.with_totp_secret(TEST_TOTP_SECRET)
    }

    /// 2FA aktif dengan secret base32 `secret`, disimpan terenkripsi seperti hasil setup.
    #[must_use]
    pub fn with_totp_secret(mut self, secret: &str) -> Self {
        self.totp = Some(secret.to_string());
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
        if let Some(secret) = self.totp {
            let key = settings.encryption_key_bytes().unwrap();
            active.totp_secret_encrypted =
                Set(Some(crypto::encrypt(&key, secret.as_bytes()).unwrap()));
            active.totp_enabled_at = Set(Some(now));
        }
        active.update(&ctx.db).await.unwrap()
    }
}

/// Workspace baru milik `owner`, dibuat lewat jalur produksi (`create::run`: workspace, empat
/// role bawaan, `owner` sebagai Owner, audit). Slug default dari nama.
pub struct WorkspaceBuilder<'a> {
    owner: &'a users::Model,
    name: String,
    slug: String,
}

#[must_use]
pub fn a_workspace(owner: &users::Model) -> WorkspaceBuilder<'_> {
    let suffix = uuid::Uuid::now_v7().simple().to_string();
    WorkspaceBuilder {
        owner,
        name: "Acme".to_string(),
        slug: format!("ws-{}", &suffix[suffix.len() - 12..]),
    }
}

impl WorkspaceBuilder<'_> {
    #[must_use]
    pub fn name(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }

    #[must_use]
    pub fn slug(mut self, slug: &str) -> Self {
        self.slug = slug.to_string();
        self
    }

    pub async fn create(self, ctx: &AppContext) -> workspaces::Model {
        create::run(
            ctx,
            self.owner,
            NewWorkspace {
                name: self.name,
                slug: self.slug,
            },
            None,
        )
        .await
        .unwrap()
    }
}

/// Jadikan `user` anggota `workspace` dengan role bawaan `key`.
pub async fn add_member(
    ctx: &AppContext,
    workspace: &workspaces::Model,
    user: &users::Model,
    key: RoleKey,
) -> workspace_members::Model {
    let wid = workspace.workspace_id();
    let role = roles::Model::find_builtin(&ctx.db, wid, key)
        .await
        .unwrap()
        .unwrap();
    workspace_members::ActiveModel::add(&ctx.db, wid, user.id, role.id)
        .await
        .unwrap()
}

/// Role bawaan `key` di `workspace`.
pub async fn builtin_role(
    ctx: &AppContext,
    workspace: &workspaces::Model,
    key: RoleKey,
) -> roles::Model {
    roles::Model::find_builtin(&ctx.db, workspace.workspace_id(), key)
        .await
        .unwrap()
        .unwrap()
}

/// Project `key` di `workspace`, lewat model (tanpa grant pembuat dan audit: tes yang
/// membutuhkan grant memberikannya sendiri lewat [`grant`]).
pub async fn a_project(
    ctx: &AppContext,
    workspace: &workspaces::Model,
    key: &str,
) -> projects::Model {
    projects::ActiveModel::create(
        &ctx.db,
        workspace.workspace_id(),
        NewProject {
            key: key.to_string(),
            name: format!("Project {key}"),
            description: String::new(),
            color: String::new(),
            created_by: None,
        },
    )
    .await
    .unwrap()
}

/// Tim `name` di `workspace` dengan anggota `members` (harus sudah anggota workspace).
pub async fn a_team(
    ctx: &AppContext,
    workspace: &workspaces::Model,
    name: &str,
    members: &[&users::Model],
) -> teams::Model {
    let wid = workspace.workspace_id();
    let team = teams::ActiveModel::create(&ctx.db, wid, name)
        .await
        .unwrap();
    for user in members {
        assert!(
            team_members::ActiveModel::add(&ctx.db, wid, team.id, user.id)
                .await
                .unwrap()
        );
    }
    team
}

/// Beri `subject` akses ke `project` dengan `role`.
pub async fn grant(
    ctx: &AppContext,
    workspace: &workspaces::Model,
    project: &projects::Model,
    subject: GrantSubject,
    role: &roles::Model,
) -> project_grants::Model {
    let wid = workspace.workspace_id();
    project_grants::ActiveModel::grant(&ctx.db, wid, project.id, subject, role.id)
        .await
        .unwrap()
}

/// Role custom `name` dengan izin `permissions` di `workspace`.
pub async fn custom_role(
    ctx: &AppContext,
    workspace: &workspaces::Model,
    name: &str,
    permissions: &[Permission],
) -> roles::Model {
    roles::ActiveModel::create_custom(
        &ctx.db,
        workspace.workspace_id(),
        name,
        permissions.to_vec(),
    )
    .await
    .unwrap()
}
