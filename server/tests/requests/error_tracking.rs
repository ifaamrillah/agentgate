//! Error tracking Sentry (R1): mati tanpa DSN (tanpa meta/skrip, CSP tetap); dengan DSN,
//! layout memuat reporter browser dan CSP `connect-src` mengizinkan origin Sentry saja.

use agentgate_server::app::App;
use loco_rs::testing::prelude::*;

use crate::support::request_with_settings;

#[tokio::test]
async fn sentry_is_off_without_dsn() {
    request::<App, _, _>(|request, _ctx| async move {
        let res = request.get("/login").await;
        let csp = res
            .header("content-security-policy")
            .to_str()
            .unwrap()
            .to_string();
        assert!(!csp.contains("connect-src"), "{csp}");
        let html = res.text();
        assert_css_not_exists(&html, "meta[name='ag-sentry-dsn']");
        assert!(!html.contains("/static/js/sentry.js"));
    })
    .await;
}

#[tokio::test]
async fn sentry_dsn_adds_browser_reporter_and_connect_src() {
    request_with_settings(
        |s| {
            s["system"]["sentry_dsn"] = "https://publickey@o1.ingest.sentry.test/42".into();
            s["system"]["sentry_environment"] = "staging".into();
        },
        |request, _ctx| async move {
            let res = request.get("/login").await;
            let csp = res
                .header("content-security-policy")
                .to_str()
                .unwrap()
                .to_string();
            assert!(
                csp.ends_with("connect-src 'self' https://o1.ingest.sentry.test"),
                "{csp}"
            );
            assert!(csp.contains("script-src 'self';"), "{csp}");
            let html = res.text();
            assert_css_exists(
                &html,
                "meta[name='ag-sentry-dsn'][content='https://publickey@o1.ingest.sentry.test/42']",
            );
            assert_css_exists(
                &html,
                "meta[name='ag-sentry-environment'][content='staging']",
            );
            assert!(html.contains("/static/js/sentry.js"));
        },
    )
    .await;
}
