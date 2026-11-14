//! Error tracking (R1, `services::error_tracking`): respons 5xx dilaporkan ke Sentry (hanya
//! metode, pola route, dan status; tanpa query, header, atau body), dan CSP `connect-src`
//! diperluas dengan origin Sentry agar reporter browser bisa mengirim. Tanpa `SENTRY_DSN`
//! layer ini tidak mengubah apa pun.
use std::sync::Arc;

use axum::{
    extract::{MatchedPath, Request, State},
    http::{HeaderValue, header},
    middleware::Next,
    response::Response,
};

use crate::services::error_tracking;

/// Origin Sentry untuk CSP (`None` = Sentry browser mati).
#[derive(Debug, Clone, Default)]
pub struct ErrorReport {
    pub origin: Option<String>,
}

/// Laporkan respons 5xx (dipasang sebagai `route_layer`: pola route tersedia).
pub async fn capture(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let route = req
        .extensions()
        .get::<MatchedPath>()
        .map_or_else(|| "(tanpa route)".to_string(), |p| p.as_str().to_string());
    let res = next.run(req).await;
    if res.status().is_server_error() && error_tracking::enabled() {
        let status = res.status().as_u16();
        sentry::with_scope(
            |scope| {
                scope.set_tag("http.method", method.as_str());
                scope.set_tag("http.route", &route);
                scope.set_tag("http.status", status);
            },
            || {
                sentry::capture_message(
                    &format!("HTTP {status} {method} {route}"),
                    sentry::Level::Error,
                )
            },
        );
    }
    res
}

/// Perluas CSP aplikasi dengan origin Sentry (dipasang di luar stack Loco, setelah header
/// keamanan terpasang).
pub async fn csp(State(state): State<Arc<ErrorReport>>, req: Request, next: Next) -> Response {
    let mut res = next.run(req).await;
    if let Some(origin) = &state.origin
        && let Some(csp) = res
            .headers()
            .get(header::CONTENT_SECURITY_POLICY)
            .and_then(|v| v.to_str().ok())
            // Hanya CSP aplikasi (skrip dari origin sendiri), bukan CSP sandbox pratinjau/logo.
            .filter(|v| v.contains("script-src 'self'"))
        && let Ok(v) = HeaderValue::from_str(&error_tracking::csp_with_sentry(csp, origin))
    {
        res.headers_mut().insert(header::CONTENT_SECURITY_POLICY, v);
    }
    res
}
