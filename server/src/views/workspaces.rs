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
/// Kunci Fluent yang dikirim sebagai variabel ke template (dijaga tes i18n).
pub const MESSAGE_KEYS: &[&str] = &[FLASH_SETTINGS_SAVED];

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
}

/// Indeks Pengaturan. `form` = bagian Umum (nama, slug), `None` bila role tidak memegang
/// `ManageWorkspace`; `links` = bagian lain yang boleh dibuka role.
pub fn settings(
    v: &impl ViewRenderer,
    shell: &AppShell,
    action: &str,
    form: Option<&WorkspaceFormView>,
    links: &SettingsLinks,
    flash: Option<Flash>,
    status: StatusCode,
) -> Result<Response> {
    format::render().status(status).view(
        v,
        "workspaces/settings.html",
        app_page(
            shell,
            data!({ "action": action, "form": form, "links": links, "flash": flash }),
        ),
    )
}
