//! Respons error sebagai halaman HTML lengkap.
//!
//! HTMX 4 men-swap semua respons kecuali 204/304, termasuk error. Error bawaan Loco berupa
//! JSON, dan fallback Axum berupa body kosong; keduanya tampil rusak bila di-swap ke halaman.
//! Layer ini mengganti setiap respons >= 400 yang bukan HTML dengan halaman error ber-layout.
//! Respons yang sudah HTML (mis. form 422 dengan pesan validasi) dibiarkan.

use axum::{
    extract::{Request, State},
    http::{HeaderValue, StatusCode, header},
    middleware::Next,
    response::{Html, IntoResponse, Response},
};
use loco_rs::controller::views::engines::TeraView;

use crate::views;

pub async fn not_found() -> StatusCode {
    StatusCode::NOT_FOUND
}

pub async fn layer(State(view): State<TeraView>, req: Request, next: Next) -> Response {
    let res = next.run(req).await;
    let status = res.status();
    if !(status.is_client_error() || status.is_server_error()) || is_html(&res) {
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
