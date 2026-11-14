//! Endpoint operasional (M12): `GET /_metrics` (Prometheus, bearer `settings.system.metrics_token`;
//! tanpa token terkonfigurasi atau token salah → 404, supaya keberadaannya tidak terlihat).
//! `/_health` (liveness) dan `/_readiness` (database + queue) bawaan Loco.

use axum::http::{HeaderMap, HeaderValue, header};
use loco_rs::prelude::*;

use crate::{common::settings::Settings, services::metrics};

#[debug_handler]
async fn metrics_page(State(ctx): State<AppContext>, headers: HeaderMap) -> Result<Response> {
    let expected = Settings::from_ctx(&ctx)?.system.metrics_token;
    let given = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or_default();
    if expected.is_empty() || !crate::common::crypto::ct_eq(given.as_bytes(), expected.as_bytes()) {
        return Err(Error::NotFound);
    }
    let body = metrics::render(&ctx).await?;
    let mut res = body.into_response();
    res.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; version=0.0.4; charset=utf-8"),
    );
    res.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(res)
}

pub fn routes() -> Routes {
    Routes::new().add("/_metrics", get(metrics_page))
}
