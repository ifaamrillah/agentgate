//! `Require<P>`: guard izin workspace (rules §6). Diletakkan sebelum extractor body, sehingga
//! user tanpa izin mendapat 404 sebelum body dibaca dan divalidasi (tidak pernah melihat pesan
//! validasi). Memakai `Tenant`, jadi penolakan tenant (404, 2FA wajib) berlaku lebih dulu.
//!
//! Satu marker type per izin workspace (`Require<ManageMembers>`), ditambah
//! [`AnySettings`] (salah satu izin yang punya halaman Pengaturan, untuk indeks Pengaturan),
//! [`ManageJobTitles`] (jabatan tim, aturan `ManageMembers`), dan [`WorkspaceOwner`] (role
//! bawaan Owner).
//! Izin project
//! (`AdministerProject`, `EditProject`, `ViewProject`) dicek per project lewat `ProjectAccess`
//! dan `ProjectAdmin`, bukan lewat role workspace, jadi tidak punya marker di sini.

use std::marker::PhantomData;

use axum::{
    extract::{FromRef, FromRequestParts},
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use loco_rs::app::AppContext;

use super::Tenant;
use crate::models::{permission::Permission, roles};

/// Aturan izin yang diwakili marker type; jawabannya selalu dari model role.
pub trait PermissionMarker {
    fn allows(role: &roles::Model) -> bool;
}

macro_rules! permission_markers {
    ($($name:ident),* $(,)?) => {
        $(
            #[doc = concat!("Marker `Require<", stringify!($name), ">`.")]
            #[derive(Debug, Clone, Copy)]
            pub struct $name;

            impl PermissionMarker for $name {
                fn allows(role: &roles::Model) -> bool {
                    role.has(Permission::$name)
                }
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
    ManageTicketSettings,
);

/// Marker `Require<AnySettings>`: role membuka setidaknya satu halaman Pengaturan
/// (`roles::Model::has_settings_access`, predikat yang sama dengan tautan Pengaturan di sidebar).
#[derive(Debug, Clone, Copy)]
pub struct AnySettings;

impl PermissionMarker for AnySettings {
    fn allows(role: &roles::Model) -> bool {
        role.has_settings_access()
    }
}

/// Marker `Require<WorkspaceOwner>`: role bawaan Owner, bukan sekadar pemegang
/// `ManageWorkspace` (hapus workspace dan pembatalannya, serta wajibkan 2FA: spec menyebut
/// Owner).
#[derive(Debug, Clone, Copy)]
pub struct WorkspaceOwner;

impl PermissionMarker for WorkspaceOwner {
    fn allows(role: &roles::Model) -> bool {
        role.is_owner()
    }
}

/// Marker `Require<ManageJobTitles>`: jabatan adalah atribut member, jadi dikelola pemegang
/// `ManageMembers`.
#[derive(Debug, Clone, Copy)]
pub struct ManageJobTitles;

impl PermissionMarker for ManageJobTitles {
    fn allows(role: &roles::Model) -> bool {
        role.has(Permission::ManageMembers)
    }
}

/// Tenant yang role-nya memenuhi aturan `P`.
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
        if !P::allows(&tenant.role) {
            return Err(StatusCode::NOT_FOUND.into_response());
        }
        Ok(Self {
            tenant,
            _permission: PhantomData,
        })
    }
}
