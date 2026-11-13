//! Endpoint OAuth 2.1 untuk MCP: metadata (RFC 8414, RFC 9728), registrasi client (RFC 7591),
//! otorisasi + halaman consent, token, pencabutan (RFC 7009). Endpoint API (`/.well-known/…`,
//! `/oauth/register|token|revoke`) selalu membalas JSON; `/oauth/authorize` halaman biasa
//! (login wajib lewat gate). Logika di `services::oauth`.
use axum::{
    Json,
    extract::Query,
    http::{HeaderMap, HeaderValue, StatusCode, header},
};
use loco_rs::prelude::*;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    common::settings::Settings,
    controllers::extractors::{CurrentUser, RequestId},
    models::{oauth_clients, users},
    services::oauth::{
        self as service, AuthorizeCheck, AuthorizeParams, ClientMetadata, OAuthError,
    },
    views::oauth::{ConsentPage, WORKSPACE_REQUIRED},
};

/// URL resource MCP (audience token).
#[must_use]
pub fn resource_url(base: &str) -> String {
    format!("{base}/mcp")
}

pub(crate) fn no_store(mut res: Response) -> Response {
    res.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    res
}

pub(crate) fn json_error(status: StatusCode, e: &OAuthError) -> Response {
    no_store(
        (
            status,
            Json(serde_json::json!({ "error": e.code, "error_description": e.description })),
        )
            .into_response(),
    )
}

#[debug_handler]
async fn protected_resource(State(ctx): State<AppContext>) -> Result<Response> {
    let base = Settings::from_ctx(&ctx)?.public_base_url;
    Ok(Json(serde_json::json!({
        "resource": resource_url(&base),
        "authorization_servers": [base],
        "bearer_methods_supported": ["header"],
        "scopes_supported": [service::SCOPE],
        "resource_name": "Agent Gate",
    }))
    .into_response())
}

#[debug_handler]
async fn authorization_server(State(ctx): State<AppContext>) -> Result<Response> {
    let base = Settings::from_ctx(&ctx)?.public_base_url;
    Ok(Json(serde_json::json!({
        "issuer": base,
        "authorization_endpoint": format!("{base}/oauth/authorize"),
        "token_endpoint": format!("{base}/oauth/token"),
        "registration_endpoint": format!("{base}/oauth/register"),
        "revocation_endpoint": format!("{base}/oauth/revoke"),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code", "refresh_token", crate::services::device::GRANT_TYPE],
        "device_authorization_endpoint": format!("{base}/oauth/device_authorization"),
        "code_challenge_methods_supported": ["S256"],
        "token_endpoint_auth_methods_supported": ["none"],
        "revocation_endpoint_auth_methods_supported": ["none"],
        "scopes_supported": [service::SCOPE],
        "authorization_response_iss_parameter_supported": true,
    }))
    .into_response())
}

#[debug_handler]
async fn register(
    State(ctx): State<AppContext>,
    Json(meta): Json<ClientMetadata>,
) -> Result<Response> {
    match service::register(&ctx.db, &meta).await {
        Ok(c) => Ok(no_store(
            (
                StatusCode::CREATED,
                Json(serde_json::json!({
                    "client_id": c.client_id,
                    "client_name": c.name,
                    "redirect_uris": c.redirect_uris,
                    "client_uri": c.client_uri,
                    "token_endpoint_auth_method": "none",
                    "grant_types": ["authorization_code", "refresh_token"],
                    "response_types": ["code"],
                    "client_id_issued_at": c.created_at.timestamp(),
                })),
            )
                .into_response(),
        )),
        Err(e) => Ok(json_error(StatusCode::BAD_REQUEST, &e)),
    }
}

fn bad_client(v: &TeraView) -> Result<Response> {
    format::render()
        .status(StatusCode::BAD_REQUEST)
        .view(v, "oauth/error.html", data!({}))
}

fn hidden(p: &AuthorizeParams) -> Vec<(&'static str, String)> {
    let mut out = vec![
        ("response_type", p.response_type.clone()),
        ("client_id", p.client_id.clone()),
        ("redirect_uri", p.redirect_uri.clone()),
        ("code_challenge", p.code_challenge.clone()),
        ("code_challenge_method", p.code_challenge_method.clone()),
    ];
    for (k, v) in [
        ("state", &p.state),
        ("resource", &p.resource),
        ("scope", &p.scope),
    ] {
        if let Some(v) = v {
            out.push((k, v.clone()));
        }
    }
    out
}

async fn render_consent(
    ctx: &AppContext,
    v: &TeraView,
    user: &users::Model,
    client: &oauth_clients::Model,
    params: &AuthorizeParams,
    error: &'static str,
    status: StatusCode,
) -> Result<Response> {
    let choices = service::consent_workspaces(&ctx.db, user).await?;
    let page = ConsentPage::new(
        client,
        user,
        &choices,
        hidden(params),
        &params.redirect_uri,
        error,
    );
    format::render()
        .status(status)
        .view(v, "oauth/consent.html", data!({ "page": page }))
        .map(|res| with_consent_csp(res, &params.redirect_uri))
}

/// Sumber CSP untuk redirect URI client: `https://host:port`, `http://127.0.0.1:port`, atau
/// `skema:` untuk skema privat. `None` bila ada karakter yang tidak boleh di header CSP.
fn csp_source(redirect_uri: &str) -> Option<String> {
    let (scheme, rest) = redirect_uri.split_once(':')?;
    let source = if scheme == "http" || scheme == "https" {
        let authority = rest.strip_prefix("//")?.split(['/', '?', '#']).next()?;
        format!("{scheme}://{authority}")
    } else {
        format!("{scheme}:")
    };
    source
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '/' | '.' | '-' | '+' | '[' | ']'))
        .then_some(source)
}

/// Halaman consent: `form-action` CSP default hanya `'self'`, dan Chrome menerapkannya juga pada
/// redirect setelah submit form. Tambahkan asal redirect URI client (yang sudah terdaftar dan
/// dicek) supaya redirect ke client setelah Izinkan/Tolak tidak diblokir. Header ini menang
/// atas header aman bawaan (Loco tidak menimpa header yang sudah ada).
fn with_consent_csp(mut res: Response, redirect_uri: &str) -> Response {
    let extra = csp_source(redirect_uri).unwrap_or_default();
    let csp = format!(
        "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; \
         object-src 'none'; base-uri 'self'; form-action 'self' {extra}; frame-ancestors 'none'"
    );
    if let Ok(v) = HeaderValue::from_str(&csp) {
        res.headers_mut().insert(header::CONTENT_SECURITY_POLICY, v);
    }
    res
}

#[debug_handler]
async fn authorize(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    CurrentUser(user): CurrentUser,
    Query(params): Query<AuthorizeParams>,
) -> Result<Response> {
    let base = Settings::from_ctx(&ctx)?.public_base_url;
    match service::check_authorize(&ctx.db, &params, &resource_url(&base), &base).await? {
        AuthorizeCheck::BadClient => bad_client(&v),
        AuthorizeCheck::Redirect(to) => Ok(format::redirect(&to)?),
        AuthorizeCheck::Ok(client) => {
            render_consent(&ctx, &v, &user, &client, &params, "", StatusCode::OK).await
        }
    }
}

#[derive(Debug, Deserialize)]
struct Decision {
    #[serde(flatten)]
    params: AuthorizeParams,
    #[serde(default)]
    decision: String,
    #[serde(default)]
    workspace: String,
}

#[debug_handler]
async fn decide(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    CurrentUser(user): CurrentUser,
    RequestId(request_id): RequestId,
    Form(d): Form<Decision>,
) -> Result<Response> {
    let base = Settings::from_ctx(&ctx)?.public_base_url;
    let resource = resource_url(&base);
    let client = match service::check_authorize(&ctx.db, &d.params, &resource, &base).await? {
        AuthorizeCheck::BadClient => return bad_client(&v),
        AuthorizeCheck::Redirect(to) => return format::redirect(&to),
        AuthorizeCheck::Ok(client) => client,
    };
    let state = d.params.state.clone().unwrap_or_default();
    let with_state = |mut pairs: Vec<(&'static str, String)>| {
        if !state.is_empty() {
            pairs.push(("state", state.clone()));
        }
        pairs.push(("iss", base.clone()));
        let borrowed: Vec<(&str, &str)> = pairs.iter().map(|(k, v)| (*k, v.as_str())).collect();
        service::redirect_with(&d.params.redirect_uri, &borrowed)
    };
    if d.decision != "allow" {
        let to = with_state(vec![
            ("error", "access_denied".to_string()),
            ("error_description", "user denied access".to_string()),
        ]);
        return format::redirect(&to);
    }
    let Ok(workspace) = Uuid::try_parse(d.workspace.trim()) else {
        return render_consent(
            &ctx,
            &v,
            &user,
            &client,
            &d.params,
            WORKSPACE_REQUIRED,
            StatusCode::UNPROCESSABLE_ENTITY,
        )
        .await;
    };
    match service::approve(
        &ctx.db, &client, &user, workspace, &d.params, &resource, request_id,
    )
    .await?
    {
        Some(code) => Ok(format::redirect(&with_state(vec![("code", code)]))?),
        None => {
            render_consent(
                &ctx,
                &v,
                &user,
                &client,
                &d.params,
                WORKSPACE_REQUIRED,
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
    }
}

#[derive(Debug, Deserialize)]
struct TokenForm {
    #[serde(default)]
    grant_type: String,
    #[serde(default)]
    code: String,
    #[serde(default)]
    redirect_uri: String,
    #[serde(default)]
    client_id: String,
    #[serde(default)]
    code_verifier: String,
    #[serde(default)]
    refresh_token: String,
    #[serde(default)]
    device_code: String,
}

#[debug_handler]
async fn token(State(ctx): State<AppContext>, Form(f): Form<TokenForm>) -> Result<Response> {
    let result = match f.grant_type.as_str() {
        "authorization_code" => {
            service::exchange_code(
                &ctx.db,
                &f.client_id,
                &f.code,
                &f.redirect_uri,
                &f.code_verifier,
            )
            .await
        }
        "refresh_token" => service::refresh(&ctx.db, &f.client_id, &f.refresh_token).await,
        crate::services::device::GRANT_TYPE => {
            crate::services::device::poll(&ctx.db, &f.client_id, &f.device_code).await
        }
        _ => Err(OAuthError {
            code: "unsupported_grant_type",
            description: "grant_type must be authorization_code, refresh_token, or device_code",
        }),
    };
    Ok(match result {
        Ok(t) => no_store(Json(t).into_response()),
        Err(e) => json_error(StatusCode::BAD_REQUEST, &e),
    })
}

#[derive(Debug, Deserialize)]
struct RevokeForm {
    #[serde(default)]
    token: String,
}

#[debug_handler]
async fn revoke(State(ctx): State<AppContext>, Form(f): Form<RevokeForm>) -> Result<Response> {
    if !f.token.is_empty() {
        service::revoke_token(&ctx.db, &f.token).await?;
    }
    Ok(no_store(StatusCode::OK.into_response()))
}

/// `WWW-Authenticate` untuk 401 di `/mcp` (RFC 9728 §5.1).
#[must_use]
pub fn www_authenticate(base: &str) -> HeaderMap {
    let mut h = HeaderMap::new();
    if let Ok(v) = HeaderValue::from_str(&format!(
        "Bearer resource_metadata=\"{base}/.well-known/oauth-protected-resource/mcp\""
    )) {
        h.insert(header::WWW_AUTHENTICATE, v);
    }
    h
}

pub fn routes() -> Routes {
    Routes::new()
        .add(
            "/.well-known/oauth-protected-resource",
            get(protected_resource),
        )
        .add(
            "/.well-known/oauth-protected-resource/mcp",
            get(protected_resource),
        )
        .add(
            "/.well-known/oauth-authorization-server",
            get(authorization_server),
        )
        .add("/oauth/register", post(register))
        .add("/oauth/authorize", get(authorize).post(decide))
        .add("/oauth/token", post(token))
        .add("/oauth/revoke", post(revoke))
}

#[cfg(test)]
mod tests {
    use super::csp_source;

    #[test]
    fn csp_source_of_redirect_uris() {
        assert_eq!(
            csp_source("http://127.0.0.1:53682/callback").as_deref(),
            Some("http://127.0.0.1:53682")
        );
        assert_eq!(
            csp_source("https://claude.ai/api/mcp/auth_callback?x=1").as_deref(),
            Some("https://claude.ai")
        );
        assert_eq!(
            csp_source("cursor://anysphere.cursor-retrieval/oauth").as_deref(),
            Some("cursor:")
        );
        assert_eq!(csp_source("https://a.b'; script-src *"), None);
    }
}
