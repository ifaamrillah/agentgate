//! Gate login: tertutup secara default (rules §7 Gate). Request ke route non-publik
//! (`public_routes::is_public`) tanpa sesi ber-`user_id` diarahkan ke login.
//!
//! Ini pengaman kedua: gate hanya memeriksa ada sesi login. Pemeriksaan lengkap (user ada,
//! aktif, umur mutlak) ada di extractor `CurrentUser` yang tetap dipakai setiap handler.
//!
//! Dipasang di dalam stack middleware Loco, tepat di dalam `auth_session` (lihat
//! `App::middlewares`): `Session` server-side hanya bisa dibaca di dalam middleware itu.
//! CSRF (dipasang di `App::after_routes`, di luar stack Loco) tetap berjalan lebih dulu.
//! Dipasang sebagai `route_layer`, jadi path yang tidak ada tetap 404, bukan redirect.
//! Path API (`api_routes::is_api`) tidak memakai sesi: handler-nya memeriksa token bearer.

use axum::{
    Router as AxumRouter,
    extract::{Request, State},
    middleware::{Next, from_fn_with_state},
    response::{IntoResponse, Response},
};
use loco_rs::{app::AppContext, auth::session::Session, controller::middleware::MiddlewareLayer};

use crate::controllers::{api_routes, extractors::sign_in_redirect, public_routes};

/// Layer gate untuk `Hooks::middlewares`.
pub struct Gate {
    ctx: AppContext,
}

impl Gate {
    #[must_use]
    pub fn new(ctx: &AppContext) -> Self {
        Self { ctx: ctx.clone() }
    }
}

impl MiddlewareLayer for Gate {
    fn name(&self) -> &'static str {
        "gate"
    }

    fn config(&self) -> serde_json::Result<serde_json::Value> {
        Ok(serde_json::json!({ "enable": true }))
    }

    fn apply(&self, app: AxumRouter<AppContext>) -> loco_rs::Result<AxumRouter<AppContext>> {
        Ok(app.route_layer(from_fn_with_state(self.ctx.clone(), layer)))
    }
}

async fn layer(State(ctx): State<AppContext>, req: Request, next: Next) -> Response {
    if public_routes::is_public(req.uri().path()) || api_routes::is_api(req.uri().path()) {
        return next.run(req).await;
    }
    let (parts, body) = req.into_parts();
    match Session::from_parts(&parts, &ctx).await {
        Ok(session) if session.user_id().is_some() => {
            next.run(Request::from_parts(parts, body)).await
        }
        Ok(_) => sign_in_redirect(&parts.method, &parts.uri),
        Err(err) => err.into_response(),
    }
}
