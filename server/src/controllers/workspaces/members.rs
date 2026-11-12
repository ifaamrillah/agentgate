//! Pengaturan → Anggota (`Require<ManageMembers>`): tabel anggota dengan ganti role dan jabatan,
//! keluarkan anggota (konfirmasi lewat halaman biasa), undangan tertunda dan form undang
//! (handler undang di `invitations.rs`). Juga "Keluar dari workspace" (`/w/{slug}/leave`,
//! semua anggota, `MemberIgnoring2fa`: tanpa redirect setup 2FA).
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
            IdOr404, MemberIgnoring2fa, PermissionMarker, RequestId, Require, Tenant, ValidForm,
            require::ManageMembers,
        },
        flash,
        shell::app_shell,
    },
    models::{
        invitations, job_titles, roles, team_members, teams, users,
        workspace_members::{self, MemberQuery, MemberSort},
    },
    services::workspaces::{
        Acting,
        membership::{self, MembershipError},
    },
    views::{
        self,
        flash::Flash,
        datatable::{FilterOption, TableView},
        members::{
            InvitationRow, InviteFormView, MEMBERS_TABLE, MemberRow, MembersPage, OptionView,
            TeamTag,
        },
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

pub(super) use crate::controllers::extractors::form_id;

/// Isian jabatan: kosong = tanpa jabatan.
pub(super) fn optional_form_id(raw: &str) -> Result<Option<Uuid>> {
    if raw.trim().is_empty() {
        Ok(None)
    } else {
        form_id(raw).map(Some)
    }
}

/// Halaman anggota dengan isian form undang `invite` dan pesan `flash` (tabel di keadaan awal).
pub(super) async fn render_index(
    ctx: &AppContext,
    v: &TeraView,
    tenant: &Tenant,
    invite: InviteFormView,
    flash: Option<Flash>,
    status: StatusCode,
) -> Result<Response> {
    render_table(ctx, v, tenant, &[], invite, flash, status).await
}

/// Nilai filter id (role/tim) dari query string; id tidak valid diabaikan.
fn ids(values: &[String]) -> Vec<Uuid> {
    values.iter().filter_map(|v| Uuid::parse_str(v).ok()).collect()
}

/// Halaman anggota dengan tabel server-side menurut `query` (cari, filter, sort, halaman).
async fn render_table(
    ctx: &AppContext,
    v: &TeraView,
    tenant: &Tenant,
    query: &[(String, String)],
    invite: InviteFormView,
    flash: Option<Flash>,
    status: StatusCode,
) -> Result<Response> {
    let wid = tenant.workspace_id();
    let base = members_path(&tenant.workspace.slug);
    let invite_action = super::invitations_path(&tenant.workspace.slug);
    let roles = roles::Model::in_workspace(&ctx.db, wid).await?;
    let titles = job_titles::Model::in_workspace(&ctx.db, wid).await?;
    let teams = teams::Model::in_workspace(&ctx.db, wid).await?;
    let state = MEMBERS_TABLE.parse(query);
    let tfa = state.filter("tfa");
    let member_query = MemberQuery {
        q: state.query(),
        role_ids: ids(state.filter("role")),
        team_ids: ids(state.filter("team")),
        two_factor: match (tfa.iter().any(|v| v == "on"), tfa.iter().any(|v| v == "off")) {
            (true, false) => Some(true),
            (false, true) => Some(false),
            _ => None,
        },
        sort: match state.sort {
            "role" => MemberSort::Role,
            "joined" => MemberSort::Joined,
            _ => MemberSort::Name,
        },
        desc: state.desc,
        offset: state.offset(),
        limit: state.per,
    };
    let (rows, total) = workspace_members::Model::page_in(&ctx.db, wid, &member_query).await?;
    // Halaman di luar jangkauan: TableView menjepit nomor halaman, ambil ulang halaman itu.
    let table = TableView::new(&MEMBERS_TABLE, &state, &base, total);
    let rows = if table.page == state.page {
        rows
    } else {
        let clamped = MemberQuery {
            offset: (table.page - 1) * table.per,
            ..member_query
        };
        workspace_members::Model::page_in(&ctx.db, wid, &clamped).await?.0
    };
    let memberships =
        team_members::Model::in_teams_of(&ctx.db, wid, rows.iter().map(|(m, ..)| m.user_id)).await?;
    let teams_base = super::teams_path(&tenant.workspace.slug);
    let members = rows
        .iter()
        .map(|(member, user, role)| {
            let title = member
                .job_title_id
                .and_then(|id| titles.iter().find(|t| t.id == id));
            let manageable = roles::can_assign(&tenant.role, role);
            let tags = memberships
                .iter()
                .filter(|m| m.user_id == user.id)
                .filter_map(|m| teams.iter().find(|t| t.id == m.team_id))
                .map(|t| TeamTag {
                    name: t.name.clone(),
                    href: format!("{teams_base}/{}", t.id),
                })
                .collect();
            MemberRow::new(member, user, role, title, manageable, &base).with_teams(tags)
        })
        .collect();
    let table = table
        .with_filter_options(
            "role",
            roles
                .iter()
                .map(|r| FilterOption::raw(&r.id.to_string(), &r.name))
                .collect(),
        )
        .with_filter_options(
            "team",
            teams
                .iter()
                .map(|t| FilterOption::raw(&t.id.to_string(), &t.name))
                .collect(),
        )
        .with_filter_options(
            "tfa",
            vec![
                FilterOption::key("on", "member-two-factor-on"),
                FilterOption::key("off", "member-two-factor-off"),
            ],
        );
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
        table,
        invitations,
        roles: roles
            .iter()
            .filter(|role| roles::can_assign(&tenant.role, role))
            .map(OptionView::role)
            .collect(),
        job_titles: titles.iter().map(OptionView::job_title).collect(),
        invite,
        invite_action,
        invite_open: status == StatusCode::UNPROCESSABLE_ENTITY && flash.is_none(),
        flash,
    };
    let shell = app_shell(ctx, &tenant.user, Some(tenant), None)
        .await?
        .at("members");
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
    Query(query): Query<Vec<(String, String)>>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let (session, flash) = flash::take(session);
    let invite = empty_invite(&ctx, tenant).await?;
    let res = render_table(&ctx, &v, tenant, &query, invite, flash, StatusCode::OK).await?;
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
    let shell = app_shell(&ctx, &tenant.user, Some(tenant), None)
        .await?
        .at("members");
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

/// Konfirmasi "Keluar dari workspace", terbuka untuk semua anggota, termasuk yang belum
/// mengaktifkan 2FA di workspace yang mewajibkannya (`MemberIgnoring2fa`). Shell dirender tanpa
/// konteks workspace supaya daftar project tidak terlihat sebelum syarat 2FA dipenuhi.
#[debug_handler]
pub(super) async fn confirm_leave(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    MemberIgnoring2fa(tenant): MemberIgnoring2fa,
) -> Result<Response> {
    let shell = app_shell(&ctx, &tenant.user, None, None).await?;
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
    MemberIgnoring2fa(tenant): MemberIgnoring2fa,
    RequestId(request_id): RequestId,
) -> Result<Response> {
    match membership::leave(&ctx, &acting(&tenant, request_id)).await {
        Ok(()) => format::redirect("/"),
        Err(e) => {
            let Some(code) = e.code() else {
                return Err(e.into());
            };
            let shell = app_shell(&ctx, &tenant.user, None, None).await?;
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
