//! Pengaturan → Jabatan: daftar jabatan dengan form ganti nama per baris, form tambah, dan
//! tombol hapus.
use axum::http::StatusCode;
use loco_rs::{prelude::*, validation::ModelValidationErrors};
use serde::Serialize;

use super::{
    flash::Flash,
    form::{FieldError, FieldErrors, field_errors},
    layout::{AppShell, app_page},
};
use crate::models::job_titles;

pub const FLASH_CREATED: &str = "flash-job-title-created";
pub const FLASH_UPDATED: &str = "flash-job-title-updated";
pub const FLASH_DELETED: &str = "flash-job-title-deleted";
/// Kunci Fluent yang dikirim sebagai variabel ke template (dijaga tes i18n).
pub const MESSAGE_KEYS: &[&str] = &[FLASH_CREATED, FLASH_UPDATED, FLASH_DELETED];

/// Satu jabatan: form ganti nama (`field_id`, `name`, `error`) dan aksi hapus.
#[derive(Debug, Serialize)]
pub struct JobTitleRow {
    pub field_id: String,
    pub name: String,
    pub action: String,
    pub delete_action: String,
    pub error: FieldError,
}

impl JobTitleRow {
    /// `base` = path daftar jabatan (`/w/{slug}/settings/job-titles`).
    #[must_use]
    pub fn new(title: &job_titles::Model, base: &str) -> Self {
        Self {
            field_id: format!("job-title-{}", title.id),
            name: title.name.clone(),
            action: format!("{base}/{}", title.id),
            delete_action: format!("{base}/{}/delete", title.id),
            error: FieldError::default(),
        }
    }

    /// Baris yang gagal diganti namanya: isian user dan error-nya.
    #[must_use]
    pub fn with_error(mut self, name: &str, errors: &ModelValidationErrors) -> Self {
        self.name = name.to_string();
        self.error = field_errors(&["name"], Some(errors))
            .remove("name")
            .unwrap_or_default();
        self
    }
}

/// Isian form tambah jabatan.
#[derive(Debug, Serialize)]
pub struct JobTitleFormView {
    pub name: String,
    pub errors: FieldErrors,
}

impl JobTitleFormView {
    #[must_use]
    pub fn new(name: &str, errors: Option<&ModelValidationErrors>) -> Self {
        Self {
            name: name.to_string(),
            errors: field_errors(&["name"], errors),
        }
    }
}

pub fn index(
    v: &impl ViewRenderer,
    shell: &AppShell,
    titles: &[JobTitleRow],
    create: &JobTitleFormView,
    action: &str,
    flash: Option<Flash>,
    status: StatusCode,
) -> Result<Response> {
    format::render().status(status).view(
        v,
        "workspaces/job_titles/index.html",
        app_page(
            shell,
            data!({ "titles": titles, "create": create, "action": action, "flash": flash }),
        ),
    )
}
