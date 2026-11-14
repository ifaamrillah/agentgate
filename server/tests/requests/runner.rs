//! API CLI & runner (M5): job dibuat saat tiket masuk stage berpersona, klaim oleh executor
//! (atomik), daftar runner, start → `JobSpec`, log/langkah, evidence bertanda tangan Ed25519
//! membuka gate `tests_passed`/`screenshot_exists`, finish → stage diajukan review, izin aksi
//! berbahaya, lepas/batal, `@agent` di komentar, dan isolasi antar user/workspace.

use agentgate_core::runner::{
    protocol::{EvidenceKind, EvidenceSummary},
    sha256_hex,
    signing::{evidence_payload, public_key_b64, sign},
};
use agentgate_server::{
    app::App,
    models::{job_evidence, job_logs, jobs},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use loco_rs::{TestServer, app::AppContext, testing::prelude::*};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::{Value, json};
use uuid::Uuid;

use super::tickets::{
    Fixture, create_ticket, fixture,
    stages::{member, use_workflow},
    ticket,
};
use crate::support::{a_user, a_workspace, oauth_flow::device_login, sign_in_as};

pub(super) const SECRET: [u8; 32] = [7; 32];
const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDRfake";

fn flow_yaml(executor: &str) -> String {
    format!(
        r"workflow: runner
nama: Alur Runner
stages:
  - id: todo
    nama: To do
    kategori: todo
  - id: dev
    nama: Development
    kategori: in_progress
    persona: dev
    mode: otomatis
    gate: [tests_passed, screenshot_exists]
    transisi: tunggu_approve
    izin_bahaya:
      deploy: allow
    penugasan:
      executor: [{executor}]
  - id: done
    nama: Selesai
    kategori: done
"
    )
}

pub(super) async fn get_json(request: &TestServer, token: &str, url: &str) -> (u16, Value) {
    let res = request
        .get(url)
        .add_header("authorization", format!("Bearer {token}"))
        .await;
    let status = res.status_code().as_u16();
    (
        status,
        serde_json::from_str(&res.text()).unwrap_or(Value::Null),
    )
}

pub(super) async fn post_json(
    request: &TestServer,
    token: &str,
    runner: Option<&str>,
    url: &str,
    body: &Value,
) -> (u16, Value) {
    let mut req = request
        .post(url)
        .add_header("authorization", format!("Bearer {token}"));
    if let Some(r) = runner {
        req = req.add_header("x-agentgate-runner", r.to_string());
    }
    let res = req.json(body).await;
    let status = res.status_code().as_u16();
    (
        status,
        serde_json::from_str(&res.text()).unwrap_or(Value::Null),
    )
}

pub(super) async fn register(request: &TestServer, token: &str, secret: &[u8; 32]) -> String {
    let (status, body) = post_json(
        request,
        token,
        None,
        "/api/runner/register",
        &json!({
            "name": "MacBook Olivia",
            "os": "macos",
            "client": "claude",
            "version": "0.1.0",
            "public_key": public_key_b64(secret),
        }),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    body["id"].as_str().unwrap().to_string()
}

fn evidence(
    job: &str,
    kind: EvidenceKind,
    name: &str,
    content_type: &str,
    data: &[u8],
    summary: &EvidenceSummary,
    secret: &[u8; 32],
) -> Value {
    let sha = sha256_hex(data);
    let payload = evidence_payload(Uuid::parse_str(job).unwrap(), kind, name, &sha, summary);
    json!({
        "kind": kind.as_str(),
        "name": name,
        "content_type": content_type,
        "content": STANDARD.encode(data),
        "sha256": sha,
        "summary": summary,
        "signature": sign(secret, &payload),
    })
}

/// Tiket KSR-1 di stage `dev` (job stage `queued`) + token CLI owner.
async fn ticket_in_dev(request: &TestServer, ctx: &AppContext, f: &Fixture) -> (String, String) {
    use_workflow(request, ctx, f, &flow_yaml(&f.owner.email)).await;
    create_ticket(request, &f.cookie, &[("title", "Checkout QRIS")]).await;
    let res = request
        .post("/w/acme/t/KSR-1/stage/complete")
        .add_cookie(f.cookie.clone())
        .form(&[("x", "")])
        .await;
    assert_eq!(res.status_code(), 303, "{}", res.text());
    let token = device_login(request, &f.cookie, &f.workspace.id.to_string()).await;
    let (status, list) = get_json(request, &token, "/api/cli/jobs").await;
    assert_eq!(status, 200, "{list}");
    let job = list
        .as_array()
        .unwrap()
        .iter()
        .find(|j| j["ticket"] == "KSR-1")
        .expect("job antre untuk KSR-1")
        .clone();
    assert_eq!(job["status"], "queued");
    assert_eq!(job["stage"], "dev");
    assert_eq!(job["can_claim"], true);
    (job["id"].as_str().unwrap().to_string(), token)
}

#[tokio::test]
async fn api_requires_a_bearer_token() {
    request::<App, _, _>(|request, _ctx| async move {
        let res = request.get("/api/cli/jobs").await;
        assert_eq!(res.status_code(), 401);
        let res = request
            .get("/api/cli/me")
            .add_header("authorization", "Bearer palsu")
            .await;
        assert_eq!(res.status_code(), 401);
        assert_eq!(res.json::<Value>()["error"], "invalid_token");
    })
    .await;
}

#[tokio::test]
async fn me_and_rules_describe_the_session() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let token = device_login(&request, &f.cookie, &f.workspace.id.to_string()).await;
        let (status, me) = get_json(&request, &token, "/api/cli/me").await;
        assert_eq!(status, 200, "{me}");
        assert_eq!(me["email"], f.owner.email.as_str());
        assert_eq!(me["slug"], "acme");

        let (status, rules) = get_json(&request, &token, "/api/cli/rules?project=ksr").await;
        assert_eq!(status, 200, "{rules}");
        assert_eq!(rules["project"], "KSR");
        let paths: Vec<&str> = rules["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["path"].as_str().unwrap())
            .collect();
        assert!(paths.contains(&"AGENTS.md") && paths.contains(&".mcp.json"));
        assert!(!rules["checksum"].as_str().unwrap().is_empty());
        let (status, _) = get_json(&request, &token, "/api/cli/rules?project=XYZ").await;
        assert_eq!(status, 404);
    })
    .await;
}

#[tokio::test]
async fn runner_executes_stage_job_with_signed_evidence_then_requests_review() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let (job, token) = ticket_in_dev(&request, &ctx, &f).await;
        let runner = register(&request, &token, &SECRET).await;

        // Belum diklaim → start ditolak.
        let start_url = format!("/api/runner/jobs/{job}/start");
        let (status, body) =
            post_json(&request, &token, Some(&runner), &start_url, &json!({})).await;
        assert_eq!(status, 422, "{body}");
        assert_eq!(body["error"], "job-not-holder");

        // Ambil (executor), klaim kedua ditolak.
        let claim_url = format!("/api/cli/jobs/{job}/claim");
        let (status, body) = post_json(
            &request,
            &token,
            None,
            &claim_url,
            &json!({ "runner_id": runner }),
        )
        .await;
        assert_eq!(status, 200, "{body}");
        assert_eq!(body["status"], "claimed");
        let (status, body) = post_json(&request, &token, None, &claim_url, &json!({})).await;
        assert_eq!(status, 422);
        assert_eq!(body["error"], "job-not-queued");

        // Start → spesifikasi lengkap.
        let (status, spec) =
            post_json(&request, &token, Some(&runner), &start_url, &json!({})).await;
        assert_eq!(status, 200, "{spec}");
        assert_eq!(spec["ticket"], "KSR-1");
        assert_eq!(spec["stage"], "dev");
        assert_eq!(spec["persona"], "dev");
        assert_eq!(spec["project"], "KSR");
        assert!(spec["branch"].as_str().unwrap().starts_with("ag/KSR-1-"));
        assert!(spec["mcp_url"].as_str().unwrap().ends_with("/mcp"));
        assert!(spec["prompt"].as_str().unwrap().contains("KSR-1"));
        assert!(
            spec["danger"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["level"] == "block")
        );
        // Izin aksi berbahaya stage (R3) menimpa persona Developer (deploy = block).
        let deploy = spec["danger"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["action"] == "deploy")
            .cloned();
        assert_eq!(
            deploy.map(|d| d["level"].clone()),
            Some(json!("allow")),
            "{spec}"
        );

        // Log dan langkah.
        let (status, _) = post_json(
            &request,
            &token,
            Some(&runner),
            &format!("/api/runner/jobs/{job}/logs"),
            &json!({ "lines": [
                { "stream": "out", "text": "Membaca konteks tiket" },
                { "stream": "tool", "text": "ag_context KSR-1" },
            ]}),
        )
        .await;
        assert_eq!(status, 204);
        let (status, _) = post_json(
            &request,
            &token,
            Some(&runner),
            &format!("/api/runner/jobs/{job}/steps"),
            &json!({ "steps": [
                { "name": "Baca konteks", "state": "ok", "seconds": 3 },
                { "name": "Tulis kode", "state": "now" },
            ]}),
        )
        .await;
        assert_eq!(status, 204);
        let job_id = Uuid::parse_str(&job).unwrap();
        let row = jobs::Entity::find_by_id(job_id)
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.status, "running");
        assert_eq!(row.steps.as_array().unwrap().len(), 2);
        let logs = job_logs::Entity::find()
            .filter(job_logs::Column::JobId.eq(job_id))
            .all(&ctx.db)
            .await
            .unwrap();
        assert!(
            logs.iter()
                .any(|l| l.body == "ag_context KSR-1" && l.stream == "tool")
        );

        // Selesai tanpa evidence → gate belum terpenuhi, tiket tetap aktif di dev.
        let ev_url = format!("/api/runner/jobs/{job}/evidence");
        let passed = EvidenceSummary {
            passed: Some(true),
            total: Some(12),
            failed: Some(0),
            note: "cargo test".into(),
        };
        // Tanda tangan dengan kunci lain → ditolak.
        let forged = evidence(
            &job,
            EvidenceKind::TestReport,
            "junit.xml",
            "application/xml",
            b"<ok/>",
            &passed,
            &[9; 32],
        );
        let (status, body) = post_json(&request, &token, Some(&runner), &ev_url, &forged).await;
        assert_eq!(status, 422, "{body}");
        assert_eq!(body["error"], "evidence-signature-invalid");
        // Isi diubah setelah ditandatangani → ditolak.
        let mut tampered = evidence(
            &job,
            EvidenceKind::TestReport,
            "junit.xml",
            "application/xml",
            b"<ok/>",
            &passed,
            &SECRET,
        );
        tampered["content"] = json!(STANDARD.encode(b"<ko/>"));
        let (status, body) = post_json(&request, &token, Some(&runner), &ev_url, &tampered).await;
        assert_eq!(status, 422);
        assert_eq!(body["error"], "evidence-content-invalid");
        // Screenshot palsu (bukan PNG) → ditolak.
        let fake = evidence(
            &job,
            EvidenceKind::Screenshot,
            "a.png",
            "image/png",
            b"bukan png",
            &EvidenceSummary::default(),
            &SECRET,
        );
        let (status, body) = post_json(&request, &token, Some(&runner), &ev_url, &fake).await;
        assert_eq!(status, 422);
        assert_eq!(body["error"], "evidence-type-invalid");

        let t = ticket(&ctx, &f.workspace, "KSR-1").await;
        assert_eq!(
            agentgate_server::services::jobs::evidence::gate_status(
                &ctx.db,
                f.workspace.workspace_id(),
                &t
            )
            .await
            .unwrap(),
            (false, false)
        );

        // Evidence sah.
        let ok = evidence(
            &job,
            EvidenceKind::TestReport,
            "junit.xml",
            "application/xml",
            b"<ok/>",
            &passed,
            &SECRET,
        );
        let (status, body) = post_json(&request, &token, Some(&runner), &ev_url, &ok).await;
        assert_eq!(status, 201, "{body}");
        let shot = evidence(
            &job,
            EvidenceKind::Screenshot,
            "checkout.png",
            "image/png",
            PNG,
            &EvidenceSummary::default(),
            &SECRET,
        );
        let (status, body) = post_json(&request, &token, Some(&runner), &ev_url, &shot).await;
        assert_eq!(status, 201, "{body}");
        let rows = job_evidence::Entity::find()
            .filter(job_evidence::Column::JobId.eq(job_id))
            .all(&ctx.db)
            .await
            .unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|r| r.stage_key == "dev"));
        let t = ticket(&ctx, &f.workspace, "KSR-1").await;
        assert_eq!(
            agentgate_server::services::jobs::evidence::gate_status(
                &ctx.db,
                f.workspace.workspace_id(),
                &t
            )
            .await
            .unwrap(),
            (true, true)
        );

        // Selesai → stage diajukan review.
        let (status, body) = post_json(
            &request,
            &token,
            Some(&runner),
            &format!("/api/runner/jobs/{job}/finish"),
            &json!({
                "outcome": "succeeded", "summary": "Checkout QRIS selesai",
                "usage": { "input": 700, "output": 300, "cache_read": 150, "cache_write": 50 },
                "cost_usd": 0.42,
                "changed_files": [
                    { "repo": "kasir-web", "path": "src/qris.ts" },
                    { "repo": "kasir-web", "path": "../rahasia" }
                ]
            }),
        )
        .await;
        assert_eq!(status, 200, "{body}");
        assert_eq!(body["status"], "succeeded");
        // Pemakaian token rinci dan file yang diubah tersimpan (M10).
        let saved =
            agentgate_server::models::jobs::Entity::find_by_id(Uuid::parse_str(&job).unwrap())
                .one(&ctx.db)
                .await
                .unwrap()
                .unwrap();
        assert_eq!(saved.tokens, Some(1200));
        assert_eq!(
            (
                saved.tokens_input,
                saved.tokens_output,
                saved.tokens_cache_read,
                saved.tokens_cache_write
            ),
            (Some(700), Some(300), Some(150), Some(50))
        );
        assert_eq!(saved.cost_usd_micros, Some(420_000));
        assert_eq!(
            saved.changed_files,
            json!([{ "repo": "kasir-web", "path": "src/qris.ts" }])
        );
        let t = ticket(&ctx, &f.workspace, "KSR-1").await;
        assert_eq!(t.stage_key, "dev");
        assert_eq!(t.stage_state, "review");

        // Job tertutup → endpoint runner menolak.
        let (status, body) = post_json(
            &request,
            &token,
            Some(&runner),
            &format!("/api/runner/jobs/{job}/finish"),
            &json!({ "outcome": "failed" }),
        )
        .await;
        assert_eq!(status, 422);
        assert_eq!(body["error"], "job-closed");
    })
    .await;
}

#[tokio::test]
async fn failing_latest_test_report_keeps_gate_closed() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let (job, token) = ticket_in_dev(&request, &ctx, &f).await;
        let runner = register(&request, &token, &SECRET).await;
        post_json(
            &request,
            &token,
            None,
            &format!("/api/cli/jobs/{job}/claim"),
            &json!({ "runner_id": runner }),
        )
        .await;
        post_json(
            &request,
            &token,
            Some(&runner),
            &format!("/api/runner/jobs/{job}/start"),
            &json!({}),
        )
        .await;
        let ev_url = format!("/api/runner/jobs/{job}/evidence");
        let summary = |passed| EvidenceSummary {
            passed: Some(passed),
            total: Some(3),
            failed: Some(u32::from(!passed)),
            note: String::new(),
        };
        for passed in [true, false] {
            let body = evidence(
                &job,
                EvidenceKind::TestReport,
                "out.txt",
                "text/plain",
                b"hasil",
                &summary(passed),
                &SECRET,
            );
            let (status, res) = post_json(&request, &token, Some(&runner), &ev_url, &body).await;
            assert_eq!(status, 201, "{res}");
        }
        let t = ticket(&ctx, &f.workspace, "KSR-1").await;
        let (tests, _) = agentgate_server::services::jobs::evidence::gate_status(
            &ctx.db,
            f.workspace.workspace_id(),
            &t,
        )
        .await
        .unwrap();
        assert!(!tests, "laporan terbaru gagal");

        // Finish gagal → tiket tetap aktif, job gagal.
        let (status, body) = post_json(
            &request,
            &token,
            Some(&runner),
            &format!("/api/runner/jobs/{job}/finish"),
            &json!({ "outcome": "succeeded" }),
        )
        .await;
        assert_eq!(status, 200, "{body}");
        let t = ticket(&ctx, &f.workspace, "KSR-1").await;
        assert_eq!(t.stage_state, "active", "gate tests_passed belum terpenuhi");
    })
    .await;
}

#[tokio::test]
async fn only_the_stage_executor_may_claim_and_others_cannot_drive_the_job() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let (job, token) = ticket_in_dev(&request, &ctx, &f).await;
        let (_, mia_cookie) = member(&request, &ctx, &f, "Mia Member").await;
        let mia = device_login(&request, &mia_cookie, &f.workspace.id.to_string()).await;

        let (status, list) = get_json(&request, &mia, "/api/cli/jobs").await;
        assert_eq!(status, 200);
        let row = list
            .as_array()
            .unwrap()
            .iter()
            .find(|j| j["id"] == job.as_str())
            .unwrap()
            .clone();
        assert_eq!(row["can_claim"], false);
        let claim_url = format!("/api/cli/jobs/{job}/claim");
        let (status, body) = post_json(&request, &mia, None, &claim_url, &json!({})).await;
        assert_eq!(status, 422);
        assert_eq!(body["error"], "job-not-executor");

        // Owner mengambil; runner Mia tidak bisa menjalankan job owner.
        let (status, _) = post_json(&request, &token, None, &claim_url, &json!({})).await;
        assert_eq!(status, 200);
        let mia_runner = register(&request, &mia, &[3; 32]).await;
        let (status, body) = post_json(
            &request,
            &mia,
            Some(&mia_runner),
            &format!("/api/runner/jobs/{job}/start"),
            &json!({}),
        )
        .await;
        assert_eq!(status, 422, "{body}");
        assert_eq!(body["error"], "job-not-holder");
        let (status, _) = post_json(
            &request,
            &mia,
            Some(&mia_runner),
            &format!("/api/runner/jobs/{job}/logs"),
            &json!({ "lines": [] }),
        )
        .await;
        assert_eq!(status, 404);
        // Owner tidak bisa memakai runner Mia.
        let (status, _) = post_json(
            &request,
            &token,
            Some(&mia_runner),
            &format!("/api/runner/jobs/{job}/start"),
            &json!({}),
        )
        .await;
        assert_eq!(status, 404);

        // Mia tidak bisa melepas lock owner; owner bisa.
        let (status, body) = post_json(
            &request,
            &mia,
            None,
            &format!("/api/cli/jobs/{job}/release"),
            &json!({}),
        )
        .await;
        assert_eq!(status, 422, "{body}");
        let (status, body) = post_json(
            &request,
            &token,
            None,
            &format!("/api/cli/jobs/{job}/release"),
            &json!({ "note": "pindah laptop" }),
        )
        .await;
        assert_eq!(status, 200, "{body}");
        assert_eq!(body["status"], "queued");
    })
    .await;
}

#[tokio::test]
async fn concurrent_claims_lock_the_job_once() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let (job, token) = ticket_in_dev(&request, &ctx, &f).await;
        let url = format!("/api/cli/jobs/{job}/claim");
        let empty = json!({});
        let (a, b) = tokio::join!(
            post_json(&request, &token, None, &url, &empty),
            post_json(&request, &token, None, &url, &empty),
        );
        let mut codes = [a.0, b.0];
        codes.sort_unstable();
        assert_eq!(codes, [200, 422]);
    })
    .await;
}

#[tokio::test]
async fn dangerous_actions_pause_the_job_until_decided() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let (job, token) = ticket_in_dev(&request, &ctx, &f).await;
        let runner = register(&request, &token, &SECRET).await;
        post_json(&request, &token, None, &format!("/api/cli/jobs/{job}/claim"), &json!({ "runner_id": runner })).await;
        post_json(&request, &token, Some(&runner), &format!("/api/runner/jobs/{job}/start"), &json!({})).await;
        let perm_url = format!("/api/runner/jobs/{job}/permissions");

        // Aksi yang selalu diblokir tidak bisa diminta.
        let (status, body) = post_json(&request, &token, Some(&runner), &perm_url, &json!({ "command": "git push -f origin main", "action": "force_push_main" })).await;
        assert_eq!(status, 422, "{body}");
        assert_eq!(body["error"], "job-action-blocked");

        let (status, perm) = post_json(
            &request,
            &token,
            Some(&runner),
            &perm_url,
            &json!({ "command": "npx prisma migrate deploy", "action": "db_migrate", "reason": "skema baru" }),
        )
        .await;
        assert_eq!(status, 200, "{perm}");
        assert_eq!(perm["status"], "pending");
        let job_id = Uuid::parse_str(&job).unwrap();
        let row = jobs::Entity::find_by_id(job_id).one(&ctx.db).await.unwrap().unwrap();
        assert_eq!(row.status, "paused");

        // Diputuskan dari web (service) → runner melihat status baru.
        let pid = Uuid::parse_str(perm["id"].as_str().unwrap()).unwrap();
        let session = agentgate_server::services::mcp::Session::load(
            &ctx.db,
            &agentgate_server::services::oauth::authenticate(&ctx.db, &token).await.unwrap().unwrap(),
        )
        .await
        .unwrap()
        .unwrap();
        agentgate_server::services::jobs::decide_permission(
            &ctx.db,
            &session.acting(),
            pid,
            agentgate_core::runner::protocol::PermissionStatus::AllowedTicket,
        )
        .await
        .unwrap();
        let (status, info) = get_json(&request, &token, &format!("{perm_url}/{pid}")).await;
        // GET tanpa header runner → 404.
        assert_eq!(status, 404, "{info}");
        let res = request
            .get(&format!("{perm_url}/{pid}"))
            .add_header("authorization", format!("Bearer {token}"))
            .add_header("x-agentgate-runner", runner.clone())
            .await;
        assert_eq!(res.status_code(), 200);
        assert_eq!(res.json::<Value>()["status"], "allowed_ticket");
        let row = jobs::Entity::find_by_id(job_id).one(&ctx.db).await.unwrap().unwrap();
        assert_eq!(row.status, "running");

        // Jawaban dari CLI: izin yang sudah dijawab tidak bisa dijawab lagi.
        let (status, body) = post_json(
            &request,
            &token,
            None,
            &format!("/api/cli/permissions/{pid}"),
            &json!({ "status": "denied" }),
        )
        .await;
        assert_eq!(status, 422, "{body}");
        assert_eq!(body["error"], "job-permission-closed");

        // Aksi yang sama di tiket ini langsung diizinkan.
        let (_, again) = post_json(&request, &token, Some(&runner), &perm_url, &json!({ "command": "npx prisma migrate deploy", "action": "db_migrate" })).await;
        assert_eq!(again["status"], "allowed_ticket");
    })
    .await;
}

#[tokio::test]
async fn events_announce_claimed_jobs_to_the_runner() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let (job, token) = ticket_in_dev(&request, &ctx, &f).await;
        let runner = register(&request, &token, &SECRET).await;
        post_json(
            &request,
            &token,
            None,
            &format!("/api/cli/jobs/{job}/claim"),
            &json!({ "runner_id": runner }),
        )
        .await;
        let (status, events) = get_json(
            &request,
            &token,
            &format!("/api/runner/events?runner={runner}"),
        )
        .await;
        assert_eq!(status, 200, "{events}");
        assert!(
            events
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["type"] == "job_ready" && e["job"]["id"] == job.as_str()),
            "{events}"
        );

        // Batalkan → job_cancelled.
        let (status, body) = post_json(
            &request,
            &token,
            None,
            &format!("/api/cli/jobs/{job}/cancel"),
            &json!({}),
        )
        .await;
        assert_eq!(status, 200, "{body}");
        assert_eq!(body["status"], "cancelled");
        let (_, events) = get_json(
            &request,
            &token,
            &format!("/api/runner/events?runner={runner}"),
        )
        .await;
        assert!(
            events
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["type"] == "job_cancelled" && e["job_id"] == job.as_str()),
            "{events}"
        );
    })
    .await;
}

/// M12: perubahan job membangunkan pendengar realtime lewat trigger `pg_notify` (bukan polling),
/// hanya dengan id workspace job itu.
#[tokio::test]
async fn job_changes_wake_realtime_listeners_via_listen_notify() {
    request::<App, _, _>(|request, ctx| async move {
        let hub = agentgate_server::services::realtime::Hub::of(&ctx).expect("hub terpasang");
        let f = fixture(&request, &ctx).await;
        let mut rx = hub.subscribe(f.workspace.id);
        let mut other = hub.subscribe(uuid::Uuid::now_v7());
        // Tunggu listener tersambung sebelum ada perubahan.
        for _ in 0..50 {
            if hub.stats().connected {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        assert!(hub.stats().connected, "listener tersambung");
        let (job, token) = ticket_in_dev(&request, &ctx, &f).await;
        let got = tokio::time::timeout(std::time::Duration::from_secs(3), rx.recv())
            .await
            .expect("notifikasi < 3 dtk")
            .unwrap();
        assert_eq!(got, f.workspace.id);
        // Klaim (UPDATE jobs) juga membangunkan.
        while rx.try_recv().is_ok() {}
        let (status, _) = post_json(
            &request,
            &token,
            None,
            &format!("/api/cli/jobs/{job}/claim"),
            &json!({}),
        )
        .await;
        assert_eq!(status, 200);
        let got = tokio::time::timeout(std::time::Duration::from_secs(3), rx.recv())
            .await
            .expect("notifikasi klaim")
            .unwrap();
        assert_eq!(got, f.workspace.id);
        assert!(hub.stats().received >= 2);
        assert_eq!(hub.stats().listeners, 2);
        // Workspace lain tidak dibangunkan.
        assert!(other.try_recv().is_err());
    })
    .await;
}

#[tokio::test]
async fn agent_mention_queues_a_comment_job_for_the_author() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let code = create_ticket(&request, &f.cookie, &[("title", "Laporan error")]).await;
        let res = request
            .post(&format!("/w/acme/t/{code}/comments"))
            .add_cookie(f.cookie.clone())
            .form(&[("body", "@agent-qa tolong cek log error ini")])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let token = device_login(&request, &f.cookie, &f.workspace.id.to_string()).await;
        let (_, list) = get_json(&request, &token, "/api/cli/jobs").await;
        let job = list
            .as_array()
            .unwrap()
            .iter()
            .find(|j| j["kind"] == "comment")
            .expect("job komentar")
            .clone();
        assert_eq!(job["status"], "claimed");
        assert_eq!(job["persona"], "qa");
        assert_eq!(job["mine"], true);
        let id = job["id"].as_str().unwrap();
        let page = request
            .get(&format!("/w/acme/jobs/{id}"))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(page.status_code(), 200);
        assert!(
            page.text().contains(&format!("Job QA · {code}")),
            "judul job komentar memakai nama persona"
        );

        // Runner menjalankan dan membalas sebagai komentar.
        let runner = register(&request, &token, &SECRET).await;
        let (status, spec) = post_json(
            &request,
            &token,
            Some(&runner),
            &format!("/api/runner/jobs/{id}/start"),
            &json!({}),
        )
        .await;
        assert_eq!(status, 200, "{spec}");
        assert!(spec["prompt"].as_str().unwrap().contains("tolong cek log"));
        let (status, _) = post_json(
            &request,
            &token,
            Some(&runner),
            &format!("/api/runner/jobs/{id}/finish"),
            &json!({ "outcome": "succeeded", "summary": "Log menunjukkan timeout DB." }),
        )
        .await;
        assert_eq!(status, 200);
        let page = request
            .get(&format!("/w/acme/t/{code}"))
            .add_cookie(f.cookie.clone())
            .await;
        assert!(page.text().contains("Log menunjukkan timeout DB."));
    })
    .await;
}

#[tokio::test]
async fn jobs_and_runners_are_isolated_per_workspace() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let (job, _token) = ticket_in_dev(&request, &ctx, &f).await;
        let other = a_user().name("Oscar Other").create(&ctx).await;
        let ws = a_workspace(&other).slug("lain").create(&ctx).await;
        let cookie = sign_in_as(&request, &other).await;
        let token = device_login(&request, &cookie, &ws.id.to_string()).await;
        let (_, list) = get_json(&request, &token, "/api/cli/jobs").await;
        assert!(list.as_array().unwrap().is_empty());
        let (status, body) = post_json(
            &request,
            &token,
            None,
            &format!("/api/cli/jobs/{job}/claim"),
            &json!({}),
        )
        .await;
        assert_eq!(status, 404, "{body}");
        let (status, _) = post_json(
            &request,
            &token,
            None,
            &format!("/api/cli/jobs/{job}/cancel"),
            &json!({}),
        )
        .await;
        assert_eq!(status, 404);
    })
    .await;
}

#[tokio::test]
async fn register_rejects_bad_keys_and_reuses_known_runner() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let token = device_login(&request, &f.cookie, &f.workspace.id.to_string()).await;
        let (status, body) = post_json(
            &request,
            &token,
            None,
            "/api/runner/register",
            &json!({ "name": "X", "os": "linux", "client": "claude", "version": "0.1.0", "public_key": "bukan-kunci" }),
        )
        .await;
        assert_eq!(status, 422);
        assert_eq!(body["error"], "runner-key-invalid");
        let id = register(&request, &token, &SECRET).await;
        let (status, again) = post_json(
            &request,
            &token,
            None,
            "/api/runner/register",
            &json!({ "runner_id": id, "name": "MacBook Baru", "os": "macos", "client": "claude", "version": "0.1.1", "public_key": public_key_b64(&SECRET) }),
        )
        .await;
        assert_eq!(status, 200, "{again}");
        assert_eq!(again["id"], id.as_str());
        assert_eq!(again["name"], "MacBook Baru");
    })
    .await;
}

/// Runner tanpa `--client` memakai "Client default" pengaturan runner web; client eksplisit menang.
#[tokio::test]
async fn register_falls_back_to_the_default_client_preference() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let token = device_login(&request, &f.cookie, &f.workspace.id.to_string()).await;
        let res = web_post(
            &request,
            &f.cookie,
            "/w/acme/runner/prefs",
            &[("default_client", "codex"), ("quota_policy", "wait")],
        )
        .await;
        assert_eq!(res.status_code(), 303);
        let body = |client: &str| {
            json!({ "name": "Laptop", "os": "linux", "client": client, "version": "0.1.0",
                    "public_key": public_key_b64(&SECRET) })
        };
        let (status, info) =
            post_json(&request, &token, None, "/api/runner/register", &body("")).await;
        assert_eq!(status, 200, "{info}");
        assert_eq!(info["client"], "codex");
        let (status, info) = post_json(
            &request,
            &token,
            None,
            "/api/runner/register",
            &body("gemini"),
        )
        .await;
        assert_eq!(status, 200, "{info}");
        assert_eq!(info["client"], "gemini");
    })
    .await;
}

// ---------------------------------------------------------------------------------------------
// Web: Runner & CLI, Log job, kartu job di tiket.

async fn web_post(
    request: &TestServer,
    cookie: &loco_rs::auth::cookies::Cookie<'static>,
    url: &str,
    form: &[(&str, &str)],
) -> axum_test::TestResponse {
    request
        .post(url)
        .add_cookie(cookie.clone())
        .form(form)
        .await
}

/// Job berjalan di runner owner dengan log, langkah, dan evidence.
pub(super) async fn running_job(
    request: &TestServer,
    ctx: &AppContext,
    f: &Fixture,
) -> (String, String, String) {
    let (job, token) = ticket_in_dev(request, ctx, f).await;
    let runner = register(request, &token, &SECRET).await;
    post_json(
        request,
        &token,
        None,
        &format!("/api/cli/jobs/{job}/claim"),
        &json!({ "runner_id": runner }),
    )
    .await;
    post_json(
        request,
        &token,
        Some(&runner),
        &format!("/api/runner/jobs/{job}/start"),
        &json!({}),
    )
    .await;
    post_json(
        request,
        &token,
        Some(&runner),
        &format!("/api/runner/jobs/{job}/logs"),
        &json!({ "lines": [{ "stream": "tool", "text": "ag_context KSR-1" }] }),
    )
    .await;
    post_json(
        request,
        &token,
        Some(&runner),
        &format!("/api/runner/jobs/{job}/steps"),
        &json!({ "steps": [{ "name": "Baca konteks", "state": "ok", "seconds": 4 }, { "name": "Tulis kode", "state": "now" }] }),
    )
    .await;
    let shot = evidence(
        &job,
        EvidenceKind::Screenshot,
        "checkout.png",
        "image/png",
        PNG,
        &EvidenceSummary::default(),
        &SECRET,
    );
    let (status, _) = post_json(
        request,
        &token,
        Some(&runner),
        &format!("/api/runner/jobs/{job}/evidence"),
        &shot,
    )
    .await;
    assert_eq!(status, 201);
    (job, token, runner)
}

#[tokio::test]
async fn runner_pages_render_every_panel() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let (job, _token, runner) = running_job(&request, &ctx, &f).await;
        let res = request
            .get("/w/acme/runner")
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(res.header("location"), "/w/acme/runner/mine");
        let res = request.get("/runner").add_cookie(f.cookie.clone()).await;
        assert_eq!(res.header("location"), "/w/acme/runner");
        for panel in ["perm", "queue", "mine", "team", "settings", "drift", "cli"] {
            let res = request
                .get(&format!("/w/acme/runner/{panel}"))
                .add_cookie(f.cookie.clone())
                .await;
            assert_eq!(res.status_code(), 200, "{panel}: {}", res.text());
            let html = res.text();
            assert_css_exists(&html, &format!("[data-runner-page='{panel}']"));
            assert_css_exists(&html, "[data-subnav] [data-nav='queue']");
            assert_css_exists(&html, "[data-nav='runner'][aria-current='page']");
        }
        let html = request
            .get("/w/acme/runner/mine")
            .add_cookie(f.cookie.clone())
            .await
            .text();
        assert_css_exists(
            &html,
            &format!("[data-runner='{runner}'][data-state='busy']"),
        );
        assert_css_exists(&html, &format!("[data-held] [data-job='{job}']"));
        let html = request
            .get("/w/acme/runner/queue")
            .add_cookie(f.cookie.clone())
            .await
            .text();
        assert_css_exists(
            &html,
            &format!("tr[data-job='{job}'][data-status='running']"),
        );
        let res = request
            .get("/w/acme/runner/nope")
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}

#[tokio::test]
async fn job_page_shows_steps_log_evidence_and_live_partial() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let (job, _, _) = running_job(&request, &ctx, &f).await;
        let res = request
            .get(&format!("/w/acme/jobs/{job}"))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 200);
        let html = res.text();
        assert_css_exists(&html, &format!("[data-job-page='{job}']"));
        assert_css_exists(&html, "#job-live[hx-trigger]");
        assert_css_exists(&html, "[data-job-steps] li[data-state='ok']");
        assert_css_exists(&html, "[data-log] [data-stream='tool']");
        assert_css_exists(&html, "[data-evidence='screenshot']");
        assert_css_exists(&html, "[data-release]");
        assert_css_exists(&html, "[data-cancel]");
        let res = request
            .get(&format!("/w/acme/jobs/{job}/live"))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 200);
        assert_css_exists(&res.text(), "#job-live [data-log]");

        // Evidence: gambar inline dengan nosniff.
        let id = job_evidence::Entity::find()
            .filter(job_evidence::Column::JobId.eq(Uuid::parse_str(&job).unwrap()))
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap()
            .id;
        let res = request
            .get(&format!("/w/acme/evidence/{id}"))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 200);
        assert_eq!(res.header("content-type"), "image/png");
        assert_eq!(res.header("x-content-type-options"), "nosniff");

        // Tiket: kartu stage menampilkan job dan evidence.
        let html = request
            .get("/w/acme/t/KSR-1")
            .add_cookie(f.cookie.clone())
            .await
            .text();
        assert_css_exists(&html, "[data-stage-job='running'] [data-job-link]");
        assert_css_exists(&html, "[data-stage-evidence] [data-evidence='screenshot']");

        // Workspace lain → 404.
        let other = a_user().name("Oscar Other").create(&ctx).await;
        a_workspace(&other).slug("lain").create(&ctx).await;
        let cookie = sign_in_as(&request, &other).await;
        let res = request
            .get(&format!("/w/lain/jobs/{job}"))
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(res.status_code(), 404);
        let res = request
            .get(&format!("/w/lain/evidence/{id}"))
            .add_cookie(cookie)
            .await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}

#[tokio::test]
async fn web_claim_release_cancel_and_permission() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let (job, token) = ticket_in_dev(&request, &ctx, &f).await;
        let job_id = Uuid::parse_str(&job).unwrap();
        let status = |ctx: AppContext| async move {
            jobs::Entity::find_by_id(job_id).one(&ctx.db).await.unwrap().unwrap().status
        };

        // Kartu stage: tombol Ambil untuk executor.
        let html = request.get("/w/acme/t/KSR-1").add_cookie(f.cookie.clone()).await.text();
        assert_css_exists(&html, "[data-stage-job='queued'] [data-claim]");
        let res = web_post(&request, &f.cookie, &format!("/w/acme/jobs/{job}/claim"), &[("next", "/w/acme/t/KSR-1"), ("runner", "")]).await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(res.header("location"), "/w/acme/t/KSR-1");
        assert_eq!(status(ctx.clone()).await, "claimed");
        // Klaim kedua → flash error, tetap redirect.
        let res = web_post(&request, &f.cookie, &format!("/w/acme/jobs/{job}/claim"), &[("next", "https://evil.test/")]).await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(res.header("location"), format!("/w/acme/jobs/{job}"));

        // Izin dari runner → muncul di panel izin → izinkan dari web.
        let runner = register(&request, &token, &SECRET).await;
        post_json(&request, &token, Some(&runner), &format!("/api/runner/jobs/{job}/start"), &json!({})).await;
        let (_, perm) = post_json(
            &request,
            &token,
            Some(&runner),
            &format!("/api/runner/jobs/{job}/permissions"),
            &json!({ "command": "npm install left-pad", "action": "install_package", "reason": "butuh paket" }),
        )
        .await;
        let pid = perm["id"].as_str().unwrap().to_string();
        let res = request.get("/w/acme/runner").add_cookie(f.cookie.clone()).await;
        assert_eq!(res.header("location"), "/w/acme/runner/perm");
        let html = request.get("/w/acme/runner/perm").add_cookie(f.cookie.clone()).await.text();
        assert_css_exists(&html, &format!("[data-permission='{pid}'] [data-deny]"));
        let res = web_post(&request, &f.cookie, &format!("/w/acme/runner/permissions/{pid}"), &[("decision", "allow"), ("next", "/w/acme/runner/perm")]).await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(status(ctx.clone()).await, "running");
        let p = agentgate_server::models::job_permissions::Entity::find_by_id(Uuid::parse_str(&pid).unwrap())
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(p.status, "allowed");

        // Lepas lalu batalkan.
        let res = web_post(&request, &f.cookie, &format!("/w/acme/jobs/{job}/release"), &[("note", "pindah laptop")]).await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(status(ctx.clone()).await, "queued");
        let res = web_post(&request, &f.cookie, &format!("/w/acme/jobs/{job}/cancel"), &[]).await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(status(ctx.clone()).await, "cancelled");
    })
    .await;
}

#[tokio::test]
async fn runner_settings_and_revoke() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let token = device_login(&request, &f.cookie, &f.workspace.id.to_string()).await;
        let runner = register(&request, &token, &SECRET).await;
        let res = web_post(
            &request,
            &f.cookie,
            "/w/acme/runner/prefs",
            &[
                ("auto_claim", "1"),
                ("max_parallel", "3"),
                ("default_client", "claude"),
                ("quota_policy", "release"),
                ("terminal_linux", "kitty"),
                ("terminal_macos", "rm -rf /"),
            ],
        )
        .await;
        assert_eq!(res.status_code(), 303);
        let prefs = agentgate_server::models::runner_prefs::Model::of(
            &ctx.db,
            f.workspace.workspace_id(),
            f.owner.id,
        )
        .await
        .unwrap();
        assert!(prefs.auto_claim);
        assert_eq!(prefs.max_parallel, Some(3));
        assert_eq!(prefs.terminals["linux"], "kitty");
        assert_eq!(prefs.terminals["macos"], "");
        let html = request
            .get("/w/acme/runner/settings")
            .add_cookie(f.cookie.clone())
            .await
            .text();
        assert_css_exists(&html, "input[name='auto_claim'][checked]");
        assert_css_exists(
            &html,
            "input[name='quota_policy'][value='release'][checked]",
        );

        // Member lain tidak bisa mencabut runner owner.
        let (_, mia) = member(&request, &ctx, &f, "Mia Member").await;
        let res = web_post(
            &request,
            &mia,
            &format!("/w/acme/runner/runners/{runner}/revoke"),
            &[],
        )
        .await;
        assert_eq!(res.status_code(), 404);
        let res = web_post(
            &request,
            &f.cookie,
            &format!("/w/acme/runner/runners/{runner}/revoke"),
            &[],
        )
        .await;
        assert_eq!(res.status_code(), 303);
        let (status, _) = get_json(
            &request,
            &token,
            &format!("/api/runner/events?runner={runner}"),
        )
        .await;
        assert_eq!(status, 404, "runner dicabut");
    })
    .await;
}

/// "Job paralel maksimal" (utang M5): server menolak `start` bila runner sudah menjalankan
/// `max_parallel` job; slot kosong lagi setelah job selesai.
#[tokio::test]
async fn start_refuses_jobs_beyond_max_parallel() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let (first, token) = ticket_in_dev(&request, &ctx, &f).await;
        create_ticket(&request, &f.cookie, &[("title", "Struk WhatsApp")]).await;
        let res = request
            .post("/w/acme/t/KSR-2/stage/complete")
            .add_cookie(f.cookie.clone())
            .form(&[("x", "")])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let (_, list) = get_json(&request, &token, "/api/cli/jobs").await;
        let second = list
            .as_array()
            .unwrap()
            .iter()
            .find(|j| j["ticket"] == "KSR-2")
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();
        let runner = register(&request, &token, &SECRET).await;
        let res = web_post(
            &request,
            &f.cookie,
            "/w/acme/runner/prefs",
            &[
                ("max_parallel", "1"),
                ("default_client", "claude"),
                ("quota_policy", "wait"),
            ],
        )
        .await;
        assert_eq!(res.status_code(), 303);
        for job in [&first, &second] {
            let (status, body) = post_json(
                &request,
                &token,
                None,
                &format!("/api/cli/jobs/{job}/claim"),
                &json!({ "runner_id": runner }),
            )
            .await;
            assert_eq!(status, 200, "{body}");
        }
        let start = |job: String| {
            let (request, token, runner) = (&request, token.clone(), runner.clone());
            async move {
                post_json(
                    request,
                    &token,
                    Some(&runner),
                    &format!("/api/runner/jobs/{job}/start"),
                    &json!({}),
                )
                .await
            }
        };
        assert_eq!(start(first.clone()).await.0, 200);
        // Start ulang job yang sama (lanjutkan) tetap boleh.
        assert_eq!(start(first.clone()).await.0, 200);
        let (status, body) = start(second.clone()).await;
        assert_eq!(status, 422, "{body}");
        assert_eq!(body["error"], "job-runner-busy");

        let (status, _) = post_json(
            &request,
            &token,
            Some(&runner),
            &format!("/api/runner/jobs/{first}/finish"),
            &json!({ "outcome": "failed", "summary": "x" }),
        )
        .await;
        assert_eq!(status, 200);
        assert_eq!(start(second).await.0, 200);
    })
    .await;
}

/// R1: panel Runner tim dan pengaturan tim hanya Owner/Admin; runner pribadi tidak auto-ambil
/// (pengaturan pribadi default mati), runner tim auto-ambil sejak ditandai.
#[tokio::test]
async fn team_runner_settings_are_admin_only_and_team_runners_auto_claim() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let (job, token) = ticket_in_dev(&request, &ctx, &f).await;
        let runner = register(&request, &token, &SECRET).await;

        // Member: tidak melihat panel tim dan tidak bisa mengubah runner tim.
        let (_, mia) = member(&request, &ctx, &f, "Mia Member").await;
        let res = request
            .get("/w/acme/runner/team")
            .add_cookie(mia.clone())
            .await;
        assert_eq!(res.status_code(), 404);
        let html = request
            .get("/w/acme/runner/mine")
            .add_cookie(mia.clone())
            .await
            .text();
        assert_css_exists(&html, "[data-subnav] [data-nav='mine']");
        assert_css_not_exists(&html, "[data-subnav] [data-nav='team']");
        let res = web_post(
            &request,
            &mia,
            &format!("/w/acme/runner/runners/{runner}/team"),
            &[("team", "1")],
        )
        .await;
        assert_eq!(res.status_code(), 404);

        // Runner pribadi: auto-ambil mati → event tidak mengunci job.
        let (status, _) = get_json(
            &request,
            &token,
            &format!("/api/runner/events?runner={runner}"),
        )
        .await;
        assert_eq!(status, 200);
        let row =
            agentgate_server::models::jobs::Entity::find_by_id(Uuid::parse_str(&job).unwrap())
                .one(&ctx.db)
                .await
                .unwrap()
                .unwrap();
        assert_eq!(row.status, "queued");

        // Owner menandai runner tim → auto-ambil menyala (default runner tim), lalu mengambil job.
        let res = web_post(
            &request,
            &f.cookie,
            &format!("/w/acme/runner/runners/{runner}/team"),
            &[("team", "1"), ("max_parallel", "2")],
        )
        .await;
        assert_eq!(res.status_code(), 303);
        let r = agentgate_server::models::runners::Entity::find_by_id(
            Uuid::parse_str(&runner).unwrap(),
        )
        .one(&ctx.db)
        .await
        .unwrap()
        .unwrap();
        assert!(r.team && r.auto_claim);
        assert_eq!(r.max_parallel, Some(2));
        let html = request
            .get("/w/acme/runner/team")
            .add_cookie(f.cookie.clone())
            .await
            .text();
        assert_css_exists(
            &html,
            &format!("[data-runner='{runner}'] [data-team-runner]"),
        );
        assert_css_exists(&html, "[data-team-form]");
        get_json(
            &request,
            &token,
            &format!("/api/runner/events?runner={runner}"),
        )
        .await;
        let row =
            agentgate_server::models::jobs::Entity::find_by_id(Uuid::parse_str(&job).unwrap())
                .one(&ctx.db)
                .await
                .unwrap()
                .unwrap();
        assert_eq!(row.status, "claimed");
        assert_eq!(row.runner_id, Some(r.id));
        let audit = crate::support::newest_audit_events(&ctx.db, f.workspace.workspace_id(), 1)
            .await
            .unwrap();
        assert_eq!(audit[0].action, "runner.updated");
    })
    .await;
}
