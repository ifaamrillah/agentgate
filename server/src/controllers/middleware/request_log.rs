//! Pengganti middleware `logger` Loco: span `http-request` yang sama (method, URI, versi,
//! user agent, environment, request id), tetapi token di URL tidak pernah ditulis ke log (rules
//! §7 Reset password): query string route yang membawa token, dan segmen token di path
//! undangan (`/invitations/{token}/…`). Loco mencatat `request.uri()` utuh, sehingga token akan
//! ikut di setiap baris log di dalam span request itu.
//!
//! Dipasang lewat `stack.replace("logger", …)` di `App::middlewares`, di posisi yang sama
//! dengan logger Loco, dan mengikuti `server.middlewares.logger.enable` yang sama.

use std::time::Instant;

use axum::{
    Router as AxumRouter,
    extract::{Request, State},
    http::{Uri, header},
    middleware::{Next, from_fn_with_state},
    response::Response,
};
use loco_rs::{
    app::AppContext,
    controller::middleware::{MiddlewareLayer, request_id::LocoRequestId},
    environment::Environment,
};
use tracing::Instrument;

/// Path yang query string-nya berisi token sekali pakai (email, atau `dev_login` yang setara
/// password selama 10 menit). `/login`: `return_to` dari halaman undangan berisi path
/// undangan beserta tokennya. Callback OAuth (path dinamis) dicek `is_oauth_callback`.
const TOKEN_PATHS: &[&str] = &["/verify-email", "/password/reset", "/_dev/login", "/login"];

/// Prefix path yang segmen berikutnya adalah token undangan (`/invitations/{token}/…`).
const INVITATION_PREFIX: &str = "/invitations/";

pub struct RequestLog {
    enable: bool,
    environment: Environment,
}

impl RequestLog {
    #[must_use]
    pub fn new(ctx: &AppContext) -> Self {
        Self {
            // Default sama dengan Loco: aktif bila tidak dikonfigurasi.
            enable: ctx
                .config
                .server
                .middlewares
                .logger
                .as_ref()
                .is_none_or(|l| l.enable),
            environment: ctx.environment.clone(),
        }
    }
}

impl MiddlewareLayer for RequestLog {
    fn name(&self) -> &'static str {
        "logger"
    }

    fn is_enabled(&self) -> bool {
        self.enable
    }

    fn config(&self) -> serde_json::Result<serde_json::Value> {
        Ok(serde_json::json!({ "enable": self.enable }))
    }

    fn apply(&self, app: AxumRouter<AppContext>) -> loco_rs::Result<AxumRouter<AppContext>> {
        Ok(app.layer(from_fn_with_state(self.environment.clone(), layer)))
    }
}

async fn layer(State(environment): State<Environment>, req: Request, next: Next) -> Response {
    let request_id = req
        .extensions()
        .get::<LocoRequestId>()
        .map_or_else(|| "req-id-none".to_string(), |r| r.get().to_string());
    let user_agent = req
        .headers()
        .get(header::USER_AGENT)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .to_string();
    let span = tracing::error_span!(
        "http-request",
        "http.method" = %req.method(),
        "http.uri" = %logged_uri(req.uri()),
        "http.version" = ?req.version(),
        "http.user_agent" = %user_agent,
        "environment" = %environment,
        request_id = %request_id,
    );
    async move {
        let started = Instant::now();
        tracing::debug!("started processing request");
        let res = next.run(req).await;
        tracing::debug!(
            latency = ?started.elapsed(),
            status = res.status().as_u16(),
            "finished processing request"
        );
        res
    }
    .instrument(span)
    .await
}

/// URI untuk log: query string route bertoken dan token undangan di path diganti
/// `[redacted]`.
fn logged_uri(uri: &Uri) -> String {
    let raw_path = uri.path();
    let path = redacted_path(raw_path);
    match uri.query() {
        Some(_)
            if TOKEN_PATHS.contains(&raw_path)
                || is_oauth_callback(raw_path)
                || path != raw_path =>
        {
            format!("{path}?[redacted]")
        }
        Some(query) => format!("{path}?{query}"),
        None => path,
    }
}

/// Path untuk log: segmen token undangan (`/invitations/{token}/…`) diganti `[redacted]`.
/// Dipakai juga log lain yang mencatat path (mis. penolakan CSRF).
#[must_use]
pub fn redacted_path(path: &str) -> String {
    match path.strip_prefix(INVITATION_PREFIX) {
        Some(rest) if !rest.is_empty() => {
            let tail = rest.find('/').map_or("", |i| &rest[i..]);
            format!("{INVITATION_PREFIX}[redacted]{tail}")
        }
        _ => path.to_string(),
    }
}

/// `/auth/{provider}/callback`: query-nya berisi authorization code dan state OAuth.
fn is_oauth_callback(path: &str) -> bool {
    path.starts_with("/auth/") && path.ends_with("/callback")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_query_strings_are_redacted() {
        let uri = |s: &str| s.parse::<Uri>().unwrap();
        assert_eq!(
            logged_uri(&uri("/password/reset?token=abc")),
            "/password/reset?[redacted]"
        );
        assert_eq!(
            logged_uri(&uri("/verify-email?token=abc")),
            "/verify-email?[redacted]"
        );
        assert_eq!(
            logged_uri(&uri("/_dev/login?token=x")),
            "/_dev/login?[redacted]"
        );
        // Callback OAuth membawa authorization code dan state.
        assert_eq!(
            logged_uri(&uri("/auth/google/callback?code=c0de&state=s7ate")),
            "/auth/google/callback?[redacted]"
        );
        assert_eq!(
            logged_uri(&uri("/auth/github/callback?error=access_denied")),
            "/auth/github/callback?[redacted]"
        );
        assert_eq!(logged_uri(&uri("/auth/google?x=1")), "/auth/google?x=1");
        assert_eq!(logged_uri(&uri("/verify-email")), "/verify-email");
        assert_eq!(logged_uri(&uri("/projects?tab=a")), "/projects?tab=a");
    }

    #[test]
    fn invitation_tokens_in_the_path_are_redacted() {
        let uri = |s: &str| s.parse::<Uri>().unwrap();
        assert_eq!(
            logged_uri(&uri("/invitations/s3cr3t")),
            "/invitations/[redacted]"
        );
        assert_eq!(
            logged_uri(&uri("/invitations/s3cr3t/register")),
            "/invitations/[redacted]/register"
        );
        assert_eq!(
            logged_uri(&uri("/invitations/s3cr3t/accept?x=1")),
            "/invitations/[redacted]/accept?[redacted]"
        );
        assert_eq!(
            redacted_path("/invitations/s3cr3t"),
            "/invitations/[redacted]"
        );
        assert_eq!(redacted_path("/invitations"), "/invitations");
        assert_eq!(redacted_path("/w/acme"), "/w/acme");
        // `return_to` login dari halaman undangan membawa token.
        assert_eq!(
            logged_uri(&uri("/login?return_to=%2Finvitations%2Fs3cr3t")),
            "/login?[redacted]"
        );
    }
}
