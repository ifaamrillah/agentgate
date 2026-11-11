//! `Tenant`: workspace dari path `/w/{slug}`, di-resolve sekali per request bersama keanggotaan
//! dan role user yang login (rules "Ditunda → Multi-tenancy"). Handler di bawah `/w/{slug}`
//! memakai extractor ini (atau `Require<P>`, yang memakainya), tidak mencari workspace sendiri.
//!
//! Penolakan:
//! - belum login → redirect login (`CurrentUser`);
//! - workspace tidak ada, user bukan anggota, atau workspace dijadwalkan hapus dan user bukan
//!   Owner → 404, sama persis, supaya keberadaan workspace tidak bocor;
//! - workspace mewajibkan 2FA dan user belum mengaktifkannya → 303 ke setup 2FA. Dicek setelah
//!   keanggotaan: bukan anggota tetap 404. Pengecualian: [`MemberIgnoring2fa`] untuk keluar
//!   dari workspace.
//!
//! Slug dicocokkan tanpa membedakan huruf besar; `/w/ACME` me-resolve `acme` tanpa redirect,
//! dan link yang dibangun halaman memakai slug kanonis dari database.

use axum::{
    extract::{FromRef, FromRequestParts, RawPathParams},
    http::{StatusCode, header, request::Parts},
    response::{IntoResponse, Response},
};
use loco_rs::app::AppContext;

use super::CurrentUser;
use crate::models::{
    permission::Permission,
    project_access::Accessor,
    roles, users, workspace_members,
    workspaces::{self, WorkspaceId},
};

/// Workspace yang dibuka, keanggotaan, dan role user yang login di dalamnya.
#[derive(Debug, Clone)]
pub struct Tenant {
    pub workspace: workspaces::Model,
    pub member: workspace_members::Model,
    pub role: roles::Model,
    pub user: users::Model,
}

impl Tenant {
    #[must_use]
    pub fn workspace_id(&self) -> WorkspaceId {
        self.workspace.workspace_id()
    }

    /// Penanya aturan akses project (`project_access`): user ini dengan role workspace-nya.
    #[must_use]
    pub fn accessor(&self) -> Accessor<'_> {
        Accessor {
            workspace_id: self.workspace_id(),
            user_id: self.user.id,
            role: &self.role,
        }
    }

    /// Role user di workspace ini memegang `permission`.
    #[must_use]
    pub fn can(&self, permission: Permission) -> bool {
        self.role.has(permission)
    }
}

/// Halaman setup 2FA untuk workspace yang mewajibkannya.
#[must_use]
pub fn two_factor_required_path(slug: &str) -> String {
    format!("/account/2fa?required={slug}")
}

impl<S> FromRequestParts<S> for Tenant
where
    AppContext: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        if let Some(tenant) = parts.extensions.get::<Self>() {
            return Ok(tenant.clone());
        }
        let MemberIgnoring2fa(tenant) = MemberIgnoring2fa::from_request_parts(parts, state).await?;
        if tenant.workspace.requires_two_factor_from(&tenant.user) {
            let location = two_factor_required_path(&tenant.workspace.slug);
            return Err((StatusCode::SEE_OTHER, [(header::LOCATION, location)]).into_response());
        }
        parts.extensions.insert(tenant.clone());
        Ok(tenant)
    }
}

/// Seperti [`Tenant`] (login, keanggotaan, workspace terbuka untuk role-nya; selain itu 404),
/// tanpa redirect ke setup 2FA bila workspace mewajibkannya. Hanya untuk "Keluar dari
/// workspace": anggota tanpa 2FA tidak dipaksa mengaktifkan 2FA hanya untuk keluar. Tidak
/// disimpan di extensions request, supaya `Tenant` di request yang sama tetap memeriksa 2FA.
#[derive(Debug, Clone)]
pub struct MemberIgnoring2fa(pub Tenant);

impl<S> FromRequestParts<S> for MemberIgnoring2fa
where
    AppContext: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Response;

    /// Login, slug dari path, dan keanggotaan, tanpa aturan 2FA workspace.
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let CurrentUser(user) = CurrentUser::from_request_parts(parts, state).await?;
        let slug = RawPathParams::from_request_parts(parts, state)
            .await
            .map_err(IntoResponse::into_response)?
            .iter()
            .find(|(name, _)| *name == "slug")
            .map(|(_, value)| value.to_string());
        let Some(slug) = slug else {
            tracing::error!("Tenant extractor used on a route without {{slug}}");
            return Err(StatusCode::NOT_FOUND.into_response());
        };
        let ctx = AppContext::from_ref(state);
        resolve(&ctx, user, &slug)
            .await
            .map_err(|e| loco_rs::Error::from(e).into_response())?
            .map(Self)
            .ok_or_else(|| StatusCode::NOT_FOUND.into_response())
    }
}

/// Workspace `slug` bila `user` anggotanya dan workspace itu terbuka untuk role-nya.
async fn resolve(
    ctx: &AppContext,
    user: users::Model,
    slug: &str,
) -> Result<Option<Tenant>, sea_orm::DbErr> {
    let Some(workspace) = workspaces::Model::find_by_slug(&ctx.db, slug).await? else {
        return Ok(None);
    };
    let Some((member, role)) =
        workspace_members::Model::find_with_role(&ctx.db, workspace.workspace_id(), user.id)
            .await?
    else {
        return Ok(None);
    };
    if !workspace.is_open_to(&role) {
        return Ok(None);
    }
    Ok(Some(Tenant {
        workspace,
        member,
        role,
        user,
    }))
}
