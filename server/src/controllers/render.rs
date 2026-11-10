//! Satu tempat yang memutuskan halaman lengkap atau partial untuk request HTMX.
//!
//! Request boosted (navigasi link/form) juga membawa `HX-Request`, padahal butuh halaman
//! lengkap; pembedanya `HX-Request-Type`: `partial` → partial, selain itu → halaman lengkap.

use axum::http::HeaderMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderMode {
    Full,
    Partial,
}

impl RenderMode {
    #[must_use]
    pub fn from_headers(headers: &HeaderMap) -> Self {
        match headers.get("hx-request-type").and_then(|v| v.to_str().ok()) {
            Some("partial") => Self::Partial,
            _ => Self::Full,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.insert(*k, v.parse().unwrap());
        }
        h
    }

    #[test]
    fn plain_request_renders_full_page() {
        assert_eq!(RenderMode::from_headers(&headers(&[])), RenderMode::Full);
    }

    #[test]
    fn boosted_request_renders_full_page() {
        let h = headers(&[("hx-request", "true"), ("hx-request-type", "full")]);
        assert_eq!(RenderMode::from_headers(&h), RenderMode::Full);
    }

    #[test]
    fn partial_request_renders_partial() {
        let h = headers(&[("hx-request", "true"), ("hx-request-type", "partial")]);
        assert_eq!(RenderMode::from_headers(&h), RenderMode::Partial);
    }
}
