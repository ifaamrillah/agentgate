//! Kontrak config: setiap environment yang di-deploy ter-parse dan punya bagian wajib.
//! Mengecek keberadaan dan bentuk, bukan nilai, kecuali CSP dan pengaturan keamanan.

use std::{path::Path, str::FromStr};

use agentgate_server::common::settings::Settings;
use loco_rs::{config::Config, environment::Environment};

const CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'";

/// Env var yang dibutuhkan staging/production; dev/test mendapatkannya dari bin/*.
const DEPLOY_ENV: &[(&str, &str)] = &[
    ("APP_URL", "https://agentgate.test"),
    ("SMTP_HOST", "smtp.agentgate.test"),
    ("SMTP_PORT", "587"),
    ("SMTP_USER", "user"),
    ("SMTP_PASSWORD", "secret"),
];

fn load(env: &str) -> loco_rs::Result<Config> {
    Config::from_folder(&Environment::from_str(env).unwrap(), Path::new("config"))
}

fn with_deploy_env<T>(f: impl FnOnce() -> T) -> T {
    for (k, v) in DEPLOY_ENV {
        unsafe { std::env::set_var(k, v) };
    }
    let out = f();
    for (k, _) in DEPLOY_ENV {
        unsafe { std::env::remove_var(k) };
    }
    out
}

#[test]
fn every_environment_config_has_required_sections() {
    with_deploy_env(|| {
        for env in ["development", "test", "staging", "production"] {
            let config = load(env).unwrap_or_else(|e| panic!("{env}: {e}"));
            assert!(config.queue.is_some(), "{env}: queue");
            assert!(config.mailer.is_some(), "{env}: mailer");
            let mw = &config.server.middlewares;
            assert!(
                mw.fallback.as_ref().is_some_and(|m| !m.enable),
                "{env}: fallback Loco harus mati"
            );
            assert!(
                mw.request_id.as_ref().is_some_and(|m| m.enable),
                "{env}: request_id"
            );
            assert!(
                mw.remote_ip.as_ref().is_some_and(|m| m.enable),
                "{env}: remote_ip"
            );
            let headers = mw.secure_headers.as_ref().expect("secure_headers");
            assert!(headers.enable, "{env}: secure_headers");
            let overrides = headers.overrides.as_ref().expect("overrides");
            assert_eq!(
                overrides.get("Content-Security-Policy").map(String::as_str),
                Some(CSP),
                "{env}: CSP"
            );
            Settings::from_json(config.settings.as_ref()).unwrap_or_else(|e| panic!("{env}: {e}"));
        }
    });
}

#[test]
fn deployed_environments_are_locked_down() {
    with_deploy_env(|| {
        for env in ["staging", "production"] {
            let config = load(env).unwrap();
            assert!(!config.database.auto_migrate, "{env}: auto_migrate");
            assert!(
                !config.database.dangerously_truncate,
                "{env}: dangerously_truncate"
            );
            assert!(
                !config.database.dangerously_recreate,
                "{env}: dangerously_recreate"
            );
            assert!(!config.logger.pretty_backtrace, "{env}: pretty_backtrace");
            let overrides = config
                .server
                .middlewares
                .secure_headers
                .unwrap()
                .overrides
                .unwrap();
            assert!(
                !overrides.contains_key("Strict-Transport-Security"),
                "{env}: HSTS dari preset"
            );
        }
    });
}

#[test]
fn development_config_without_database_url_fails_to_load() {
    let saved = std::env::var("DATABASE_URL").ok();
    unsafe { std::env::remove_var("DATABASE_URL") };
    let result = load("development");
    if let Some(v) = saved {
        unsafe { std::env::set_var("DATABASE_URL", v) };
    }
    assert!(
        result.is_err(),
        "development.yaml harus gagal tanpa DATABASE_URL"
    );
}

#[test]
fn test_config_never_reads_the_dev_database_url() {
    let raw = std::fs::read_to_string("config/test.yaml").unwrap();
    assert!(
        !raw.contains(r#"name="DATABASE_URL""#),
        "test.yaml harus memakai TEST_DATABASE_URL supaya tes tanpa bin/test tidak menyentuh database dev"
    );
    let config = load("test").unwrap();
    assert!(
        !config.database.dangerously_recreate,
        "recreate menghapus skema setiap boot"
    );
}
