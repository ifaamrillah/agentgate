//! Pratinjau mockup HTML (M10): `GET /preview/{token}`, hanya di host pratinjau (layer
//! `middleware::preview_host`). Token bertanda tangan menentukan versi artifact; tanpa sesi.
//! Respons membawa CSP sandbox sendiri (lihat `services::mockups`).
use axum::http::{HeaderValue, StatusCode, header};
use loco_rs::prelude::*;
use sea_orm::EntityTrait;

use crate::{
    common::settings::Settings,
    models::{artifact_versions, ticket_artifacts},
    services::mockups,
};

#[debug_handler]
async fn show(State(ctx): State<AppContext>, Path(token): Path<String>) -> Result<Response> {
    let settings = Settings::from_ctx(&ctx)?;
    let key = mockups::signing_key(&settings.auth.encryption_key_bytes()?);
    let id =
        mockups::verify(&key, &token, chrono::Utc::now().timestamp()).ok_or(Error::NotFound)?;
    let version = artifact_versions::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or(Error::NotFound)?;
    ticket_artifacts::Entity::find_by_id(version.artifact_id)
        .one(&ctx.db)
        .await?
        .filter(|a| a.workspace_id == version.workspace_id && mockups::is_mockup(&a.name))
        .ok_or(Error::NotFound)?;
    let mut res = (StatusCode::OK, version.body).into_response();
    let h = res.headers_mut();
    let set = |h: &mut axum::http::HeaderMap, k: header::HeaderName, v: &str| {
        if let Ok(v) = HeaderValue::from_str(v) {
            h.insert(k, v);
        }
    };
    set(h, header::CONTENT_TYPE, "text/html; charset=utf-8");
    set(
        h,
        header::CONTENT_SECURITY_POLICY,
        &mockups::preview_csp(&settings.public_base_url),
    );
    set(h, header::CACHE_CONTROL, "private, no-store");
    set(h, header::REFERRER_POLICY, "no-referrer");
    set(h, header::X_CONTENT_TYPE_OPTIONS, "nosniff");
    Ok(res)
}

pub fn routes() -> Routes {
    Routes::new().add("/preview/{token}", get(show))
}
