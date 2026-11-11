//! Katalog komponen `/_dev/ui`. Data contoh statis (tanpa database), termasuk shell dengan
//! workspace dan project supaya sidebar lengkap ikut terlihat.
use loco_rs::prelude::*;

use super::layout::{AppShell, ProjectLink, UserView, WorkspaceLink, WorkspaceView, app_page};

fn sample_shell() -> AppShell {
    AppShell::new(UserView::new("Ana Nugraha", "ana@agentgate.test"))
        .with_workspaces(vec![
            WorkspaceLink::new("Acme Studio", "acme"),
            WorkspaceLink::new("Beta Labs", "beta"),
        ])
        .with_current_workspace(WorkspaceView::new("Acme Studio", "acme"))
        .with_projects(vec![
            ProjectLink::new("acme", "Website", "WEB"),
            ProjectLink::new("acme", "Aplikasi Mobile", "APP"),
        ])
        .with_current_project(ProjectLink::new("acme", "Website", "WEB"))
        .can_manage_workspace(true)
}

pub fn ui(v: &impl ViewRenderer) -> Result<Response> {
    format::render().view(v, "dev/ui.html", app_page(&sample_shell(), data!({})))
}
