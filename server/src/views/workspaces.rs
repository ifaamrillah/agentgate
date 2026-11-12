//! Halaman workspace: pilih workspace (`/`), buat workspace, beranda workspace, Pengaturan.
use axum::http::StatusCode;
use loco_rs::{prelude::*, validation::ModelValidationErrors};
use serde::Serialize;

use super::{
    flash::Flash,
    form::{FieldErrors, field_errors},
    layout::{AppShell, WorkspaceLink, app_page},
    time::LongDate,
    ui::Stat,
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

/// Data beranda workspace. Daftar project = `shell.projects` (sama dengan sidebar).
#[derive(Debug, Serialize)]
pub struct HomePage<'a> {
    pub workspace_name: &'a str,
    /// Nama depan user untuk salam.
    pub first_name: String,
    pub today: LongDate,
    /// Form buat project, `None` bila role tidak memegang `CreateProjects`.
    pub create_href: Option<&'a str>,
    pub stats: Vec<Stat>,
    /// Log audit, bila role boleh membukanya (tautan di kartu Aktivitas).
    pub audit_href: Option<&'a str>,
}

pub fn home(v: &impl ViewRenderer, shell: &AppShell, page: &HomePage<'_>) -> Result<Response> {
    format::render().view(v, "workspaces/home.html", app_page(shell, page))
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

/// Zona berbahaya: toggle 2FA dan hapus workspace (keduanya Owner bawaan).
#[derive(Debug, Default, Serialize)]
pub struct DangerZoneView {
    pub require_2fa: Option<RequireTwoFactorView>,
    pub delete: Option<DeleteWorkspaceView>,
}

/// Indeks Pengaturan. `form` = bagian Umum (nama, slug), `None` bila role tidak memegang
/// `ManageWorkspace`; `danger` = zona berbahaya. Bagian lain ada di menu kiri (`shell.settings`).
#[derive(Debug, Serialize)]
pub struct SettingsPage<'a> {
    pub action: &'a str,
    pub form: Option<&'a WorkspaceFormView>,
    pub danger: &'a DangerZoneView,
    /// Sub-halaman yang dibuka: `profile`, `security`, atau `danger`.
    pub section: &'static str,
    /// Menu subpage (varian E) Umum: hanya sub-halaman yang boleh dibuka role.
    pub subnav: Vec<super::nav::NavItem>,
    /// Anggota tanpa 2FA aktif (kartu Keamanan).
    pub without_two_factor: u64,
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
                "danger": page.danger,
                "section": page.section,
                "subnav": page.subnav,
                "without_two_factor": page.without_two_factor,
                "flash": flash,
            }),
        ),
    )
}

/// Opsi combobox orang (`GET /w/{slug}/people`): hanya `<option>`.
pub fn people_options(
    v: &impl ViewRenderer,
    options: &[super::ui::ComboOption],
) -> Result<Response> {
    format::render().view(v, "workspaces/people_options.html", data!({ "options": options }))
}
