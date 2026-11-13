//! `agentgate runner start --yes --once`: register → polling event (WebSocket tidak ada) →
//! start → client palsu (stream-json) → log/langkah → tes + screenshot sebagai evidence
//! bertanda tangan → commit lokal → finish.

use std::{
    fs,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use agentgate_core::runner::{
    protocol::{EvidenceKind, EvidenceUpload, RegisterRunner},
    rules,
    signing::{evidence_payload, verify},
};
use serde_json::{Value, json};

use crate::{
    install::rules_response,
    support::{Env, git, git_repo, serve, text},
};

const JOB: &str = "0193a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b";
const RUNNER: &str = "0193a1b2-0000-7000-8000-000000000001";
const BRANCH: &str = "ag/KSR-26-validasi-stok";
const PROMPT: &str = "Kerjakan stage \"Development\" untuk KSR-26; $(touch /tmp/pwned) `id`";

const FAKE_CLAUDE: &str = r#"#!/bin/sh
for a in "$@"; do printf '%s\n' "$a"; done > "$AGENTGATE_STATE_DIR/claude-args.txt"
printf '%s' "$AGENTGATE_JOB_ID" > "$AGENTGATE_STATE_DIR/claude-job.txt"
mkdir -p .agentgate/evidence
printf '\211PNG\r\n\032\nfake' > .agentgate/evidence/layar.png
echo 'pub fn stok() {}' > stok.rs
echo '{"type":"system","subtype":"init","model":"claude-test-model","tools":["Bash"]}'
echo '{"type":"assistant","message":{"content":[{"type":"text","text":"Saya muat konteks."},{"type":"tool_use","id":"t1","name":"Bash","input":{"command":"cargo test -p kasir"}}]}}'
echo '{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t2","name":"TodoWrite","input":{"todos":[{"content":"Tambah validasi","status":"completed","activeForm":"x"}]}}]}}'
echo 'warning: sesuatu' >&2
echo '{"type":"result","subtype":"success","is_error":false,"num_turns":3,"result":"Validasi stok selesai.","total_cost_usd":0.12,"usage":{"input_tokens":100,"output_tokens":50}}'
"#;

fn job_summary() -> Value {
    json!({
        "id": JOB, "kind": "stage", "mode": "auto", "status": "claimed",
        "ticket": "KSR-26", "title": "Validasi stok", "stage": "dev",
        "stage_name": "Development", "persona": "dev", "priority": 2,
        "mine": true, "can_claim": false, "created_at": "2026-11-13T00:00:00Z"
    })
}

fn job_spec(base: &str) -> Value {
    json!({
        "id": JOB, "kind": "stage", "mode": "auto",
        "ticket": "KSR-26", "title": "Validasi stok",
        "url": format!("{base}/w/kasir/t/KSR-26"),
        "project": "KSR", "repos": ["kasir"],
        "stage": "dev", "stage_name": "Development",
        "persona": "dev", "persona_name": "Dev",
        "model": "claude-test-model", "effort": "", "client": "claude",
        "command": "/ag-dev", "prompt": PROMPT, "branch": BRANCH,
        "mcp_url": format!("{base}/mcp"),
        "rules": rules::agent_gate_files(base, "KSR"),
        "danger": [{ "action": "deploy", "level": "block" }],
        "allowed_actions": []
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn runner_once_runs_a_job_and_reports_signed_evidence() {
    let started = Arc::new(AtomicBool::new(false));
    let flag = started.clone();
    let fake = serve(Arc::new(move |fake, req| {
        let job = format!("/api/runner/jobs/{JOB}/");
        match (req.method.as_str(), req.path.as_str()) {
            ("GET", "/api/cli/rules") => rules_response(&fake.base, req),
            ("POST", "/api/runner/register") => (
                200,
                json!({
                    "id": RUNNER, "name": "laptop", "workspace": "Kasir",
                    "user_email": "ani@example.com", "auto_claim": false, "max_parallel": null
                }),
            ),
            ("GET", "/api/runner/events") if !flag.load(Ordering::SeqCst) => {
                (200, json!([{ "type": "job_ready", "job": job_summary() }]))
            }
            ("GET", "/api/runner/events") => (200, json!([])),
            ("POST", p) if p == format!("{job}start") => {
                flag.store(true, Ordering::SeqCst);
                (200, job_spec(&fake.base))
            }
            ("POST", p) if p == format!("{job}logs") || p == format!("{job}steps") => {
                (204, Value::Null)
            }
            ("POST", p) if p == format!("{job}evidence") => {
                (201, json!({ "id": RUNNER, "kind": "test_report" }))
            }
            ("POST", p) if p == format!("{job}finish") => {
                (200, json!({ "id": JOB, "status": "succeeded" }))
            }
            _ => (404, json!({ "error": "not_found" })),
        }
    }))
    .await;

    let mut env = Env::new("runner");
    env.login(&fake.base);
    let repo = env.root.join("kasir");
    git_repo(&repo);
    let out = env.run(&repo, &["init", "--project", "KSR", "--yes"]).await;
    assert!(out.status.success(), "{}", text(&out));
    let toml = repo.join(".agentgate/project.toml");
    let body = fs::read_to_string(&toml).unwrap().replace(
        "command = \"\"",
        "command = \"echo 'test result: ok. 3 passed; 0 failed; 0 ignored'\"",
    );
    fs::write(&toml, body).unwrap();
    git(&repo, &["commit", "-qam", "tes"]);

    let bin = env.root.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let claude = bin.join("claude");
    fs::write(&claude, FAKE_CLAUDE).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&claude, fs::Permissions::from_mode(0o755)).unwrap();
    }
    env.extra_path = Some(bin);

    let out = env
        .run(&repo, &["runner", "start", "--yes", "--once"])
        .await;
    assert!(out.status.success(), "{}", text(&out));

    // Pendaftaran: kunci publik runner, id disimpan.
    let register = fake.find("POST", "/api/runner/register");
    assert_eq!(register.len(), 1);
    let reg: RegisterRunner = serde_json::from_str(&register[0].body).unwrap();
    assert!(reg.runner_id.is_none());
    assert_eq!(reg.client, "claude");
    let identity: Value =
        serde_json::from_str(&fs::read_to_string(env.config.join("runner.json")).unwrap()).unwrap();
    assert_eq!(identity["runner_id"], RUNNER);
    // WebSocket tidak tersedia → polling.
    assert!(!fake.find("GET", "/api/runner/events").is_empty());
    assert!(
        fake.find("GET", "/api/runner/events")[0]
            .query
            .contains(RUNNER)
    );

    // Semua endpoint job membawa header runner.
    let job_reqs: Vec<_> = fake
        .requests()
        .into_iter()
        .filter(|r| r.path.starts_with("/api/runner/jobs/"))
        .collect();
    assert!(!job_reqs.is_empty());
    for r in &job_reqs {
        assert_eq!(
            r.headers.get("x-agentgate-runner").map(String::as_str),
            Some(RUNNER),
            "{}",
            r.path
        );
        assert_eq!(
            r.headers.get("authorization").map(String::as_str),
            Some("Bearer at-test")
        );
    }
    let order: Vec<&str> = job_reqs
        .iter()
        .map(|r| r.path.rsplit('/').next().unwrap())
        .collect();
    assert_eq!(order.first(), Some(&"start"));
    assert_eq!(order.last(), Some(&"finish"));

    // Prompt sampai utuh sebagai satu argumen (tanpa shell), dengan flag isolasi dan model.
    let args = fs::read_to_string(env.state.join("claude-args.txt")).unwrap();
    let args: Vec<&str> = args.lines().collect();
    assert_eq!(args[0], "-p");
    assert_eq!(args[1], PROMPT);
    for flag in [
        "--strict-mcp-config",
        "--verbose",
        "stream-json",
        "acceptEdits",
    ] {
        assert!(args.contains(&flag), "{args:?}");
    }
    assert!(
        args.windows(2)
            .any(|w| w == ["--model", "claude-test-model"])
    );
    assert_eq!(
        fs::read_to_string(env.state.join("claude-job.txt")).unwrap(),
        JOB
    );

    // Log dari stream-json dan stderr.
    let lines: Vec<Value> = fake
        .find("POST", "/logs")
        .iter()
        .flat_map(|r| r.json()["lines"].as_array().cloned().unwrap_or_default())
        .collect();
    let has = |stream: &str, text: &str| {
        lines
            .iter()
            .any(|l| l["stream"] == stream && l["text"].as_str().unwrap_or("").contains(text))
    };
    assert!(has("info", "model claude-test-model"), "{lines:?}");
    assert!(has("tool", "Bash: cargo test -p kasir"), "{lines:?}");
    assert!(has("err", "warning: sesuatu"), "{lines:?}");
    assert!(has("ok", "Agent selesai"), "{lines:?}");
    assert!(has("ok", "Tes lulus"), "{lines:?}");
    // Langkah dari TodoWrite.
    assert!(fake.find("POST", "/steps").iter().any(|r| {
        r.json()["steps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["name"] == "Tambah validasi" && s["state"] == "ok")
    }));

    // Evidence: tes lulus + screenshot, keduanya bertanda tangan kunci runner.
    let evidence: Vec<EvidenceUpload> = fake
        .find("POST", "/evidence")
        .iter()
        .map(|r| serde_json::from_str(&r.body).unwrap())
        .collect();
    assert_eq!(evidence.len(), 2, "{evidence:?}");
    let job_id = uuid::Uuid::try_parse(JOB).unwrap();
    for e in &evidence {
        let payload = evidence_payload(job_id, e.kind, &e.name, &e.sha256, &e.summary);
        assert!(
            verify(&reg.public_key, &payload, &e.signature),
            "{}",
            e.name
        );
    }
    let report = evidence
        .iter()
        .find(|e| e.kind == EvidenceKind::TestReport)
        .unwrap();
    assert_eq!(report.summary.passed, Some(true));
    assert_eq!(report.summary.total, Some(3));
    assert_eq!(report.summary.failed, Some(0));
    assert_eq!(report.content_type, "text/plain");
    let shot = evidence
        .iter()
        .find(|e| e.kind == EvidenceKind::Screenshot)
        .unwrap();
    assert_eq!(shot.name, "layar.png");
    assert_eq!(shot.content_type, "image/png");

    let finish = fake.find("POST", "/finish");
    assert_eq!(finish.len(), 1);
    let report = finish[0].json();
    assert_eq!(report["outcome"], "succeeded");
    assert_eq!(report["summary"], "Validasi stok selesai.");
    assert_eq!(report["tokens"], 150);

    // Hasil kerja di-commit di branch tiket (tanpa file aturan dan bukti).
    let wt = env.state.join("wt/kasir").join(BRANCH);
    assert_eq!(git(&wt, &["branch", "--show-current"]), BRANCH);
    assert_eq!(
        git(&wt, &["show", "--name-only", "--format=%s", "HEAD"]),
        "feat(KSR-26): Validasi stok\n\nstok.rs"
    );
    // State hook dibersihkan setelah job selesai.
    assert!(!env.state.join(format!("jobs/{JOB}.json")).exists());
}
