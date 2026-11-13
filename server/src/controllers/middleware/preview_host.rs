//! Pemisah domain pratinjau mockup (M10). Request ber-`Host` domain pratinjau hanya boleh
//! `GET /preview/{token}`; path lain di host itu 404 (halaman aplikasi, form login, dan cookie
//! sesi tidak pernah ada di origin pratinjau). Sebaliknya host aplikasi tidak melayani
//! `/preview/*`. Respons pratinjau dibersihkan dari `Set-Cookie` dan `X-Frame-Options` (pembingkaian
//! diatur `frame-ancestors` di CSP handler).
use std::sync::Arc;

use axum::{
    extract::{Request, State},
    http::{Method, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};

fn host(req: &Request) -> String {
    req.headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .map(str::to_string)
        .or_else(|| req.uri().authority().map(ToString::to_string))
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn is_preview_path(path: &str) -> bool {
    path == "/preview" || path.starts_with("/preview/")
}

/// Path pratinjau yang sah: `/preview/{token}` satu segmen.
#[must_use]
pub fn is_token_path(path: &str) -> bool {
    path.strip_prefix("/preview/")
        .is_some_and(|t| !t.is_empty() && !t.contains('/'))
}

pub async fn layer(State(preview_host): State<Arc<String>>, req: Request, next: Next) -> Response {
    let on_preview = host(&req) == *preview_host;
    let path = req.uri().path().to_string();
    if !on_preview {
        if is_preview_path(&path) {
            return StatusCode::NOT_FOUND.into_response();
        }
        return next.run(req).await;
    }
    if !matches!(*req.method(), Method::GET | Method::HEAD) || !is_token_path(&path) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let mut res = next.run(req).await;
    let headers = res.headers_mut();
    headers.remove(header::SET_COOKIE);
    headers.remove(header::X_FRAME_OPTIONS);
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_single_segment_tokens_are_preview_paths() {
        assert!(is_token_path("/preview/abc.1.sig"));
        assert!(!is_token_path("/preview/"));
        assert!(!is_token_path("/preview/a/b"));
        assert!(!is_token_path("/login"));
        assert!(is_preview_path("/preview"));
    }
}
