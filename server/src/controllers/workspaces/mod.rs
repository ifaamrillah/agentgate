//! Workspace: pilih (`/`), buat (`/w/new`), beranda (`/w/{slug}`), dan Pengaturan
//! (`/w/{slug}/settings`). Semua route di bawah `/w/{slug}` memakai extractor `Tenant`.
use loco_rs::prelude::*;
use serde::Deserialize;

mod home;
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
}
