//! Webhook provider git (M6): `POST /webhooks/github/{connection}`. Tanpa sesi dan tanpa
//! cookie (gate dan CSRF tidak berlaku: path API, tanpa `Origin`). Autentikasi = tanda tangan
//! HMAC `X-Hub-Signature-256` dengan secret koneksi; logika di `services::git::webhook`.
//!
//! Balasan: 204 dipakai, 202 event diabaikan, 401 tanda tangan salah, 404 koneksi tidak dikenal,
//! 400 payload rusak, 413 body > 1 MB.

use axum::{
    Json,
    body::Bytes,
    extract::{DefaultBodyLimit, Path},
    http::{HeaderMap, StatusCode},
};
use loco_rs::prelude::*;
use uuid::Uuid;

use crate::{
    common::settings::Settings,
    services::git::webhook::{self, BODY_MAX, Handled, WebhookError},
};

fn json_error(status: StatusCode, code: &str) -> Response {
    (status, Json(serde_json::json!({ "error": code }))).into_response()
}

#[debug_handler]
async fn github(
    State(ctx): State<AppContext>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response> {
    let Ok(id) = Uuid::try_parse(&id) else {
        return Ok(json_error(StatusCode::NOT_FOUND, "not_found"));
    };
    let settings = Settings::from_ctx(&ctx)?;
    let header = |name: &str| headers.get(name).and_then(|v| v.to_str().ok());
    let event = header("x-github-event").unwrap_or_default();
    match webhook::handle(
        &ctx.db,
        &settings,
        id,
        event,
        header("x-hub-signature-256"),
        &body,
    )
    .await
    {
        Ok(Handled::Applied) => Ok(StatusCode::NO_CONTENT.into_response()),
        Ok(Handled::Ignored) => Ok(StatusCode::ACCEPTED.into_response()),
        Err(WebhookError::UnknownConnection) => Ok(json_error(StatusCode::NOT_FOUND, "not_found")),
        Err(WebhookError::BadSignature) => {
            Ok(json_error(StatusCode::UNAUTHORIZED, "bad_signature"))
        }
        Err(WebhookError::BadPayload) => Ok(json_error(StatusCode::BAD_REQUEST, "bad_payload")),
        Err(WebhookError::Db(e)) => Err(e.into()),
    }
}

pub fn routes() -> Routes {
    Routes::new()
        .add("/webhooks/github/{id}", post(github))
        .layer(DefaultBodyLimit::max(BODY_MAX))
}
