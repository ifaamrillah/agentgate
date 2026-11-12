//! `GET /w/{slug}/settings`: indeks Pengaturan, terbuka untuk role yang memegang salah satu izin
//! Pengaturan (`Require<AnySettings>`, sama dengan tautan di sidebar). Setiap bagian tampil
//! sesuai izinnya sendiri: nama/slug (`ManageWorkspace`), tautan Anggota dan Jabatan
//! (`ManageMembers`), Role (`ManageRoles`), Tim (`ManageTeams`), Audit log (`ViewAuditLog`),
//! serta toggle "Wajibkan 2FA" dan "Hapus workspace" (keduanya role bawaan Owner).
//!
//! POST, masing-masing dengan guard yang sama dengan bagiannya:
//! - `/settings`: nama/slug (`ManageWorkspace`);
//! - `/settings/require-2fa`: wajibkan/berhenti mewajibkan 2FA (Owner);
//! - `/settings/delete`: jadwalkan hapus setelah slug diketik (Owner), lalu 303 ke beranda
//!   workspace yang menampilkan banner tenggat dan tombol batalkan;
//! - `/settings/delete/cancel`: batalkan jadwal hapus (Owner).
use axum::http::StatusCode;
use loco_rs::prelude::*;
use serde::Deserialize;

use super::{
    WorkspaceForm, audit, job_titles, members, members::acting, roles, settings_path, teams,
};
use crate::{
    controllers::{
        extractors::{
            PermissionMarker, RequestId, Require, Tenant, ValidForm,
            require::{AnySettings, ManageWorkspace, WorkspaceOwner},
        },
        flash,
        shell::app_shell,
    },
    models::{workspace_members, workspaces::WorkspaceError},
    services::workspaces::{
        deletion::{self, DELETE_CONFIRM_MISMATCH, DeletionError},
        require_two_factor::{self, REQUIRE_2FA_ACTOR_MISSING, RequireTwoFactorError},
        update::{self, WorkspaceDetails},
    },
    views::{
        self,
        flash::Flash,
        layout::SettingsLinks,
        nav::{self, NavItem},
        workspaces::{
            DangerZoneView, DeleteWorkspaceView, RequireTwoFactorView, SettingsPage,
            WorkspaceFormView,
        },
    },
};

/// Isian toggle 2FA: status yang diminta.
#[derive(Debug, Deserialize)]
pub struct RequireTwoFactorForm {
    enabled: bool,
}

/// Isian konfirmasi hapus. Field yang tidak dikirim dianggap kosong (→ 422).
#[derive(Debug, Deserialize)]
pub struct DeleteForm {
    #[serde(default)]
    confirm_slug: String,
}

/// Isian halaman yang dirender ulang dengan 422.
#[derive(Default)]
struct Rejected<'a> {
    form: Option<&'a WorkspaceFormView>,
    delete_error: Option<&'a str>,
    flash: Option<Flash>,
}

/// Sub-halaman Pengaturan → Umum (subpage varian E, setiap sub-halaman rute server sendiri).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    /// `/settings`: nama dan slug (`ManageWorkspace`).
    Profile,
    /// `/settings/security`: wajibkan 2FA (Owner).
    Security,
    /// `/settings/danger`: hapus workspace (Owner).
    Danger,
}

impl Section {
    const fn key(self) -> &'static str {
        match self {
            Self::Profile => "profile",
            Self::Security => "security",
            Self::Danger => "danger",
        }
    }
}

/// Render sub-halaman `section` dengan `status`. `rejected` = isian dan error yang ditolak.
async fn render(
    ctx: &AppContext,
    v: &TeraView,
    tenant: &Tenant,
    section: Section,
    rejected: Rejected<'_>,
    status: StatusCode,
) -> Result<Response> {
    let shell = app_shell(ctx, &tenant.user, Some(tenant), None)
        .await?
        .at("general");
    let current = general_form(tenant)
        .then(|| WorkspaceFormView::new(&tenant.workspace.name, &tenant.workspace.slug, None));
    let form = rejected.form.or(current.as_ref());
    let without_two_factor =
        workspace_members::Model::without_two_factor_count(&ctx.db, tenant.workspace_id()).await?;
    let base = settings_path(&tenant.workspace.slug);
    let mut subnav = Vec::new();
    if general_form(tenant) {
        subnav.push(
            NavItem::link("profile", "workspace-settings-profile", "building", &base)
                .describe("workspace-settings-profile-desc"),
        );
    }
    if WorkspaceOwner::allows(&tenant.role) {
        subnav.push(
            NavItem::link("security", "workspace-security-title", "shield-check", &format!("{base}/security"))
                .describe("workspace-security-desc")
                .counted(without_two_factor, without_two_factor > 0),
        );
        subnav.push(
            NavItem::link("danger", "workspace-danger-title", "alert-triangle", &format!("{base}/danger"))
                .describe("workspace-danger-desc"),
        );
    }
    let subnav = nav::mark(subnav, section.key());
    views::workspaces::settings(
        v,
        &shell,
        &SettingsPage {
            action: &base,
            form,
            danger: &danger_zone(tenant, rejected.delete_error),
            section: section.key(),
            subnav,
            without_two_factor,
        },
        rejected.flash,
        status,
    )
}

/// GET sub-halaman: flash sekali tampil diambil dari sesi.
async fn show_section(
    ctx: &AppContext,
    v: &TeraView,
    tenant: &Tenant,
    section: Section,
    session: Session,
) -> Result<Response> {
    let (session, flash) = flash::take(session);
    let rejected = Rejected {
        flash,
        ..Rejected::default()
    };
    let res = render(ctx, v, tenant, section, rejected, StatusCode::OK).await?;
    Ok((session, res).into_response())
}

/// `GET /w/{slug}/settings/security` (Owner).
#[debug_handler]
pub(super) async fn security(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<WorkspaceOwner>,
    session: Session,
) -> Result<Response> {
    show_section(&ctx, &v, &access.tenant, Section::Security, session).await
}

/// `GET /w/{slug}/settings/danger` (Owner).
#[debug_handler]
pub(super) async fn danger(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<WorkspaceOwner>,
    session: Session,
) -> Result<Response> {
    show_section(&ctx, &v, &access.tenant, Section::Danger, session).await
}

#[debug_handler]
pub(super) async fn show(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<AnySettings>,
    session: Session,
) -> Result<Response> {
    show_section(&ctx, &v, &access.tenant, Section::Profile, session).await
}

/// Tautan menu kiri Pengaturan, masing-masing menurut guard halaman itu sendiri (dipakai
/// `controllers::shell::app_shell`).
pub(crate) fn links(tenant: &Tenant) -> SettingsLinks {
    SettingsLinks {
        members: members::settings_link(tenant),
        roles: roles::settings_link(tenant),
        job_titles: job_titles::settings_link(tenant),
        teams: teams::settings_link(tenant),
        audit: audit::settings_link(tenant),
    }
}

/// Bagian Umum (nama, slug) tampil hanya untuk `ManageWorkspace`: aturan yang sama dengan guard
/// `POST`.
fn general_form(tenant: &Tenant) -> bool {
    ManageWorkspace::allows(&tenant.role)
}

/// Zona berbahaya menurut guard POST masing-masing. Form hapus tidak tampil bila workspace
/// sudah terjadwal hapus (banner layout menampilkan tombol batalkan).
fn danger_zone(tenant: &Tenant, delete_error: Option<&str>) -> DangerZoneView {
    let base = settings_path(&tenant.workspace.slug);
    DangerZoneView {
        require_2fa: WorkspaceOwner::allows(&tenant.role).then(|| RequireTwoFactorView {
            action: format!("{base}/require-2fa"),
            enabled: tenant.workspace.require_2fa_at.is_some(),
        }),
        delete: (WorkspaceOwner::allows(&tenant.role)
            && tenant.workspace.deletion_scheduled_at.is_none())
        .then(|| DeleteWorkspaceView {
            action: format!("{base}/delete"),
            slug: tenant.workspace.slug.clone(),
            error: delete_error.unwrap_or_default().to_string(),
        }),
    }
}

/// `Require` sebelum `ValidForm`: tanpa izin → 404 sebelum body dibaca (rules §6).
#[debug_handler]
pub(super) async fn update(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageWorkspace>,
    session: Session,
    RequestId(request_id): RequestId,
    ValidForm(form): ValidForm<WorkspaceForm>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let details = WorkspaceDetails {
        name: form.name.clone(),
        slug: form.slug.clone(),
    };
    match update::run(&ctx, &acting(tenant, request_id), details).await {
        Ok(workspace) => {
            let session = flash::set(
                session,
                &Flash::success(views::workspaces::FLASH_SETTINGS_SAVED),
            )?;
            Ok((session, format::redirect(&settings_path(&workspace.slug))?).into_response())
        }
        Err(WorkspaceError::Invalid(errors)) => {
            let form = WorkspaceFormView::new(
                form.name.trim(),
                &form.slug.trim().to_lowercase(),
                Some(&errors),
            );
            let rejected = Rejected {
                form: Some(&form),
                ..Rejected::default()
            };
            render(&ctx, &v, tenant, Section::Profile, rejected, StatusCode::UNPROCESSABLE_ENTITY).await
        }
        Err(e) => Err(e.into()),
    }
}

#[debug_handler]
pub(super) async fn require_2fa(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<WorkspaceOwner>,
    session: Session,
    RequestId(request_id): RequestId,
    ValidForm(form): ValidForm<RequireTwoFactorForm>,
) -> Result<Response> {
    let tenant = &access.tenant;
    match require_two_factor::set(&ctx.db, &acting(tenant, request_id), form.enabled).await {
        Ok(workspace) => {
            let key = if form.enabled {
                views::workspaces::FLASH_REQUIRE_2FA_ON
            } else {
                views::workspaces::FLASH_REQUIRE_2FA_OFF
            };
            let session = flash::set(session, &Flash::success(key))?;
            let to = format!("{}/security", settings_path(&workspace.slug));
            Ok((session, format::redirect(&to)?).into_response())
        }
        Err(RequireTwoFactorError::ActorMissing) => {
            let rejected = Rejected {
                flash: Some(Flash::error(REQUIRE_2FA_ACTOR_MISSING)),
                ..Rejected::default()
            };
            render(&ctx, &v, tenant, Section::Security, rejected, StatusCode::UNPROCESSABLE_ENTITY).await
        }
        Err(RequireTwoFactorError::NotFound) => Err(Error::NotFound),
        Err(RequireTwoFactorError::Db(e)) => Err(e.into()),
    }
}

#[debug_handler]
pub(super) async fn schedule_deletion(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<WorkspaceOwner>,
    session: Session,
    RequestId(request_id): RequestId,
    ValidForm(form): ValidForm<DeleteForm>,
) -> Result<Response> {
    let tenant = &access.tenant;
    match deletion::schedule(&ctx.db, &acting(tenant, request_id), &form.confirm_slug).await {
        Ok(workspace) => {
            let session = flash::set(
                session,
                &Flash::success(views::workspaces::FLASH_DELETION_SCHEDULED),
            )?;
            Ok((
                session,
                format::redirect(&format!("/w/{}", workspace.slug))?,
            )
                .into_response())
        }
        Err(DeletionError::ConfirmMismatch) => {
            let rejected = Rejected {
                delete_error: Some(DELETE_CONFIRM_MISMATCH),
                ..Rejected::default()
            };
            render(&ctx, &v, tenant, Section::Danger, rejected, StatusCode::UNPROCESSABLE_ENTITY).await
        }
        Err(DeletionError::NotFound) => Err(Error::NotFound),
        Err(DeletionError::Db(e)) => Err(e.into()),
    }
}

#[debug_handler]
pub(super) async fn cancel_deletion(
    State(ctx): State<AppContext>,
    access: Require<WorkspaceOwner>,
    session: Session,
    RequestId(request_id): RequestId,
) -> Result<Response> {
    let tenant = &access.tenant;
    match deletion::cancel(&ctx.db, &acting(tenant, request_id)).await {
        Ok(workspace) => {
            let session = flash::set(
                session,
                &Flash::success(views::workspaces::FLASH_DELETION_CANCELLED),
            )?;
            Ok((
                session,
                format::redirect(&format!("/w/{}", workspace.slug))?,
            )
                .into_response())
        }
        // Pembatalan tidak pernah memeriksa konfirmasi; varian itu tidak terjadi di sini.
        Err(DeletionError::NotFound | DeletionError::ConfirmMismatch) => Err(Error::NotFound),
        Err(DeletionError::Db(e)) => Err(e.into()),
    }
}
