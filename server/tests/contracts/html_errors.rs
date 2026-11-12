//! Layer html_errors pada router Axum mentah: header yang harus ikut dan yang harus dibuang.

use agentgate_server::controllers::middleware::html_errors;
use axum::{
    Router,
    body::Body,
    http::{HeaderValue, Request, StatusCode, header},
    middleware::from_fn_with_state,
    response::{IntoResponse, Response},
    routing::get,
};
use loco_rs::controller::views::engines::TeraView;
use tower::ServiceExt;

fn view() -> TeraView {
    let dir = std::env::temp_dir().join("agentgate-html-errors-test/views");
    std::fs::create_dir_all(dir.join("errors")).unwrap();
    std::fs::write(
        dir.join("errors/error.html"),
        "<!doctype html><p>{{ status }} {{ key }}</p>",
    )
    .unwrap();
    TeraView::from_custom_dir(&dir, |_| Ok(())).unwrap()
}

async fn json_error_with_cookies() -> Response {
    // Dibangun manual dengan `append`: tuple header Axum memakai `insert` dan akan menyisakan satu cookie.
    let mut res = (StatusCode::NOT_FOUND, "{}").into_response();
    let h = res.headers_mut();
    h.append(header::SET_COOKIE, HeaderValue::from_static("a=1"));
    h.append(header::SET_COOKIE, HeaderValue::from_static("b=2"));
    h.insert(header::CONTENT_ENCODING, HeaderValue::from_static("gzip"));
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    res
}

#[tokio::test]
async fn error_page_keeps_every_cookie_and_drops_body_headers() {
    let app = Router::new()
        .route("/x", get(json_error_with_cookies))
        .layer(from_fn_with_state(view(), html_errors::layer));
    let res = app
        .oneshot(Request::get("/x").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let cookies: Vec<_> = res.headers().get_all(header::SET_COOKIE).iter().collect();
    assert_eq!(cookies.len(), 2);
    assert!(res.headers().get(header::CONTENT_ENCODING).is_none());
    assert_eq!(
        res.headers()[header::CONTENT_TYPE],
        "text/html; charset=utf-8"
    );
}

async fn plain_unauthorized() -> Response {
    let mut res = StatusCode::UNAUTHORIZED.into_response();
    res.headers_mut().insert(
        header::WWW_AUTHENTICATE,
        HeaderValue::from_static("Bearer resource_metadata=\"x\""),
    );
    res
}

async fn json_unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        [(header::CONTENT_TYPE, "application/json")],
        r#"{"error":"invalid_token"}"#,
    )
        .into_response()
}

#[tokio::test]
async fn api_path_errors_become_json_and_keep_headers() {
    let app = Router::new()
        .route("/mcp", get(plain_unauthorized))
        .route("/oauth/token", get(json_unauthorized))
        .layer(from_fn_with_state(view(), html_errors::layer));
    let res = app
        .clone()
        .oneshot(Request::get("/mcp").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(res.headers()[header::CONTENT_TYPE], "application/json");
    assert!(res.headers().get(header::WWW_AUTHENTICATE).is_some());
    let body = axum::body::to_bytes(res.into_body(), 1024).await.unwrap();
    assert_eq!(&body[..], br#"{"error":"unauthorized"}"#);

    // JSON yang sudah ada dibiarkan apa adanya.
    let res = app
        .oneshot(Request::get("/oauth/token").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let body = axum::body::to_bytes(res.into_body(), 1024).await.unwrap();
    assert_eq!(&body[..], br#"{"error":"invalid_token"}"#);
}
