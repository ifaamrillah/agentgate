use agentgate_server::app::App;
use loco_rs::testing::prelude::*;

#[tokio::test]
async fn component_catalog_renders_every_button_variant() {
    request::<App, _, _>(|request, _ctx| async move {
        let res = request.get("/_dev/ui").await;
        assert_eq!(res.status_code(), 200, "{}", res.text());
        let html = res.text();
        for variant in [
            "primary",
            "secondary",
            "outline",
            "ghost",
            "destructive",
            "link",
        ] {
            assert!(
                html.contains(&format!("data-variant=\"{variant}\"")),
                "{variant}"
            );
        }
        assert!(html.contains("<svg"), "ikon tidak dirender");
    })
    .await;
}

#[tokio::test]
async fn component_catalog_renders_field_and_flash() {
    request::<App, _, _>(|request, _ctx| async move {
        let html = request.get("/_dev/ui").await.text();
        // field: label terhubung ke input, error diterjemahkan dari kode dan ditautkan aria.
        assert!(html.contains("for=\"catalog-email\""), "{html}");
        assert!(html.contains("aria-invalid=\"true\""));
        assert!(html.contains("aria-describedby=\"catalog-password-error\""));
        assert!(html.contains("Password minimal 12 karakter."));
        for kind in ["info", "success", "error"] {
            assert!(html.contains(&format!("data-flash=\"{kind}\"")), "{kind}");
        }
    })
    .await;
}

#[tokio::test]
async fn component_catalog_renders_shared_components() {
    request::<App, _, _>(|request, _ctx| async move {
        let res = request.get("/_dev/ui").await;
        assert_eq!(res.status_code(), 200, "{}", res.text());
        let html = res.text();
        // Kerangka halaman: breadcrumb, kepala (judul, aksi), tab dari views::nav.
        assert_css_exists(&html, ".app-topbar [data-crumbs] a");
        assert_css_eq(&html, ".app-page-head h1", "Katalog UI");
        assert_css_exists(&html, ".app-page-actions .btn");
        assert_css_exists(
            &html,
            "[data-tabs] a[data-nav=\"overview\"][aria-current=\"page\"]",
        );
        assert_css_exists(
            &html,
            "[data-tabs] a[data-nav=\"board\"][aria-disabled=\"true\"]:not([href])",
        );
        // subnav: item aktif ditandai dari data.
        assert_css_exists(
            &html,
            "#layout [data-subnav] a[data-nav=\"password\"][aria-current=\"page\"]",
        );
        // panel: judul, deskripsi, tautan aksi, varian, flush.
        assert_css_exists(
            &html,
            "#table .app-panel[data-flush] .app-panel-header .app-panel-action",
        );
        assert_css_exists(&html, ".app-panel[data-variant=\"danger\"]");
        assert_css_exists(&html, ".app-panel[data-variant=\"tint\"]");
        // table: header objek, kolom tersembunyi, kolom aksi dengan action_menu.
        assert_count(&html, "#table table.table thead th", 5);
        assert_css_exists(&html, "#table th[data-hide=\"md\"]");
        assert_css_exists(
            &html,
            "#table td[data-actions] .popover [data-popover] form",
        );
        // setting_row, disclosure, stat, key_tile, meta_list, person.
        assert_count(&html, "#form .app-setting-row", 2);
        assert_css_exists(&html, "#disclosure details.app-disclosure summary");
        assert_count(&html, "#stat .app-stat", 4);
        assert_css_exists(&html, ".app-key-tile[data-size=\"lg\"]");
        assert_css_exists(&html, "#tokens dl.app-meta-list dd.app-code");
        assert_css_exists(&html, ".app-person .avatar");
        // field berpasangan: select dan textarea.
        assert_css_exists(&html, "#catalog-select option[value=\"2\"][selected]");
        assert_css_exists(&html, "textarea#catalog-textarea");
        // badge: setiap varian.
        for variant in ["primary", "secondary", "outline", "destructive"] {
            assert_css_exists(
                &html,
                &format!("#tokens .badge[data-variant=\"{variant}\"]"),
            );
        }
        // empty_state bergaris dengan aksi.
        assert_css_exists(&html, "#feedback .empty.border-dashed footer .btn");
        // avatar: semua ukuran.
        for size in ["sm", "default", "lg"] {
            assert_css_exists(&html, &format!(".avatar[data-size=\"{size}\"] > span"));
        }
        // sidebar_nav: halaman ini, bagian aktif, nonaktif "segera", dan sub-menu.
        assert_css_exists(&html, "#sidebar_nav li > a[aria-current=\"page\"][href]");
        assert_css_exists(&html, "#sidebar_nav li > a[data-active=\"true\"]");
        assert_css_exists(
            &html,
            "#sidebar_nav li > a[aria-disabled=\"true\"]:not([href])",
        );
        assert_css_exists(&html, "#sidebar_nav li > ul > li > a");
    })
    .await;
}

#[tokio::test]
async fn component_catalog_uses_app_shell_with_sample_workspaces() {
    request::<App, _, _>(|request, _ctx| async move {
        let html = request.get("/_dev/ui").await.text();
        assert_css_exists(&html, "aside.sidebar#app-sidebar > nav");
        assert!(select(&html, "[data-workspace-item]").len() >= 2);
    })
    .await;
}
