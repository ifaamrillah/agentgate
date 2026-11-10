use agentgate_server::app::App;
use std::sync::{
    Arc,
    atomic::{AtomicU16, Ordering},
};

use axum::http::Method;
use loco_rs::testing::prelude::*;

async fn status(method: Method, headers: &[(&'static str, &'static str)]) -> u16 {
    let headers: Vec<_> = headers.to_vec();
    let code = Arc::new(AtomicU16::new(0));
    let out = code.clone();
    request::<App, _, _>(|request, _ctx| async move {
        let mut req = request.method(method, "/_dev/echo");
        for (k, v) in headers {
            req = req.add_header(k, v);
        }
        out.store(req.await.status_code().as_u16(), Ordering::SeqCst);
    })
    .await;
    code.load(Ordering::SeqCst)
}

#[tokio::test]
async fn cross_site_post_is_rejected() {
    assert_eq!(
        status(Method::POST, &[("sec-fetch-site", "cross-site")]).await,
        403
    );
}

#[tokio::test]
async fn same_site_post_is_rejected() {
    assert_eq!(
        status(Method::POST, &[("sec-fetch-site", "same-site")]).await,
        403
    );
}

#[tokio::test]
async fn foreign_origin_without_fetch_metadata_is_rejected() {
    assert_eq!(
        status(Method::POST, &[("origin", "http://evil.test")]).await,
        403
    );
}

#[tokio::test]
async fn cross_site_delete_is_rejected() {
    assert_eq!(
        status(Method::DELETE, &[("sec-fetch-site", "cross-site")]).await,
        403
    );
}

#[tokio::test]
async fn same_origin_post_is_allowed() {
    assert_eq!(
        status(Method::POST, &[("sec-fetch-site", "same-origin")]).await,
        200
    );
}

#[tokio::test]
async fn non_browser_post_without_headers_is_allowed() {
    assert_eq!(status(Method::POST, &[]).await, 200);
}

#[tokio::test]
async fn rejection_is_a_full_html_page() {
    request::<App, _, _>(|request, _ctx| async move {
        let res = request
            .post("/_dev/echo")
            .add_header("sec-fetch-site", "cross-site")
            .await;
        assert_eq!(res.status_code(), 403);
        assert!(res.text().contains("Akses ditolak"));
    })
    .await;
}
