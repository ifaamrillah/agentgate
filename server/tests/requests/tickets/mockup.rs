//! Pratinjau mockup HTML (M10): halaman mockup membingkai domain pratinjau terpisah, domain
//! pratinjau hanya melayani `/preview/{token}` bertanda tangan (CSP sandbox, tanpa cookie), host
//! aplikasi tidak melayani `/preview`, dan approve/revisi dari halaman mockup kembali ke halaman itu.

use agentgate_server::{
    app::App,
    common::settings::Settings,
    models::{artifact_versions, ticket_artifacts},
    services::mockups,
};
use loco_rs::{app::AppContext, testing::prelude::*};

use super::{
    create_ticket, fixture,
    stages::{FLOW_YAML, use_workflow},
    ticket,
};

const MOCKUP: &str = "<!doctype html><h1>Checkout</h1><script>document.title='x'</script>";

fn settings(ctx: &AppContext) -> Settings {
    Settings::from_ctx(ctx).unwrap()
}

#[tokio::test]
async fn mockup_runs_only_on_the_preview_domain() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let flow = FLOW_YAML
            .replace("output: prd.md", "output: checkout.html")
            .replace("# PRD", "<h1>Checkout</h1>");
        use_workflow(&request, &ctx, &f, &flow).await;
        let code = create_ticket(&request, &f.cookie, &[("title", "Checkout QRIS")]).await;
        for (path, form) in [
            ("/w/acme/t/KSR-1/stage/complete", vec![]),
            ("/w/acme/t/KSR-1/artifacts", vec![("name", "")]),
        ] {
            let res = request
                .post(path)
                .add_cookie(f.cookie.clone())
                .form(&form)
                .await;
            assert_eq!(res.status_code(), 303, "{path}: {}", res.text());
        }
        let wid = f.workspace.workspace_id();
        let t = ticket(&ctx, &f.workspace, &code).await;
        let art = ticket_artifacts::Model::find_named(&ctx.db, wid, t.id, "checkout.html")
            .await
            .unwrap()
            .expect("mockup dibuat");
        let res = request
            .post(&format!("/w/acme/t/KSR-1/artifacts/{}", art.id))
            .add_cookie(f.cookie.clone())
            .form(&[("body", MOCKUP)])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let res = request
            .post("/w/acme/t/KSR-1/stage/complete")
            .add_cookie(f.cookie.clone())
            .form(&Vec::<(&str, &str)>::new())
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let latest = artifact_versions::Model::latest(&ctx.db, wid, art.id)
            .await
            .unwrap()
            .unwrap();

        // Halaman artifact menautkan pratinjau mockup.
        let page = format!("/w/acme/t/KSR-1/mockup/{}", art.id);
        let res = request
            .get("/w/acme/t/KSR-1/artifacts")
            .add_cookie(f.cookie.clone())
            .await;
        assert!(res.text().contains(&page), "tautan mockup");

        // Halaman mockup: iframe sandbox ke domain pratinjau, CSP mengizinkan frame-src itu saja.
        let s = settings(&ctx);
        let res = request.get(&page).add_cookie(f.cookie.clone()).await;
        assert_eq!(res.status_code(), 200, "{}", res.text());
        let csp = res.header("content-security-policy");
        let csp = csp.to_str().unwrap();
        assert!(
            csp.ends_with(&format!("frame-src {}", s.preview_base_url)),
            "{csp}"
        );
        let html = res.text();
        assert_css_exists(&html, "iframe[data-mockup-frame][sandbox='allow-scripts']");
        assert!(
            !html.contains("<h1>Checkout</h1><script>"),
            "isi mockup tidak di origin aplikasi"
        );
        assert_css_exists(&html, "[data-mockup-approve]");
        let src = html
            .split("data-mockup-frame")
            .next()
            .and_then(|h| h.rsplit("src=\"").next())
            .and_then(|h| h.split('"').next())
            .unwrap()
            .to_string();
        let token_path = src
            .strip_prefix(&s.preview_base_url)
            .unwrap_or_else(|| panic!("src ke domain pratinjau: {src}"))
            .to_string();
        assert!(token_path.starts_with("/preview/"));

        // Mode mobile dan bandingkan tetap 200.
        for q in ["?dev=mobile", "?tab=compare", "?v=1"] {
            let res = request
                .get(&format!("{page}{q}"))
                .add_cookie(f.cookie.clone())
                .await;
            assert_eq!(res.status_code(), 200, "{q}");
        }

        // Domain pratinjau: isi mockup + CSP sandbox, tanpa cookie/X-Frame-Options.
        let host = s.preview_host();
        let res = request
            .get(&token_path)
            .add_header("host", host.clone())
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 200);
        assert_eq!(res.text(), MOCKUP);
        let pcsp = res.header("content-security-policy");
        let pcsp = pcsp.to_str().unwrap().to_string();
        assert!(pcsp.starts_with("sandbox allow-scripts"), "{pcsp}");
        assert!(!pcsp.contains("allow-same-origin"));
        assert!(res.maybe_header("set-cookie").is_none());
        assert!(res.maybe_header("x-frame-options").is_none());

        // Path lain di domain pratinjau 404 (tidak ada login/halaman aplikasi di sana).
        for path in ["/login", "/w/acme", "/preview", "/preview/a/b"] {
            let res = request.get(path).add_header("host", host.clone()).await;
            assert_eq!(res.status_code(), 404, "{path}");
        }
        let res = request
            .post(&token_path)
            .add_header("host", host.clone())
            .form(&Vec::<(&str, &str)>::new())
            .await;
        assert_eq!(res.status_code(), 404, "POST di domain pratinjau");

        // Token dirusak, kedaluwarsa, atau di host aplikasi → 404.
        let key = mockups::signing_key(&s.auth.encryption_key_bytes().unwrap());
        let expired = mockups::sign(&key, latest.id, chrono::Utc::now().timestamp() - 5);
        let forged = format!("{}x", &token_path);
        for path in [format!("/preview/{expired}"), forged] {
            let res = request.get(&path).add_header("host", host.clone()).await;
            assert_eq!(res.status_code(), 404, "{path}");
        }
        let res = request.get(&token_path).add_cookie(f.cookie.clone()).await;
        assert_eq!(
            res.status_code(),
            404,
            "host aplikasi tidak melayani /preview"
        );

        // Approve dari halaman mockup kembali ke halaman mockup; return_to asing diabaikan.
        let res = request
            .post("/w/acme/t/KSR-1/stage/approve")
            .add_cookie(f.cookie.clone())
            .form(&[("note", ""), ("return_to", &format!("{page}?v=2"))])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(
            res.header("location").to_str().unwrap(),
            format!("{page}?v=2")
        );
    })
    .await;
}

#[tokio::test]
async fn unsafe_return_to_is_ignored_and_non_html_is_not_a_mockup() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        use_workflow(&request, &ctx, &f, FLOW_YAML).await;
        let code = create_ticket(&request, &f.cookie, &[("title", "PRD")]).await;
        for (path, form) in [
            ("/w/acme/t/KSR-1/stage/complete", vec![]),
            ("/w/acme/t/KSR-1/artifacts", vec![("name", "")]),
            ("/w/acme/t/KSR-1/stage/complete", vec![]),
        ] {
            let res = request
                .post(path)
                .add_cookie(f.cookie.clone())
                .form(&form)
                .await;
            assert_eq!(res.status_code(), 303, "{path}: {}", res.text());
        }
        let t = ticket(&ctx, &f.workspace, &code).await;
        let prd = ticket_artifacts::Model::find_named(
            &ctx.db,
            f.workspace.workspace_id(),
            t.id,
            "prd.md",
        )
        .await
        .unwrap()
        .unwrap();
        let res = request
            .get(&format!("/w/acme/t/KSR-1/mockup/{}", prd.id))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 404, "prd.md bukan mockup");

        let res = request
            .post("/w/acme/t/KSR-1/stage/reject")
            .add_cookie(f.cookie.clone())
            .form(&[
                ("note", "Kurang"),
                ("return_to", "https://evil.example/w/acme/x"),
            ])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let to = res.header("location").to_str().unwrap().to_string();
        assert!(to.starts_with("/w/acme/t/KSR-1"), "{to}");
        assert!(!to.contains("evil"));
    })
    .await;
}
