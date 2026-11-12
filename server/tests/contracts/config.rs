//! Kontrak config: setiap environment yang di-deploy ter-parse dan punya bagian wajib.
//! Mengecek keberadaan dan bentuk, bukan nilai, kecuali CSP dan pengaturan keamanan.

use std::{path::Path, str::FromStr};

use agentgate_server::common::settings::Settings;
use loco_rs::{
    config::{Config, QueueConfig, SessionStoreKind, WorkerMode},
    environment::Environment,
};

const CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; object-src 'none'; base-uri 'self'; form-action 'self' https://accounts.google.com https://github.com; frame-ancestors 'none'";

/// Env var yang dibutuhkan staging/production; dev/test mendapatkannya dari bin/*.
const DEPLOY_ENV: &[(&str, &str)] = &[
    ("APP_URL", "https://agentgate.test"),
    ("MAIL_FROM", "Agent Gate <noreply@agentgate.test>"),
    ("SMTP_HOST", "smtp.agentgate.test"),
    ("SMTP_PORT", "587"),
    ("SMTP_USER", "user"),
    ("SMTP_PASSWORD", "secret"),
    (
        "SESSION_SECRET",
        "deploy-session-secret-for-contract-tests-only-0123456789abcdef",
    ),
    // base64 dari 32 byte nol.
    (
        "ENCRYPTION_KEY",
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
    ),
    ("STORAGE_DIR", "/var/lib/agentgate/storage"),
];

/// Parameter argon2 minimal OWASP (m = 19 MiB, t = 2); staging/production tidak boleh di bawahnya.
const ARGON2_MIN_MEMORY_KIB: u32 = 19456;
const ARGON2_MIN_ITERATIONS: u32 = 2;

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
            let session = config
                .auth
                .as_ref()
                .and_then(|a| a.session.as_ref())
                .unwrap_or_else(|| panic!("{env}: auth.session"));
            assert_eq!(
                session.store,
                SessionStoreKind::Database,
                "{env}: auth.session.store"
            );
            assert!(session.secret.is_some(), "{env}: auth.session.secret");
            assert!(
                session.cookie_name.is_some(),
                "{env}: auth.session.cookie_name"
            );
            assert!(session.ttl.is_some(), "{env}: auth.session.ttl");
            assert!(
                session.remember_for.is_none(),
                "{env}: remember-me mati (remember_for tidak di-set)"
            );
            assert!(
                config.auth.as_ref().is_some_and(|a| a.jwt.is_none()),
                "{env}: tanpa auth.jwt (rules §7)"
            );
            Settings::from_json(config.settings.as_ref()).unwrap_or_else(|e| panic!("{env}: {e}"));
        }
    });
}

#[test]
fn deployed_environments_use_secure_host_cookies_and_baseline_argon2() {
    with_deploy_env(|| {
        for env in ["staging", "production"] {
            let config = load(env).unwrap();
            let session = config.auth.unwrap().session.unwrap();
            assert_eq!(session.secure, Some(true), "{env}: auth.session.secure");
            assert!(
                session
                    .cookie_name
                    .as_deref()
                    .is_some_and(|n| n.starts_with("__Host-")),
                "{env}: cookie_name berawalan __Host-"
            );
            let settings = Settings::from_json(config.settings.as_ref()).unwrap();
            assert!(
                settings.auth.argon2_memory_kib >= ARGON2_MIN_MEMORY_KIB,
                "{env}: argon2_memory_kib"
            );
            assert!(
                settings.auth.argon2_iterations >= ARGON2_MIN_ITERATIONS,
                "{env}: argon2_iterations"
            );
        }
    });
}

#[test]
fn deployed_environments_have_no_default_secrets() {
    for env in ["staging", "production"] {
        let raw = std::fs::read_to_string(format!("config/{env}.yaml")).unwrap();
        for name in ["SESSION_SECRET", "ENCRYPTION_KEY"] {
            assert!(
                raw.contains(&format!(r#"get_env(name="{name}") "#)),
                "{env}: {name} dibaca dari env tanpa default"
            );
        }
    }
}

const OAUTH_ENV: &[(&str, &str)] = &[
    ("GOOGLE_CLIENT_ID", "google-id"),
    ("GOOGLE_CLIENT_SECRET", "google-secret"),
    ("GITHUB_CLIENT_ID", "github-id"),
    ("GITHUB_CLIENT_SECRET", "github-secret"),
];

fn oauth_providers(env: &str) -> Vec<String> {
    let config = load(env).unwrap_or_else(|e| panic!("{env}: {e}"));
    let oauth = config
        .auth
        .and_then(|a| a.oauth)
        .unwrap_or_else(|| panic!("{env}: auth.oauth"));
    oauth.providers.keys().cloned().collect()
}

#[test]
fn oauth_providers_exist_only_when_their_client_id_is_set() {
    with_deploy_env(|| {
        for env in ["development", "test", "staging", "production"] {
            assert!(
                oauth_providers(env).is_empty(),
                "{env}: tanpa env var OAuth"
            );
        }
        unsafe { std::env::set_var("GITHUB_CLIENT_ID", "github-id") };
        unsafe { std::env::set_var("GITHUB_CLIENT_SECRET", "github-secret") };
        for env in ["development", "staging", "production"] {
            assert_eq!(oauth_providers(env), ["github"], "{env}");
        }
        for (k, v) in OAUTH_ENV {
            unsafe { std::env::set_var(k, v) };
        }
        for env in ["development", "staging", "production"] {
            assert_eq!(oauth_providers(env), ["github", "google"], "{env}");
            let config = load(env).unwrap();
            let oauth = config.auth.unwrap().oauth.unwrap();
            assert_eq!(oauth.providers["google"].client_secret, "google-secret");
            assert!(oauth.base_url.is_some(), "{env}: auth.oauth.base_url");
        }
        // Test tidak membaca env var OAuth: provider tes disuntikkan oleh tes sendiri.
        assert!(oauth_providers("test").is_empty());
        // Client id tanpa secret: config gagal di-load, bukan provider tanpa secret.
        unsafe { std::env::remove_var("GOOGLE_CLIENT_SECRET") };
        assert!(load("production").is_err());
        for (k, _) in OAUTH_ENV {
            unsafe { std::env::remove_var(k) };
        }
    });
}

#[test]
fn test_environment_stubs_mail_and_runs_jobs_inline() {
    let config = load("test").unwrap();
    assert!(
        config.mailer.as_ref().is_some_and(|m| m.stub),
        "test: mailer.stub"
    );
    assert!(
        matches!(config.workers.mode, WorkerMode::ForegroundBlocking),
        "test: workers.mode"
    );
}

#[test]
fn settings_reject_an_encryption_key_that_is_not_32_bytes() {
    let mut value = load_settings_value("test");
    for bad in [
        "not base64!",
        "AAAA",
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA==",
    ] {
        value["auth"]["encryption_key"] = serde_json::Value::String(bad.into());
        assert!(
            Settings::from_json(Some(&value)).is_err(),
            "encryption_key {bad:?} harus ditolak"
        );
    }
}

fn load_settings_value(env: &str) -> serde_json::Value {
    load(env).unwrap().settings.unwrap()
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

/// Job mailer membawa token mentah di args-nya (rules §8): job selesai harus dihapus, bukan
/// disimpan dengan status `completed`.
#[test]
fn deployed_environments_purge_finished_jobs() {
    with_deploy_env(|| {
        for env in ["development", "staging", "production"] {
            let Some(QueueConfig::Postgres(queue)) = load(env).unwrap().queue else {
                panic!("{env}: queue Postgres");
            };
            let retention = queue
                .retention
                .unwrap_or_else(|| panic!("{env}: queue.retention"));
            assert_eq!(retention.completed_days, Some(0), "{env}: completed_days");
            assert!(retention.failed_days.is_some(), "{env}: failed_days");
            assert!(
                retention.interval_seconds <= 600,
                "{env}: interval_seconds harus jauh di bawah umur token reset"
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

/// Scheduler (rules §8): job pembersihan harian 03:00 UTC di development/staging/production,
/// menjalankan task yang benar-benar terdaftar. Test tanpa scheduler (tes memanggil job-nya
/// langsung).
#[test]
fn deployed_environments_schedule_daily_cleanup() {
    with_deploy_env(|| {
        for env in ["development", "staging", "production"] {
            let config = load(env).unwrap_or_else(|e| panic!("{env}: {e}"));
            let scheduler = config
                .scheduler
                .as_ref()
                .unwrap_or_else(|| panic!("{env}: scheduler"));
            let job = scheduler
                .jobs
                .get("cleanup")
                .unwrap_or_else(|| panic!("{env}: job cleanup"));
            assert_eq!(job.run, "cleanup confirm:yes", "{env}");
            // Jam yang sama dengan yang ditampilkan banner hapus workspace (`PURGE_HOUR_UTC`).
            assert_eq!(
                job.cron,
                format!(
                    "0 0 {} * * *",
                    agentgate_server::models::workspaces::PURGE_HOUR_UTC
                ),
                "{env}: 03:00 UTC"
            );
            assert_eq!(job.cron, "0 0 3 * * *", "{env}: 03:00 UTC");
            assert!(!job.shell, "{env}");
            loco_rs::scheduler::Scheduler::new::<agentgate_server::app::App>(
                scheduler,
                &Environment::from_str(env).unwrap(),
            )
            .unwrap_or_else(|e| panic!("{env}: {e}"));
        }
        assert!(load("test").unwrap().scheduler.is_none());
    });
}
