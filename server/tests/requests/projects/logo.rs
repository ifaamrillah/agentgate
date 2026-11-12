//! Logo dan warna project: `POST …/settings/logo` (multipart, PNG/JPEG/WebP ≤ 1 MB, SVG
//! ditolak), `POST …/settings/logo/delete`, `GET …/logo` (butuh akses project, selain itu 404).

use agentgate_server::{
    app::App,
    models::{audit_events::AuditAction, projects, roles::RoleKey},
};
use axum_test::multipart::{MultipartForm, Part};
use loco_rs::testing::prelude::*;

use crate::support::{a_user, a_workspace, add_member, sign_in_as};

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01\x08\x06\0\0\0";

fn file(bytes: &[u8], name: &str, mime: &str) -> MultipartForm {
    MultipartForm::new().add_part(
        "file",
        Part::bytes(bytes.to_vec()).file_name(name).mime_type(mime),
    )
}

#[tokio::test]
async fn logo_upload_serve_replace_and_delete() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let outsider = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let wid = workspace.workspace_id();
        add_member(&ctx, &workspace, &outsider, RoleKey::Member).await;
        let cookie = sign_in_as(&request, &owner).await;
        let res = request
            .post("/w/acme/projects/new")
            .add_cookie(cookie.clone())
            .form(&[("key", "KSR"), ("name", "Kasir"), ("description", "")])
            .await;
        assert_eq!(res.status_code(), 303);

        // Belum ada logo.
        let res = request
            .get("/w/acme/p/KSR/logo")
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(res.status_code(), 404);

        let res = request
            .post("/w/acme/p/KSR/settings/logo")
            .add_cookie(cookie.clone())
            .multipart(file(PNG, "logo.png", "image/png"))
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(res.header("location"), "/w/acme/p/KSR/settings");
        let project = projects::Model::find_by_key(&ctx.db, wid, "KSR")
            .await
            .unwrap()
            .unwrap();
        let logo = project.logo().expect("logo tersimpan");
        assert_eq!(logo.content_type, "image/png");
        assert_eq!(logo.size, i32::try_from(PNG.len()).unwrap());

        let res = request
            .get("/w/acme/p/KSR/logo")
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(res.status_code(), 200);
        assert_eq!(res.header("content-type"), "image/png");
        assert!(
            res.header("cache-control")
                .to_str()
                .unwrap()
                .starts_with("private")
        );
        assert_eq!(res.as_bytes().as_ref(), PNG);

        // Halaman memakai URL logo berversi.
        let page = request
            .get("/w/acme/p/KSR/settings")
            .add_cookie(cookie.clone())
            .await;
        assert_css_exists(
            &page.text(),
            "[data-project-logo] img[src^=\"/w/acme/p/KSR/logo?v=\"]",
        );

        // Anggota workspace tanpa akses ke project: 404.
        let other = sign_in_as(&request, &outsider).await;
        let res = request
            .get("/w/acme/p/KSR/logo")
            .add_cookie(other.clone())
            .await;
        assert_eq!(res.status_code(), 404);
        let res = request
            .post("/w/acme/p/KSR/settings/logo")
            .add_cookie(other.clone())
            .multipart(file(PNG, "logo.png", "image/png"))
            .await;
        assert_eq!(res.status_code(), 404);
        let res = request
            .post("/w/acme/p/KSR/settings/logo/delete")
            .add_cookie(other)
            .await;
        assert_eq!(res.status_code(), 404);

        let res = request
            .post("/w/acme/p/KSR/settings/logo/delete")
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let project = projects::Model::find_by_key(&ctx.db, wid, "KSR")
            .await
            .unwrap()
            .unwrap();
        assert!(project.logo().is_none());
        let res = request.get("/w/acme/p/KSR/logo").add_cookie(cookie).await;
        assert_eq!(res.status_code(), 404);
        assert!(
            ctx.storage
                .download::<Vec<u8>>(std::path::Path::new(&logo.key))
                .await
                .is_err()
        );

        let events = crate::support::newest_audit_events(&ctx.db, wid, 10)
            .await
            .unwrap();
        assert_eq!(events[0].action().unwrap(), AuditAction::ProjectUpdated);
        assert!(events[0].changes["logo"]["to"].is_null());
        assert_eq!(events[1].changes["logo"]["to"], "image/png");
    })
    .await;
}

#[tokio::test]
async fn logo_rejects_wrong_type_and_too_large() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let wid = workspace.workspace_id();
        let cookie = sign_in_as(&request, &owner).await;
        request
            .post("/w/acme/projects/new")
            .add_cookie(cookie.clone())
            .form(&[("key", "KSR"), ("name", "Kasir"), ("description", "")])
            .await;

        // SVG ditolak walau diklaim PNG: tipe diperiksa dari isi.
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>"#;
        let res = request
            .post("/w/acme/p/KSR/settings/logo")
            .add_cookie(cookie.clone())
            .multipart(file(svg, "logo.png", "image/png"))
            .await;
        assert_eq!(res.status_code(), 422);
        assert_css_exists(&res.text(), "[data-project-logo] .app-field-error");

        let mut big = PNG.to_vec();
        big.resize(1024 * 1024 + 1, 0);
        let res = request
            .post("/w/acme/p/KSR/settings/logo")
            .add_cookie(cookie.clone())
            .multipart(file(&big, "logo.png", "image/png"))
            .await;
        assert_eq!(res.status_code(), 422);
        assert_css_exists(&res.text(), "[data-project-logo] .app-field-error");

        let res = request
            .post("/w/acme/p/KSR/settings/logo")
            .add_cookie(cookie)
            .multipart(MultipartForm::new().add_text("other", "x"))
            .await;
        assert_eq!(res.status_code(), 422);

        let project = projects::Model::find_by_key(&ctx.db, wid, "KSR")
            .await
            .unwrap()
            .unwrap();
        assert!(project.logo().is_none());
    })
    .await;
}

#[tokio::test]
async fn project_color_validated_and_saved() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let wid = workspace.workspace_id();
        let cookie = sign_in_as(&request, &owner).await;

        let res = request
            .post("/w/acme/projects/new")
            .add_cookie(cookie.clone())
            .form(&[
                ("key", "KSR"),
                ("name", "Kasir"),
                ("description", ""),
                ("color", "red"),
            ])
            .await;
        assert_eq!(res.status_code(), 422);
        assert_css_exists(&res.text(), "#project-color[data-invalid] .app-field-error");

        let res = request
            .post("/w/acme/projects/new")
            .add_cookie(cookie.clone())
            .form(&[
                ("key", "KSR"),
                ("name", "Kasir"),
                ("description", ""),
                ("color", "#0D9488"),
            ])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let project = projects::Model::find_by_key(&ctx.db, wid, "KSR")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(project.color.as_deref(), Some("#0d9488"));

        let res = request
            .post("/w/acme/p/KSR/settings")
            .add_cookie(cookie.clone())
            .form(&[("name", "Kasir"), ("description", ""), ("color", "#12345")])
            .await;
        assert_eq!(res.status_code(), 422);
        assert_css_exists(&res.text(), "#project-color[data-invalid] .app-field-error");

        let res = request
            .post("/w/acme/p/KSR/settings")
            .add_cookie(cookie)
            .form(&[("name", "Kasir"), ("description", ""), ("color", "#7c3aed")])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let project = projects::Model::find_by_key(&ctx.db, wid, "KSR")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(project.color.as_deref(), Some("#7c3aed"));
        let events = crate::support::newest_audit_events(&ctx.db, wid, 1)
            .await
            .unwrap();
        assert_eq!(
            events[0].changes["color"],
            serde_json::json!({ "from": "#0d9488", "to": "#7c3aed" })
        );
    })
    .await;
}
