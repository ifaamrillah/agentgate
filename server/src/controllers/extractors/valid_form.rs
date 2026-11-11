//! `ValidForm<T>`: body form bertipe dengan penolakan yang seragam.
//!
//! Form yang tidak bisa di-parse (field wajib hilang, tipe salah, content type lain) tidak
//! mungkin dirender ulang sebagai form yang sama, jadi jawabannya 400 yang dirender layer
//! `html_errors` sebagai halaman error HTML. Validasi isi (`validator`) bukan urusan extractor
//! ini: handler merender ulang form dengan 422.

use axum::{
    Form,
    extract::{FromRequest, Request},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::de::DeserializeOwned;

#[derive(Debug, Clone)]
pub struct ValidForm<T>(pub T);

impl<S, T> FromRequest<S> for ValidForm<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = Response;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Form::<T>::from_request(req, state).await {
            Ok(Form(value)) => Ok(Self(value)),
            Err(rejection) => {
                // Pesan rejection bisa berisi potongan input; cukup jenisnya di log.
                tracing::debug!(status = %rejection.status(), "form rejected");
                Err(StatusCode::BAD_REQUEST.into_response())
            }
        }
    }
}
