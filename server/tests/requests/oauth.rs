//! OAuth 2.1 untuk MCP: metadata, DCR, authorize (login, consent, tolak), token (PKCE, kode
//! sekali pakai), refresh (rotasi, reuse mencabut grant), revoke, dan cabut dari Akun → Client AI.

use agentgate_server::{
    app::App,
    models::audit_events::{AuditAction, AuditSubject},
    services::oauth,
};
use loco_rs::testing::prelude::*;

use crate::support::{
    a_user, a_workspace, newest_audit_events,
    oauth_flow::{self, REDIRECT, VERIFIER, approve, authorize_params, exchange, register_client},
    sign_in_as,
};

fn query(params: &[(&str, String)]) -> String {
    serde_urlencoded::to_string(params).unwrap()
}

#[tokio::test]
async fn metadata_and_registration() {
    request::<App, _, _>(|request, _ctx| async move {
        let res = request.get("/.well-known/oauth-authorization-server").await;
        assert_eq!(res.status_code(), 200);
        let meta: serde_json::Value = res.json();
        let issuer = meta["issuer"].as_str().unwrap().to_string();
        assert_eq!(
            meta["authorization_endpoint"],
            format!("{issuer}/oauth/authorize")
        );
        assert_eq!(meta["token_endpoint"], format!("{issuer}/oauth/token"));
        assert_eq!(
            meta["registration_endpoint"],
            format!("{issuer}/oauth/register")
        );
        assert_eq!(meta["code_challenge_methods_supported"][0], "S256");

        for path in [
            "/.well-known/oauth-protected-resource",
            "/.well-known/oauth-protected-resource/mcp",
        ] {
            let res = request.get(path).await;
            assert_eq!(res.status_code(), 200, "{path}");
            let meta: serde_json::Value = res.json();
            assert_eq!(meta["resource"], format!("{issuer}/mcp"));
            assert_eq!(meta["authorization_servers"][0], issuer);
        }

        // Redirect bukan https/loopback/skema privat → 400 JSON.
        let res = request
            .post("/oauth/register")
            .json(&serde_json::json!({
                "client_name": "Jahat",
                "redirect_uris": ["http://evil.example/cb"],
            }))
            .await;
        assert_eq!(res.status_code(), 400);
        assert_eq!(
            res.json::<serde_json::Value>()["error"],
            "invalid_redirect_uri"
        );

        // Client confidential tidak didukung.
        let res = request
            .post("/oauth/register")
            .json(&serde_json::json!({
                "redirect_uris": [REDIRECT],
                "token_endpoint_auth_method": "client_secret_basic",
            }))
            .await;
        assert_eq!(res.status_code(), 400);

        // Browser dengan Origin asing tetap bisa mendaftar (bukan cookie → dikecualikan CSRF).
        let res = request
            .post("/oauth/register")
            .add_header("origin", "https://claude.ai")
            .json(&serde_json::json!({
                "client_name": "Claude Code",
                "redirect_uris": [REDIRECT, "https://claude.ai/api/mcp/auth_callback"],
            }))
            .await;
        assert_eq!(res.status_code(), 201, "{}", res.text());
        let body: serde_json::Value = res.json();
        assert!(body["client_id"].as_str().unwrap().starts_with("ag_"));
        assert_eq!(body["token_endpoint_auth_method"], "none");
        assert_eq!(res.header("cache-control"), "no-store");
    })
    .await;
}

#[tokio::test]
async fn authorize_requires_login_and_validates_request() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().name("Olivia Owner").create(&ctx).await;
        a_workspace(&owner)
            .name("PT Contoh")
            .slug("contoh")
            .create(&ctx)
            .await;
        let client_id = register_client(&request, "Claude Code").await;
        let url = format!("/oauth/authorize?{}", query(&authorize_params(&client_id)));

        // Tanpa login → halaman login dengan return_to.
        let res = request.get(&url).await;
        assert_eq!(res.status_code(), 303);
        let location = res.header("location").to_str().unwrap().to_string();
        assert!(location.starts_with("/login?return_to="), "{location}");

        let c = sign_in_as(&request, &owner).await;
        let page = request.get(&url).add_cookie(c.clone()).await;
        assert_eq!(page.status_code(), 200, "{}", page.text());
        let html = page.text();
        assert_css_exists(
            &html,
            "form[data-oauth-consent][action=\"/oauth/authorize\"]",
        );
        assert_css_exists(&html, "select[name=workspace]");
        assert_css_exists(&html, "input[type=hidden][name=code_challenge]");
        assert!(html.contains("Claude Code"));
        assert!(html.contains("PT Contoh"));
        // Redirect ke client setelah submit tidak diblokir CSP form-action.
        let csp = page
            .header("content-security-policy")
            .to_str()
            .unwrap()
            .to_string();
        assert!(
            csp.contains("form-action 'self' http://127.0.0.1:53682;"),
            "{csp}"
        );

        // Client tidak dikenal / redirect tidak terdaftar → halaman error, tanpa redirect.
        let mut bad = authorize_params(&client_id);
        bad[2].1 = "http://127.0.0.1:9/lain".into();
        let res = request
            .get(&format!("/oauth/authorize?{}", query(&bad)))
            .add_cookie(c.clone())
            .await;
        assert_eq!(res.status_code(), 400);
        assert_css_exists(&res.text(), "[data-oauth-error]");

        // Tanpa PKCE → redirect ke client dengan error.
        let mut no_pkce = authorize_params(&client_id);
        no_pkce.retain(|(k, _)| !k.starts_with("code_challenge"));
        let res = request
            .get(&format!("/oauth/authorize?{}", query(&no_pkce)))
            .add_cookie(c.clone())
            .await;
        assert_eq!(res.status_code(), 303);
        let location = res.header("location").to_str().unwrap().to_string();
        assert!(location.starts_with(REDIRECT));
        assert_eq!(
            oauth_flow::query_param(&location, "error").as_deref(),
            Some("invalid_request")
        );
        assert_eq!(
            oauth_flow::query_param(&location, "state").as_deref(),
            Some("xyz")
        );

        // Tolak → access_denied.
        let mut form = authorize_params(&client_id);
        form.push(("decision", "deny".into()));
        let res = request
            .post("/oauth/authorize")
            .add_cookie(c.clone())
            .form(&form)
            .await;
        assert_eq!(res.status_code(), 303);
        let location = res.header("location").to_str().unwrap().to_string();
        assert_eq!(
            oauth_flow::query_param(&location, "error").as_deref(),
            Some("access_denied")
        );

        // Izinkan tanpa workspace / workspace orang lain → 422.
        let stranger = a_user().create(&ctx).await;
        let other = a_workspace(&stranger).slug("lain").create(&ctx).await;
        for ws in [String::new(), other.id.to_string()] {
            let mut form = authorize_params(&client_id);
            form.push(("decision", "allow".into()));
            form.push(("workspace", ws));
            let res = request
                .post("/oauth/authorize")
                .add_cookie(c.clone())
                .form(&form)
                .await;
            assert_eq!(res.status_code(), 422);
        }
    })
    .await;
}

#[tokio::test]
async fn code_exchange_refresh_rotation_and_revocation() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let c = sign_in_as(&request, &owner).await;
        let client_id = register_client(&request, "Claude Code").await;
        let code = approve(&request, &c, &client_id, &workspace.id.to_string()).await;

        // PKCE salah → invalid_grant.
        let res = request
            .post("/oauth/token")
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code.as_str()),
                ("redirect_uri", REDIRECT),
                ("client_id", client_id.as_str()),
                ("code_verifier", "x".repeat(43).as_str()),
            ])
            .await;
        assert_eq!(res.status_code(), 400);
        assert_eq!(res.json::<serde_json::Value>()["error"], "invalid_grant");
        // M12: percobaan gagal membatalkan kode; verifier benar pun ditolak sesudahnya.
        let res = request
            .post("/oauth/token")
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code.as_str()),
                ("redirect_uri", REDIRECT),
                ("client_id", client_id.as_str()),
                ("code_verifier", VERIFIER),
            ])
            .await;
        assert_eq!(res.status_code(), 400);
        assert_eq!(res.json::<serde_json::Value>()["error"], "invalid_grant");
        let code = approve(&request, &c, &client_id, &workspace.id.to_string()).await;

        let tokens = exchange(&request, &client_id, &code).await;
        assert_eq!(tokens["token_type"], "Bearer");
        let access = tokens["access_token"].as_str().unwrap().to_string();
        let refresh = tokens["refresh_token"].as_str().unwrap().to_string();
        let principal = oauth::authenticate(&ctx.db, &access)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(principal.user_id, owner.id);
        assert_eq!(principal.workspace_id, workspace.id);
        let events = newest_audit_events(&ctx.db, workspace.workspace_id(), 1)
            .await
            .unwrap();
        assert_eq!(events[0].action().unwrap(), AuditAction::McpConnected);
        assert_eq!(events[0].subject().unwrap(), AuditSubject::McpClient);

        // Refresh → rotasi.
        let res = request
            .post("/oauth/token")
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh.as_str()),
                ("client_id", client_id.as_str()),
            ])
            .await;
        assert_eq!(res.status_code(), 200, "{}", res.text());
        let rotated: serde_json::Value = res.json();
        let access2 = rotated["access_token"].as_str().unwrap().to_string();
        assert_ne!(rotated["refresh_token"], refresh.as_str());
        assert!(
            oauth::authenticate(&ctx.db, &access2)
                .await
                .unwrap()
                .is_some()
        );

        // Refresh lama dipakai ulang → grant dicabut, semua token mati.
        let res = request
            .post("/oauth/token")
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh.as_str()),
                ("client_id", client_id.as_str()),
            ])
            .await;
        assert_eq!(res.status_code(), 400);
        assert!(
            oauth::authenticate(&ctx.db, &access2)
                .await
                .unwrap()
                .is_none()
        );

        // Kode dipakai ulang → invalid_grant dan grant (baru) dicabut.
        let code = approve(&request, &c, &client_id, &workspace.id.to_string()).await;
        let tokens = exchange(&request, &client_id, &code).await;
        let access3 = tokens["access_token"].as_str().unwrap().to_string();
        assert!(
            oauth::authenticate(&ctx.db, &access3)
                .await
                .unwrap()
                .is_some()
        );
        let res = request
            .post("/oauth/token")
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code.as_str()),
                ("redirect_uri", REDIRECT),
                ("client_id", client_id.as_str()),
                ("code_verifier", VERIFIER),
            ])
            .await;
        assert_eq!(res.status_code(), 400);
        assert!(
            oauth::authenticate(&ctx.db, &access3)
                .await
                .unwrap()
                .is_none()
        );

        // Revoke (RFC 7009): refresh → grant dicabut; token tak dikenal tetap 200.
        let (_, access4, refresh4) =
            oauth_flow::connect(&request, &c, &workspace.id.to_string()).await;
        let res = request
            .post("/oauth/revoke")
            .form(&[("token", "nope")])
            .await;
        assert_eq!(res.status_code(), 200);
        let res = request
            .post("/oauth/revoke")
            .form(&[("token", refresh4.as_str())])
            .await;
        assert_eq!(res.status_code(), 200);
        assert!(
            oauth::authenticate(&ctx.db, &access4)
                .await
                .unwrap()
                .is_none()
        );

        // grant_type lain → unsupported_grant_type.
        let res = request
            .post("/oauth/token")
            .form(&[("grant_type", "password")])
            .await;
        assert_eq!(res.status_code(), 400);
        assert_eq!(
            res.json::<serde_json::Value>()["error"],
            "unsupported_grant_type"
        );
    })
    .await;
}

#[tokio::test]
async fn connected_clients_are_listed_and_revocable_from_account() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner)
            .name("PT Contoh")
            .slug("contoh")
            .create(&ctx)
            .await;
        let c = sign_in_as(&request, &owner).await;
        let (_, access, _) = oauth_flow::connect(&request, &c, &workspace.id.to_string()).await;

        let page = request
            .get("/account/connect?section=clients")
            .add_cookie(c.clone())
            .await;
        assert_eq!(page.status_code(), 200, "{}", page.text());
        let html = page.text();
        assert_css_exists(&html, ".app-side a[href=\"/account/connect\"]");
        assert_css_exists(&html, "[data-connect-clients] [data-client='Claude Code']");
        let grants = oauth::grants_of(&ctx.db, owner.id).await.unwrap();
        let grant = &grants[0].0;
        let action = format!("/account/connect/{}/revoke", grant.id);
        assert_css_exists(&html, &format!("form[action=\"{action}\"]"));

        for section in ["steps", "tools", "slash"] {
            let page = request
                .get(&format!("/account/connect?section={section}"))
                .add_cookie(c.clone())
                .await;
            assert_eq!(page.status_code(), 200, "{section}");
            assert_css_exists(&page.text(), &format!("[data-connect='{section}']"));
        }

        // Langkah pemasangan untuk keenam client (aturan v2).
        let steps = request
            .get("/account/connect?section=steps")
            .add_cookie(c.clone())
            .await
            .text();
        assert_css_exists(&steps, "[data-init-command]");
        for client in agentgate_core::persona::CLIENTS {
            assert_css_exists(&steps, &format!("[data-mcp-command='{}']", client.key));
            assert_css_exists(&steps, &format!("[data-login-client='{}']", client.key));
        }
        assert!(steps.contains("codex mcp login agentgate"));
        assert!(steps.contains("~/.cursor/mcp.json"));
        assert!(
            !steps.contains("connect-login-"),
            "kunci Fluent tanpa terjemahan"
        );

        // User lain tidak bisa mencabut grant ini.
        let other = a_user().create(&ctx).await;
        let oc = sign_in_as(&request, &other).await;
        let res = request.post(&action).add_cookie(oc).await;
        assert_eq!(res.status_code(), 404);

        let res = request.post(&action).add_cookie(c.clone()).await;
        assert_eq!(res.status_code(), 303);
        assert!(
            oauth::authenticate(&ctx.db, &access)
                .await
                .unwrap()
                .is_none()
        );
        let events = newest_audit_events(&ctx.db, workspace.workspace_id(), 1)
            .await
            .unwrap();
        assert_eq!(events[0].action().unwrap(), AuditAction::McpRevoked);
        let page = request
            .get("/account/connect?section=clients")
            .add_cookie(c.clone())
            .await;
        assert_css_not_exists(&page.text(), "[data-client]");
    })
    .await;
}

/// M12: pendaftaran client dinamis dibatasi 20 per IP per jam; yang ke-21 → 429 + Retry-After.
#[tokio::test]
async fn dynamic_client_registration_is_rate_limited_per_ip() {
    request::<App, _, _>(|request, _ctx| async move {
        for _ in 0..20 {
            register_client(&request, "Claude Code").await;
        }
        let res = request
            .post("/oauth/register")
            .json(&serde_json::json!({
                "client_name": "Banjir",
                "redirect_uris": [REDIRECT],
            }))
            .await;
        assert_eq!(res.status_code(), 429);
        assert_eq!(
            res.json::<serde_json::Value>()["error"],
            "too_many_requests"
        );
        assert_eq!(res.header("retry-after"), "3600");
    })
    .await;
}

/// R1: client dinamis baru tidak mendapat token sebelum disetujui user; persetujuan consent
/// pertama menandainya, dan client yang persetujuannya hilang tidak bisa merotasi token.
#[tokio::test]
async fn client_needs_user_approval_before_any_token() {
    use agentgate_server::models::oauth_clients;
    use sea_orm::{ActiveModelTrait, ActiveValue::Set, IntoActiveModel};
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let ws = a_workspace(&owner).slug("acme").create(&ctx).await;
        let cookie = sign_in_as(&request, &owner).await;
        let client_id = register_client(&request, "Klien baru").await;
        let client = oauth::find_client(&ctx.db, &client_id)
            .await
            .unwrap()
            .unwrap();
        assert!(client.approved_at.is_none());

        let code = approve(&request, &cookie, &client_id, &ws.id.to_string()).await;
        let client = oauth::find_client(&ctx.db, &client_id)
            .await
            .unwrap()
            .unwrap();
        assert!(client.approved_at.is_some());
        let t = exchange(&request, &client_id, &code).await;

        // Persetujuan dicabut (mis. baris dipulihkan tanpa approved_at): refresh ditolak.
        let mut m: oauth_clients::ActiveModel = client.into_active_model();
        m.approved_at = Set(None);
        m.update(&ctx.db).await.unwrap();
        let res = request
            .post("/oauth/token")
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", t["refresh_token"].as_str().unwrap()),
                ("client_id", client_id.as_str()),
            ])
            .await;
        assert_eq!(res.status_code(), 400, "{}", res.text());
        assert_eq!(res.json::<serde_json::Value>()["error"], "invalid_grant");
    })
    .await;
}
