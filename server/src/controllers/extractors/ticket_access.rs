//! `TicketAccess`: tiket dari path `/w/{slug}/t/{code}` beserta project dan izin project user
//! (`ProjectAccess`). Kode dicocokkan tanpa membedakan huruf besar, termasuk alias (kode lama
//! sebelum template diganti).
//!
//! Penolakan: semua penolakan `Tenant` lebih dulu; tiket tidak ada di workspace ini, di sampah,
//! atau user tanpa akses ke project-nya → 404 yang sama (keberadaan tiket tidak bocor). GET ke
//! alias → 301 ke kode sekarang; metode lain memakai alias apa adanya.

use axum::{
    extract::{FromRef, FromRequestParts, RawPathParams},
    http::{Method, StatusCode, header, request::Parts},
    response::{IntoResponse, Response},
};
use loco_rs::{Error, app::AppContext};

use super::{ProjectAccess, Tenant};
use crate::models::{permission::Permission, project_access, projects, tickets};

#[derive(Debug, Clone)]
pub struct TicketAccess {
    pub access: ProjectAccess,
    pub ticket: tickets::Model,
}

impl TicketAccess {
    /// User memegang `permission` di project tiket ini, atau 404.
    ///
    /// # Errors
    ///
    /// [`Error::NotFound`].
    pub fn require(&self, permission: Permission) -> Result<(), Error> {
        self.access.require(permission)
    }

    #[must_use]
    pub fn can(&self, permission: Permission) -> bool {
        self.access.role.has(permission)
    }
}

impl<S> FromRequestParts<S> for TicketAccess
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
        let code = RawPathParams::from_request_parts(parts, state)
            .await
            .map_err(IntoResponse::into_response)?
            .iter()
            .find(|(name, _)| *name == "code")
            .map(|(_, value)| value.to_string());
        let Some(code) = code else {
            tracing::error!("TicketAccess extractor used on a route without {{code}}");
            return Err(StatusCode::NOT_FOUND.into_response());
        };
        let ctx = AppContext::from_ref(state);
        let not_found = || StatusCode::NOT_FOUND.into_response();
        let db_error = |e: sea_orm::DbErr| Error::from(e).into_response();
        let (ticket, current) = tickets::Model::find_by_code(&ctx.db, tenant.workspace_id(), &code)
            .await
            .map_err(db_error)?
            .filter(|(t, _)| !t.is_trashed())
            .ok_or_else(not_found)?;
        let project = projects::Model::find_in(&ctx.db, tenant.workspace_id(), ticket.project_id)
            .await
            .map_err(db_error)?
            .ok_or_else(not_found)?;
        let role = project_access::effective_role(&ctx.db, &tenant.accessor(), project.id)
            .await
            .map_err(db_error)?
            .filter(|r| r.has(Permission::ViewProject))
            .ok_or_else(not_found)?;
        if !current && parts.method == Method::GET {
            let path = parts.uri.path();
            let prefix = format!("/w/{}/t/", tenant.workspace.slug);
            let rest = path
                .strip_prefix(&prefix)
                .and_then(|r| r.split_once('/').map(|(_, tail)| format!("/{tail}")))
                .unwrap_or_default();
            let location = format!("{prefix}{}{rest}", ticket.code);
            return Err((
                StatusCode::MOVED_PERMANENTLY,
                [(header::LOCATION, location)],
            )
                .into_response());
        }
        let access = Self {
            access: ProjectAccess {
                tenant,
                project,
                role,
            },
            ticket,
        };
        parts.extensions.insert(access.clone());
        Ok(access)
    }
}
