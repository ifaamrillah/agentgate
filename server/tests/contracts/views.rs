//! View engine aplikasi: fungsi template custom terdaftar dan ketat.

use agentgate_server::{
    initializers::view_engine::{IconFn, build_view_engine},
    views::layout::{AppShell, ProjectLink, UserView, WorkspaceLink, WorkspaceView, app_page},
};
use loco_rs::{controller::views::ViewRenderer, testing::prelude::*};

#[test]
fn view_engine_loads_every_template() {
    build_view_engine().unwrap();
}

#[test]
fn unknown_icon_name_is_a_render_error() {
    let dir = std::env::temp_dir().join("agentgate-icon-test/views");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("x.html"), r#"{{ icon(name="tidak-ada") }}"#).unwrap();
    let view = loco_rs::controller::views::engines::TeraView::from_custom_dir(&dir, |tera| {
        let icons =
            agentgate_server::common::icons::IconSet::load(std::path::Path::new("assets/icons"))
                .unwrap();
        tera.register_function("icon", IconFn(std::sync::Arc::new(icons)));
        Ok(())
    })
    .unwrap();
    assert!(view.render("x.html", serde_json::json!({})).is_err());
}

fn shell_with_workspaces(can_manage: bool) -> AppShell {
    let acme = WorkspaceView::new("Acme", "acme");
    AppShell::new(UserView::new("Ana Tes", "ana@example.test"))
        .with_workspaces(vec![
            WorkspaceLink::new("Acme", "acme"),
            WorkspaceLink::new("Beta Labs", "beta"),
        ])
        .with_current_workspace(acme)
        .with_projects(vec![
            ProjectLink::new("acme", "Website", "WEB"),
            ProjectLink::new("acme", "Mobile App", "APP").colored(Some("#0d9488")),
        ])
        .with_current_project(ProjectLink::new("acme", "Mobile App", "APP"))
        .can_manage_workspace(can_manage)
}

fn render_home(shell: &AppShell) -> String {
    let page = app_page(
        shell,
        serde_json::json!({
            "workspace_name": "Acme",
            "first_name": "Ana",
            "today": {"weekday_key": "weekday-3", "day": 12, "month_key": "month-11", "year": 2026, "greeting_key": "greeting-day"},
            "create_href": null,
            "stats": [],
            "audit_href": null,
        }),
    );
    build_view_engine()
        .unwrap()
        .render("workspaces/home.html", serde_json::to_value(&page).unwrap())
        .unwrap()
}

#[test]
fn app_layout_lists_workspaces_and_expands_active_project() {
    let html = render_home(&shell_with_workspaces(true));
    // Switcher: semua workspace, yang aktif ditandai.
    assert_count(&html, "[data-workspace-item]", 2);
    assert_css_exists(&html, "a[data-workspace-item][href=\"/w/beta\"]");
    assert_css_exists(
        &html,
        "a[data-workspace-item][href=\"/w/acme\"][aria-current=\"true\"]",
    );
    assert_css_eq(&html, "#ws-menu-trigger [data-workspace-name]", "Acme");
    // Beranda menuju beranda workspace.
    assert_css_exists(
        &html,
        "aside.app-side a.app-ni[data-nav=\"home\"][href=\"/w/acme\"]",
    );
    // Project: semua terdaftar dengan titik warna; yang aktif terbuka dengan bagian "segera".
    assert_count(&html, "[data-project-list] a.app-ni:not([data-sub])", 2);
    assert_css_exists(
        &html,
        "[data-project-list] a[data-nav=\"project-APP\"][href=\"/w/acme/p/APP\"] .app-dot[style*=\"#0d9488\"]",
    );
    assert_count(
        &html,
        "[data-project-list] a[data-sub][aria-disabled=\"true\"]:not([href])",
        4,
    );
    for label in ["Board", "Backlog", "Knowledge", "Code graph"] {
        assert!(html.contains(label), "{label}");
    }
    // Breadcrumb: workspace / project.
    assert_css_exists(&html, ".app-topbar [data-crumbs] a[href=\"/w/acme/p/APP\"]");
    // Pengaturan (menu profil) hanya bila boleh.
    assert_css_exists(&html, "#me-menu a[href=\"/w/acme/settings\"]");
    let html = render_home(&shell_with_workspaces(false));
    assert_css_not_exists(&html, "a[href=\"/w/acme/settings\"]");
}

#[test]
fn avatar_initials_come_from_name_or_email() {
    assert_eq!(UserView::new("Ana Tes", "a@x.test").initials, "AT");
    assert_eq!(UserView::new("budi", "b@x.test").initials, "B");
    assert_eq!(UserView::new("  ", "citra@x.test").initials, "C");
    assert_eq!(UserView::new("Dewi Ayu Lestari", "d@x.test").initials, "DA");
}
