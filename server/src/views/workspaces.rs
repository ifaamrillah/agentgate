//! Halaman workspace: pilih workspace (`/`), buat workspace, beranda workspace, Pengaturan.
use axum::http::StatusCode;
use loco_rs::{prelude::*, validation::ModelValidationErrors};
use serde::Serialize;

use super::{
    flash::Flash,
    form::{FieldErrors, field_errors},
    layout::{AppShell, WorkspaceLink, app_page},
};

pub const FLASH_SETTINGS_SAVED: &str = "flash-workspace-settings-saved";
pub const FLASH_REQUIRE_2FA_ON: &str = "flash-workspace-require-2fa-on";
pub const FLASH_REQUIRE_2FA_OFF: &str = "flash-workspace-require-2fa-off";
pub const FLASH_DELETION_SCHEDULED: &str = "flash-workspace-deletion-scheduled";
pub const FLASH_DELETION_CANCELLED: &str = "flash-workspace-deletion-cancelled";
/// Kunci Fluent yang dikirim sebagai variabel ke template (dijaga tes i18n).
pub const MESSAGE_KEYS: &[&str] = &[
    FLASH_SETTINGS_SAVED,
    FLASH_REQUIRE_2FA_ON,
    FLASH_REQUIRE_2FA_OFF,
    FLASH_DELETION_SCHEDULED,
    FLASH_DELETION_CANCELLED,
];

pub const WORKSPACE_FIELDS: &[&str] = &["name", "slug"];

/// Isian form nama + slug (buat workspace dan Pengaturan) dengan error per field.
#[derive(Debug, Serialize)]
pub struct WorkspaceFormView {
    pub name: String,
    pub slug: String,
    pub errors: FieldErrors,
}

impl WorkspaceFormView {
    #[must_use]
    pub fn new(name: &str, slug: &str, errors: Option<&ModelValidationErrors>) -> Self {
        Self {
            name: name.to_string(),
            slug: slug.to_string(),
            errors: field_errors(WORKSPACE_FIELDS, errors),
        }
    }
}

/// `/` untuk user dengan beberapa workspace.
pub fn choose(
    v: &impl ViewRenderer,
    shell: &AppShell,
    workspaces: &[WorkspaceLink],
) -> Result<Response> {
    format::render().view(
        v,
        "workspaces/choose.html",
        app_page(shell, data!({ "choices": workspaces })),
    )
}

pub fn new(
    v: &impl ViewRenderer,
    shell: &AppShell,
    form: &WorkspaceFormView,
    status: StatusCode,
) -> Result<Response> {
    format::render().status(status).view(
        v,
        "workspaces/new.html",
        app_page(shell, data!({ "form": form })),
    )
}

/// Beranda workspace: daftar project yang bisa dibuka (`shell.projects`, sama dengan
/// sidebar). `create_href` = form buat project, `None` bila role tidak memegang
/// `CreateProjects`.
pub fn home(
    v: &impl ViewRenderer,
    shell: &AppShell,
    name: &str,
    create_href: Option<&str>,
) -> Result<Response> {
    format::render().view(
        v,
        "workspaces/home.html",
        app_page(
            shell,
            data!({ "workspace_name": name, "create_href": create_href }),
        ),
    )
}

/// Tautan ke halaman Pengaturan lain di indeks; `None` bila role tidak membuka halaman itu.
#[derive(Debug, Default, Serialize)]
pub struct SettingsLinks {
    pub members: Option<String>,
    pub roles: Option<String>,
    pub job_titles: Option<String>,
    pub teams: Option<String>,
    pub audit: Option<String>,
}

/// Toggle "Wajibkan 2FA": `enabled` = status sekarang; form mengirim kebalikannya.
#[derive(Debug, Serialize)]
pub struct RequireTwoFactorView {
    pub action: String,
    pub enabled: bool,
}

/// Form "Hapus workspace": ketik `slug` untuk konfirmasi. `error` = kode error isian
/// (`""` = tanpa error). `None` di [`DangerZoneView`] bila sudah terjadwal (banner layout
/// menampilkan tombol batalkan).
#[derive(Debug, Serialize)]
pub struct DeleteWorkspaceView {
    pub action: String,
    pub slug: String,
    pub error: String,
}

/// Zona berbahaya: toggle 2FA (`ManageWorkspace`) dan hapus workspace (Owner bawaan).
#[derive(Debug, Default, Serialize)]
pub struct DangerZoneView {
    pub require_2fa: Option<RequireTwoFactorView>,
    pub delete: Option<DeleteWorkspaceView>,
}

/// Indeks Pengaturan. `form` = bagian Umum (nama, slug), `None` bila role tidak memegang
/// `ManageWorkspace`; `links` = bagian lain yang boleh dibuka role; `danger` = zona berbahaya.
#[derive(Debug, Serialize)]
pub struct SettingsPage<'a> {
    pub action: &'a str,
    pub form: Option<&'a WorkspaceFormView>,
    pub links: &'a SettingsLinks,
    pub danger: &'a DangerZoneView,
}

pub fn settings(
    v: &impl ViewRenderer,
    shell: &AppShell,
    page: &SettingsPage<'_>,
    flash: Option<Flash>,
    status: StatusCode,
) -> Result<Response> {
    format::render().status(status).view(
        v,
        "workspaces/settings.html",
        app_page(
            shell,
            data!({
                "action": page.action,
                "form": page.form,
                "links": page.links,
                "danger": page.danger,
                "flash": flash,
            }),
        ),
    )
}
