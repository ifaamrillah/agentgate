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
            "danger",
            "link",
        ] {
            assert_css_exists(&html, &format!("#buttons .btn[data-variant=\"{variant}\"]"));
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
        for kind in ["info", "success", "warn", "error"] {
            assert_css_exists(&html, &format!("#feedback [data-flash=\"{kind}\"]"));
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
        // Shell: breadcrumb, pbar dengan tab tampilan project, kepala halaman.
        assert_css_exists(&html, ".app-topbar [data-crumbs] a");
        assert_css_exists(&html, ".app-topbar [data-crumbs] [aria-current=\"page\"]");
        assert_css_eq(&html, ".app-page-head h1", "Katalog UI");
        assert_css_exists(&html, ".app-page-acts .btn");
        assert_css_exists(
            &html,
            ".app-pbar [data-tabs] a[data-nav=\"overview\"][aria-current=\"page\"]",
        );
        assert_css_exists(&html, ".app-pbar [data-tabs] a[data-nav=\"board\"][href]");
        assert_css_exists(
            &html,
            "dialog#views-dialog form [data-view-row] input.app-switch[name=\"view\"]",
        );
        // Komponen tiket M2.
        assert_css_exists(
            &html,
            "#tickets article.app-tcard[data-ticket=\"KSR-12\"] .app-prio[data-p=\"2\"]",
        );
        assert_css_exists(&html, "#tickets .app-type[data-tile]");
        assert_css_exists(&html, "#workflow [data-stepper] [aria-current=\"step\"]");
        assert_css_exists(&html, "#workflow [data-gates] [data-ok=\"false\"]");
        assert_css_exists(&html, "#workflow .app-tcard-state");
        assert_css_exists(&html, "#tickets .app-status-pill[data-status=\"done\"]");
        // subpage E: item aktif dari data.
        assert_css_exists(
            &html,
            "#layout [data-subnav] a[data-nav=\"password\"][aria-current=\"page\"]",
        );
        // card: tautan "lihat semua", varian, form dengan savebar.
        assert_css_exists(&html, "#lists .app-card .app-seeall");
        assert_css_exists(&html, ".app-card[data-variant=\"danger\"]");
        assert_css_exists(&html, ".app-card[data-variant=\"accent\"]");
        assert_css_exists(&html, "#form form .app-savebar [data-save]");
        // stats: empat kartu, salah satu tautan.
        assert_count(&html, "#stats [data-stat]", 4);
        // datatable: kolom sort, kolom tersembunyi, footer total, aksi per baris.
        assert_css_exists(&html, "#catalog-table thead a.app-dt-sort[data-on]");
        assert_css_exists(&html, "#catalog-table th[data-hide=\"md\"]");
        assert_count(&html, "#catalog-table tbody tr", 10);
        assert_css_exists(&html, "#catalog-table .app-dt-pg a[aria-current=\"page\"]");
        assert_css_exists(
            &html,
            "#catalog-table td[data-actions] .popover [data-popover] form",
        );
        // combo, color_picker, uploader.
        assert_css_exists(&html, "#combo [data-combo] select#catalog-person");
        assert_css_exists(&html, "#combo [data-combo] select[multiple]");
        assert_count(
            &html,
            "#catalog-color input[type=\"radio\"][name=\"color\"]",
            19,
        );
        assert_css_exists(
            &html,
            "form[enctype=\"multipart/form-data\"] input[type=\"file\"]",
        );
        // dialog, setting_row, avatar.
        assert_css_exists(&html, "dialog.dialog#catalog-dialog form footer");
        // md_editor: textarea tetap nilai form; footer Batal + Simpan.
        assert_css_exists(&html, "#editor [data-md-editor] textarea[name=\"body\"]");
        assert_css_exists(&html, "#editor form .app-editor-foot [data-md-cancel]");
        assert_count(&html, "#form .app-srow", 2);
        for size in ["sm", "md", "lg", "xl"] {
            assert_css_exists(&html, &format!(".app-av[data-size=\"{size}\"]"));
        }
    })
    .await;
}

#[tokio::test]
async fn component_catalog_table_sorts_filters_and_pages() {
    request::<App, _, _>(|request, _ctx| async move {
        // Halaman 2 dari 12 baris per 10 = 2 baris.
        let html = request.get("/_dev/ui?page=2").await.text();
        assert_count(&html, "#catalog-table tbody tr", 2);
        assert_css_exists(&html, "#catalog-table .app-dt-foot");
        assert!(html.contains("11–12 dari 12"), "{html}");
        // Filter role: chip aktif dengan tautan lepas, opsi tercentang.
        let html = request.get("/_dev/ui?role=admin").await.text();
        assert_count(&html, "#catalog-table tbody tr", 2);
        assert_css_exists(&html, "#catalog-table .app-fpill a[href=\"/_dev/ui\"]");
        assert_css_exists(
            &html,
            "#catalog-table input[name=\"role\"][value=\"admin\"][checked]",
        );
        // Sort nama menurun: baris pertama Lina.
        let html = request.get("/_dev/ui?sort=name&dir=desc").await.text();
        let first = select(&html, "#catalog-table tbody tr td")
            .first()
            .cloned()
            .unwrap_or_default();
        assert!(first.contains("Lina Marlina"), "{first}");
        // Cari tanpa hasil: empty state "tidak ada yang cocok".
        let html = request.get("/_dev/ui?q=zzz").await.text();
        assert_css_exists(&html, "#catalog-table .app-dt-empty .app-empty");
    })
    .await;
}

#[tokio::test]
async fn component_catalog_uses_app_shell_with_sample_workspaces() {
    request::<App, _, _>(|request, _ctx| async move {
        let html = request.get("/_dev/ui").await.text();
        assert_css_exists(&html, "aside.app-side#app-side");
        assert_css_exists(
            &html,
            ".app-topbar button[data-nav-toggle][aria-controls=\"app-side\"]",
        );
        assert!(select(&html, "[data-workspace-item]").len() >= 2);
        assert_css_exists(
            &html,
            "[data-project-list] a.app-ni[data-nav=\"project-WEB\"] .app-dot",
        );
        assert_css_exists(
            &html,
            "[data-project-list] a[data-create-project][href=\"/w/acme/projects/new\"]",
        );
    })
    .await;
}
