//! `BearerSession`: pemegang token bearer OAuth (device flow CLI atau client MCP) untuk API
//! runner/CLI. User, keanggotaan, dan role dibaca ulang setiap request (`mcp::Session::load`).
//! Tanpa token valid → 401 JSON `{"error":"invalid_token"}`.

use axum::{
    Json,
    extract::{FromRef, FromRequestParts},
    http::{StatusCode, header, request::Parts},
    response::{IntoResponse, Response},
};
use loco_rs::app::AppContext;

use crate::services::{
    mcp::Session,
    oauth::{self, Principal},
};

pub struct BearerSession {
    pub session: Session,
    pub principal: Principal,
}

fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({ "error": "invalid_token" })),
    )
        .into_response()
}

/// Token bearer dari header `Authorization`.
#[must_use]
pub fn bearer_token(parts: &Parts) -> Option<String> {
    parts
        .headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| {
            v.strip_prefix("Bearer ")
                .or_else(|| v.strip_prefix("bearer "))
        })
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_string)
}

impl<S> FromRequestParts<S> for BearerSession
where
    AppContext: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let ctx = AppContext::from_ref(state);
        let token = bearer_token(parts).ok_or_else(unauthorized)?;
        let principal = match oauth::authenticate(&ctx.db, &token).await {
            Ok(Some(p)) => p,
            Ok(None) => return Err(unauthorized()),
            Err(e) => {
                tracing::error!(error = %e, "bearer auth db error");
                return Err(StatusCode::INTERNAL_SERVER_ERROR.into_response());
            }
        };
        match Session::load(&ctx.db, &principal).await {
            Ok(Some(session)) => Ok(Self { session, principal }),
            Ok(None) => Err(unauthorized()),
            Err(e) => {
                tracing::error!(error = %e, "bearer session db error");
                Err(StatusCode::INTERNAL_SERVER_ERROR.into_response())
            }
        }
    }
}
