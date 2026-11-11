//! `IdOr404<T>`: UUID dari parameter path `{id}` (rules §6). Id yang bukan UUID menjadi 404,
//! sama seperti id yang tidak ada, bukan 400. `T` hanya penanda entity yang dimaksud
//! (`IdOr404<roles::Entity>`), supaya signature handler menyebut jenis id-nya; keberadaan dan
//! tenant-nya tetap dicek query model (`find_in(db, workspace_id, id)`).

use std::marker::PhantomData;

use axum::{
    extract::{FromRequestParts, RawPathParams},
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use uuid::Uuid;

/// UUID dari isian form (mis. `role_id`, `user_id`); nilai lain (form diubah di luar browser)
/// → 404, sama dengan id path.
///
/// # Errors
///
/// [`loco_rs::Error::NotFound`] bila `raw` bukan UUID.
pub fn form_id(raw: &str) -> Result<Uuid, loco_rs::Error> {
    Uuid::try_parse(raw.trim()).map_err(|_| loco_rs::Error::NotFound)
}

#[derive(Debug, Clone, Copy)]
pub struct IdOr404<T> {
    pub id: Uuid,
    _entity: PhantomData<fn() -> T>,
}

impl<S, T> FromRequestParts<S> for IdOr404<T>
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let params = RawPathParams::from_request_parts(parts, state)
            .await
            .map_err(IntoResponse::into_response)?;
        let Some((_, raw)) = params.iter().find(|(name, _)| *name == "id") else {
            tracing::error!("IdOr404 extractor used on a route without {{id}}");
            return Err(StatusCode::NOT_FOUND.into_response());
        };
        Uuid::try_parse(raw)
            .map(|id| Self {
                id,
                _entity: PhantomData,
            })
            .map_err(|_| StatusCode::NOT_FOUND.into_response())
    }
}

#[cfg(test)]
mod tests {
    use axum::{Router, body::Body, http::Request, routing::get};
    use tower::ServiceExt;

    use super::*;

    struct Thing;

    async fn status(uri: &str) -> (StatusCode, String) {
        let app = Router::new().route(
            "/things/{id}",
            get(|id: IdOr404<Thing>| async move { id.id.to_string() }),
        );
        let res = app
            .oneshot(Request::get(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = res.status();
        let body = axum::body::to_bytes(res.into_body(), 1024).await.unwrap();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    #[tokio::test]
    async fn valid_uuid_passes_and_anything_else_is_404() {
        let id = Uuid::now_v7();
        assert_eq!(
            status(&format!("/things/{id}")).await,
            (StatusCode::OK, id.to_string())
        );
        for bad in ["not-a-uuid", "123", "%27%3B--"] {
            assert_eq!(
                status(&format!("/things/{bad}")).await.0,
                StatusCode::NOT_FOUND,
                "{bad}"
            );
        }
    }
}
