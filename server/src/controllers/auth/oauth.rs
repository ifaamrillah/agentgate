//! `POST /auth/{provider}` (mulai login Google/GitHub) dan `GET /auth/{provider}/callback`.
//! Logika alur dan aturan tautan akun ada di `services::auth::oauth`; di sini hanya cookie
//! state, sesi, dan redirect. Start hanya lewat POST (form `hx-boost="false"` di halaman
//! login): GET tidak memulai alur, sehingga halaman lain tidak bisa memicu login diam-diam.
use axum::extract::{Path, Query};
use loco_rs::{
    auth::{
        cookies::{self, EncryptedCookies},
        oauth::{CallbackParams, state::FlowState},
    },
    prelude::*,
};
use serde::Deserialize;

use super::login::HOME;
use crate::{
    controllers::extractors::ValidForm,
    services::auth::{
        oauth::{self, Callback, OAuthOutcome},
        sign_in::{self, SignInStep},
    },
};

#[derive(Debug, Deserialize)]
pub struct StartForm {
    #[serde(default)]
    return_to: Option<String>,
}

#[debug_handler]
async fn start(
    State(ctx): State<AppContext>,
    Path(name): Path<String>,
    jar: EncryptedCookies,
    ValidForm(form): ValidForm<StartForm>,
) -> Result<Response> {
    let provider = oauth::provider(&ctx, &name)?.ok_or(Error::NotFound)?;
    let (flow, location) = oauth::start(&ctx, &provider, form.return_to.as_deref())?;
    let cookie = jar
        .options()
        .clone()
        .with_max_age(Some(oauth::STATE_TTL_SECONDS))
        .cookie(
            oauth::cookie_name(&ctx, &name),
            cookies::seal(&flow, oauth::STATE_TTL_SECONDS)?,
        );
    Ok((jar.add(cookie), format::redirect(&location)?).into_response())
}

#[debug_handler]
async fn callback(
    State(ctx): State<AppContext>,
    Path(name): Path<String>,
    Query(params): Query<CallbackParams>,
    jar: EncryptedCookies,
    session: Session,
) -> Result<Response> {
    let provider = oauth::provider(&ctx, &name)?.ok_or(Error::NotFound)?;
    let cookie_name = oauth::cookie_name(&ctx, &name);
    let flow = jar
        .get(&cookie_name)
        .and_then(|c| cookies::unseal::<FlowState>(c.value()));
    // Transaksi sekali pakai: cookie dihapus di setiap respons di bawah, apa pun hasilnya.
    let jar = jar.remove(&cookie_name);

    let (identity, return_to) = match oauth::finish(&ctx, &provider, params, flow).await? {
        Callback::Identity {
            identity,
            return_to,
        } => (identity, return_to),
        Callback::Failed { kind, detail } => {
            // Detail dari Loco (status endpoint, kode error provider) tidak memuat token;
            // tetap hanya di level debug, yang mati di staging/production.
            let detail = detail.as_deref().unwrap_or("");
            tracing::debug!(provider = %name, kind = kind.as_str(), detail, "oauth detail");
            return failure(
                session,
                jar,
                &name,
                kind.as_str(),
                oauth::failure_code(kind),
            );
        }
    };
    let user = match oauth::resolve_user(&ctx, &identity).await? {
        OAuthOutcome::SignedIn(user) => user,
        OAuthOutcome::Rejected(code) => return failure(session, jar, &name, code, code),
    };
    match sign_in::complete(&ctx, session, &user, return_to.as_deref())? {
        SignInStep::Done(session) => {
            let target = return_to.as_deref().unwrap_or(HOME);
            Ok((session, jar, format::redirect(target)?).into_response())
        }
        SignInStep::NeedsSecondFactor(session) => {
            Ok((session, jar, format::redirect(sign_in::SECOND_FACTOR_PATH)?).into_response())
        }
    }
}

/// `303 /login?oauth_error=<code>`. Log hanya provider dan jenis kegagalan, tanpa token.
fn failure(
    session: Session,
    jar: EncryptedCookies,
    provider: &str,
    kind: &str,
    code: &str,
) -> Result<Response> {
    tracing::warn!(provider, kind, "oauth sign-in failed");
    let location = format!("/login?oauth_error={code}");
    Ok((session, jar, format::redirect(&location)?).into_response())
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/auth")
        .add("/{provider}", post(start))
        .add("/{provider}/callback", get(callback))
}
