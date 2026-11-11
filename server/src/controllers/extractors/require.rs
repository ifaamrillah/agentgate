//! `Require<P>`: guard izin workspace (rules §6). Diletakkan sebelum extractor body, sehingga
//! user tanpa izin mendapat 404 sebelum body dibaca dan divalidasi (tidak pernah melihat pesan
//! validasi). Memakai `Tenant`, jadi penolakan tenant (404, 2FA wajib) berlaku lebih dulu.
//!
//! Satu marker type per izin workspace (`Require<ManageMembers>`). Izin project
//! (`AdministerProject`, `EditProject`, `ViewProject`) dicek per project lewat `ProjectAccess`
//! (Task 12), bukan lewat role workspace, jadi tidak punya marker di sini.

use std::marker::PhantomData;

use axum::{
    extract::{FromRef, FromRequestParts},
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use loco_rs::app::AppContext;

use super::Tenant;
use crate::models::permission::Permission;

/// Izin yang diwakili marker type.
pub trait PermissionMarker {
    const PERMISSION: Permission;
}

macro_rules! permission_markers {
    ($($name:ident),* $(,)?) => {
        $(
            #[doc = concat!("Marker `Require<", stringify!($name), ">`.")]
            #[derive(Debug, Clone, Copy)]
            pub struct $name;

            impl PermissionMarker for $name {
                const PERMISSION: Permission = Permission::$name;
            }
        )*
    };
}

permission_markers!(
    ManageWorkspace,
    ManageMembers,
    ManageRoles,
    ManageTeams,
    CreateProjects,
    ViewAuditLog,
);

/// Tenant yang role-nya memegang izin `P`.
#[derive(Debug, Clone)]
pub struct Require<P> {
    pub tenant: Tenant,
    _permission: PhantomData<fn() -> P>,
}

impl<S, P> FromRequestParts<S> for Require<P>
where
    AppContext: FromRef<S>,
    S: Send + Sync,
    P: PermissionMarker,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let tenant = Tenant::from_request_parts(parts, state).await?;
        if !tenant.can(P::PERMISSION) {
            return Err(StatusCode::NOT_FOUND.into_response());
        }
        Ok(Self {
            tenant,
            _permission: PhantomData,
        })
    }
}
