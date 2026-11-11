//! Proteksi CSRF berbasis asal request (pola `http.CrossOriginProtection` Go), tanpa token per form.
//!
//! Request dengan method selain GET/HEAD/OPTIONS ditolak 403 bila:
//! - `Sec-Fetch-Site` ada dan nilainya bukan `same-origin` atau `none` (termasuk `same-site`:
//!   SameSite menganggap semua subdomain satu situs, jadi aplikasi lain di subdomain yang
//!   punya celah XSS bisa mengirim POST lengkap dengan cookie sesi); atau
//! - `Sec-Fetch-Site` tidak ada, tetapi `Origin` ada dan host-nya bukan host request.
//!
//! Request tanpa kedua header (klien non-browser: CLI, runner, webhook) diloloskan; endpoint
//! itu diautentikasi dengan token, bukan cookie.

use axum::{
    extract::Request,
    http::{HeaderMap, Method, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};

/// Path yang dikecualikan dari cek CSRF. Setiap entri wajib punya komentar alasan.
/// Dimulai kosong.
pub const EXEMPT_PATHS: &[&str] = &[];

pub async fn layer(req: Request, next: Next) -> Response {
    if is_allowed(req.method(), req.uri().path(), req.headers()) {
        next.run(req).await
    } else {
        tracing::warn!(
            path = %super::request_log::redacted_path(req.uri().path()),
            "cross-origin request rejected"
        );
        StatusCode::FORBIDDEN.into_response()
    }
}

/// Satu-satunya implementasi aturan CSRF.
#[must_use]
pub fn is_allowed(method: &Method, path: &str, headers: &HeaderMap) -> bool {
    if matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS) {
        return true;
    }
    if EXEMPT_PATHS.contains(&path) {
        return true;
    }
    if let Some(site) = headers.get("sec-fetch-site") {
        return matches!(site.to_str(), Ok("same-origin" | "none"));
    }
    match headers.get(header::ORIGIN) {
        None => true,
        Some(origin) => {
            let host = headers.get(header::HOST).and_then(|h| h.to_str().ok());
            match (origin.to_str().ok().and_then(origin_host), host) {
                (Some(origin_host), Some(host)) => origin_host.eq_ignore_ascii_case(host),
                _ => false,
            }
        }
    }
}

/// `https://example.com:8443` → `example.com:8443`. `null` dan nilai rusak → `None`.
fn origin_host(origin: &str) -> Option<&str> {
    let rest = origin
        .strip_prefix("https://")
        .or_else(|| origin.strip_prefix("http://"))?;
    let host = rest.split('/').next()?;
    (!host.is_empty()).then_some(host)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
        let mut m = HeaderMap::new();
        for (k, v) in pairs {
            m.insert(*k, v.parse().unwrap());
        }
        m
    }

    #[test]
    fn origin_null_is_rejected() {
        let headers = h(&[("origin", "null"), ("host", "app.test")]);
        assert!(!is_allowed(&Method::POST, "/x", &headers));
    }

    #[test]
    fn same_origin_by_origin_header_is_allowed() {
        let headers = h(&[
            ("origin", "http://app.test:5150"),
            ("host", "app.test:5150"),
        ]);
        assert!(is_allowed(&Method::POST, "/x", &headers));
    }

    #[test]
    fn origin_with_different_port_is_rejected() {
        let headers = h(&[
            ("origin", "http://app.test:9999"),
            ("host", "app.test:5150"),
        ]);
        assert!(!is_allowed(&Method::POST, "/x", &headers));
    }
}
