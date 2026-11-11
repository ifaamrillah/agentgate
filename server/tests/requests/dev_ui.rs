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
        // card: ukuran default dan sm, judul + deskripsi + isi.
        assert_css_exists(&html, "#card section.card:not([data-size]) > header > h2");
        assert_css_exists(&html, "#card section.card[data-size=\"sm\"] > section");
        // badge: setiap varian.
        for variant in ["primary", "secondary", "outline", "destructive"] {
            assert_css_exists(&html, &format!("#badge .badge[data-variant=\"{variant}\"]"));
        }
        // empty_state: varian polos dan bergaris, aksi lewat body.
        assert_css_exists(&html, "#empty_state .empty:not(.border) figure svg");
        assert_css_exists(
            &html,
            "#empty_state .empty.border.border-dashed footer .btn",
        );
        // table: header dari array, baris dari body.
        assert_count(&html, "#table .table-container table.table thead th", 3);
        assert_css_exists(&html, "#table table.table tbody tr td");
        // page_header: judul, deskripsi, dan aksi.
        assert_css_eq(&html, "#page_header .app-page-header h1", "Project");
        assert_css_exists(&html, "#page_header .app-page-header .btn");
        // avatar: inisial, semua ukuran.
        for size in ["sm", "default", "lg"] {
            assert_css_exists(
                &html,
                &format!("#avatar .avatar[data-size=\"{size}\"] > span"),
            );
        }
        assert!(html.contains(">AN</span>"), "inisial avatar");
        // sidebar_nav: halaman ini, bagian aktif, nonaktif "segera", dan sub-menu.
        assert_css_exists(&html, "#sidebar_nav li > a[aria-current=\"page\"][href]");
        assert_css_exists(&html, "#sidebar_nav li > a[data-active=\"true\"]");
        assert_css_exists(
            &html,
            "#sidebar_nav li > a[aria-disabled=\"true\"]:not([href])",
        );
        assert_css_exists(&html, "#sidebar_nav li > ul > li > a");
        assert!(html.contains("segera"));
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
