//! Respons error sebagai halaman HTML lengkap.
//!
//! HTMX 4 men-swap semua respons kecuali 204/304, termasuk error. Error bawaan Loco berupa
//! JSON, dan fallback Axum berupa body kosong; keduanya tampil rusak bila di-swap ke halaman.
//! Layer ini mengganti setiap respons >= 400 yang bukan HTML dengan halaman error ber-layout.
//! Respons yang sudah HTML (mis. form 422 dengan pesan validasi) dibiarkan.
//!
//! Path API (`api_routes::is_api`: MCP, OAuth, API publik) tidak pernah diubah menjadi HTML:
//! kliennya bukan browser. Error di path itu yang belum JSON diberi body `{"error": "..."}`.

use axum::{
    extract::{Request, State},
    http::{HeaderValue, StatusCode, header},
    middleware::Next,
    response::{Html, IntoResponse, Response},
};
use loco_rs::controller::views::engines::TeraView;

use crate::{controllers::api_routes, views};

pub async fn not_found() -> StatusCode {
    StatusCode::NOT_FOUND
}

pub async fn layer(State(view): State<TeraView>, req: Request, next: Next) -> Response {
    let api = api_routes::is_api(req.uri().path());
    let res = next.run(req).await;
    let status = res.status();
    if !(status.is_client_error() || status.is_server_error()) {
        return res;
    }
    if api {
        return if is_json(&res) { res } else { json_error(res) };
    }
    if is_html(&res) {
        return res;
    }
    let request_id = res
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let mut page = match views::errors::page(&view, status, &request_id) {
        Ok(html) => (status, Html(html)).into_response(),
        Err(err) => {
            tracing::error!(error = %err, "failed to render error page");
            (status, Html(views::errors::FALLBACK_HTML)).into_response()
        }
    };
    // Header keamanan, request id, dan cookie dari respons asli tetap dibawa. `append`, bukan
    // `insert`: header bernilai banyak (Set-Cookie) tidak boleh menyusut jadi satu. Header yang
    // menggambarkan body lama tidak dibawa karena body-nya sudah diganti.
    for (name, value) in res.headers() {
        if !describes_body(name) {
            page.headers_mut().append(name.clone(), value.clone());
        }
    }
    page.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    page
}

/// Kode error JSON untuk status tanpa body JSON (nama mengikuti gaya RFC 6749).
fn error_code(status: StatusCode) -> &'static str {
    match status {
        StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY => "invalid_request",
        StatusCode::UNAUTHORIZED => "unauthorized",
        StatusCode::FORBIDDEN => "forbidden",
        StatusCode::NOT_FOUND => "not_found",
        StatusCode::METHOD_NOT_ALLOWED => "method_not_allowed",
        StatusCode::PAYLOAD_TOO_LARGE => "payload_too_large",
        StatusCode::UNSUPPORTED_MEDIA_TYPE => "unsupported_media_type",
        StatusCode::TOO_MANY_REQUESTS => "too_many_requests",
        s if s.is_server_error() => "server_error",
        _ => "invalid_request",
    }
}

fn json_error(res: Response) -> Response {
    let status = res.status();
    let body = serde_json::json!({ "error": error_code(status) }).to_string();
    let mut out = (status, body).into_response();
    for (name, value) in res.headers() {
        if !describes_body(name) {
            out.headers_mut().append(name.clone(), value.clone());
        }
    }
    out.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    out
}

fn is_json(res: &Response) -> bool {
    res.headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("application/json"))
}

fn describes_body(name: &header::HeaderName) -> bool {
    [
        header::CONTENT_TYPE,
        header::CONTENT_LENGTH,
        header::CONTENT_ENCODING,
        header::CONTENT_RANGE,
        header::TRANSFER_ENCODING,
        header::ETAG,
    ]
    .contains(name)
}

fn is_html(res: &Response) -> bool {
    res.headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("text/html"))
}
