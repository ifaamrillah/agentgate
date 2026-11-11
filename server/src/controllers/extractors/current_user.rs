//! `CurrentUser` dan `MaybeCurrentUser`: satu-satunya tempat sesi di-resolve menjadi user
//! (rules §7 Sesi). Membungkus `Session` Loco dan aturan `sign_in::current_user` (user ada,
//! aktif, umur mutlak). Handler tidak memakai `CurrentUser<String>` Loco langsung.

use axum::{
    extract::{FromRef, FromRequestParts, OriginalUri},
    http::{Method, StatusCode, Uri, header, request::Parts},
    response::{IntoResponse, Response},
};
use loco_rs::{
    app::AppContext,
    auth::session::{self, Session},
};

use crate::{models::users, services::auth::sign_in};

/// User yang login. Request tanpa sesi valid ditolak: sesinya di-`clear()` dan browser
/// diarahkan ke login ([`sign_in_redirect`]).
#[derive(Debug, Clone)]
pub struct CurrentUser(pub users::Model);

/// User yang login, atau `None`. Tidak pernah menolak dan tidak menulis sesi.
#[derive(Debug, Clone)]
pub struct MaybeCurrentUser(pub Option<users::Model>);

/// Hasil resolve per request, supaya beberapa extractor tidak mengulang query.
#[derive(Clone)]
struct Resolved(Option<users::Model>);

async fn resolve(
    parts: &mut Parts,
    ctx: &AppContext,
) -> loco_rs::Result<(Session, Option<users::Model>)> {
    let session = Session::from_parts(parts, ctx).await?;
    if let Some(Resolved(user)) = parts.extensions.get::<Resolved>() {
        return Ok((session, user.clone()));
    }
    let user = sign_in::current_user(ctx, &session).await?;
    parts.extensions.insert(Resolved(user.clone()));
    Ok((session, user))
}

impl<S> FromRequestParts<S> for CurrentUser
where
    AppContext: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let ctx = AppContext::from_ref(state);
        match resolve(parts, &ctx)
            .await
            .map_err(IntoResponse::into_response)?
        {
            (_, Some(user)) => Ok(Self(user)),
            (session, None) => {
                let uri = parts
                    .extensions
                    .get::<OriginalUri>()
                    .map_or(&parts.uri, |o| &o.0);
                Err((session.clear(), sign_in_redirect(&parts.method, uri)).into_response())
            }
        }
    }
}

impl<S> FromRequestParts<S> for MaybeCurrentUser
where
    AppContext: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let ctx = AppContext::from_ref(state);
        let (_, user) = resolve(parts, &ctx)
            .await
            .map_err(IntoResponse::into_response)?;
        Ok(Self(user))
    }
}

/// Redirect ke login untuk request yang butuh sesi: `303 /login?return_to=<path+query>` untuk
/// GET/HEAD (halaman yang bisa dibuka lagi), `303 /login` untuk method lain (body POST tidak
/// bisa diulang setelah login). Dipakai gate dan `CurrentUser`.
#[must_use]
pub fn sign_in_redirect(method: &Method, uri: &Uri) -> Response {
    let location = if matches!(*method, Method::GET | Method::HEAD) {
        let here = uri.path_and_query().map_or("/", |p| p.as_str());
        format!("/login?return_to={}", session::encode(here))
    } else {
        "/login".to_string()
    };
    (StatusCode::SEE_OTHER, [(header::LOCATION, location)]).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn location(method: Method, uri: &str) -> String {
        let res = sign_in_redirect(&method, &uri.parse().unwrap());
        assert_eq!(res.status(), StatusCode::SEE_OTHER);
        res.headers()[header::LOCATION]
            .to_str()
            .unwrap()
            .to_string()
    }

    #[test]
    fn get_keeps_path_and_query_as_return_to() {
        assert_eq!(
            location(Method::GET, "/projects?tab=a"),
            "/login?return_to=%2Fprojects%3Ftab%3Da"
        );
    }

    #[test]
    fn non_get_goes_to_plain_login() {
        assert_eq!(location(Method::POST, "/projects"), "/login");
        assert_eq!(location(Method::DELETE, "/projects/1"), "/login");
    }
}
