use agentgate_server::app::App;
use loco_rs::testing::prelude::*;

const CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'";

#[tokio::test]
async fn pages_carry_strict_csp_and_request_id() {
    request::<App, _, _>(|request, _ctx| async move {
        let res = request.get("/").await;
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
