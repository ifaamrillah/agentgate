//! Project di dalam workspace: buat (`/w/{slug}/projects/new`, `Require<CreateProjects>`),
//! beranda project (`/w/{slug}/p/{key}`, `ProjectAccess`), dan pengaturan project (Umum, Akses,
//! Repo, logo; `ProjectAdmin` = `AdministerProject`). Gambar logo (`/w/{slug}/p/{key}/logo`)
//! untuk semua yang punya akses project. Project tanpa akses, milik workspace lain, atau
//! key yang tidak ada → 404.
use loco_rs::prelude::*;

mod access;
mod logo;
#[allow(clippy::module_inception)]
mod projects;
mod repos;

use super::extractors::ProjectAccess;
use crate::{
    services::workspaces::Acting,
    views::projects::{ProjectPaths, ProjectSummary},
};

/// Form buat project.
#[must_use]
pub fn new_project_path(slug: &str) -> String {
    format!("/w/{slug}/projects/new")
}

fn paths(access: &ProjectAccess) -> ProjectPaths {
    ProjectPaths::new(&access.tenant.workspace.slug, &access.project.key)
}

fn summary(access: &ProjectAccess) -> ProjectSummary {
    ProjectSummary::new(&access.project, access.role.label(), &paths(access))
}

fn acting(access: &ProjectAccess, request_id: Option<String>) -> Acting<'_> {
    Acting {
        workspace: &access.tenant.workspace,
        user: &access.tenant.user,
        role: &access.tenant.role,
        request_id,
    }
}

pub fn routes() -> Routes {
    Routes::new()
        .add(
            "/w/{slug}/projects/new",
            get(projects::new).post(projects::create),
        )
        .add("/w/{slug}/p/{key}", get(projects::show))
        .add("/w/{slug}/p/{key}/logo", get(logo::show))
        .add("/w/{slug}/p/{key}/settings/logo", post(logo::upload))
        .add("/w/{slug}/p/{key}/settings/logo/delete", post(logo::delete))
        .add(
            "/w/{slug}/p/{key}/settings",
            get(projects::settings).post(projects::update),
        )
        .add(
            "/w/{slug}/p/{key}/settings/archive",
            post(projects::archive),
        )
        .add(
            "/w/{slug}/p/{key}/settings/access",
            get(access::index).post(access::grant),
        )
        .add(
            "/w/{slug}/p/{key}/settings/access/{id}/role",
            post(access::change_role),
        )
        .add(
            "/w/{slug}/p/{key}/settings/access/{id}/revoke",
            post(access::revoke),
        )
        .add(
            "/w/{slug}/p/{key}/settings/repos",
            get(repos::index).post(repos::create),
        )
        .add("/w/{slug}/p/{key}/settings/repos/{id}", post(repos::update))
        .add(
            "/w/{slug}/p/{key}/settings/repos/{id}/delete",
            post(repos::delete),
        )
}
