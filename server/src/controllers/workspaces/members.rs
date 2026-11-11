//! Pengaturan → Anggota (`Require<ManageMembers>`): tabel anggota dengan ganti role dan jabatan,
//! keluarkan anggota (konfirmasi lewat halaman biasa), undangan tertunda dan form undang
//! (handler undang di `invitations.rs`). Juga "Keluar dari workspace" (`/w/{slug}/leave`,
//! semua anggota, `Tenant`).
//!
//! Penolakan aturan (`member-role-not-assignable`, `workspace-last-owner`) → 422 halaman anggota
//! dengan pesan error; member, role, atau jabatan workspace lain dan id tidak valid → 404.
use axum::http::StatusCode;
use loco_rs::prelude::*;
use serde::Deserialize;
use uuid::Uuid;

use super::members_path;
use crate::{
    controllers::{
        extractors::{
            IdOr404, PermissionMarker, RequestId, Require, Tenant, ValidForm,
            require::ManageMembers,
        },
        flash,
        shell::app_shell,
    },
    models::{invitations, job_titles, roles, users, workspace_members},
    services::workspaces::{
        Acting,
        membership::{self, MembershipError},
    },
    views::{
        self,
        flash::Flash,
        members::{InvitationRow, InviteFormView, MemberRow, MembersPage, OptionView},
    },
};

#[derive(Debug, Deserialize)]
pub struct RoleForm {
    #[serde(default)]
    role_id: String,
}

#[derive(Debug, Deserialize)]
pub struct JobTitleForm {
    #[serde(default)]
    job_title_id: String,
}

pub(super) fn acting(tenant: &Tenant, request_id: Option<String>) -> Acting<'_> {
    Acting {
        workspace: &tenant.workspace,
        user: &tenant.user,
        role: &tenant.role,
        request_id,
    }
}

/// UUID dari isian form; nilai lain (form diubah di luar browser) → 404, seperti id path.
pub(super) fn form_id(raw: &str) -> Result<Uuid> {
    Uuid::try_parse(raw.trim()).map_err(|_| Error::NotFound)
}

/// Isian jabatan: kosong = tanpa jabatan.
pub(super) fn optional_form_id(raw: &str) -> Result<Option<Uuid>> {
    if raw.trim().is_empty() {
        Ok(None)
    } else {
        form_id(raw).map(Some)
    }
}

/// Halaman anggota dengan isian form undang `invite` dan pesan `flash`.
pub(super) async fn render_index(
    ctx: &AppContext,
    v: &TeraView,
    tenant: &Tenant,
    invite: InviteFormView,
    flash: Option<Flash>,
    status: StatusCode,
) -> Result<Response> {
    let wid = tenant.workspace_id();
    let base = members_path(&tenant.workspace.slug);
    let invite_action = super::invitations_path(&tenant.workspace.slug);
    let roles = roles::Model::in_workspace(&ctx.db, wid).await?;
    let titles = job_titles::Model::in_workspace(&ctx.db, wid).await?;
    let members = workspace_members::Model::list_in(&ctx.db, wid)
        .await?
        .iter()
        .map(|(member, user, role)| {
            let title = member
                .job_title_id
                .and_then(|id| titles.iter().find(|t| t.id == id));
            let manageable = roles::can_assign(&tenant.role, role);
            MemberRow::new(member, user, role, title, manageable, &base)
        })
        .collect();
    let invitations = invitations::Model::pending_in(&ctx.db, wid)
        .await?
        .iter()
        .map(|invitation| {
            let role = roles.iter().find(|r| r.id == invitation.role_id);
            let role_name = role.map_or("", |r| r.name.as_str());
            let manageable = role.is_some_and(|r| roles::can_assign(&tenant.role, r));
            InvitationRow::new(invitation, role_name, manageable, &invite_action)
        })
        .collect();
    let page = MembersPage {
        members,
        invitations,
        roles: roles
            .iter()
            .filter(|role| roles::can_assign(&tenant.role, role))
            .map(OptionView::role)
            .collect(),
        job_titles: titles.iter().map(OptionView::job_title).collect(),
        invite,
        invite_action,
        flash,
    };
    let shell = app_shell(ctx, &tenant.user, Some(tenant)).await?;
    views::members::index(v, &shell, &page, status)
}

/// Form undang kosong dengan role awal Member (bila boleh diberikan).
pub(super) async fn empty_invite(ctx: &AppContext, tenant: &Tenant) -> Result<InviteFormView> {
    let member = roles::Model::find_builtin(&ctx.db, tenant.workspace_id(), roles::RoleKey::Member)
        .await?
        .filter(|role| roles::can_assign(&tenant.role, role))
        .map(|role| role.id.to_string())
        .unwrap_or_default();
    Ok(InviteFormView::new("", &member, "", None))
}

pub(super) async fn redirect_with(
    session: Session,
    tenant: &Tenant,
    key: &str,
) -> Result<Response> {
    let session = flash::set(session, &Flash::success(key))?;
    Ok((
        session,
        format::redirect(&members_path(&tenant.workspace.slug))?,
    )
        .into_response())
}

/// Penolakan aturan → 422 halaman anggota dengan pesannya; lainnya error biasa (404, 500).
async fn rejected(
    ctx: &AppContext,
    v: &TeraView,
    tenant: &Tenant,
    err: MembershipError,
) -> Result<Response> {
    let Some(code) = err.code() else {
        return Err(err.into());
    };
    let invite = empty_invite(ctx, tenant).await?;
    render_index(
        ctx,
        v,
        tenant,
        invite,
        Some(Flash::error(code)),
        StatusCode::UNPROCESSABLE_ENTITY,
    )
    .await
}

#[debug_handler]
pub(super) async fn index(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageMembers>,
    session: Session,
) -> Result<Response> {
    let tenant = &access.tenant;
    let (session, flash) = flash::take(session);
    let invite = empty_invite(&ctx, tenant).await?;
    let res = render_index(&ctx, &v, tenant, invite, flash, StatusCode::OK).await?;
    Ok((session, res).into_response())
}

/// `Require` sebelum `ValidForm`: tanpa izin → 404 sebelum body dibaca (rules §6).
#[debug_handler]
pub(super) async fn change_role(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageMembers>,
    path: IdOr404<workspace_members::Entity>,
    session: Session,
    RequestId(request_id): RequestId,
    ValidForm(form): ValidForm<RoleForm>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let role_id = form_id(&form.role_id)?;
    match membership::change_role(&ctx, &acting(tenant, request_id), path.id, role_id).await {
        Ok(_) => redirect_with(session, tenant, views::members::FLASH_ROLE_CHANGED).await,
        Err(e) => rejected(&ctx, &v, tenant, e).await,
    }
}

#[debug_handler]
pub(super) async fn set_job_title(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageMembers>,
    path: IdOr404<workspace_members::Entity>,
    session: Session,
    RequestId(request_id): RequestId,
    ValidForm(form): ValidForm<JobTitleForm>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let job_title_id = optional_form_id(&form.job_title_id)?;
    match membership::set_job_title(&ctx, &acting(tenant, request_id), path.id, job_title_id).await
    {
        Ok(_) => redirect_with(session, tenant, views::members::FLASH_JOB_TITLE_CHANGED).await,
        Err(e) => rejected(&ctx, &v, tenant, e).await,
    }
}

/// Halaman konfirmasi keluarkan anggota (tanpa JS). Member yang tidak boleh dikeluarkan pelaku
/// tetap ditolak oleh POST.
#[debug_handler]
pub(super) async fn confirm_remove(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageMembers>,
    path: IdOr404<workspace_members::Entity>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let wid = tenant.workspace_id();
    let (member, role) = workspace_members::Model::find_in(&ctx.db, wid, path.id)
        .await?
        .ok_or(Error::NotFound)?;
    let user = users::Model::find_by_id(&ctx.db, member.user_id)
        .await?
        .ok_or(Error::NotFound)?;
    let base = members_path(&tenant.workspace.slug);
    let manageable = roles::can_assign(&tenant.role, &role);
    let row = MemberRow::new(&member, &user, &role, None, manageable, &base);
    let shell = app_shell(&ctx, &tenant.user, Some(tenant)).await?;
    views::members::confirm_remove(&v, &shell, &row, &base)
}

#[debug_handler]
pub(super) async fn remove(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageMembers>,
    path: IdOr404<workspace_members::Entity>,
    session: Session,
    RequestId(request_id): RequestId,
) -> Result<Response> {
    let tenant = &access.tenant;
    match membership::remove(&ctx, &acting(tenant, request_id), path.id).await {
        Ok(()) => redirect_with(session, tenant, views::members::FLASH_REMOVED).await,
        Err(e) => rejected(&ctx, &v, tenant, e).await,
    }
}

/// Konfirmasi "Keluar dari workspace", terbuka untuk semua anggota (`Tenant`).
#[debug_handler]
pub(super) async fn confirm_leave(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    tenant: Tenant,
) -> Result<Response> {
    let shell = app_shell(&ctx, &tenant.user, Some(&tenant)).await?;
    views::members::confirm_leave(
        &v,
        &shell,
        &tenant.workspace.name,
        &super::leave_path(&tenant.workspace.slug),
        "",
        StatusCode::OK,
    )
}

/// Keluar lalu ke `/` (pilih workspace lain atau buat baru). Owner terakhir → 422.
#[debug_handler]
pub(super) async fn leave(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    tenant: Tenant,
    RequestId(request_id): RequestId,
) -> Result<Response> {
    match membership::leave(&ctx, &acting(&tenant, request_id)).await {
        Ok(()) => format::redirect("/"),
        Err(e) => {
            let Some(code) = e.code() else {
                return Err(e.into());
            };
            let shell = app_shell(&ctx, &tenant.user, Some(&tenant)).await?;
            views::members::confirm_leave(
                &v,
                &shell,
                &tenant.workspace.name,
                &super::leave_path(&tenant.workspace.slug),
                code,
                StatusCode::UNPROCESSABLE_ENTITY,
            )
        }
    }
}

/// Tautan indeks Pengaturan ke halaman ini, bila role membukanya.
pub(super) fn settings_link(tenant: &Tenant) -> Option<String> {
    ManageMembers::allows(&tenant.role).then(|| members_path(&tenant.workspace.slug))
}
