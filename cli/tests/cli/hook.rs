//! `agentgate hook pre-tool` di dalam job: aksi `ask` → permintaan izin → menunggu jawaban
//! executor; aksi `block` ditolak tanpa ke server; di luar job hook diam.

use std::{
    fs,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use serde_json::{Value, json};

use crate::support::{Env, serve, text};

const JOB: &str = "0193a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b";
const RUNNER: &str = "0193a1b2-0000-7000-8000-000000000001";
const PERM: &str = "0193a1b2-0000-7000-8000-0000000000aa";

fn input(cmd: &str) -> String {
    json!({
        "session_id": "s", "hook_event_name": "PreToolUse", "cwd": "/tmp",
        "tool_name": "Bash", "tool_input": { "command": cmd, "description": "x" }
    })
    .to_string()
}

fn write_job_state(env: &Env) {
    let dir = env.state.join("jobs");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join(format!("{JOB}.json")),
        json!({
            "job_id": JOB, "runner_id": RUNNER, "ticket": "KSR-26", "title": "Validasi stok",
            "url": "http://x/w/kasir/t/KSR-26", "stage_name": "Development", "persona_name": "Dev",
            "danger": [
                { "action": "network", "level": "ask" },
                { "action": "deploy", "level": "block" }
            ],
            "allowed_actions": []
        })
        .to_string(),
    )
    .unwrap();
}

fn decision(out: &std::process::Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap_or(Value::Null)
}

#[tokio::test(flavor = "multi_thread")]
async fn ask_waits_for_the_executor_and_block_is_denied_locally() {
    let polls = Arc::new(AtomicUsize::new(0));
    let counter = polls.clone();
    let fake = serve(Arc::new(move |_, req| {
        let base = format!("/api/runner/jobs/{JOB}/permissions");
        match (req.method.as_str(), req.path.as_str()) {
            ("POST", p) if p == base => (200, json!({ "id": PERM, "status": "pending" })),
            ("GET", p) if p == format!("{base}/{PERM}") => {
                let status = if counter.fetch_add(1, Ordering::SeqCst) < 2 {
                    "pending"
                } else {
                    "allowed"
                };
                (200, json!({ "id": PERM, "status": status }))
            }
            _ => (404, json!({ "error": "not_found" })),
        }
    }))
    .await;
    let env = Env::new("hook");
    env.login(&fake.base);
    write_job_state(&env);
    let vars = [("AGENTGATE_JOB_ID", JOB), ("PATH", "/usr/bin:/bin")];

    let out = env
        .run_with(
            &env.root,
            &["hook", "pre-tool"],
            &vars,
            Some(&input("curl https://example.com")),
        )
        .await;
    assert!(out.status.success(), "{}", text(&out));
    let d = decision(&out);
    assert_eq!(
        d["hookSpecificOutput"]["permissionDecision"],
        "allow",
        "{}",
        text(&out)
    );
    let ask = fake.find("POST", "/permissions");
    assert_eq!(ask.len(), 1);
    assert_eq!(ask[0].json()["action"], "network");
    assert_eq!(ask[0].json()["command"], "curl https://example.com");
    assert_eq!(
        ask[0].headers.get("x-agentgate-runner").map(String::as_str),
        Some(RUNNER)
    );
    assert_eq!(polls.load(Ordering::SeqCst), 3);

    let out = env
        .run_with(
            &env.root,
            &["hook", "pre-tool"],
            &vars,
            Some(&input("vercel --prod")),
        )
        .await;
    let d = decision(&out);
    assert_eq!(d["hookSpecificOutput"]["permissionDecision"], "deny");
    assert!(
        d["hookSpecificOutput"]["permissionDecisionReason"]
            .as_str()
            .unwrap()
            .contains("deploy")
    );
    assert_eq!(fake.find("POST", "/permissions").len(), 1);

    // Di luar job: tidak ada keputusan (alur izin normal client).
    let out = env
        .run_with(
            &env.root,
            &["hook", "pre-tool"],
            &[("PATH", "/usr/bin:/bin")],
            Some(&input("curl https://example.com")),
        )
        .await;
    assert!(out.status.success());
    assert!(out.stdout.is_empty(), "{}", text(&out));
}
