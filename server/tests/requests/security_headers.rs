use agentgate_server::app::App;
use loco_rs::testing::prelude::*;

const CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; object-src 'none'; base-uri 'self'; form-action 'self' https://accounts.google.com https://github.com; frame-ancestors 'none'";

#[tokio::test]
async fn pages_carry_strict_csp_and_request_id() {
    request::<App, _, _>(|request, _ctx| async move {
        let res = request.get("/login").await;
        assert_eq!(res.header("content-security-policy").to_str().unwrap(), CSP);
        assert!(!res.header("x-request-id").is_empty());
    })
    .await;
}

#[tokio::test]
async fn error_pages_keep_security_headers() {
    request::<App, _, _>(|request, _ctx| async move {
        let res = request.get("/tidak-ada").await;
        assert_eq!(res.header("content-security-policy").to_str().unwrap(), CSP);
    })
    .await;
}

/// M12: header keamanan lengkap di setiap halaman (HSTS, nosniff, referrer, permissions, frame).
#[tokio::test]
async fn pages_carry_hardening_headers() {
    request::<App, _, _>(|request, _ctx| async move {
        for path in ["/login", "/tidak-ada"] {
            let res = request.get(path).await;
            let h = |n: &str| res.header(n).to_str().unwrap().to_string();
            assert!(
                h("strict-transport-security").starts_with("max-age="),
                "{path}"
            );
            assert_eq!(h("x-content-type-options"), "nosniff", "{path}");
            // Halaman bertoken (login, undangan) memasang `no-referrer` sendiri.
            let referrer = h("referrer-policy");
            assert!(
                referrer == "same-origin" || referrer == "no-referrer",
                "{path}: {referrer}"
            );
            assert!(h("permissions-policy").contains("camera=()"), "{path}");
            assert_eq!(h("x-frame-options"), "sameorigin", "{path}");
        }
    })
    .await;
}
