//! Git & PR (M6) terhadap GitHub palsu (`support::fake_github`): hubungkan provider (token
//! divalidasi, disimpan terenkripsi), repo tertaut + webhook otomatis, webhook bertanda tangan
//! HMAC (PR, check_run, status), runner membuka PR, gate `pr_open`/`ci_green` menyelesaikan
//! stage otomatis, merge otomatis saat review di-approve, PR sinkron aturan, konvensi git
//! project, dan halaman PR.

use agentgate_server::{
    models::{git_connections, pull_requests, repos},
    services::git::webhook::sign,
};
use loco_rs::{TestServer, app::AppContext};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::{Value, json};

use super::{
    runner::{SECRET, get_json, post_json, register},
    tickets::{
        Fixture, create_ticket, fixture,
        stages::{member, use_workflow},
        ticket,
    },
};
use crate::support::{
    fake_github::{self, FakeGithub, LOGIN, TOKEN},
    newest_audit_events,
    oauth_flow::device_login,
    request_with_settings,
};

const REPO: &str = "pt-contoh/kasir-api";

fn settings_for(base: String) -> impl FnOnce(&mut Value) {
    move |s| {
        s["git"] =
            json!({ "github_api_base": base, "github_web_base": base, "timeout_seconds": 5 });
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

async fn connect(request: &TestServer, f: &Fixture, token: &str) -> axum_test::TestResponse {
    request
        .post("/w/acme/settings/git/connect")
        .add_cookie(f.cookie.clone())
        .form(&[("kind", "github"), ("token", token)])
        .await
}

/// Provider terhubung + repo `kasir-api` tertaut (webhook terpasang) → (koneksi, secret).
async fn connected(
    request: &TestServer,
    ctx: &AppContext,
    f: &Fixture,
    gh: &FakeGithub,
) -> (git_connections::Model, String) {
    gh.add_repo(REPO);
    add_repo(request, f).await;
    let res = connect(request, f, TOKEN).await;
    assert_eq!(res.status_code(), 303, "{}", res.text());
    let conn = git_connections::Entity::find()
        .one(&ctx.db)
        .await
        .unwrap()
        .expect("koneksi tersimpan");
    let hook = gh.hooks().into_iter().next().expect("webhook dipasang");
    (conn, hook.secret)
}

async fn webhook(
    request: &TestServer,
    conn: &str,
    event: &str,
    secret: &str,
    payload: &Value,
) -> u16 {
    let body = serde_json::to_vec(payload).unwrap();
    request
        .post(&format!("/webhooks/github/{conn}"))
        .add_header("x-github-event", event.to_string())
        .add_header("x-hub-signature-256", sign(secret, &body))
        .add_header("content-type", "application/json")
        .bytes(body.into())
        .await
        .status_code()
        .as_u16()
}

fn pull_payload(number: i64, branch: &str, sha: &str, state: &str, merged: bool) -> Value {
    json!({
        "action": "opened",
        "repository": { "full_name": REPO },
        "pull_request": {
            "number": number, "html_url": format!("https://github.com/{REPO}/pull/{number}"),
            "title": "feat: checkout QRIS", "state": state, "draft": false, "merged": merged,
            "head": { "ref": branch, "sha": sha }, "base": { "ref": "main", "sha": "b0" },
            "additions": 10, "deletions": 2, "user": { "login": "dewi" }
        }
    })
}

fn check_payload(sha: &str, name: &str, conclusion: &str) -> Value {
    json!({
        "action": "completed",
        "repository": { "full_name": REPO },
        "check_run": {
            "name": name, "status": "completed", "conclusion": conclusion, "head_sha": sha,
            "html_url": "https://github.com/pt-contoh/kasir-api/runs/1"
        }
    })
}

#[tokio::test]
async fn connect_validates_token_encrypts_it_and_links_repos() {
    let gh = fake_github::start().await;
    request_with_settings(
        settings_for(gh.base_url.clone()),
        |request, ctx| async move {
            let f = fixture(&request, &ctx).await;
            gh.add_repo(REPO);
            add_repo(&request, &f).await;
            assert!(gh.hooks().is_empty(), "tanpa koneksi belum ada webhook");

            let res = request
                .get("/w/acme/settings/git/cloud")
                .add_cookie(f.cookie.clone())
                .await;
            assert_eq!(res.status_code(), 200, "{}", res.text());
            assert!(res.text().contains("data-provider=\"github\""));

            // Token ditolak provider → 422, dialog terbuka lagi, tidak tersimpan.
            let res = connect(&request, &f, "ghp_salah").await;
            assert_eq!(res.status_code(), 422);
            assert!(res.text().contains("data-open-on-load"));
            assert!(
                git_connections::Entity::find()
                    .one(&ctx.db)
                    .await
                    .unwrap()
                    .is_none()
            );

            let res = connect(&request, &f, TOKEN).await;
            assert_eq!(res.status_code(), 303, "{}", res.text());
            let conn = git_connections::Entity::find()
                .one(&ctx.db)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                (conn.account.as_str(), conn.host.as_str()),
                (LOGIN, "github.com")
            );
            assert!(
                !conn
                    .token_encrypted
                    .windows(TOKEN.len())
                    .any(|w| w == TOKEN.as_bytes()),
                "token tidak disimpan polos"
            );

            let hooks = gh.hooks();
            assert_eq!(hooks.len(), 1);
            assert_eq!(hooks[0].repo, REPO);
            assert!(
                hooks[0]
                    .url
                    .ends_with(&format!("/webhooks/github/{}", conn.id))
            );
            assert!(hooks[0].events.iter().any(|e| e == "pull_request"));
            let repo = repos::Entity::find().one(&ctx.db).await.unwrap().unwrap();
            assert_eq!(repo.connection_id, Some(conn.id));
            assert_eq!(repo.full_name, REPO);
            assert_eq!(repo.webhook_id, Some(9001));

            let actions: Vec<String> = newest_audit_events(&ctx.db, f.workspace.workspace_id(), 10)
                .await
                .unwrap()
                .into_iter()
                .map(|e| e.action)
                .collect();
            assert!(actions.iter().any(|a| a == "git.connected"), "{actions:?}");
            assert!(actions.iter().any(|a| a == "repo.linked"), "{actions:?}");

            let res = request
                .get("/w/acme/settings/git/repos")
                .add_cookie(f.cookie.clone())
                .await;
            assert_eq!(res.status_code(), 200);
            assert!(res.text().contains("data-webhook=\"active\""));

            // Pasang ulang memasang webhook baru.
            let res = request
                .post(&format!("/w/acme/settings/git/repos/{}/relink", repo.id))
                .add_cookie(f.cookie.clone())
                .form(&[("x", "")])
                .await;
            assert_eq!(res.status_code(), 303);
            assert_eq!(gh.hooks().len(), 2);

            // Anggota tanpa ManageIntegrations → 404.
            let (_, cookie) = member(&request, &ctx, &f, "Mira Member").await;
            let res = request
                .get("/w/acme/settings/git/cloud")
                .add_cookie(cookie.clone())
                .await;
            assert_eq!(res.status_code(), 404);
            let res = request
                .post("/w/acme/settings/git/connect")
                .add_cookie(cookie)
                .form(&[("kind", "github"), ("token", TOKEN)])
                .await;
            assert_eq!(res.status_code(), 404);

            let res = request
                .post(&format!("/w/acme/settings/git/{}/disconnect", conn.id))
                .add_cookie(f.cookie.clone())
                .form(&[("x", "")])
                .await;
            assert_eq!(res.status_code(), 303);
            assert!(
                git_connections::Entity::find()
                    .one(&ctx.db)
                    .await
                    .unwrap()
                    .is_none()
            );
            let repo = repos::Entity::find().one(&ctx.db).await.unwrap().unwrap();
            assert_eq!(repo.connection_id, None);
        },
    )
    .await;
}

#[tokio::test]
async fn webhook_checks_signature_then_tracks_pr_and_ci() {
    let gh = fake_github::start().await;
    request_with_settings(
        settings_for(gh.base_url.clone()),
        |request, ctx| async move {
            let f = fixture(&request, &ctx).await;
            let (conn, secret) = connected(&request, &ctx, &f, &gh).await;
            let code = create_ticket(&request, &f.cookie, &[("title", "Checkout QRIS")]).await;
            let id = conn.id.to_string();
            let branch = format!("ag/{code}-checkout-qris");
            let opened = pull_payload(77, &branch, "s1", "open", false);

            assert_eq!(
                webhook(&request, &id, "pull_request", "salah", &opened).await,
                401
            );
            assert_eq!(
                webhook(
                    &request,
                    &uuid::Uuid::now_v7().to_string(),
                    "pull_request",
                    &secret,
                    &opened
                )
                .await,
                404
            );
            let res = request.post("/webhooks/github/bukan-uuid").await;
            assert_eq!(res.status_code(), 404);
            assert_eq!(
                webhook(&request, &id, "ping", &secret, &json!({ "zen": "ok" })).await,
                204
            );
            assert_eq!(
                webhook(&request, &id, "star", &secret, &json!({})).await,
                202
            );

            assert_eq!(
                webhook(&request, &id, "pull_request", &secret, &opened).await,
                204
            );
            let pr = pull_requests::Entity::find()
                .one(&ctx.db)
                .await
                .unwrap()
                .unwrap();
            let t = ticket(&ctx, &f.workspace, &code).await;
            assert_eq!(
                pr.ticket_id,
                Some(t.id),
                "PR tertaut lewat kode di nama branch"
            );
            assert_eq!((pr.number, pr.state.as_str()), (77, "open"));

            assert_eq!(
                webhook(
                    &request,
                    &id,
                    "check_run",
                    &secret,
                    &check_payload("s1", "test", "failure")
                )
                .await,
                204
            );
            let pr = pull_requests::Entity::find()
                .one(&ctx.db)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(pr.ci_state, "failure");
            let status = json!({
                "repository": { "full_name": REPO }, "sha": "s1", "state": "success",
                "context": "build", "target_url": ""
            });
            assert_eq!(
                webhook(&request, &id, "status", &secret, &status).await,
                204
            );
            assert_eq!(
                webhook(
                    &request,
                    &id,
                    "check_run",
                    &secret,
                    &check_payload("s1", "test", "success")
                )
                .await,
                204
            );
            let pr = pull_requests::Entity::find()
                .one(&ctx.db)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(pr.ci_state, "success");
            assert_eq!(pr.checks.as_array().unwrap().len(), 2);

            // Halaman tiket, board, dan daftar PR menampilkan PR.
            let html = request
                .get(&format!("/w/acme/t/{code}/pr"))
                .add_cookie(f.cookie.clone())
                .await
                .text();
            assert!(html.contains("data-pr=\"77\""), "{html}");
            assert!(html.contains("data-check=\"build\""));
            let html = request
                .get(&format!("/w/acme/t/{code}"))
                .add_cookie(f.cookie.clone())
                .await
                .text();
            assert!(html.contains("data-ticket-pr"));
            let html = request
                .get("/w/acme/p/KSR/board")
                .add_cookie(f.cookie.clone())
                .await
                .text();
            assert!(html.contains("data-ticket-pr"));
            let res = request
                .get("/w/acme/p/KSR/prs")
                .add_cookie(f.cookie.clone())
                .await;
            assert_eq!(res.status_code(), 200);
            assert!(res.text().contains("data-pr=\"77\""));
            let html = request
                .get("/w/acme/p/KSR/prs?tab=merged")
                .add_cookie(f.cookie.clone())
                .await
                .text();
            assert!(!html.contains("data-pr=\"77\""));

            // Merged.
            let merged = pull_payload(77, &branch, "s1", "closed", true);
            assert_eq!(
                webhook(&request, &id, "pull_request", &secret, &merged).await,
                204
            );
            let pr = pull_requests::Entity::find()
                .one(&ctx.db)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(pr.state, "merged");
        },
    )
    .await;
}

fn dev_flow(executor: &str) -> String {
    format!(
        r"workflow: git
nama: Alur Git
stages:
  - id: todo
    nama: To do
    kategori: todo
  - id: dev
    nama: Development
    kategori: in_progress
    persona: dev
    mode: otomatis
    gate: [pr_open, ci_green]
    transisi: tunggu_approve
    penugasan:
      executor: [{executor}]
  - id: done
    nama: Selesai
    kategori: done
"
    )
}

#[tokio::test]
async fn runner_opens_pr_and_green_ci_completes_the_stage() {
    let gh = fake_github::start().await;
    request_with_settings(settings_for(gh.base_url.clone()), |request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let (conn, secret) = connected(&request, &ctx, &f, &gh).await;
        use_workflow(&request, &ctx, &f, &dev_flow(&f.owner.email)).await;
        create_ticket(&request, &f.cookie, &[("title", "Checkout QRIS")]).await;
        let res = request
            .post("/w/acme/t/KSR-1/stage/complete")
            .add_cookie(f.cookie.clone())
            .form(&[("x", "")])
            .await;
        assert_eq!(res.status_code(), 303);
        let token = device_login(&request, &f.cookie, &f.workspace.id.to_string()).await;
        let (_, list) = get_json(&request, &token, "/api/cli/jobs").await;
        let job = list[0]["id"].as_str().unwrap().to_string();
        let runner = register(&request, &token, &SECRET).await;
        post_json(&request, &token, None, &format!("/api/cli/jobs/{job}/claim"), &json!({ "runner_id": runner })).await;
        let (status, spec) =
            post_json(&request, &token, Some(&runner), &format!("/api/runner/jobs/{job}/start"), &json!({})).await;
        assert_eq!(status, 200, "{spec}");
        assert_eq!(spec["base_branch"], "main");
        assert_eq!(spec["git"]["code_output"], "pr");
        let branch = spec["branch"].as_str().unwrap().to_string();

        let pr_url = format!("/api/runner/jobs/{job}/pull-request");
        let body = json!({ "repo": "kasir-api", "branch": branch, "base": "", "title": "feat: checkout QRIS", "body": "KSR-1" });
        let (status, info) = post_json(&request, &token, Some(&runner), &pr_url, &body).await;
        assert_eq!(status, 200, "{info}");
        assert_eq!((info["number"].as_i64(), info["created"].as_bool()), (Some(140), Some(true)));
        let (_, again) = post_json(&request, &token, Some(&runner), &pr_url, &body).await;
        assert_eq!((again["number"].as_i64(), again["created"].as_bool()), (Some(140), Some(false)));
        let pulls = gh.pulls();
        assert_eq!(pulls.len(), 1);
        assert_eq!((pulls[0].head.as_str(), pulls[0].base.as_str()), (branch.as_str(), "main"));
        let (status, body) = post_json(
            &request, &token, Some(&runner), &pr_url,
            &json!({ "repo": "tidak-ada", "branch": branch, "base": "", "title": "x", "body": "" }),
        )
        .await;
        assert_eq!(status, 404, "{body}");

        // Selesai: CI belum hijau → stage tetap aktif.
        let (status, _) = post_json(
            &request, &token, Some(&runner), &format!("/api/runner/jobs/{job}/finish"),
            &json!({ "outcome": "succeeded", "summary": "ok" }),
        )
        .await;
        assert_eq!(status, 200);
        let t = ticket(&ctx, &f.workspace, "KSR-1").await;
        assert_eq!((t.stage_key.as_str(), t.stage_state.as_str()), ("dev", "active"));

        // CI hijau dari webhook → stage diajukan review.
        let id = conn.id.to_string();
        assert_eq!(
            webhook(&request, &id, "check_run", &secret, &check_payload("sha140", "test", "success")).await,
            204
        );
        let t = ticket(&ctx, &f.workspace, "KSR-1").await;
        assert_eq!((t.stage_key.as_str(), t.stage_state.as_str()), ("dev", "review"));
    })
    .await;
}

fn review_flow() -> &'static str {
    r"workflow: review
nama: Alur Review
stages:
  - id: todo
    nama: To do
    kategori: todo
  - id: review
    nama: Code Review
    kategori: in_progress
    persona: reviewer
    mode: manual
    gate: [pr_open, ci_green]
    transisi: tunggu_approve
  - id: done
    nama: Selesai
    kategori: done
"
}

#[tokio::test]
async fn approving_review_merges_green_prs_when_auto_merge_is_on() {
    let gh = fake_github::start().await;
    request_with_settings(settings_for(gh.base_url.clone()), |request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let (conn, secret) = connected(&request, &ctx, &f, &gh).await;
        use_workflow(&request, &ctx, &f, review_flow()).await;
        create_ticket(&request, &f.cookie, &[("title", "Checkout QRIS")]).await;
        let res = request
            .post("/w/acme/p/KSR/settings/git")
            .add_cookie(f.cookie.clone())
            .form(&[("panel", "out"), ("code_output", "pr"), ("merge_mode", "auto_on_review")])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let complete = |request: &TestServer| {
            request
                .post("/w/acme/t/KSR-1/stage/complete")
                .add_cookie(f.cookie.clone())
                .form(&[("x", "")])
        };
        assert_eq!(complete(&request).await.status_code(), 303);

        // PR dibuka di provider (bukan lewat runner) dan CI hijau.
        let gh_pr = {
            let mut st = gh.state.lock().unwrap();
            st.pulls.push(fake_github::Pull {
                repo: REPO.into(), number: 150, title: "feat".into(), body: String::new(),
                head: "ag/KSR-1-checkout".into(), base: "main".into(), sha: "z1".into(), merged: false,
            });
            st.pulls[0].json()
        };
        let id = conn.id.to_string();
        let payload = json!({ "action": "opened", "repository": { "full_name": REPO }, "pull_request": gh_pr });
        assert_eq!(webhook(&request, &id, "pull_request", &secret, &payload).await, 204);
        // Gate CI belum hijau → selesai ditolak.
        assert_eq!(complete(&request).await.status_code(), 422);
        assert_eq!(
            webhook(&request, &id, "check_run", &secret, &check_payload("z1", "test", "success")).await,
            204
        );
        assert_eq!(complete(&request).await.status_code(), 303);
        let t = ticket(&ctx, &f.workspace, "KSR-1").await;
        assert_eq!(t.stage_state, "review");

        let res = request
            .post("/w/acme/t/KSR-1/stage/approve")
            .add_cookie(f.cookie.clone())
            .form(&[("x", "")])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(gh.merges(), vec![(REPO.to_string(), 150)]);
        let pr = pull_requests::Entity::find()
            .filter(pull_requests::Column::Number.eq(150))
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(pr.state, "merged");
        let actions: Vec<String> = newest_audit_events(&ctx.db, f.workspace.workspace_id(), 5)
            .await
            .unwrap()
            .into_iter()
            .map(|e| e.action)
            .collect();
        assert!(actions.iter().any(|a| a == "pull_request.merged"), "{actions:?}");
    })
    .await;
}

#[tokio::test]
async fn rules_sync_opens_a_pull_request() {
    let gh = fake_github::start().await;
    request_with_settings(
        settings_for(gh.base_url.clone()),
        |request, ctx| async move {
            let f = fixture(&request, &ctx).await;
            let token = device_login(&request, &f.cookie, &f.workspace.id.to_string()).await;
            gh.add_repo(REPO);
            add_repo(&request, &f).await;
            let body = json!({ "project": "KSR", "repo": "kasir-api" });
            let (status, err) =
                post_json(&request, &token, None, "/api/cli/rules/pull-request", &body).await;
            assert_eq!(status, 422, "{err}");
            assert_eq!(err["error"], "git-not-connected");

            assert_eq!(connect(&request, &f, TOKEN).await.status_code(), 303);
            let (status, info) =
                post_json(&request, &token, None, "/api/cli/rules/pull-request", &body).await;
            assert_eq!(status, 200, "{info}");
            assert_eq!(info["created"], true);
            let pulls = gh.pulls();
            assert_eq!(pulls[0].head, "agentgate/sync-rules");
            assert_eq!(pulls[0].title, "Update aturan Agent Gate");
            let (_, again) =
                post_json(&request, &token, None, "/api/cli/rules/pull-request", &body).await;
            assert_eq!(again["created"], false);
            assert_eq!(gh.pulls().len(), 1);
            drop(ctx);
        },
    )
    .await;
}

#[tokio::test]
async fn project_git_conventions_are_validated_and_audited() {
    let gh = fake_github::start().await;
    request_with_settings(
        settings_for(gh.base_url.clone()),
        |request, ctx| async move {
            let f = fixture(&request, &ctx).await;
            for panel in ["git", "out", "ci"] {
                let res = request
                    .get(&format!("/w/acme/p/KSR/settings/git/{panel}"))
                    .add_cookie(f.cookie.clone())
                    .await;
                assert_eq!(res.status_code(), 200, "{panel}");
            }
            let res = request
                .get("/w/acme/p/KSR/settings/git/env")
                .add_cookie(f.cookie.clone())
                .await;
            assert_eq!(res.status_code(), 404);

            let res = request
                .post("/w/acme/p/KSR/settings/git")
                .add_cookie(f.cookie.clone())
                .form(&[
                    ("panel", "git"),
                    ("branch_pattern", "fitur/{slug}"),
                    ("commit_style", "free"),
                ])
                .await;
            assert_eq!(res.status_code(), 422);
            assert!(res.text().contains("data-git-form"));

            let res = request
                .post("/w/acme/p/KSR/settings/git")
                .add_cookie(f.cookie.clone())
                .form(&[
                    ("panel", "git"),
                    ("branch_pattern", "fitur/{kode}"),
                    ("commit_style", "free"),
                    ("commit_language", "id"),
                    ("pr_language", "id"),
                    ("code_language", "en"),
                ])
                .await;
            assert_eq!(res.status_code(), 303);
            let res = request
                .post("/w/acme/p/KSR/settings/git")
                .add_cookie(f.cookie.clone())
                .form(&[("panel", "ci"), ("required_checks", "build, test, build")])
                .await;
            assert_eq!(res.status_code(), 303);
            let project = agentgate_server::models::projects::Entity::find()
                .filter(agentgate_server::models::projects::Column::Key.eq("KSR"))
                .one(&ctx.db)
                .await
                .unwrap()
                .unwrap();
            let git = project.git_settings();
            assert_eq!(git.branch_pattern, "fitur/{kode}");
            assert_eq!(
                git.required_checks,
                vec!["build".to_string(), "test".to_string()]
            );
            assert!(!git.require_ci);
            let actions: Vec<String> = newest_audit_events(&ctx.db, f.workspace.workspace_id(), 3)
                .await
                .unwrap()
                .into_iter()
                .map(|e| e.action)
                .collect();
            assert!(
                actions.iter().any(|a| a == "project.git_updated"),
                "{actions:?}"
            );

            let (_, cookie) = member(&request, &ctx, &f, "Mira Member").await;
            let res = request
                .get("/w/acme/p/KSR/settings/git/git")
                .add_cookie(cookie.clone())
                .await;
            assert_eq!(res.status_code(), 404);
            let res = request.get("/w/acme/p/KSR/prs").add_cookie(cookie).await;
            assert_eq!(res.status_code(), 200);
        },
    )
    .await;
}
