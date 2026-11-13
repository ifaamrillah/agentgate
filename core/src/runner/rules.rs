//! File aturan Agent Gate di repo (spec "File aturan di repo", "Sinkronisasi aturan").
//!
//! Aturan di repo dibuat setipis mungkin: `AGENTS.md` (pembuka), `CLAUDE.md` (`@AGENTS.md`),
//! `.mcp.json` (koneksi MCP), dan `.claude/settings.json` (hooks yang hanya memanggil
//! `agentgate hook …`). Isi instruksi stage, persona, dan skill diambil lewat MCP.
//!
//! Server menyajikan file ini (`GET /api/cli/rules`) supaya CLI dan runner selalu memakai versi
//! server; [`checksum`] dipakai untuk deteksi drift.

use super::protocol::RuleFile;

/// Versi isi file aturan (naik bila bentuk file berubah → drift → PR sync).
pub const RULES_VERSION: u32 = 1;

/// File aturan Agent Gate untuk project `project_key` di server `server_url`.
#[must_use]
pub fn agent_gate_files(server_url: &str, project_key: &str) -> Vec<RuleFile> {
    let server = server_url.trim_end_matches('/');
    let agents = format!(
        "# Agent Gate\n\n\
         Project ini memakai [Agent Gate]({server}) (project `{project_key}`). Aturan, instruksi \
         stage, persona, dan skill disajikan server lewat MCP `agentgate`.\n\n\
         - Panggil tool `ag_context` sebelum mulai bekerja, lalu ikuti instruksi stage tiket.\n\
         - Simpan hasil stage dengan `ag_artifact` dan ajukan pindah stage dengan `ag_transition`; \
         gate dicek server.\n\
         - Bila ragu, tanya manusia dengan `ag_ask`. Jangan menebak requirement.\n\
         - Evidence (hasil test, screenshot) dibuat runner. Simpan screenshot di \
         `.agentgate/evidence/`.\n\n\
         <!-- agentgate-rules v{RULES_VERSION} -->\n"
    );
    let mcp = format!(
        "{{\n  \"mcpServers\": {{\n    \"agentgate\": {{\n      \"type\": \"http\",\n      \
         \"url\": \"{server}/mcp\"\n    }}\n  }}\n}}\n"
    );
    let settings = "{\n  \"hooks\": {\n    \"SessionStart\": [\n      {\n        \"hooks\": [\n          \
        { \"type\": \"command\", \"command\": \"agentgate hook session-start\" }\n        ]\n      }\n    ],\n    \
        \"PreToolUse\": [\n      {\n        \"matcher\": \"Bash\",\n        \"hooks\": [\n          \
        { \"type\": \"command\", \"command\": \"agentgate hook pre-tool\", \"timeout\": 1800 }\n        ]\n      }\n    ]\n  }\n}\n"
        .to_string();
    vec![
        RuleFile {
            path: "AGENTS.md".into(),
            body: agents,
        },
        RuleFile {
            path: "CLAUDE.md".into(),
            body: "@AGENTS.md\n".into(),
        },
        RuleFile {
            path: ".mcp.json".into(),
            body: mcp,
        },
        RuleFile {
            path: ".claude/settings.json".into(),
            body: settings,
        },
    ]
}

/// Checksum gabungan file aturan (urut path).
#[must_use]
pub fn checksum(files: &[RuleFile]) -> String {
    let mut sorted: Vec<&RuleFile> = files.iter().collect();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));
    let mut buf = Vec::new();
    for f in sorted {
        buf.extend_from_slice(f.path.as_bytes());
        buf.push(0);
        buf.extend_from_slice(super::sha256_hex(f.body.as_bytes()).as_bytes());
        buf.push(b'\n');
    }
    super::sha256_hex(&buf)
}

/// Path file aturan yang isinya berbeda dari `expected` (hilang = berbeda).
pub fn changed_files(expected: &[RuleFile], read: impl Fn(&str) -> Option<String>) -> Vec<String> {
    expected
        .iter()
        .filter(|f| read(&f.path).as_deref() != Some(f.body.as_str()))
        .map(|f| f.path.clone())
        .collect()
}

/// File aturan client lain yang diganti `agentgate init` (path relatif repo, pemisah `/`).
/// `CLAUDE.md`/`AGENTS.md`/`GEMINI.md` di semua level folder; direktori dan file konfigurasi
/// client di root.
#[must_use]
pub fn is_legacy_rule_path(rel: &str) -> bool {
    let rel = rel.trim_start_matches("./");
    if rel.starts_with(".agentgate/") || rel.starts_with(".git/") {
        return false;
    }
    let name = rel.rsplit('/').next().unwrap_or(rel);
    if matches!(
        name,
        "CLAUDE.md" | "AGENTS.md" | "GEMINI.md" | "CLAUDE.local.md"
    ) {
        return true;
    }
    const ROOT: &[&str] = &[
        ".claude/skills/",
        ".claude/commands/",
        ".claude/agents/",
        ".cursor/rules/",
        ".agents/skills/",
        ".gemini/commands/",
        ".opencode/commands/",
        ".codex/",
        ".github/hooks/",
    ];
    const ROOT_FILES: &[&str] = &[
        ".claude/settings.json",
        ".claude/settings.local.json",
        ".mcp.json",
        ".cursorrules",
        ".gemini/settings.json",
        ".cursor/mcp.json",
        ".cursor/hooks.json",
        "opencode.json",
        ".github/copilot-instructions.md",
    ];
    ROOT.iter().any(|p| rel.starts_with(p)) || ROOT_FILES.contains(&rel)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_reference_server_and_hooks() {
        let files = agent_gate_files("https://ag.example/", "KSR");
        let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(
            paths,
            [
                "AGENTS.md",
                "CLAUDE.md",
                ".mcp.json",
                ".claude/settings.json"
            ]
        );
        assert!(files[0].body.contains("ag_context") && files[0].body.contains("`KSR`"));
        assert!(files[2].body.contains("https://ag.example/mcp"));
        let settings: serde_json::Value = serde_json::from_str(&files[3].body).unwrap();
        assert_eq!(
            settings["hooks"]["PreToolUse"][0]["hooks"][0]["command"],
            "agentgate hook pre-tool"
        );
        // Izin aksi menunggu jawaban manusia: batas bawaan hook Claude Code (60 dtk) terlalu pendek.
        assert_eq!(
            settings["hooks"]["PreToolUse"][0]["hooks"][0]["timeout"],
            1800
        );
        let mcp: serde_json::Value = serde_json::from_str(&files[2].body).unwrap();
        assert_eq!(mcp["mcpServers"]["agentgate"]["type"], "http");
    }

    #[test]
    fn checksum_and_drift() {
        let files = agent_gate_files("http://x", "A");
        assert_eq!(
            checksum(&files),
            checksum(&files.iter().rev().cloned().collect::<Vec<_>>())
        );
        let changed = changed_files(&files, |p| {
            (p != "CLAUDE.md")
                .then(|| files.iter().find(|f| f.path == p).map(|f| f.body.clone()))
                .flatten()
        });
        assert_eq!(changed, ["CLAUDE.md"]);
    }

    #[test]
    fn legacy_paths() {
        for p in [
            "CLAUDE.md",
            "web/AGENTS.md",
            ".claude/skills/x/SKILL.md",
            ".claude/settings.json",
            ".mcp.json",
            ".cursor/rules/a.mdc",
            ".github/copilot-instructions.md",
        ] {
            assert!(is_legacy_rule_path(p), "{p}");
        }
        for p in [
            "README.md",
            ".agentgate/project.toml",
            "src/claude.rs",
            ".github/workflows/ci.yml",
        ] {
            assert!(!is_legacy_rule_path(p), "{p}");
        }
    }
}
