//! Error tracking Sentry (R1): server (panic + respons 5xx) dan browser (`static/js/sentry.js`).
//!
//! **Mati bila `SENTRY_DSN` kosong**: tidak ada client, tidak ada request keluar, tidak ada
//! `<meta>` DSN di halaman, dan CSP tidak berubah.
//!
//! Data sensitif tidak dikirim: `send_default_pii = false`, dan [`scrub`] membuang cookie, body,
//! query string, header selain allowlist kecil (tanpa `authorization`/`cookie`/`x-runner-*`),
//! data user, serta data breadcrumb sebelum event dikirim. Event 5xx dari server hanya membawa
//! metode, pola route, dan status (lihat `controllers::middleware::error_report`).
//!
//! DSN browser adalah DSN publik yang sama (Sentry memang menerbitkan DSN untuk dipakai di
//! browser); host-nya ditambahkan ke CSP `connect-src` hanya bila DSN diisi.

use std::{borrow::Cow, sync::OnceLock};

use sentry::protocol::{Event, Request};

/// Header request yang boleh ikut event (tanpa nilai rahasia).
const SAFE_HEADERS: &[&str] = &["user-agent", "content-type", "accept", "x-request-id"];

/// Konfigurasi Sentry untuk browser (diisi saat boot dari settings).
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct BrowserConfig {
    pub dsn: String,
    pub environment: String,
    pub release: String,
}

static BROWSER: OnceLock<BrowserConfig> = OnceLock::new();

/// Simpan konfigurasi browser (sekali per proses; panggilan berikutnya diabaikan).
pub fn set_browser(config: BrowserConfig) {
    let _ = BROWSER.set(config);
}

/// Konfigurasi browser; `None` bila Sentry mati.
#[must_use]
pub fn browser() -> Option<&'static BrowserConfig> {
    BROWSER.get().filter(|c| !c.dsn.is_empty())
}

/// Origin endpoint ingest dari DSN (`https://<key>@o1.ingest.sentry.io/42` →
/// `https://o1.ingest.sentry.io`). `None` bila DSN tidak valid.
#[must_use]
pub fn dsn_origin(dsn: &str) -> Option<String> {
    let parsed: sentry::types::Dsn = dsn.trim().parse().ok()?;
    let scheme = parsed.scheme();
    let host = parsed.host();
    Some(match parsed.port() {
        443 if scheme.to_string() == "https" => format!("{scheme}://{host}"),
        80 if scheme.to_string() == "http" => format!("{scheme}://{host}"),
        port => format!("{scheme}://{host}:{port}"),
    })
}

/// CSP dengan `connect-src` yang mengizinkan endpoint Sentry. Tanpa DSN valid → CSP tidak
/// berubah. `connect-src` yang sudah ada diperluas; bila belum ada, ditambahkan dengan `'self'`
/// (fallback sebelumnya dari `default-src 'self'`).
#[must_use]
pub fn csp_with_sentry(csp: &str, origin: &str) -> String {
    if csp.contains(origin) {
        return csp.to_string();
    }
    let mut parts: Vec<String> = csp.split(';').map(|p| p.trim().to_string()).collect();
    if let Some(p) = parts.iter_mut().find(|p| p.starts_with("connect-src")) {
        p.push(' ');
        p.push_str(origin);
    } else {
        parts.push(format!("connect-src 'self' {origin}"));
    }
    parts
        .into_iter()
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("; ")
}

/// Buang data sensitif dari event sebelum dikirim.
#[must_use]
pub fn scrub(mut event: Event<'static>) -> Option<Event<'static>> {
    if let Some(req) = event.request.as_mut() {
        scrub_request(req);
    }
    event.user = None;
    event.server_name = None;
    for b in &mut event.breadcrumbs.values {
        b.data.clear();
    }
    Some(event)
}

fn scrub_request(req: &mut Request) {
    req.cookies = None;
    req.data = None;
    req.query_string = None;
    req.env.clear();
    req.headers
        .retain(|k, _| SAFE_HEADERS.contains(&k.to_ascii_lowercase().as_str()));
    if let Some(url) = req.url.as_mut() {
        url.set_query(None);
        url.set_fragment(None);
    }
}

/// Inisialisasi Sentry server dari env (`SENTRY_DSN`, `SENTRY_ENVIRONMENT`, fallback
/// `LOCO_ENV`). DSN kosong/tidak valid → `None` (mati). Guard harus hidup sepanjang proses.
#[must_use]
pub fn init_from_env() -> Option<sentry::ClientInitGuard> {
    let dsn = std::env::var("SENTRY_DSN").unwrap_or_default();
    let dsn: sentry::types::Dsn = dsn.trim().parse().ok()?;
    let environment = std::env::var("SENTRY_ENVIRONMENT")
        .or_else(|_| std::env::var("LOCO_ENV"))
        .unwrap_or_else(|_| "development".into());
    let mut options = sentry::ClientOptions::default();
    options.dsn = Some(dsn);
    options.environment = Some(Cow::Owned(environment));
    options.release = Some(Cow::Owned(format!("agentgate@{}", agentgate_core::VERSION)));
    options.send_default_pii = false;
    options.attach_stacktrace = true;
    options.before_send = Some(std::sync::Arc::new(scrub));
    Some(sentry::init(options))
}

/// Sentry server aktif di proses ini.
#[must_use]
pub fn enabled() -> bool {
    sentry::Hub::current()
        .client()
        .is_some_and(|c| c.is_enabled())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use sentry::protocol::{Breadcrumb, User, Value};

    use super::*;

    #[test]
    fn scrub_removes_cookies_auth_headers_body_query_and_user() {
        let mut headers = BTreeMap::new();
        headers.insert(
            "Authorization".to_string(),
            "Bearer ag_pat_rahasia".to_string(),
        );
        headers.insert("Cookie".to_string(), "__Host-agentgate=sesi".to_string());
        headers.insert("X-Runner-Id".to_string(), "r1".to_string());
        headers.insert("User-Agent".to_string(), "Firefox".to_string());
        let mut crumb = Breadcrumb::default();
        crumb
            .data
            .insert("body".into(), Value::from("password=rahasia"));
        let event = Event {
            request: Some(Request {
                url: "https://ag.test/reset?token=rahasia#x".parse().ok(),
                cookies: Some("sesi=rahasia".into()),
                data: Some("password=rahasia".into()),
                query_string: Some("token=rahasia".into()),
                headers,
                ..Default::default()
            }),
            user: Some(User {
                email: Some("a@b.test".into()),
                ..Default::default()
            }),
            breadcrumbs: vec![crumb].into(),
            ..Default::default()
        };
        let out = scrub(event).unwrap();
        let req = out.request.unwrap();
        assert!(req.cookies.is_none() && req.data.is_none() && req.query_string.is_none());
        assert_eq!(req.url.unwrap().as_str(), "https://ag.test/reset");
        assert_eq!(req.headers.keys().collect::<Vec<_>>(), vec!["User-Agent"]);
        assert!(out.user.is_none());
        assert!(out.breadcrumbs.values[0].data.is_empty());
    }

    #[test]
    fn csp_gains_connect_src_only_for_sentry_origin() {
        let csp = "default-src 'self'; script-src 'self'; img-src 'self' data:";
        let origin = dsn_origin("https://abc@o1.ingest.sentry.io/42").unwrap();
        assert_eq!(origin, "https://o1.ingest.sentry.io");
        assert_eq!(
            csp_with_sentry(csp, &origin),
            "default-src 'self'; script-src 'self'; img-src 'self' data:; connect-src 'self' https://o1.ingest.sentry.io"
        );
        assert_eq!(
            csp_with_sentry("default-src 'self'; connect-src 'self'", &origin),
            "default-src 'self'; connect-src 'self' https://o1.ingest.sentry.io"
        );
        assert!(dsn_origin("").is_none());
        assert!(dsn_origin("bukan dsn").is_none());
        assert_eq!(
            dsn_origin("http://k@localhost:9000/1").unwrap(),
            "http://localhost:9000"
        );
    }
}
