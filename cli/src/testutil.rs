//! Helper tes unit: direktori sementara, repo git, contoh `JobSpec`.

use std::{fs, path::PathBuf, process::Command};

use uuid::Uuid;

/// Direktori sementara unik (tidak dihapus; di temp OS).
pub fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("agentgate-cli-{tag}-{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

/// Repo git kosong dengan identitas commit lokal.
pub fn git_repo(tag: &str) -> PathBuf {
    let dir = temp_dir(tag);
    for args in [
        &["init", "-q", "-b", "main"][..],
        &["config", "user.name", "Tes"],
        &["config", "user.email", "tes@example.com"],
        &["config", "commit.gpgsign", "false"],
    ] {
        let ok = Command::new("git")
            .arg("-C")
            .arg(&dir)
            .args(args)
            .status()
            .unwrap()
            .success();
        assert!(ok, "git {args:?}");
    }
    dir
}

/// `JobSpec` contoh dalam bentuk JSON server.
pub fn spec_json() -> serde_json::Value {
    serde_json::json!({
        "id": "0193a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b",
        "kind": "stage",
        "mode": "auto",
        "ticket": "KSR-26",
        "title": "Validasi stok",
        "url": "http://x/w/a/t/KSR-26",
        "project": "KSR",
        "repos": ["kasir"],
        "stage": "dev",
        "stage_name": "Development",
        "persona": "dev",
        "persona_name": "Dev",
        "model": "claude-sonnet-4-5",
        "effort": "",
        "client": "claude",
        "command": "/ag-dev",
        "prompt": "Kerjakan stage \"Development\"; $(rm -rf /) `x` 'y'",
        "branch": "ag/KSR-26-validasi-stok",
        "mcp_url": "http://x/mcp",
        "rules": [],
        "danger": [],
        "allowed_actions": []
    })
}
