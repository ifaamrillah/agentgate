//! `ProjectAccess`: project dari path `/w/{slug}/p/{key}` beserta izin project efektif user
//! (`project_access::effective_role`, satu aturan dengan daftar project di sidebar). Handler di
//! bawah `/w/{slug}/p/{key}` memakai extractor ini (atau [`ProjectAdmin`]), tidak mencari project
//! sendiri.
//!
//! Penolakan: semua penolakan `Tenant` lebih dulu (login, 404 bukan anggota, 2FA wajib); lalu
//! project tidak ada di workspace ini atau user tanpa akses → 404, sama persis, supaya
//! keberadaan project tidak bocor. Key dicocokkan tanpa membedakan huruf besar.
//!
//! [`ProjectAdmin`] adalah guard `AdministerProject` untuk halaman dan POST pengaturan project,
//! diletakkan sebelum extractor body seperti `Require<P>` (rules §6): tanpa izin → 404 sebelum
//! body dibaca.

use axum::{
    extract::{FromRef, FromRequestParts, RawPathParams},
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use loco_rs::{Error, app::AppContext};

use super::Tenant;
use crate::models::{
    permission::Permission,
    project_access::{self, EffectiveRole},
    projects,
};

/// Project yang dibuka, workspace-nya (`tenant`), dan izin project user di dalamnya.
#[derive(Debug, Clone)]
pub struct ProjectAccess {
    pub tenant: Tenant,
    pub project: projects::Model,
    pub role: EffectiveRole,
}

impl ProjectAccess {
    /// User memegang `permission` di project ini, atau 404 (bukan 403: rules §6).
    ///
    /// # Errors
    ///
    /// [`Error::NotFound`] bila izin tidak dipegang.
    pub fn require(&self, permission: Permission) -> Result<(), Error> {
        if self.role.has(permission) {
            Ok(())
        } else {
            Err(Error::NotFound)
        }
    }
}

impl<S> FromRequestParts<S> for ProjectAccess
where
    AppContext: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        if let Some(access) = parts.extensions.get::<Self>() {
            return Ok(access.clone());
        }
        let tenant = Tenant::from_request_parts(parts, state).await?;
        let key = RawPathParams::from_request_parts(parts, state)
            .await
            .map_err(IntoResponse::into_response)?
            .iter()
            .find(|(name, _)| *name == "key")
            .map(|(_, value)| value.to_string());
        let Some(key) = key else {
            tracing::error!("ProjectAccess extractor used on a route without {{key}}");
            return Err(StatusCode::NOT_FOUND.into_response());
        };
        let ctx = AppContext::from_ref(state);
        let access = resolve(&ctx, tenant, &key)
            .await
            .map_err(|e| Error::from(e).into_response())?
            .ok_or_else(|| StatusCode::NOT_FOUND.into_response())?;
        parts.extensions.insert(access.clone());
        Ok(access)
    }
}

async fn resolve(
    ctx: &AppContext,
    tenant: Tenant,
    key: &str,
) -> Result<Option<ProjectAccess>, sea_orm::DbErr> {
    let Some(project) = projects::Model::find_by_key(&ctx.db, tenant.workspace_id(), key).await?
    else {
        return Ok(None);
    };
    let role = project_access::effective_role(&ctx.db, &tenant.accessor(), project.id).await?;
    Ok(role.map(|role| ProjectAccess {
        tenant,
        project,
        role,
    }))
}

/// `ProjectAccess` dengan `AdministerProject` (pengaturan, akses, repo project).
#[derive(Debug, Clone)]
pub struct ProjectAdmin(pub ProjectAccess);

impl<S> FromRequestParts<S> for ProjectAdmin
where
    AppContext: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let access = ProjectAccess::from_request_parts(parts, state).await?;
        access
            .require(Permission::AdministerProject)
            .map_err(IntoResponse::into_response)?;
        Ok(Self(access))
    }
}
