//! `ClientIp`: IP client dari middleware `remote_ip` Loco (rules §7), untuk rate limit per IP.
//! `RemoteIP::None` berarti middleware salah konfigurasi: rate limit per IP tidak boleh
//! diam-diam terlewati, jadi request ditolak (500) dan dicatat sebagai error.

use std::net::IpAddr;

use axum::{
    extract::FromRequestParts,
    http::request::Parts,
    response::{IntoResponse, Response},
};
use loco_rs::{Error, controller::middleware::remote_ip::RemoteIP};

#[derive(Debug, Clone, Copy)]
pub struct ClientIp(pub IpAddr);

impl<S> FromRequestParts<S> for ClientIp
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match RemoteIP::from_request_parts(parts, state).await {
            Ok(RemoteIP::Forwarded(ip) | RemoteIP::Socket(ip)) => Ok(Self(ip)),
            Ok(RemoteIP::None) | Err(_) => {
                tracing::error!(
                    "request rejected: client IP unavailable (check server.middlewares.remote_ip)"
                );
                Err(Error::InternalServerError.into_response())
            }
        }
    }
}
