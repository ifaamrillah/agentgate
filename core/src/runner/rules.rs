//! File aturan Agent Gate di repo (spec "File aturan di repo", "Sinkronisasi aturan").
//!
//! Aturan di repo dibuat setipis mungkin dan ditulis untuk keenam client sekaligus (satu repo
//! dipakai tim dengan client berbeda):
//!
//! | Client | File |
//! | --- | --- |
//! | semua | `AGENTS.md` (pembuka; dibaca native Codex, Cursor, OpenCode, Copilot) |
//! | Claude Code | `CLAUDE.md` (`@AGENTS.md`), `.mcp.json`, `.claude/settings.json` (hooks) |
//! | Codex CLI | `.codex/config.toml` (MCP), `.codex/hooks.json` |
//! | Gemini CLI | `.gemini/settings.json` (`context.fileName`, MCP `httpUrl`, hooks) |
//! | Cursor | `.cursor/mcp.json`, `.cursor/hooks.json` |
//! | OpenCode | `opencode.json` (MCP remote), `.opencode/plugins/agentgate.ts` |
//! | Copilot CLI | `.mcp.json` (sama dengan Claude), `.github/hooks/agentgate.json` |
//!
//! Hook selalu `agentgate hook <event> --client <c>`; isi instruksi stage, persona, dan skill
//! diambil lewat MCP. Hook izin aksi berbahaya menunggu jawaban manusia, jadi batas waktunya
//! [`PERMISSION_HOOK_SECS`] di client yang mendukung batas per hook.
//!
//! Server menyajikan file ini (`GET /api/cli/rules`) supaya CLI dan runner selalu memakai versi
//! server; [`checksum`] dipakai untuk deteksi drift. Isi file ditulis sebagai teks tetap (bukan
//! hasil serialisasi map) supaya byte-nya sama di build mana pun.

use super::protocol::RuleFile;

/// Versi isi file aturan (naik bila bentuk file berubah → drift → PR sync).
pub const RULES_VERSION: u32 = 2;

/// Batas waktu hook `pre-tool` (detik): sama dengan batas tunggu jawaban izin di CLI. Bawaan
/// client (Claude Code 60 dtk) memotong permintaan izin yang menunggu executor.
pub const PERMISSION_HOOK_SECS: u64 = 30 * 60;

/// Path plugin OpenCode (OpenCode tidak punya hook shell; plugin memanggil `agentgate hook`).
pub const OPENCODE_PLUGIN: &str = ".opencode/plugins/agentgate.ts";

/// String JSON (dengan tanda kutip). Escape JSON juga sah sebagai basic string TOML.
fn quoted(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

/// Perintah hook untuk `event` di `client`.
fn hook(event: &str, client: &str) -> String {
    format!("agentgate hook {event} --client {client}")
}

fn agents_md(server: &str, project_key: &str) -> String {
    format!(
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
    )
}

/// `.mcp.json` (Claude Code + Copilot CLI: format `type: "http"` sama).
fn mcp_json(mcp: &str) -> String {
    format!(
        "{{\n  \"mcpServers\": {{\n    \"agentgate\": {{\n      \"type\": \"http\",\n      \
         \"url\": {mcp}\n    }}\n  }}\n}}\n"
    )
}

/// Hooks format Claude Code (juga dipakai Codex: `{"hooks": {Event: [{matcher, hooks}]}}`,
/// `timeout` dalam detik).
fn claude_style_hooks(client: &str) -> String {
    format!(
        "{{\n  \"hooks\": {{\n    \"SessionStart\": [\n      {{\n        \"hooks\": [\n          \
         {{ \"type\": \"command\", \"command\": \"{session}\" }}\n        ]\n      }}\n    ],\n    \
         \"PreToolUse\": [\n      {{\n        \"matcher\": \"Bash\",\n        \"hooks\": [\n          \
         {{ \"type\": \"command\", \"command\": \"{pre}\", \"timeout\": {PERMISSION_HOOK_SECS} }}\n        \
         ]\n      }}\n    ]\n  }}\n}}\n",
        session = hook("session-start", client),
        pre = hook("pre-tool", client),
    )
}

/// `.codex/config.toml`: hanya server MCP (config project dibaca Codex bila folder trusted).
fn codex_config(mcp: &str) -> String {
    format!(
        "# Agent Gate: dibuat `agentgate init`, diperbarui `agentgate rules sync`.\n\
         [mcp_servers.agentgate]\nurl = {mcp}\n"
    )
}

/// `.gemini/settings.json`: `AGENTS.md` sebagai file konteks, MCP Streamable HTTP (`httpUrl`),
/// hooks `SessionStart` + `BeforeTool` (shell = `run_shell_command`; `timeout` milidetik).
fn gemini_settings(mcp: &str) -> String {
    format!(
        "{{\n  \"context\": {{\n    \"fileName\": [\"AGENTS.md\"]\n  }},\n  \
         \"mcpServers\": {{\n    \"agentgate\": {{\n      \"httpUrl\": {mcp}\n    }}\n  }},\n  \
         \"hooks\": {{\n    \"SessionStart\": [\n      {{\n        \"hooks\": [\n          \
         {{ \"name\": \"agentgate-session\", \"type\": \"command\", \"command\": \"{session}\" }}\n        \
         ]\n      }}\n    ],\n    \"BeforeTool\": [\n      {{\n        \
         \"matcher\": \"run_shell_command\",\n        \"hooks\": [\n          \
         {{ \"name\": \"agentgate\", \"type\": \"command\", \"command\": \"{pre}\", \
         \"timeout\": {ms} }}\n        ]\n      }}\n    ]\n  }}\n}}\n",
        session = hook("session-start", "gemini"),
        pre = hook("pre-tool", "gemini"),
        ms = PERMISSION_HOOK_SECS * 1000,
    )
}

/// `.cursor/mcp.json`.
fn cursor_mcp(mcp: &str) -> String {
    format!(
        "{{\n  \"mcpServers\": {{\n    \"agentgate\": {{\n      \"url\": {mcp}\n    }}\n  }}\n}}\n"
    )
}

/// `.cursor/hooks.json` (versi 1; shell = `Shell`; `timeout` detik).
fn cursor_hooks() -> String {
    format!(
        "{{\n  \"version\": 1,\n  \"hooks\": {{\n    \"sessionStart\": [\n      \
         {{ \"command\": \"{session}\" }}\n    ],\n    \"preToolUse\": [\n      \
         {{ \"command\": \"{pre}\", \"matcher\": \"Shell\", \"timeout\": {PERMISSION_HOOK_SECS} }}\n    \
         ]\n  }}\n}}\n",
        session = hook("session-start", "cursor"),
        pre = hook("pre-tool", "cursor"),
    )
}

/// `opencode.json`: MCP remote (OpenCode V2: `mcp.servers.<id>`).
fn opencode_json(mcp: &str) -> String {
    format!(
        "{{\n  \"mcp\": {{\n    \"servers\": {{\n      \"agentgate\": {{\n        \
         \"type\": \"remote\",\n        \"url\": {mcp}\n      }}\n    }}\n  }}\n}}\n"
    )
}

/// Plugin OpenCode: meneruskan panggilan tool shell ke `agentgate hook pre-tool --client
/// opencode` dan menerapkan jawabannya (`deny` → lempar error, `command` → ganti perintah).
/// Tidak ada logika keputusan di sini.
fn opencode_plugin() -> String {
    format!(
        "// Agent Gate: dibuat `agentgate init`, diperbarui `agentgate rules sync`.\n\
         // Plugin tipis: semua keputusan dibuat `agentgate hook`; file ini hanya meneruskan\n\
         // payload tool dan menerapkan jawabannya.\n\
         import {{ spawn }} from \"node:child_process\";\n\n\
         function agentgate(event: string, payload: unknown): Promise<any> {{\n  \
         return new Promise((resolve) => {{\n    \
         const child = spawn(\"agentgate\", [\"hook\", event, \"--client\", \"opencode\"], {{\n      \
         stdio: [\"pipe\", \"pipe\", \"ignore\"],\n    }});\n    \
         let out = \"\";\n    \
         const timer = setTimeout(() => child.kill(), {ms});\n    \
         child.stdout.on(\"data\", (d) => (out += d));\n    \
         child.on(\"error\", () => resolve(null));\n    \
         child.on(\"close\", () => {{\n      \
         clearTimeout(timer);\n      \
         try {{\n        resolve(out.trim() ? JSON.parse(out) : null);\n      }} catch {{\n        \
         resolve(null);\n      }}\n    }});\n    \
         child.stdin.end(JSON.stringify(payload));\n  }});\n}}\n\n\
         export default {{\n  id: \"agentgate\",\n  async setup(ctx: any) {{\n    \
         ctx.tool?.hook?.(\"execute.before\", async (e: any) => {{\n      \
         const input = e.input ?? e.args ?? {{}};\n      \
         const answer = await agentgate(\"pre-tool\", {{\n        \
         tool_name: e.tool ?? e.name ?? \"\",\n        \
         tool_input: input,\n        \
         cwd: ctx.directory ?? process.cwd(),\n      }});\n      \
         if (answer?.decision === \"deny\") throw new Error(answer.reason ?? \"Agent Gate: ditolak\");\n      \
         if (typeof answer?.command === \"string\") input.command = answer.command;\n    }});\n  }},\n}};\n",
        ms = PERMISSION_HOOK_SECS * 1000,
    )
}

/// `.github/hooks/agentgate.json` (Copilot CLI; tanpa matcher, hook memilah tool shell sendiri;
/// `timeoutSec`).
fn copilot_hooks() -> String {
    format!(
        "{{\n  \"version\": 1,\n  \"hooks\": {{\n    \"sessionStart\": [\n      \
         {{ \"type\": \"command\", \"bash\": \"{session}\", \"powershell\": \"{session}\" }}\n    ],\n    \
         \"preToolUse\": [\n      \
         {{ \"type\": \"command\", \"bash\": \"{pre}\", \"powershell\": \"{pre}\", \
         \"timeoutSec\": {PERMISSION_HOOK_SECS} }}\n    ]\n  }}\n}}\n",
        session = hook("session-start", "copilot"),
        pre = hook("pre-tool", "copilot"),
    )
}

/// File aturan Agent Gate untuk project `project_key` di server `server_url`.
#[must_use]
pub fn agent_gate_files(server_url: &str, project_key: &str) -> Vec<RuleFile> {
    let server = server_url.trim_end_matches('/');
    let mcp = quoted(&format!("{server}/mcp"));
    let file = |path: &str, body: String| RuleFile {
        path: path.into(),
        body,
    };
    vec![
        file("AGENTS.md", agents_md(server, project_key)),
        file("CLAUDE.md", "@AGENTS.md\n".into()),
        file(".mcp.json", mcp_json(&mcp)),
        file(".claude/settings.json", claude_style_hooks("claude")),
        file(".codex/config.toml", codex_config(&mcp)),
        file(".codex/hooks.json", claude_style_hooks("codex")),
        file(".gemini/settings.json", gemini_settings(&mcp)),
        file(".cursor/mcp.json", cursor_mcp(&mcp)),
        file(".cursor/hooks.json", cursor_hooks()),
        file("opencode.json", opencode_json(&mcp)),
        file(OPENCODE_PLUGIN, opencode_plugin()),
        file(".github/hooks/agentgate.json", copilot_hooks()),
    ]
}

/// `.claude/settings.json` aturan v1 (hook tanpa `--client`).
const V1_CLAUDE_SETTINGS: &str = "{\n  \"hooks\": {\n    \"SessionStart\": [\n      {\n        \"hooks\": [\n          \
    { \"type\": \"command\", \"command\": \"agentgate hook session-start\" }\n        ]\n      }\n    ],\n    \
    \"PreToolUse\": [\n      {\n        \"matcher\": \"Bash\",\n        \"hooks\": [\n          \
    { \"type\": \"command\", \"command\": \"agentgate hook pre-tool\", \"timeout\": 1800 }\n        ]\n      }\n    ]\n  }\n}\n";

/// File `path` berisi `body` adalah aturan Agent Gate versi sebelumnya yang dikenali pasti
/// (`AGENTS.md` bertanda versi, `CLAUDE.md` impor, hooks v1). `init` ulang di repo lama tidak
/// mem-backup file ini sebagai "aturan lama" (yang nanti dipulihkan `uninstall`). File lain yang
/// isinya berbeda dari versi sekarang tetap di-backup: bisa saja berisi tambahan milik user.
#[must_use]
pub fn is_own_rule_file(path: &str, body: &str) -> bool {
    match path {
        "AGENTS.md" => body.contains("<!-- agentgate-rules v"),
        "CLAUDE.md" => body.trim() == "@AGENTS.md",
        ".claude/settings.json" => body == V1_CLAUDE_SETTINGS,
        _ => false,
    }
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
        ".opencode/plugins/",
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

    fn body<'a>(files: &'a [RuleFile], path: &str) -> &'a str {
        &files.iter().find(|f| f.path == path).unwrap().body
    }

    fn json(files: &[RuleFile], path: &str) -> serde_json::Value {
        serde_json::from_str(body(files, path)).unwrap_or_else(|e| panic!("{path}: {e}"))
    }

    #[test]
    fn files_cover_every_client() {
        let files = agent_gate_files("https://ag.example/", "KSR");
        let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(
            paths,
            [
                "AGENTS.md",
                "CLAUDE.md",
                ".mcp.json",
                ".claude/settings.json",
                ".codex/config.toml",
                ".codex/hooks.json",
                ".gemini/settings.json",
                ".cursor/mcp.json",
                ".cursor/hooks.json",
                "opencode.json",
                ".opencode/plugins/agentgate.ts",
                ".github/hooks/agentgate.json",
            ]
        );
        let agents = body(&files, "AGENTS.md");
        assert!(agents.contains("ag_context") && agents.contains("`KSR`"));
        assert!(agents.contains("<!-- agentgate-rules v2 -->"));
        assert_eq!(body(&files, "CLAUDE.md"), "@AGENTS.md\n");
        // Semua file JSON/TOML valid dan menunjuk server MCP yang sama.
        let mcp = "https://ag.example/mcp";
        assert_eq!(
            json(&files, ".mcp.json")["mcpServers"]["agentgate"]["type"],
            "http"
        );
        assert_eq!(
            json(&files, ".mcp.json")["mcpServers"]["agentgate"]["url"],
            mcp
        );
        let codex: toml::Table = toml::from_str(body(&files, ".codex/config.toml")).unwrap();
        assert_eq!(codex["mcp_servers"]["agentgate"]["url"].as_str(), Some(mcp));
        let gemini = json(&files, ".gemini/settings.json");
        assert_eq!(
            gemini["context"]["fileName"],
            serde_json::json!(["AGENTS.md"])
        );
        assert_eq!(gemini["mcpServers"]["agentgate"]["httpUrl"], mcp);
        assert_eq!(
            json(&files, ".cursor/mcp.json")["mcpServers"]["agentgate"]["url"],
            mcp
        );
        let opencode = json(&files, "opencode.json");
        assert_eq!(opencode["mcp"]["servers"]["agentgate"]["type"], "remote");
        assert_eq!(opencode["mcp"]["servers"]["agentgate"]["url"], mcp);
    }

    #[test]
    fn hooks_call_agentgate_with_client_and_long_permission_timeout() {
        let files = agent_gate_files("http://x", "A");
        for (path, client) in [
            (".claude/settings.json", "claude"),
            (".codex/hooks.json", "codex"),
        ] {
            let v = json(&files, path);
            let pre = &v["hooks"]["PreToolUse"][0];
            assert_eq!(pre["matcher"], "Bash", "{path}");
            assert_eq!(
                pre["hooks"][0]["command"],
                format!("agentgate hook pre-tool --client {client}")
            );
            // Izin aksi menunggu jawaban manusia: bawaan client (Claude Code 60 dtk) terlalu pendek.
            assert_eq!(pre["hooks"][0]["timeout"], 1800, "{path}");
            assert_eq!(
                v["hooks"]["SessionStart"][0]["hooks"][0]["command"],
                format!("agentgate hook session-start --client {client}")
            );
        }
        let gemini = json(&files, ".gemini/settings.json");
        let before = &gemini["hooks"]["BeforeTool"][0];
        assert_eq!(before["matcher"], "run_shell_command");
        assert_eq!(
            before["hooks"][0]["command"],
            "agentgate hook pre-tool --client gemini"
        );
        assert_eq!(before["hooks"][0]["timeout"], 1_800_000);
        assert_eq!(
            gemini["hooks"]["SessionStart"][0]["hooks"][0]["command"],
            "agentgate hook session-start --client gemini"
        );
        let cursor = json(&files, ".cursor/hooks.json");
        assert_eq!(cursor["version"], 1);
        assert_eq!(
            cursor["hooks"]["preToolUse"][0]["command"],
            "agentgate hook pre-tool --client cursor"
        );
        assert_eq!(cursor["hooks"]["preToolUse"][0]["matcher"], "Shell");
        assert_eq!(cursor["hooks"]["preToolUse"][0]["timeout"], 1800);
        assert_eq!(
            cursor["hooks"]["sessionStart"][0]["command"],
            "agentgate hook session-start --client cursor"
        );
        let copilot = json(&files, ".github/hooks/agentgate.json");
        let pre = &copilot["hooks"]["preToolUse"][0];
        assert_eq!(pre["bash"], "agentgate hook pre-tool --client copilot");
        assert_eq!(pre["powershell"], pre["bash"]);
        assert_eq!(pre["timeoutSec"], 1800);
        assert_eq!(
            copilot["hooks"]["sessionStart"][0]["bash"],
            "agentgate hook session-start --client copilot"
        );
        let plugin = body(&files, OPENCODE_PLUGIN);
        assert!(plugin.contains(r#"spawn("agentgate", ["hook", event, "--client", "opencode"]"#));
        assert!(plugin.contains("\"pre-tool\"") && plugin.contains("1800000"));
    }

    #[test]
    fn server_url_is_escaped() {
        let files = agent_gate_files("http://x/\"a\\b", "A");
        assert_eq!(
            json(&files, ".mcp.json")["mcpServers"]["agentgate"]["url"],
            "http://x/\"a\\b/mcp"
        );
        let codex: toml::Table = toml::from_str(body(&files, ".codex/config.toml")).unwrap();
        assert_eq!(
            codex["mcp_servers"]["agentgate"]["url"].as_str(),
            Some("http://x/\"a\\b/mcp")
        );
    }

    #[test]
    fn previous_agent_gate_rules_are_recognised() {
        assert!(is_own_rule_file(
            "AGENTS.md",
            "# x\n<!-- agentgate-rules v1 -->\n"
        ));
        assert!(is_own_rule_file("CLAUDE.md", "@AGENTS.md\n"));
        assert!(is_own_rule_file(
            ".claude/settings.json",
            V1_CLAUDE_SETTINGS
        ));
        let v1: serde_json::Value = serde_json::from_str(V1_CLAUDE_SETTINGS).unwrap();
        assert_eq!(
            v1["hooks"]["PreToolUse"][0]["hooks"][0]["command"],
            "agentgate hook pre-tool"
        );
        assert!(!is_own_rule_file("AGENTS.md", "# aturan tim\n"));
        assert!(!is_own_rule_file(".claude/settings.json", "{}"));
        assert!(!is_own_rule_file(
            ".mcp.json",
            "{\"mcpServers\":{\"agentgate\":{}}}"
        ));
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
            ".opencode/plugins/x.ts",
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
