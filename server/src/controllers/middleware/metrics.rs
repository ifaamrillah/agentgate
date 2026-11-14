//! Pencatat metrik request (M12, `services::metrics`). Dipasang sebagai `route_layer` (setelah
//! routing), sehingga label route adalah pola Axum (`/w/{slug}/t/{code}`), bukan URL asli yang
//! membawa id/token. Request tanpa route (404 fallback) tidak tercatat di sini.

use std::{sync::Arc, time::Instant};

use axum::{
    extract::{MatchedPath, Request, State},
    middleware::Next,
    response::Response,
};

use crate::services::metrics::Registry;

pub async fn layer(State(reg): State<Arc<Registry>>, req: Request, next: Next) -> Response {
    let method = req.method().as_str().to_owned();
    let route = req
        .extensions()
        .get::<MatchedPath>()
        .map_or_else(|| "unmatched".to_owned(), |m| m.as_str().to_owned());
    let started = Instant::now();
    let res = next.run(req).await;
    reg.observe(&method, &route, res.status().as_u16(), started.elapsed());
    res
}
