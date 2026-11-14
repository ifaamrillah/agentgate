//! Logo dan warna project: `POST …/settings/logo` (multipart, PNG/JPEG/WebP/SVG tersanitasi
//! ≤ 1 MB), `POST …/settings/logo/delete`, `GET …/logo` (butuh akses project, selain itu 404).

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

        // SVG rusak / root bukan svg ditolak (tipe diperiksa dari isi, bukan nama/mime).
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg"><g><script>alert(1)</script></svg>"#;
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

/// R1: logo SVG disimpan setelah sanitasi (tanpa skrip, handler, foreignObject, href/use
/// eksternal) dan disajikan sebagai image/svg+xml dengan CSP ketat + nosniff (uji XSS).
#[tokio::test]
async fn svg_logo_is_sanitized_and_served_with_strict_csp() {
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
        let payload = br##"<?xml version="1.0"?>
<!DOCTYPE svg [<!ENTITY xxe SYSTEM "file:///etc/passwd">]>
<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 24 24" onload="alert(document.cookie)">
  <script type="text/javascript">alert('xss')</script>
  <foreignObject width="10" height="10"><body xmlns="http://www.w3.org/1999/xhtml"><iframe src="javascript:alert(1)"></iframe></body></foreignObject>
  <a xlink:href="javascript:alert(2)"><rect width="5" height="5"/></a>
  <use xlink:href="https://evil.test/sprite.svg#icon"/>
  <set attributeName="onmouseover" to="alert(3)"/>
  <circle cx="12" cy="12" r="10" fill="#5b5bd6" onmouseover="alert(4)" style="fill:url(https://evil.test/x)"/>
  <text>&xxe;</text>
</svg>"##;
        let res = request
            .post("/w/acme/p/KSR/settings/logo")
            .add_cookie(cookie.clone())
            .multipart(file(payload, "logo.svg", "image/svg+xml"))
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let project = projects::Model::find_by_key(&ctx.db, wid, "KSR").await.unwrap().unwrap();
        assert_eq!(project.logo().unwrap().content_type, "image/svg+xml");

        let res = request.get("/w/acme/p/KSR/logo").add_cookie(cookie.clone()).await;
        assert_eq!(res.status_code(), 200);
        assert_eq!(res.header("content-type"), "image/svg+xml");
        assert_eq!(res.header("x-content-type-options"), "nosniff");
        assert_eq!(
            res.header("content-security-policy"),
            "default-src 'none'; style-src 'unsafe-inline'"
        );
        let body = res.text();
        for bad in [
            "script", "onload", "onmouseover", "foreignObject", "iframe", "javascript", "evil.test",
            "<set", "xxe", "passwd", "DOCTYPE", "<a ",
        ] {
            assert!(!body.contains(bad), "{bad} lolos: {body}");
        }
        assert!(body.contains(r##"<circle cx="12" cy="12" r="10" fill="#5b5bd6"/>"##), "{body}");
        assert!(body.starts_with("<svg"), "{body}");
    })
    .await;
}
