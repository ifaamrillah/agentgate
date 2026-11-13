//! Login CLI lewat device code flow (RFC 8628): mulai, halaman `/device` (login wajib, kode
//! salah 422, setujui/tolak), polling token (`authorization_pending`, `slow_down`,
//! `access_denied`, `expired_token`, sekali pakai).

use agentgate_server::{app::App, models::oauth_device_codes, services::oauth};
use loco_rs::{TestServer, testing::prelude::*};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait, IntoActiveModel};
use serde_json::Value;

use crate::support::{a_user, a_workspace, sign_in_as};

const GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";

async fn start(request: &TestServer) -> Value {
    let res = request
        .post("/oauth/device_authorization")
        .form(&[
            ("client_id", "agentgate-cli"),
            ("device_name", "MacBook Olivia"),
        ])
        .await;
    assert_eq!(res.status_code(), 200, "{}", res.text());
    assert_eq!(res.header("cache-control"), "no-store");
    res.json()
}

async fn poll(request: &TestServer, device_code: &str) -> (u16, Value) {
    let res = request
        .post("/oauth/token")
        .form(&[
            ("grant_type", GRANT),
            ("client_id", "agentgate-cli"),
            ("device_code", device_code),
        ])
        .await;
    (res.status_code().as_u16(), res.json())
}

#[tokio::test]
async fn unknown_client_is_rejected_and_metadata_lists_device_endpoint() {
    request::<App, _, _>(|request, _ctx| async move {
        let res = request
            .post("/oauth/device_authorization")
            .form(&[("client_id", "lain")])
            .await;
        assert_eq!(res.status_code(), 400);
        assert_eq!(res.json::<Value>()["error"], "invalid_client");
        let meta: Value = request
            .get("/.well-known/oauth-authorization-server")
            .await
            .json();
        assert!(
            meta["device_authorization_endpoint"]
                .as_str()
                .unwrap()
                .ends_with("/oauth/device_authorization")
        );
        assert!(
            meta["grant_types_supported"]
                .as_array()
                .unwrap()
                .contains(&Value::from(GRANT))
        );
    })
    .await;
}

#[tokio::test]
async fn approve_then_poll_issues_token_once() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().name("Olivia Owner").create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let cookie = sign_in_as(&request, &owner).await;
        let s = start(&request).await;
        let user_code = s["user_code"].as_str().unwrap().to_string();
        let device_code = s["device_code"].as_str().unwrap().to_string();
        assert_eq!(user_code.len(), 9, "{user_code}");
        assert!(s["verification_uri"].as_str().unwrap().ends_with("/device"));
        assert_eq!(s["interval"], 5);

        // Belum disetujui, lalu polling terlalu cepat.
        let (status, body) = poll(&request, &device_code).await;
        assert_eq!(
            (status, body["error"].as_str()),
            (400, Some("authorization_pending"))
        );
        let (_, body) = poll(&request, &device_code).await;
        assert_eq!(body["error"], "slow_down");

        // Halaman /device butuh login.
        let res = request.get("/device").await;
        assert_eq!(res.status_code(), 303);
        let res = request.get("/device").add_cookie(cookie.clone()).await;
        assert_eq!(res.status_code(), 200);
        assert_css_exists(&res.text(), "[data-device=enter] input[name=user_code]");

        // Kode salah → 422; kode benar (huruf kecil, tanpa strip) → halaman setujui.
        let res = request
            .post("/device")
            .add_cookie(cookie.clone())
            .form(&[("user_code", "BBBB-BBBB")])
            .await;
        assert_eq!(res.status_code(), 422);
        let typed = user_code.replace('-', "").to_lowercase();
        let res = request
            .post("/device")
            .add_cookie(cookie.clone())
            .form(&[("user_code", typed.as_str())])
            .await;
        assert_eq!(res.status_code(), 200);
        assert_css_exists(&res.text(), "[data-device=confirm]");
        assert!(res.text().contains("MacBook Olivia"));
        assert!(res.text().contains(&user_code));

        // Tanpa workspace → 422, lalu setujui.
        let res = request
            .post("/device/decide")
            .add_cookie(cookie.clone())
            .form(&[("user_code", user_code.as_str()), ("decision", "allow")])
            .await;
        assert_eq!(res.status_code(), 422);
        let wid = workspace.id.to_string();
        let res = request
            .post("/device/decide")
            .add_cookie(cookie.clone())
            .form(&[
                ("user_code", user_code.as_str()),
                ("workspace", wid.as_str()),
                ("decision", "allow"),
            ])
            .await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(res.header("location"), "/device?done=approved");
        let res = request
            .get("/device?done=approved")
            .add_cookie(cookie.clone())
            .await;
        assert_css_exists(&res.text(), "[data-device=done]");

        // Kode yang sudah disetujui tidak bisa dipakai lagi di halaman.
        let res = request
            .post("/device")
            .add_cookie(cookie.clone())
            .form(&[("user_code", user_code.as_str())])
            .await;
        assert_eq!(res.status_code(), 422);

        let (status, body) = poll(&request, &device_code).await;
        assert_eq!(status, 200, "{body}");
        let token = body["access_token"].as_str().unwrap();
        assert!(body["refresh_token"].is_string());
        let principal = oauth::authenticate(&ctx.db, token).await.unwrap().unwrap();
        assert_eq!(principal.user_id, owner.id);
        assert_eq!(principal.workspace_id, workspace.id);
        assert_eq!(principal.client_name, "Agent Gate CLI");

        // Sekali pakai.
        let (_, body) = poll(&request, &device_code).await;
        assert_eq!(body["error"], "invalid_grant");
    })
    .await;
}

#[tokio::test]
async fn denied_and_expired_codes() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        a_workspace(&owner).slug("acme").create(&ctx).await;
        let cookie = sign_in_as(&request, &owner).await;

        let s = start(&request).await;
        let res = request
            .post("/device/decide")
            .add_cookie(cookie.clone())
            .form(&[
                ("user_code", s["user_code"].as_str().unwrap()),
                ("decision", "deny"),
            ])
            .await;
        assert_eq!(res.header("location"), "/device?done=denied");
        let (_, body) = poll(&request, s["device_code"].as_str().unwrap()).await;
        assert_eq!(body["error"], "access_denied");

        let s = start(&request).await;
        for row in oauth_device_codes::Entity::find()
            .all(&ctx.db)
            .await
            .unwrap()
        {
            let mut m = row.into_active_model();
            m.expires_at = Set((chrono::Utc::now() - chrono::Duration::minutes(1)).into());
            m.update(&ctx.db).await.unwrap();
        }
        let (_, body) = poll(&request, s["device_code"].as_str().unwrap()).await;
        assert_eq!(body["error"], "expired_token");
        let res = request
            .post("/device")
            .add_cookie(cookie.clone())
            .form(&[("user_code", s["user_code"].as_str().unwrap())])
            .await;
        assert_eq!(res.status_code(), 422);
    })
    .await;
}
