//! Satu tempat yang membangun data sidebar (`views::layout::AppShell`) untuk halaman ber-layout
//! aplikasi (rules §6: data layout lewat satu fungsi, bukan dirakit di setiap handler).
//!
//! Berisi user, daftar workspace yang boleh dibuka user (switcher), dan, bila halaman berada di
//! dalam workspace (`Some(tenant)`), workspace aktif serta tampil-tidaknya Pengaturan (role
//! memegang izin yang punya halaman Pengaturan). Project yang bisa diakses diisi di Task 12.
use loco_rs::{Result, app::AppContext};

use super::extractors::Tenant;
use crate::{
    models::{users, workspace_members},
    views::layout::{AppShell, WorkspaceLink, WorkspaceView},
};

/// # Errors
///
/// Error database saat memuat daftar workspace.
pub async fn app_shell(
    ctx: &AppContext,
    user: &users::Model,
    tenant: Option<&Tenant>,
) -> Result<AppShell> {
    let workspaces = workspace_members::Model::workspaces_of(&ctx.db, user.id)
        .await?
        .iter()
        .map(|(w, _)| WorkspaceLink::new(&w.name, &w.slug))
        .collect();
    let shell = AppShell::for_user(user).with_workspaces(workspaces);
    Ok(match tenant {
        Some(tenant) => shell
            .with_current_workspace(WorkspaceView::new(
                &tenant.workspace.name,
                &tenant.workspace.slug,
            ))
            .can_manage_workspace(tenant.role.has_settings_access()),
        None => shell,
    })
}
