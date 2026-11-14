//! `agentgate init` / `uninstall` / `rules check` di repo git sementara.

use std::{fs, sync::Arc};

use agentgate_core::runner::rules;
use serde_json::{Value, json};

use crate::support::{Env, Req, git, git_repo, serve, text};

/// Respons `GET /api/cli/rules` server palsu.
pub fn rules_response(base: &str, req: &Req) -> (u16, Value) {
    if !req.query.contains("project=KSR") {
        return (404, json!({ "error": "project_not_found" }));
    }
    let files = rules::agent_gate_files(base, "KSR");
    (
        200,
        json!({
            "project": "KSR",
            "project_id": "0193a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b",
            "project_name": "Kasir",
            "checksum": rules::checksum(&files),
            "files": files,
        }),
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn init_backs_up_legacy_rules_and_uninstall_restores_them() {
    let fake = serve(Arc::new(|fake, req| match req.path.as_str() {
        "/api/cli/rules" => rules_response(&fake.base, req),
        "/api/runner/drift" => (204, Value::Null),
        _ => (404, json!({ "error": "not_found" })),
    }))
    .await;
    let env = Env::new("init");
    env.login(&fake.base);
    let repo = env.root.join("kasir");
    git_repo(&repo);
    fs::create_dir_all(repo.join(".claude/commands")).unwrap();
    fs::create_dir_all(repo.join("web")).unwrap();
    fs::write(repo.join("CLAUDE.md"), "aturan lama\n").unwrap();
    fs::write(repo.join(".claude/commands/deploy.md"), "deploy\n").unwrap();
    fs::write(repo.join("web/AGENTS.md"), "web lama\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "aturan lama"]);

    let out = env.run(&repo, &["init", "--project", "KSR", "--yes"]).await;
    assert!(out.status.success(), "{}", text(&out));
    assert_eq!(
        fs::read_to_string(repo.join("CLAUDE.md")).unwrap(),
        "@AGENTS.md\n"
    );
    assert!(!repo.join(".claude/commands").exists());
    assert!(!repo.join("web/AGENTS.md").exists());
    assert!(
        fs::read_to_string(repo.join(".mcp.json"))
            .unwrap()
            .contains(&format!("{}/mcp", fake.base))
    );
    // Aturan v2: config tipis untuk keenam client sekaligus.
    for f in rules::agent_gate_files(&fake.base, "KSR") {
        assert_eq!(
            fs::read_to_string(repo.join(&f.path)).unwrap(),
            f.body,
            "{}",
            f.path
        );
    }
    let project = fs::read_to_string(repo.join(".agentgate/project.toml")).unwrap();
    assert!(project.contains("key = \"KSR\""), "{project}");
    assert!(project.contains("[test]"), "{project}");
    let backups: Vec<String> = fs::read_dir(repo.join(".agentgate/backup"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(backups.len(), 2, "{backups:?}");
    // Semua perubahan ada di satu commit; working tree bersih.
    assert_eq!(git(&repo, &["status", "--porcelain"]), "");
    assert_eq!(
        git(&repo, &["log", "-1", "--format=%s"]),
        "chore: pasang aturan Agent Gate"
    );
    let registry: Value =
        serde_json::from_str(&fs::read_to_string(env.config.join("repos.json")).unwrap()).unwrap();
    assert_eq!(registry["repos"][0]["name"], "kasir");
    assert_eq!(registry["repos"][0]["project"], "KSR");

    // Init ulang: aturan Agent Gate sendiri bukan aturan lama (tidak ada backup baru).
    let out = env.run(&repo, &["init", "--yes"]).await;
    assert!(out.status.success(), "{}", text(&out));
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("Tidak ada file aturan lama"),
        "{}",
        text(&out)
    );
    assert_eq!(
        fs::read_dir(repo.join(".agentgate/backup"))
            .unwrap()
            .count(),
        2
    );

    let out = env.run(&repo, &["rules", "check"]).await;
    assert!(out.status.success(), "{}", text(&out));
    let drift = fake.find("POST", "/api/runner/drift");
    assert_eq!(drift[0].json()["changed"], json!([]));
    fs::write(repo.join("AGENTS.md"), "diubah\n").unwrap();
    let out = env.run(&repo, &["rules", "check"]).await;
    assert!(!out.status.success(), "{}", text(&out));
    assert!(String::from_utf8_lossy(&out.stdout).contains("AGENTS.md"));
    let drift = fake.find("POST", "/api/runner/drift");
    assert_eq!(drift[1].json()["changed"], json!(["AGENTS.md"]));
    assert_eq!(drift[1].json()["repo"], "kasir");
    git(&repo, &["checkout", "--", "AGENTS.md"]);

    let out = env.run(&repo, &["uninstall", "--yes"]).await;
    assert!(out.status.success(), "{}", text(&out));
    assert_eq!(
        fs::read_to_string(repo.join("CLAUDE.md")).unwrap(),
        "aturan lama\n"
    );
    assert_eq!(
        fs::read_to_string(repo.join(".claude/commands/deploy.md")).unwrap(),
        "deploy\n"
    );
    assert_eq!(
        fs::read_to_string(repo.join("web/AGENTS.md")).unwrap(),
        "web lama\n"
    );
    assert!(!repo.join("AGENTS.md").exists());
    assert!(!repo.join(".mcp.json").exists());
    for dir in [".codex", ".gemini", ".cursor", ".opencode", ".github"] {
        assert!(!repo.join(dir).exists(), "{dir}");
    }
    assert!(!repo.join("opencode.json").exists());
    assert!(!repo.join(".agentgate/project.toml").exists());
    assert_eq!(git(&repo, &["status", "--porcelain"]), "");
}

#[tokio::test(flavor = "multi_thread")]
async fn rules_sync_pushes_branch_and_opens_pull_request() {
    let fake = serve(Arc::new(|fake, req| match req.path.as_str() {
        "/api/cli/rules" => rules_response(&fake.base, req),
        "/api/runner/drift" => (204, Value::Null),
        "/api/cli/rules/pull-request" => (
            200,
            json!({ "number": 9, "url": "https://github.com/pt/kasir/pull/9", "state": "open", "created": true }),
        ),
        _ => (404, json!({ "error": "not_found" })),
    }))
    .await;
    let env = Env::new("sync");
    env.login(&fake.base);
    let repo = env.root.join("kasir");
    git_repo(&repo);
    // Remote bernama seperti repo server (`…/kasir.git` → repo `kasir`).
    let origin = env.root.join("remote/kasir.git");
    git(
        &env.root,
        &[
            "init",
            "-q",
            "--bare",
            "-b",
            "main",
            origin.to_str().unwrap(),
        ],
    );
    git(
        &repo,
        &["remote", "add", "origin", origin.to_str().unwrap()],
    );
    let out = env.run(&repo, &["init", "--project", "KSR", "--yes"]).await;
    assert!(out.status.success(), "{}", text(&out));
    // Aturan diubah lokal → berbeda dari versi server.
    fs::write(repo.join("AGENTS.md"), "diubah lokal\n").unwrap();
    git(&repo, &["commit", "-qam", "ubah aturan"]);

    let out = env.run(&repo, &["rules", "sync"]).await;
    assert!(out.status.success(), "{}", text(&out));
    assert!(text(&out).contains("PR #9"), "{}", text(&out));
    assert_eq!(
        git(
            &origin,
            &["log", "-1", "--format=%s", "agentgate/sync-rules"]
        ),
        "chore: update aturan Agent Gate"
    );
    let reqs = fake.find("POST", "/api/cli/rules/pull-request");
    assert_eq!(reqs.len(), 1);
    assert_eq!(reqs[0].json(), json!({ "project": "KSR", "repo": "kasir" }));
    // Kembali ke branch semula.
    assert_eq!(git(&repo, &["branch", "--show-current"]), "main");
}
