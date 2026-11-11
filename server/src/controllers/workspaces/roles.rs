//! Pengaturan → Role (`Require<ManageRoles>`): daftar role bawaan dan custom, buat, ubah, hapus
//! role custom. Setiap handler ubah/hapus memanggil `roles::ensure_role_editable` (role bawaan
//! ditolak dengan 422 `role-builtin` di halaman daftar); role workspace lain atau id tidak valid
//! → 404 (`IdOr404` + `find_in`).
use axum::http::StatusCode;
use loco_rs::prelude::*;
use sea_orm::DbErr;

use super::roles_path;
use crate::{
    controllers::{
        extractors::{
            IdOr404, PermissionMarker, RequestId, Require, Tenant, ValidForm, require::ManageRoles,
        },
        flash,
        shell::app_shell,
    },
    models::roles::{self, RoleError},
    services::workspaces::{Actor, roles as service},
    views::{
        self,
        flash::Flash,
        roles::{RoleFormView, RoleRow},
    },
};

/// Isian form role. `permissions` berulang (satu per checkbox), jadi body dibaca sebagai pasangan
/// `(nama, nilai)`: `serde_urlencoded` tidak mendukung field `Vec`. Field lain diabaikan.
#[derive(Debug, Default)]
struct RoleForm {
    name: String,
    permissions: Vec<String>,
}

impl From<Vec<(String, String)>> for RoleForm {
    fn from(pairs: Vec<(String, String)>) -> Self {
        let mut form = Self::default();
        for (key, value) in pairs {
            match key.as_str() {
                "name" => form.name = value,
                "permissions" => form.permissions.push(value),
                _ => {}
            }
        }
        form
    }
}

fn actor(tenant: &Tenant, request_id: Option<String>) -> Actor {
    Actor {
        workspace_id: tenant.workspace_id(),
        user_id: tenant.user.id,
        request_id,
    }
}

/// Role `id` di workspace ini, atau 404.
async fn find_role(ctx: &AppContext, tenant: &Tenant, id: uuid::Uuid) -> Result<roles::Model> {
    roles::Model::find_in(&ctx.db, tenant.workspace_id(), id)
        .await?
        .ok_or(Error::NotFound)
}

async fn render_index(
    ctx: &AppContext,
    v: &TeraView,
    tenant: &Tenant,
    flash: Option<Flash>,
    status: StatusCode,
) -> Result<Response> {
    let base = roles_path(&tenant.workspace.slug);
    let rows: Vec<RoleRow> = roles::Model::in_workspace(&ctx.db, tenant.workspace_id())
        .await?
        .iter()
        .map(|role| RoleRow::new(role, &base))
        .collect();
    let shell = app_shell(ctx, &tenant.user, Some(tenant), None).await?;
    views::roles::index(v, &shell, &rows, &format!("{base}/new"), flash, status)
}

async fn render_form(
    ctx: &AppContext,
    v: &TeraView,
    tenant: &Tenant,
    form: &RoleFormView,
    action: &str,
    editing: bool,
    status: StatusCode,
) -> Result<Response> {
    let shell = app_shell(ctx, &tenant.user, Some(tenant), None).await?;
    views::roles::form(
        v,
        &shell,
        form,
        action,
        &roles_path(&tenant.workspace.slug),
        editing,
        status,
    )
}

/// Penolakan role (bawaan, masih dipakai) dirender di halaman daftar dengan 422; role yang
/// hilang di tengah jalan → 404; sisanya error biasa. `Invalid` ditangani pemanggil (form).
async fn rejected(
    ctx: &AppContext,
    v: &TeraView,
    tenant: &Tenant,
    err: RoleError,
) -> Result<Response> {
    if let Some(code) = err.code() {
        return render_index(
            ctx,
            v,
            tenant,
            Some(Flash::error(code)),
            StatusCode::UNPROCESSABLE_ENTITY,
        )
        .await;
    }
    match err {
        RoleError::Db(DbErr::RecordNotFound(_)) => Err(Error::NotFound),
        other => Err(other.into()),
    }
}

async fn redirect_with(session: Session, tenant: &Tenant, key: &str) -> Result<Response> {
    let session = flash::set(session, &Flash::success(key))?;
    Ok((
        session,
        format::redirect(&roles_path(&tenant.workspace.slug))?,
    )
        .into_response())
}

#[debug_handler]
pub(super) async fn index(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageRoles>,
    session: Session,
) -> Result<Response> {
    let (session, flash) = flash::take(session);
    let res = render_index(&ctx, &v, &access.tenant, flash, StatusCode::OK).await?;
    Ok((session, res).into_response())
}

#[debug_handler]
pub(super) async fn new(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageRoles>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let action = format!("{}/new", roles_path(&tenant.workspace.slug));
    let form = RoleFormView::new("", &[], &tenant.role, None);
    render_form(&ctx, &v, tenant, &form, &action, false, StatusCode::OK).await
}

/// `Require` sebelum `ValidForm`: tanpa izin → 404 sebelum body dibaca (rules §6).
#[debug_handler]
pub(super) async fn create(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageRoles>,
    session: Session,
    RequestId(request_id): RequestId,
    ValidForm(pairs): ValidForm<Vec<(String, String)>>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let form = RoleForm::from(pairs);
    let result = match roles::parse_permissions(&form.permissions) {
        Ok(permissions) => {
            service::create(
                &ctx.db,
                &actor(tenant, request_id),
                &tenant.role,
                &form.name,
                permissions,
            )
            .await
        }
        Err(e) => Err(e),
    };
    match result {
        Ok(_) => redirect_with(session, tenant, views::roles::FLASH_CREATED).await,
        Err(RoleError::Invalid(errors)) => {
            let action = format!("{}/new", roles_path(&tenant.workspace.slug));
            let view = RoleFormView::new(
                form.name.trim(),
                &form.permissions,
                &tenant.role,
                Some(&errors),
            );
            render_form(
                &ctx,
                &v,
                tenant,
                &view,
                &action,
                false,
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => rejected(&ctx, &v, tenant, e).await,
    }
}

#[debug_handler]
pub(super) async fn edit(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageRoles>,
    path: IdOr404<roles::Entity>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let role = find_role(&ctx, tenant, path.id).await?;
    if let Err(e) = roles::ensure_role_editable(&role) {
        return rejected(&ctx, &v, tenant, e).await;
    }
    let action = edit_path(tenant, role.id);
    let form = RoleFormView::new(&role.name, &role.permissions, &tenant.role, None);
    render_form(&ctx, &v, tenant, &form, &action, true, StatusCode::OK).await
}

#[debug_handler]
pub(super) async fn update(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageRoles>,
    path: IdOr404<roles::Entity>,
    session: Session,
    RequestId(request_id): RequestId,
    ValidForm(pairs): ValidForm<Vec<(String, String)>>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let role = find_role(&ctx, tenant, path.id).await?;
    if let Err(e) = roles::ensure_role_editable(&role) {
        return rejected(&ctx, &v, tenant, e).await;
    }
    let form = RoleForm::from(pairs);
    let result = match roles::parse_permissions(&form.permissions) {
        Ok(permissions) => {
            service::update(
                &ctx.db,
                &actor(tenant, request_id),
                &tenant.role,
                role.id,
                &form.name,
                permissions,
            )
            .await
        }
        Err(e) => Err(e),
    };
    match result {
        Ok(_) => redirect_with(session, tenant, views::roles::FLASH_UPDATED).await,
        Err(RoleError::Invalid(errors)) => {
            let view = RoleFormView::new(
                form.name.trim(),
                &form.permissions,
                &tenant.role,
                Some(&errors),
            );
            render_form(
                &ctx,
                &v,
                tenant,
                &view,
                &edit_path(tenant, role.id),
                true,
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => rejected(&ctx, &v, tenant, e).await,
    }
}

#[debug_handler]
pub(super) async fn delete(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageRoles>,
    path: IdOr404<roles::Entity>,
    session: Session,
    RequestId(request_id): RequestId,
) -> Result<Response> {
    let tenant = &access.tenant;
    let role = find_role(&ctx, tenant, path.id).await?;
    if let Err(e) = roles::ensure_role_editable(&role) {
        return rejected(&ctx, &v, tenant, e).await;
    }
    match service::delete(&ctx.db, &actor(tenant, request_id), role.id).await {
        Ok(()) => redirect_with(session, tenant, views::roles::FLASH_DELETED).await,
        Err(e) => rejected(&ctx, &v, tenant, e).await,
    }
}

fn edit_path(tenant: &Tenant, id: uuid::Uuid) -> String {
    format!("{}/{id}/edit", roles_path(&tenant.workspace.slug))
}

/// Tautan indeks Pengaturan ke halaman ini, bila role membukanya.
pub(super) fn settings_link(tenant: &Tenant) -> Option<String> {
    ManageRoles::allows(&tenant.role).then(|| roles_path(&tenant.workspace.slug))
}
