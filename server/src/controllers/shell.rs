//! Satu tempat yang membangun data sidebar (`views::layout::AppShell`) untuk halaman ber-layout
//! aplikasi (rules §6: data layout lewat satu fungsi, bukan dirakit di setiap handler).
//!
//! Berisi user, daftar workspace yang boleh dibuka user (switcher), dan, bila halaman berada di
//! dalam workspace (`Some(tenant)`), workspace aktif, tampil-tidaknya Pengaturan (role memegang
//! izin yang punya halaman Pengaturan), dan project yang bisa dibuka
//! (`projects::Model::accessible_for`, satu query dengan aturan `project_access`), serta banner
//! workspace terjadwal hapus. Halaman di dalam project (`Some(project)`) menandai project itu
//! aktif (menu project terbuka).
use loco_rs::{Result, app::AppContext};

use super::extractors::{ProjectAccess, Tenant};
use crate::{
    models::{permission::Permission, projects, users, workspace_members},
    views::layout::{AppShell, ProjectLink, WorkspaceLink, WorkspaceView},
};

/// # Errors
///
/// Error database saat memuat daftar workspace atau project.
pub async fn app_shell(
    ctx: &AppContext,
    user: &users::Model,
    tenant: Option<&Tenant>,
    project: Option<&ProjectAccess>,
) -> Result<AppShell> {
    let workspaces = workspace_members::Model::workspaces_of(&ctx.db, user.id)
        .await?
        .iter()
        .map(|(w, _)| WorkspaceLink::new(&w.name, &w.slug))
        .collect();
    let shell = AppShell::for_user(user).with_workspaces(workspaces);
    let Some(tenant) = tenant else {
        return Ok(shell);
    };
    let slug = &tenant.workspace.slug;
    let projects = projects::Model::accessible_for(&ctx.db, &tenant.accessor())
        .await?
        .iter()
        .map(|p| {
            ProjectLink::new(slug, &p.name, &p.key)
                .archived(p.is_archived())
                .described(&p.description)
        })
        .collect();
    let shell = shell
        .with_current_workspace(
            WorkspaceView::new(&tenant.workspace.name, slug)
                .scheduled_for_deletion(tenant.workspace.purge_starts_at().as_ref()),
        )
        .can_manage_workspace(tenant.role.has_settings_access())
        .admin_all_projects(tenant.role.is_admin_everywhere())
        .with_new_project_href(
            tenant
                .role
                .has(Permission::CreateProjects)
                .then(|| format!("/w/{slug}/projects/new")),
        )
        .with_settings(
            tenant
                .role
                .has_settings_access()
                .then(|| super::workspaces::settings_links(tenant)),
        )
        .with_projects(projects);
    Ok(match project {
        Some(access) => shell.with_current_project(
            ProjectLink::new(slug, &access.project.name, &access.project.key)
                .archived(access.project.is_archived()),
        ),
        None => shell,
    })
}
