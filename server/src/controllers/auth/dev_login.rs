//! `GET /_dev/login?token=...` (rules §7): satu-satunya GET yang menulis data, hanya di build
//! debug dan hanya didaftarkan saat `LOCO_ENV=development`. Token berasal dari task
//! `dev_login`; sesinya dibuat lewat `sign_in::complete`, jalur yang sama dengan login biasa.
use loco_rs::prelude::*;
use serde::Deserialize;

use crate::{
    models::users,
    services::auth::{
        dev_login,
        sign_in::{self, SignInStep},
    },
};

const HOME: &str = "/";

#[derive(Debug, Deserialize)]
pub struct Params {
    token: String,
}

#[debug_handler]
async fn login(
    State(ctx): State<AppContext>,
    session: Session,
    Query(params): Query<Params>,
) -> Result<Response> {
    let secret = ctx
        .config
        .auth
        .as_ref()
        .and_then(|a| a.session.as_ref())
        .and_then(|s| s.secret.as_deref())
        .ok_or_else(|| Error::string("auth.session.secret is not configured"))?;
    let now = chrono::Utc::now().timestamp();
    let user = match dev_login::verify(secret, &params.token, now) {
        Some(id) => users::Model::find_by_id(&ctx.db, id)
            .await?
            .filter(users::Model::is_active),
        None => None,
    };
    let Some(user) = user else {
        return Err(Error::NotFound);
    };
    let (session, target) = match sign_in::complete(&ctx, session, &user, None)? {
        SignInStep::Done(session) => (session, HOME),
        SignInStep::NeedsSecondFactor(session) => (session, sign_in::SECOND_FACTOR_PATH),
    };
    Ok((session, format::redirect(target)?).into_response())
}

pub fn routes() -> Routes {
    Routes::new().prefix("/_dev").add("/login", get(login))
}
