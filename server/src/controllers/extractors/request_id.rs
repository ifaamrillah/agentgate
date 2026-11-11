//! `RequestId`: id request dari middleware `request_id` Loco (header `x-request-id`), untuk
//! audit log (`services::audit`). Tidak pernah menolak: tanpa middleware itu nilainya `None`.

use std::convert::Infallible;

use axum::{extract::FromRequestParts, http::request::Parts};
use loco_rs::controller::middleware::request_id::LocoRequestId;

#[derive(Debug, Clone, Default)]
pub struct RequestId(pub Option<String>);

impl<S> FromRequestParts<S> for RequestId
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self(
            parts
                .extensions
                .get::<LocoRequestId>()
                .map(|id| id.get().to_string()),
        ))
    }
}
