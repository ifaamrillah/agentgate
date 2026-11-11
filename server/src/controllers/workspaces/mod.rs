//! Workspace: pilih (`/`), buat (`/w/new`), beranda (`/w/{slug}`), dan Pengaturan
//! (`/w/{slug}/settings`, dengan Role dan Jabatan di bawahnya). Semua route di bawah
//! `/w/{slug}` memakai extractor `Tenant`.
use loco_rs::prelude::*;
use serde::Deserialize;

mod home;
mod job_titles;
mod roles;
mod settings;

/// Isian form nama + slug (buat workspace dan Pengaturan). Field yang tidak dikirim dianggap
/// kosong, supaya form yang dipotong tetap dirender ulang dengan pesan validasi.
#[derive(Debug, Deserialize)]
pub struct WorkspaceForm {
    #[serde(default)]
    name: String,
    #[serde(default)]
    slug: String,
}

fn settings_path(slug: &str) -> String {
    format!("/w/{slug}/settings")
}

fn roles_path(slug: &str) -> String {
    format!("/w/{slug}/settings/roles")
}

fn job_titles_path(slug: &str) -> String {
    format!("/w/{slug}/settings/job-titles")
}

pub fn routes() -> Routes {
    Routes::new()
        .add("/", get(home::choose))
        // Rute statis menang atas `/w/{slug}`; slug `new` dilarang (workspaces::RESERVED_SLUGS).
        .add("/w/new", get(home::new).post(home::create))
        .add("/w/{slug}", get(home::show))
        .add(
            "/w/{slug}/settings",
            get(settings::show).post(settings::update),
        )
        .add("/w/{slug}/settings/roles", get(roles::index))
        // Rute statis `new` menang atas `{id}`.
        .add(
            "/w/{slug}/settings/roles/new",
            get(roles::new).post(roles::create),
        )
        .add(
            "/w/{slug}/settings/roles/{id}/edit",
            get(roles::edit).post(roles::update),
        )
        .add("/w/{slug}/settings/roles/{id}/delete", post(roles::delete))
        .add(
            "/w/{slug}/settings/job-titles",
            get(job_titles::index).post(job_titles::create),
        )
        .add(
            "/w/{slug}/settings/job-titles/{id}",
            post(job_titles::rename),
        )
        .add(
            "/w/{slug}/settings/job-titles/{id}/delete",
            post(job_titles::delete),
        )
}
