//! `agentgate login`: device flow dengan polling `authorization_pending` → `slow_down` → token.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use serde_json::{Value, json};

use crate::support::{Env, serve, text};

const DEVICE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";

#[tokio::test(flavor = "multi_thread")]
async fn device_login_polls_until_approved_and_saves_private_credentials() {
    let polls = Arc::new(AtomicUsize::new(0));
    let counter = polls.clone();
    let fake = serve(Arc::new(move |fake, req| {
        match (req.method.as_str(), req.path.as_str()) {
            ("POST", "/oauth/device_authorization") => (
                200,
                json!({
                    "device_code": "dc-123",
                    "user_code": "WDJB-MJHT",
                    "verification_uri": format!("{}/device", fake.base),
                    "verification_uri_complete": format!("{}/device?code=WDJB-MJHT", fake.base),
                    "expires_in": 900,
                    "interval": 1
                }),
            ),
            ("POST", "/oauth/token") => match counter.fetch_add(1, Ordering::SeqCst) {
                0 => (400, json!({ "error": "authorization_pending" })),
                1 => (400, json!({ "error": "slow_down" })),
                _ => (
                    200,
                    json!({
                        "access_token": "at-1",
                        "token_type": "Bearer",
                        "expires_in": 3600,
                        "refresh_token": "rt-1",
                        "scope": "mcp"
                    }),
                ),
            },
            ("GET", "/api/cli/me")
                if req.headers.get("authorization").map(String::as_str) == Some("Bearer at-1") =>
            {
                (
                    200,
                    json!({
                        "user": "Ani",
                        "email": "ani@example.com",
                        "workspace": "Kasir",
                        "slug": "kasir",
                        "server": fake.base,
                        "version": "0.1.0"
                    }),
                )
            }
            ("GET", "/api/cli/me") => (401, json!({ "error": "unauthorized" })),
            _ => (404, json!({ "error": "not_found" })),
        }
    }))
    .await;
    let env = Env::new("login");
    let out = env.run(&env.root, &["login", "--server", &fake.base]).await;
    assert!(out.status.success(), "{}", text(&out));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("WDJB-MJHT"), "{stdout}");
    assert!(stdout.contains("/device?code=WDJB-MJHT"), "{stdout}");
    assert!(stdout.contains("ani@example.com"), "{stdout}");

    let start = fake.find("POST", "/oauth/device_authorization");
    assert_eq!(start.len(), 1);
    assert_eq!(start[0].form()["client_id"], "agentgate-cli");
    assert!(start[0].form().contains_key("device_name"));

    let tokens = fake.find("POST", "/oauth/token");
    assert_eq!(tokens.len(), 3);
    for t in &tokens {
        let form = t.form();
        assert_eq!(form["grant_type"], DEVICE_GRANT);
        assert_eq!(form["client_id"], "agentgate-cli");
        assert_eq!(form["device_code"], "dc-123");
    }
    // slow_down menambah 5 satuan interval (1 → 6): jeda poll terakhir jauh lebih panjang.
    let gap = tokens[2].at - tokens[1].at;
    assert!(gap >= std::time::Duration::from_millis(30), "{gap:?}");

    let path = env.config.join("credentials.json");
    let creds: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(creds["server"], fake.base);
    assert_eq!(creds["access_token"], "at-1");
    assert_eq!(creds["refresh_token"], "rt-1");
    assert_eq!(creds["email"], "ani@example.com");
    assert_eq!(creds["workspace"], "Kasir");
    assert!(creds["expires_at"].is_string());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    let out = env.run(&env.root, &["logout"]).await;
    assert!(out.status.success(), "{}", text(&out));
    assert!(!path.exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn denied_device_login_fails_without_saving_credentials() {
    let fake = serve(Arc::new(|fake, req| match req.path.as_str() {
        "/oauth/device_authorization" => (
            200,
            json!({
                "device_code": "dc",
                "user_code": "AAAA-BBBB",
                "verification_uri": format!("{}/device", fake.base),
                "verification_uri_complete": "",
                "expires_in": 900,
                "interval": 1
            }),
        ),
        "/oauth/token" => (400, json!({ "error": "access_denied" })),
        _ => (404, Value::Null),
    }))
    .await;
    let env = Env::new("login-denied");
    let out = env.run(&env.root, &["login", "--server", &fake.base]).await;
    assert!(!out.status.success(), "{}", text(&out));
    assert!(String::from_utf8_lossy(&out.stderr).contains("ditolak"));
    assert!(!env.config.join("credentials.json").exists());
}
