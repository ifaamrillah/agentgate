//! GitHub App (R2) terhadap GitHub palsu (`support::fake_github`) dan kunci RSA tes: alur
//! instalasi (state bertanda tangan, verifikasi user lewat OAuth code), token instalasi berumur
//! pendek yang di-cache, repo yang diberi akses, webhook App satu endpoint, dan PAT tetap jalan.

use agentgate_server::{
    common::settings::Settings,
    models::{git_connections, pull_requests, repos, roles::RoleKey},
    services::git::{self as svc, webhook::sign},
};
use loco_rs::{TestServer, app::AppContext};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::{Value, json};

use super::tickets::{Fixture, fixture};
use crate::support::{
    fake_github::{
        self, APP_CLIENT_ID, APP_CLIENT_SECRET, APP_CODE, APP_ID, APP_KEY_PEM, APP_SLUG,
        APP_WEBHOOK_SECRET, FakeGithub, INSTALLATION,
    },
    request_with_settings,
};

const REPO: &str = "pt-contoh/kasir-api";

fn app_settings(base: String, with_app: bool) -> impl FnOnce(&mut Value) {
    move |s| {
        let mut git =
            json!({ "github_api_base": base, "github_web_base": base, "timeout_seconds": 5 });
        if with_app {
            git["app"] = json!({
                "app_id": APP_ID, "slug": APP_SLUG, "client_id": APP_CLIENT_ID,
                "client_secret": APP_CLIENT_SECRET, "private_key": APP_KEY_PEM,
                "webhook_secret": APP_WEBHOOK_SECRET,
            });
        }
        s["git"] = git;
    }
}

async fn add_repo(request: &TestServer, f: &Fixture) {
    let res = request
        .post("/w/acme/p/KSR/settings/repos")
        .add_cookie(f.cookie.clone())
        .form(&[
            ("name", "kasir-api"),
            ("provider", "github"),
            ("url", "https://github.com/pt-contoh/kasir-api"),
            ("default_branch", "main"),
        ])
        .await;
    assert_eq!(res.status_code(), 303, "{}", res.text());
}

fn location(res: &axum_test::TestResponse) -> String {
    res.headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string()
}

fn query_param(url: &str, key: &str) -> String {
    url.split_once('?')
        .map(|(_, q)| q)
        .unwrap_or_default()
        .split('&')
        .find_map(|kv| kv.strip_prefix(&format!("{key}=")))
        .unwrap_or_default()
        .to_string()
}

/// Mulai instalasi → state dari URL GitHub.
async fn start_install(request: &TestServer, f: &Fixture, gh: &FakeGithub) -> String {
    let res = request
        .post("/w/acme/settings/git/app/install")
        .add_cookie(f.cookie.clone())
        .await;
    assert_eq!(res.status_code(), 303, "{}", res.text());
    let to = location(&res);
    assert!(
        to.starts_with(&format!(
            "{}/apps/{APP_SLUG}/installations/new?state=",
            gh.base_url
        )),
        "{to}"
    );
    query_param(&to, "state")
}

/// Callback GitHub (setup URL global) → callback workspace → halaman Cloud.
async fn finish_install(
    request: &TestServer,
    f: &Fixture,
    installation: i64,
    code: &str,
    state: &str,
) -> axum_test::TestResponse {
    let res = request
        .get(&format!(
            "/git/github-app/callback?installation_id={installation}&setup_action=install&code={code}&state={state}"
        ))
        .add_cookie(f.cookie.clone())
        .await;
    assert_eq!(res.status_code(), 303, "{}", res.text());
    let to = location(&res);
    assert!(to.starts_with("/w/acme/settings/git/app/callback?"), "{to}");
    request.get(&to).add_cookie(f.cookie.clone()).await
}

async fn installed(
    request: &TestServer,
    ctx: &AppContext,
    f: &Fixture,
    gh: &FakeGithub,
) -> git_connections::Model {
    gh.add_repo(REPO);
    gh.set_app_repos(&[REPO]);
    add_repo(request, f).await;
    let state = start_install(request, f, gh).await;
    let res = finish_install(request, f, INSTALLATION, APP_CODE, &state).await;
    assert_eq!(res.status_code(), 303, "{}", res.text());
    assert_eq!(location(&res), "/w/acme/settings/git/cloud");
    git_connections::Entity::find()
        .one(&ctx.db)
        .await
        .unwrap()
        .expect("koneksi App tersimpan")
}

async fn app_webhook(request: &TestServer, event: &str, secret: &str, payload: &Value) -> u16 {
    let body = serde_json::to_vec(payload).unwrap();
    request
        .post("/webhooks/github-app")
        .add_header("x-github-event", event.to_string())
        .add_header("x-hub-signature-256", sign(secret, &body))
        .add_header("content-type", "application/json")
        .bytes(body.into())
        .await
        .status_code()
        .as_u16()
}

#[tokio::test]
async fn without_app_config_only_the_token_option_is_available() {
    let gh = fake_github::start().await;
    request_with_settings(
        app_settings(gh.base_url.clone(), false),
        |request, ctx| async move {
            let f = fixture(&request, &ctx).await;
            let page = request
                .get("/w/acme/settings/git/cloud")
                .add_cookie(f.cookie.clone())
                .await;
            assert_eq!(page.status_code(), 200);
            let html = page.text();
            assert!(
                html.contains("data-way=\"app\" aria-disabled=\"true\""),
                "{html}"
            );
            assert!(html.contains("data-app-unavailable"));
            assert!(html.contains("data-way=\"token\""));
            let res = request
                .post("/w/acme/settings/git/app/install")
                .add_cookie(f.cookie.clone())
                .await;
            assert_eq!(res.status_code(), 303);
            assert_eq!(location(&res), "/w/acme/settings/git/cloud");
        },
    )
    .await;
}

#[tokio::test]
async fn installing_the_app_connects_github_with_granted_repos() {
    let gh = fake_github::start().await;
    request_with_settings(
        app_settings(gh.base_url.clone(), true),
        |request, ctx| async move {
            let f = fixture(&request, &ctx).await;
            let page = request
                .get("/w/acme/settings/git/cloud")
                .add_cookie(f.cookie.clone())
                .await;
            assert!(page.text().contains("data-app-install-form"));

            let conn = installed(&request, &ctx, &f, &gh).await;
            assert!(conn.is_app());
            assert_eq!(conn.installation_id, Some(INSTALLATION));
            assert_eq!(conn.account, "pt-contoh");
            assert_eq!(conn.app_account_type, "Organization");
            assert_eq!(conn.app_repos(), vec![REPO.to_string()]);
            assert_eq!(conn.app_permissions["contents"], "write");
            // App tidak menyimpan token apa pun dan tidak memasang webhook per repo.
            let key = Settings::from_ctx(&ctx)
                .unwrap()
                .auth
                .encryption_key_bytes()
                .unwrap();
            assert_eq!(conn.token(&key).unwrap(), "");
            assert!(gh.hooks().is_empty());
            let repo = repos::Entity::find().one(&ctx.db).await.unwrap().unwrap();
            assert_eq!(repo.connection_id, Some(conn.id));
            assert_eq!(repo.full_name, REPO);
            assert_eq!(repo.webhook_error, "");

            let page = request
                .get("/w/acme/settings/git/cloud")
                .add_cookie(f.cookie.clone())
                .await
                .text();
            assert!(page.contains("data-via-app"), "{page}");
            assert!(page.contains("data-app-refresh"));
            assert!(page.contains(&format!(
                "/organizations/pt-contoh/settings/installations/{INSTALLATION}"
            )));
            assert!(!page.contains("id=\"hook-github\""), "tanpa webhook manual");
            let repos_page = request
                .get("/w/acme/settings/git/repos")
                .add_cookie(f.cookie.clone())
                .await
                .text();
            assert!(repos_page.contains("data-webhook=\"app\""), "{repos_page}");

            // Token instalasi di-cache: panggilan provider berikutnya tidak meminta token baru.
            let settings = Settings::from_ctx(&ctx).unwrap();
            let issued = gh.app_tokens_issued();
            let p = svc::provider_for(&conn, &settings).await.unwrap();
            assert_eq!(p.repo(REPO).await.unwrap().default_branch, "main");
            let p = svc::provider_for(&conn, &settings).await.unwrap();
            p.repo(REPO).await.unwrap();
            assert_eq!(gh.app_tokens_issued(), issued);
        },
    )
    .await;
}

#[tokio::test]
async fn install_callback_rejects_foreign_installation_and_bad_state() {
    let gh = fake_github::start().await;
    request_with_settings(app_settings(gh.base_url.clone(), true), |request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        gh.set_app_repos(&[REPO]);
        let state = start_install(&request, &f, &gh).await;
        // Instalasi yang tidak dapat diakses user (menebak id) ditolak.
        let res = finish_install(&request, &f, INSTALLATION + 1, APP_CODE, &state).await;
        assert_eq!(res.status_code(), 303);
        // Kode OAuth salah ditolak.
        let res = finish_install(&request, &f, INSTALLATION, "kode-palsu", &state).await;
        assert_eq!(res.status_code(), 303);
        assert!(git_connections::Entity::find().one(&ctx.db).await.unwrap().is_none());
        // State rusak/palsu: 404 sebelum menyentuh workspace mana pun.
        let res = request
            .get(&format!(
                "/git/github-app/callback?installation_id={INSTALLATION}&code={APP_CODE}&state=palsu.sig"
            ))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 404);
        // State milik user lain di workspace yang sama ditolak oleh callback workspace.
        let other = crate::support::a_user()
            .email("admin2@acme.test")
            .create(&ctx)
            .await;
        crate::support::add_member(&ctx, &f.workspace, &other, RoleKey::Admin).await;
        let key = Settings::from_ctx(&ctx).unwrap().auth.encryption_key_bytes().unwrap();
        let foreign = svc::app::sign_state(&key, f.workspace.id, other.id, chrono::Utc::now().timestamp());
        let res = request
            .get(&format!(
                "/w/acme/settings/git/app/callback?installation_id={INSTALLATION}&code={APP_CODE}&state={foreign}"
            ))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303);
        assert!(git_connections::Entity::find().one(&ctx.db).await.unwrap().is_none());
    })
    .await;
}

#[tokio::test]
async fn app_webhook_tracks_prs_and_installation_changes() {
    let gh = fake_github::start().await;
    request_with_settings(app_settings(gh.base_url.clone(), true), |request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let conn = installed(&request, &ctx, &f, &gh).await;
        let pr = json!({
            "action": "opened",
            "installation": { "id": INSTALLATION },
            "repository": { "full_name": REPO },
            "pull_request": {
                "number": 12, "html_url": format!("https://github.com/{REPO}/pull/12"),
                "title": "feat: struk", "state": "open", "draft": false, "merged": false,
                "head": { "ref": "ag/KSR-1-struk", "sha": "s1" }, "base": { "ref": "main", "sha": "b" },
                "user": { "login": "dewi" }
            }
        });
        assert_eq!(app_webhook(&request, "pull_request", "salah", &pr).await, 401);
        assert_eq!(app_webhook(&request, "pull_request", APP_WEBHOOK_SECRET, &pr).await, 204);
        let tracked = pull_requests::Entity::find()
            .filter(pull_requests::Column::Number.eq(12))
            .one(&ctx.db)
            .await
            .unwrap();
        assert!(tracked.is_some(), "PR dilacak lewat webhook App");
        // Webhook per-koneksi (PAT) tidak menerima secret App.
        let body = serde_json::to_vec(&pr).unwrap();
        let res = request
            .post(&format!("/webhooks/github/{}", conn.id))
            .add_header("x-github-event", "pull_request".to_string())
            .add_header("x-hub-signature-256", sign(APP_WEBHOOK_SECRET, &body))
            .bytes(body.into())
            .await;
        assert_eq!(res.status_code(), 401);

        // Repo baru diberi akses → daftar repo koneksi diperbarui.
        gh.add_repo("pt-contoh/kasir-web");
        gh.set_app_repos(&[REPO, "pt-contoh/kasir-web"]);
        let ev = json!({ "action": "added", "installation": { "id": INSTALLATION } });
        assert_eq!(
            app_webhook(&request, "installation_repositories", APP_WEBHOOK_SECRET, &ev).await,
            204
        );
        let conn = git_connections::Entity::find_by_id(conn.id).one(&ctx.db).await.unwrap().unwrap();
        assert_eq!(conn.app_repos().len(), 2);

        // Repo dicabut aksesnya → repo tertaut ditandai.
        gh.set_app_repos(&["pt-contoh/kasir-web"]);
        let ev = json!({ "action": "removed", "installation": { "id": INSTALLATION } });
        app_webhook(&request, "installation_repositories", APP_WEBHOOK_SECRET, &ev).await;
        let repo = repos::Entity::find().one(&ctx.db).await.unwrap().unwrap();
        assert_eq!(repo.webhook_error, svc::APP_REPO_NOT_GRANTED);

        // Ditangguhkan lalu dihapus.
        let ev = json!({ "action": "suspend", "installation": { "id": INSTALLATION } });
        app_webhook(&request, "installation", APP_WEBHOOK_SECRET, &ev).await;
        let conn = git_connections::Entity::find_by_id(conn.id).one(&ctx.db).await.unwrap().unwrap();
        assert!(conn.suspended_at.is_some());
        let settings = Settings::from_ctx(&ctx).unwrap();
        assert!(svc::provider_for(&conn, &settings).await.is_err());
        let ev = json!({ "action": "deleted", "installation": { "id": INSTALLATION } });
        assert_eq!(app_webhook(&request, "installation", APP_WEBHOOK_SECRET, &ev).await, 204);
        assert!(git_connections::Entity::find_by_id(conn.id).one(&ctx.db).await.unwrap().is_none());
        let repo = repos::Entity::find().one(&ctx.db).await.unwrap().unwrap();
        assert_eq!(repo.connection_id, None);
    })
    .await;
}

#[tokio::test]
async fn token_connection_still_works_next_to_the_app() {
    let gh = fake_github::start().await;
    request_with_settings(
        app_settings(gh.base_url.clone(), true),
        |request, ctx| async move {
            let f = fixture(&request, &ctx).await;
            let conn = installed(&request, &ctx, &f, &gh).await;
            // Mengganti ke PAT mengubah koneksi menjadi token (webhook per repo dipasang lagi).
            let res = request
                .post("/w/acme/settings/git/connect")
                .add_cookie(f.cookie.clone())
                .form(&[("kind", "github"), ("token", fake_github::TOKEN)])
                .await;
            assert_eq!(res.status_code(), 303, "{}", res.text());
            let conn = git_connections::Entity::find_by_id(conn.id)
                .one(&ctx.db)
                .await
                .unwrap()
                .unwrap();
            assert!(!conn.is_app());
            assert_eq!(conn.installation_id, None);
            assert_eq!(gh.hooks().len(), 1, "webhook PAT dipasang");
        },
    )
    .await;
}
