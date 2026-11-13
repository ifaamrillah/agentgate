//! Hook client AI: file hook di repo (aturan v2) memanggil `agentgate hook <event> --client <c>`.
//!
//! Logika keputusan sama untuk semua client ([`plan`], [`session_context`]); yang berbeda per
//! client hanya adapter I/O: bentuk payload stdin ([`HookInput::parse`]) dan bentuk jawaban
//! ([`output`], [`session_output`]):
//!
//! | Client | Payload tool | Jawaban `pre-tool` |
//! | --- | --- | --- |
//! | Claude Code, Codex | `tool_name` `Bash`, `tool_input` | `hookSpecificOutput.permissionDecision` + `updatedInput` |
//! | Gemini CLI | `tool_name` `run_shell_command` | `decision` + `hookSpecificOutput.tool_input` |
//! | Cursor | `tool_name` `Shell` | `permission` + `updated_input` |
//! | Copilot CLI | `toolName` `bash`/`powershell`, `toolArgs` (string JSON) | `permissionDecision` + `modifiedArgs` |
//! | OpenCode (plugin) | `tool_name` `bash`, `tool_input` | `decision`, `reason`, `command` |
//!
//! **Pemanggilan ganda.** Copilot (dan Cursor) juga menjalankan hook `.claude/settings.json`.
//! Runner memberi env [`JOB_CLIENT_ENV`] ke proses client job; hook dengan `--client` lain diam,
//! jadi satu perintah hanya diputuskan (dan ditanyakan ke executor) sekali.
//!
//! - **`pre-tool`** (`PreToolUse`, matcher `Bash`): perintah diklasifikasi
//!   ([`agentgate_core::runner::danger::decide`]) dengan tingkat izin persona job. `block` →
//!   ditolak; `ask` → permintaan izin ke server (job dijeda) lalu menunggu jawaban executor;
//!   `allow` → diizinkan (job headless tidak punya manusia untuk menjawab prompt izin). Di luar
//!   job runner (`AGENTGATE_JOB_ID` tidak ada) hook tidak mengambil keputusan, kecuali menulis
//!   ulang perintah aman lewat RTK.
//! - **`session-start`**: konteks singkat job/project ke stdout (disuntikkan ke sesi).
//!
//! RTK: bila `rtk` ada di PATH, `rtk rewrite "<cmd>"` memberi versi perintah yang outputnya
//! dikompresi; dikirim balik sebagai `updatedInput`.

use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};

use agentgate_core::{
    persona::Level,
    runner::{
        danger::{classify_command, decide},
        protocol::{PermissionAsk, PermissionInfo, PermissionStatus},
    },
};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    auth,
    config::{self, JobState, Paths, ProjectFile},
    runner::clients::ClientKind,
};

/// Batas tunggu jawaban izin (detik); `AGENTGATE_PERMISSION_TIMEOUT` menggantinya.
const PERMISSION_TIMEOUT: u64 = 30 * 60;
/// Interval cek status izin (detik).
const PERMISSION_POLL: u64 = 3;
/// Env client job yang diberi runner ke proses client (`claude`, `codex`, …).
pub const JOB_CLIENT_ENV: &str = "AGENTGATE_JOB_CLIENT";
/// Nama tool shell di semua client (dibandingkan tanpa beda huruf besar/kecil).
const SHELL_TOOLS: &[&str] = &[
    "bash",
    "shell",
    "run_shell_command",
    "powershell",
    "run_terminal_cmd",
];

/// Input hook (stdin), dinormalkan ke bentuk Claude Code.
#[derive(Debug, Default, Deserialize)]
pub struct HookInput {
    #[serde(default)]
    pub tool_name: String,
    #[serde(default)]
    pub tool_input: Value,
    #[serde(default)]
    pub cwd: String,
}

impl HookInput {
    /// Payload `raw` dari `client`. Rusak/kosong → input kosong (hook tidak memutuskan).
    #[must_use]
    pub fn parse(client: ClientKind, raw: &str) -> Self {
        let v: Value = serde_json::from_str(raw).unwrap_or(Value::Null);
        let text = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default();
        let mut input: Self = serde_json::from_value(v.clone()).unwrap_or_default();
        if client == ClientKind::Copilot && input.tool_name.is_empty() {
            input.tool_name = text("toolName").to_string();
            input.tool_input = match v.get("toolArgs").cloned().unwrap_or(Value::Null) {
                Value::String(s) => serde_json::from_str(&s).unwrap_or(Value::String(s)),
                other => other,
            };
        }
        if input.cwd.is_empty() {
            // Cursor: `workspace_roots`; Gemini/Copilot kadang tanpa `cwd`.
            input.cwd = v
                .pointer("/workspace_roots/0")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
        }
        input
    }

    fn command(&self) -> Option<&str> {
        let shell = SHELL_TOOLS
            .iter()
            .any(|t| t.eq_ignore_ascii_case(&self.tool_name));
        shell
            .then(|| self.tool_input.get("command").and_then(Value::as_str))
            .flatten()
            .filter(|c| !c.trim().is_empty())
    }
}

/// Keputusan lokal untuk satu perintah.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    /// Tidak memutuskan (alur izin normal client).
    Pass,
    /// Izinkan, opsional dengan perintah hasil tulis ulang RTK.
    Allow {
        rewritten: Option<String>,
    },
    Deny {
        reason: String,
    },
    /// Tanya executor lewat server.
    Ask {
        action: String,
        command: String,
    },
}

/// Rencana keputusan `pre-tool` (tanpa jaringan).
#[must_use]
pub fn plan(
    input: &HookInput,
    job: Option<&JobState>,
    rewrite: impl Fn(&str) -> Option<String>,
) -> Plan {
    let Some(command) = input.command() else {
        return Plan::Pass;
    };
    let Some(job) = job else {
        // Di luar job: hanya perintah biasa (bukan aksi berbahaya) yang ditulis ulang RTK.
        return match classify_command(command).map_or_else(|| rewrite(command), |_| None) {
            Some(r) => Plan::Allow { rewritten: Some(r) },
            None => Plan::Pass,
        };
    };
    let verdict = decide(command, |action| {
        if job.allowed_actions.iter().any(|a| a == action) {
            Some(Level::Allow)
        } else {
            job.danger
                .iter()
                .find(|d| d.action == action)
                .map(|d| d.level)
        }
    });
    match (verdict.level, verdict.action) {
        (Level::Allow, _) => Plan::Allow {
            rewritten: rewrite(command),
        },
        (Level::Block, action) => Plan::Deny {
            reason: format!(
                "Agent Gate: aksi `{}` diblokir untuk persona {}. Cari cara lain atau minta \
                 manusia menjalankannya.",
                action.unwrap_or("?"),
                job.persona_name
            ),
        },
        (Level::Ask, action) => Plan::Ask {
            action: action.unwrap_or("?").to_string(),
            command: command.to_string(),
        },
    }
}

/// Jawaban `pre-tool` untuk `client`: `decision` `allow`/`deny`, alasan, dan perintah hasil tulis
/// ulang RTK (dikirim sebagai input tool baru dalam dialek client).
#[must_use]
pub fn output(
    client: ClientKind,
    input: &HookInput,
    decision: &str,
    reason: &str,
    rewritten: Option<&str>,
) -> Value {
    let updated = rewritten.and_then(|cmd| {
        let mut updated = input.tool_input.clone();
        let obj = updated.as_object_mut()?;
        obj.insert("command".into(), Value::String(cmd.to_string()));
        Some(updated)
    });
    let mut v = match client {
        ClientKind::Claude | ClientKind::Codex => json!({ "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": decision,
            "permissionDecisionReason": reason,
        }}),
        ClientKind::Gemini => json!({ "decision": decision, "reason": reason }),
        ClientKind::Cursor => json!({
            "permission": decision,
            "user_message": reason,
            "agent_message": reason,
        }),
        ClientKind::Copilot => json!({
            "permissionDecision": decision,
            "permissionDecisionReason": reason,
        }),
        ClientKind::Opencode => json!({ "decision": decision, "reason": reason }),
    };
    if let Some(updated) = updated {
        match client {
            ClientKind::Claude | ClientKind::Codex => {
                v["hookSpecificOutput"]["updatedInput"] = updated;
            }
            ClientKind::Gemini => {
                v["hookSpecificOutput"] =
                    json!({ "hookEventName": "BeforeTool", "tool_input": updated });
            }
            ClientKind::Cursor => v["updated_input"] = updated,
            ClientKind::Copilot => v["modifiedArgs"] = updated,
            ClientKind::Opencode => v["command"] = Value::from(rewritten),
        }
    }
    v
}

/// Teks stdout `session-start` untuk `client`: Claude Code/OpenCode teks biasa, lainnya JSON
/// konteks tambahan dalam dialek client.
#[must_use]
pub fn session_output(client: ClientKind, text: &str) -> String {
    let v = match client {
        ClientKind::Claude | ClientKind::Opencode => return text.to_string(),
        ClientKind::Codex | ClientKind::Gemini => json!({ "hookSpecificOutput": {
            "hookEventName": "SessionStart",
            "additionalContext": text,
        }}),
        ClientKind::Cursor => json!({ "additional_context": text }),
        ClientKind::Copilot => json!({ "additionalContext": text }),
    };
    v.to_string()
}

/// Hook dipanggil client lain dari client job (mis. Copilot menjalankan hook Claude): diam.
fn foreign(client: ClientKind) -> bool {
    std::env::var(JOB_CLIENT_ENV)
        .ok()
        .and_then(|c| ClientKind::parse(&c))
        .is_some_and(|job| job != client)
}

/// Cari program di PATH.
#[must_use]
pub fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).find_map(|dir| {
        let candidates = if cfg!(windows) {
            vec![
                dir.join(format!("{name}.exe")),
                dir.join(format!("{name}.cmd")),
            ]
        } else {
            vec![dir.join(name)]
        };
        candidates.into_iter().find(|p| p.is_file())
    })
}

/// `rtk rewrite <cmd>` bila RTK terpasang dan memberi perintah lain.
#[must_use]
pub fn rtk_rewrite(command: &str) -> Option<String> {
    let rtk = which("rtk")?;
    let out = Command::new(rtk)
        .arg("rewrite")
        .arg(command)
        .output()
        .ok()?;
    let rewritten = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (out.status.success() && !rewritten.is_empty() && rewritten != command.trim())
        .then_some(rewritten)
}

fn job_from_env(paths: &Paths) -> Option<JobState> {
    let id = std::env::var("AGENTGATE_JOB_ID").ok()?;
    let id = Uuid::try_parse(id.trim()).ok()?;
    JobState::load(paths, id).ok().flatten()
}

fn read_input(client: ClientKind) -> HookInput {
    let mut raw = String::new();
    let _ = std::io::stdin().read_to_string(&mut raw);
    HookInput::parse(client, &raw)
}

fn emit(value: &Value) {
    let mut stdout = std::io::stdout();
    let _ = writeln!(stdout, "{value}");
    let _ = stdout.flush();
}

/// Minta izin ke server dan tunggu jawabannya. `Ok(true)` = diizinkan.
async fn ask(paths: &Paths, job: &JobState, action: &str, command: &str) -> Result<bool, String> {
    let (api, _) = auth::session(paths).await.map_err(|e| e.user_message())?;
    let api = api.with_runner(job.runner_id);
    let base = format!("/api/runner/jobs/{}/permissions", job.job_id);
    let info: PermissionInfo = api
        .post(
            &base,
            &PermissionAsk {
                command: command.to_string(),
                action: action.to_string(),
                reason: format!("Persona {} meminta izin", job.persona_name),
            },
        )
        .await
        .map_err(|e| crate::error::Error::from(e).user_message())?;
    let timeout = std::env::var("AGENTGATE_PERMISSION_TIMEOUT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(PERMISSION_TIMEOUT);
    let deadline = Instant::now() + config::seconds(timeout);
    let mut status = info.status;
    loop {
        match status {
            PermissionStatus::Allowed | PermissionStatus::AllowedTicket => return Ok(true),
            PermissionStatus::Denied => return Ok(false),
            PermissionStatus::Pending => {}
        }
        if Instant::now() >= deadline {
            return Err(format!("permintaan izin `{action}` belum dijawab"));
        }
        tokio::time::sleep(config::seconds(PERMISSION_POLL)).await;
        // Gagal jaringan sesaat: coba lagi sampai batas waktu.
        if let Ok(p) = api
            .get::<PermissionInfo>(&format!("{base}/{}", info.id))
            .await
        {
            status = p.status;
        }
    }
}

/// `agentgate hook pre-tool --client <c>`. Tidak pernah gagal: di dalam job, error = tolak
/// (fail closed).
pub async fn pre_tool(paths: &Paths, client: ClientKind) {
    if foreign(client) {
        return;
    }
    let input = read_input(client);
    let job = job_from_env(paths);
    let output = |input: &HookInput, decision: &str, reason: &str, rewritten: Option<&str>| {
        output(client, input, decision, reason, rewritten)
    };
    match plan(&input, job.as_ref(), rtk_rewrite) {
        Plan::Pass => {}
        Plan::Allow { rewritten } => emit(&output(
            &input,
            "allow",
            "Agent Gate: diizinkan",
            rewritten.as_deref(),
        )),
        Plan::Deny { reason } => emit(&output(&input, "deny", &reason, None)),
        Plan::Ask { action, command } => {
            let Some(job) = job else { return };
            let decision = ask(paths, &job, &action, &command).await;
            let value = match decision {
                Ok(true) => output(
                    &input,
                    "allow",
                    "Agent Gate: diizinkan executor",
                    rtk_rewrite(&command).as_deref(),
                ),
                Ok(false) => output(
                    &input,
                    "deny",
                    &format!(
                        "Agent Gate: executor menolak aksi `{action}`. Jangan ulangi perintah ini."
                    ),
                    None,
                ),
                Err(e) => output(&input, "deny", &format!("Agent Gate: {e}"), None),
            };
            emit(&value);
        }
    }
}

/// Teks konteks `session-start`.
#[must_use]
pub fn session_context(job: Option<&JobState>, project: Option<&ProjectFile>) -> Option<String> {
    if let Some(j) = job {
        return Some(format!(
            "Agent Gate · job tiket {} ({}) · stage {} · persona {}\n{}\nPanggil ag_context \
             dengan ticket {} sebelum mulai bekerja.",
            j.ticket, j.title, j.stage_name, j.persona_name, j.url, j.ticket
        ));
    }
    project.map(|p| {
        format!(
            "Project ini memakai Agent Gate (project {}). Panggil ag_context sebelum mulai bekerja.",
            p.project.key
        )
    })
}

/// `agentgate hook session-start --client <c>`.
pub fn session_start(paths: &Paths, client: ClientKind) {
    if foreign(client) {
        return;
    }
    let input = read_input(client);
    let job = job_from_env(paths);
    let dir = if input.cwd.is_empty() {
        std::env::current_dir().unwrap_or_default()
    } else {
        PathBuf::from(&input.cwd)
    };
    let root = crate::git::toplevel(&dir).unwrap_or(dir);
    let project = ProjectFile::load(Path::new(&root)).ok().flatten();
    if let Some(text) = session_context(job.as_ref(), project.as_ref()) {
        let mut stdout = std::io::stdout();
        let _ = writeln!(stdout, "{}", session_output(client, &text));
    }
}

#[cfg(test)]
mod tests {
    use agentgate_core::runner::protocol::DangerRule;

    use super::*;

    fn input(cmd: &str) -> HookInput {
        serde_json::from_value(json!({
            "session_id": "s",
            "hook_event_name": "PreToolUse",
            "cwd": "/wt",
            "tool_name": "Bash",
            "tool_input": { "command": cmd, "description": "x" }
        }))
        .unwrap()
    }

    fn job() -> JobState {
        JobState {
            job_id: Uuid::nil(),
            runner_id: Uuid::nil(),
            ticket: "KSR-1".into(),
            title: "T".into(),
            url: "http://x/w/a/t/KSR-1".into(),
            stage_name: "Development".into(),
            persona_name: "Dev".into(),
            danger: vec![
                DangerRule {
                    action: "network".into(),
                    level: Level::Ask,
                },
                DangerRule {
                    action: "deploy".into(),
                    level: Level::Block,
                },
                DangerRule {
                    action: "install_package".into(),
                    level: Level::Ask,
                },
            ],
            allowed_actions: vec!["install_package".into()],
        }
    }

    fn no_rtk(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn job_commands_are_allowed_blocked_or_asked() {
        let j = job();
        assert_eq!(
            plan(&input("cargo test"), Some(&j), no_rtk),
            Plan::Allow { rewritten: None }
        );
        assert!(matches!(
            plan(&input("vercel --prod"), Some(&j), no_rtk),
            Plan::Deny { reason } if reason.contains("`deploy`") && reason.contains("Dev")
        ));
        // Selalu diblokir walaupun persona tidak menyebutnya.
        assert!(matches!(
            plan(&input("git push --force origin main"), Some(&j), no_rtk),
            Plan::Deny { .. }
        ));
        assert_eq!(
            plan(&input("curl https://x"), Some(&j), no_rtk),
            Plan::Ask {
                action: "network".into(),
                command: "curl https://x".into()
            }
        );
        // "Selalu untuk tiket ini" sudah diberikan.
        assert_eq!(
            plan(&input("npm install left-pad"), Some(&j), no_rtk),
            Plan::Allow { rewritten: None }
        );
    }

    #[test]
    fn outside_a_job_the_hook_only_rewrites_safe_commands() {
        let rtk = |c: &str| Some(format!("rtk {c}"));
        assert_eq!(plan(&input("curl x"), None, no_rtk), Plan::Pass);
        assert_eq!(plan(&input("curl x"), None, rtk), Plan::Pass);
        assert_eq!(
            plan(&input("git status"), None, rtk),
            Plan::Allow {
                rewritten: Some("rtk git status".into())
            }
        );
        assert_eq!(plan(&input("git status"), None, no_rtk), Plan::Pass);
        let edit: HookInput =
            serde_json::from_value(json!({"tool_name": "Edit", "tool_input": {}})).unwrap();
        assert_eq!(plan(&edit, Some(&job()), rtk), Plan::Pass);
    }

    #[test]
    fn output_matches_claude_code_hook_schema() {
        let i = input("git status");
        let v = output(
            ClientKind::Claude,
            &i,
            "allow",
            "ok",
            Some("rtk git status"),
        );
        let s = &v["hookSpecificOutput"];
        assert_eq!(s["hookEventName"], "PreToolUse");
        assert_eq!(s["permissionDecision"], "allow");
        assert_eq!(s["permissionDecisionReason"], "ok");
        assert_eq!(s["updatedInput"]["command"], "rtk git status");
        assert_eq!(s["updatedInput"]["description"], "x");
        let v = output(ClientKind::Claude, &i, "deny", "tidak", None);
        assert_eq!(v["hookSpecificOutput"]["permissionDecision"], "deny");
        assert!(v["hookSpecificOutput"].get("updatedInput").is_none());
        // Codex memakai bentuk yang sama.
        assert_eq!(
            output(ClientKind::Codex, &i, "allow", "ok", Some("rtk git status")),
            output(
                ClientKind::Claude,
                &i,
                "allow",
                "ok",
                Some("rtk git status")
            )
        );
    }

    #[test]
    fn payloads_are_normalised_per_client() {
        let parse = |c, v: Value| HookInput::parse(c, &v.to_string());
        let cases = [
            (
                ClientKind::Codex,
                json!({"hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":"curl x"},"cwd":"/wt"}),
            ),
            (
                ClientKind::Gemini,
                json!({"hook_event_name":"BeforeTool","tool_name":"run_shell_command","tool_input":{"command":"curl x","description":"d"},"cwd":"/wt"}),
            ),
            (
                ClientKind::Cursor,
                json!({"hook_event_name":"preToolUse","tool_name":"Shell","tool_input":{"command":"curl x"},"workspace_roots":["/wt"]}),
            ),
            (
                ClientKind::Copilot,
                json!({"timestamp":1,"cwd":"/wt","toolName":"bash","toolArgs":"{\"command\":\"curl x\"}"}),
            ),
            (
                ClientKind::Opencode,
                json!({"tool_name":"bash","tool_input":{"command":"curl x"},"cwd":"/wt"}),
            ),
        ];
        for (client, payload) in cases {
            let i = parse(client, payload);
            assert_eq!(i.command(), Some("curl x"), "{client:?}");
            assert_eq!(i.cwd, "/wt", "{client:?}");
            assert_eq!(
                plan(&i, Some(&job()), no_rtk),
                Plan::Ask {
                    action: "network".into(),
                    command: "curl x".into()
                },
                "{client:?}"
            );
        }
        let edit = parse(
            ClientKind::Copilot,
            json!({"toolName":"edit","toolArgs":"{}"}),
        );
        assert_eq!(edit.command(), None);
        assert_eq!(
            HookInput::parse(ClientKind::Gemini, "rusak").command(),
            None
        );
    }

    #[test]
    fn decisions_use_each_client_dialect() {
        let i = HookInput::parse(
            ClientKind::Gemini,
            &json!({"tool_name":"run_shell_command","tool_input":{"command":"git status","description":"d"}}).to_string(),
        );
        let rtk = Some("rtk git status");
        let g = output(ClientKind::Gemini, &i, "allow", "ok", rtk);
        assert_eq!(g["decision"], "allow");
        assert_eq!(
            g["hookSpecificOutput"]["tool_input"]["command"],
            "rtk git status"
        );
        assert_eq!(g["hookSpecificOutput"]["tool_input"]["description"], "d");
        let g = output(ClientKind::Gemini, &i, "deny", "tidak", None);
        assert_eq!(
            (g["decision"].as_str(), g["reason"].as_str()),
            (Some("deny"), Some("tidak"))
        );
        assert!(g.get("hookSpecificOutput").is_none());

        let c = output(ClientKind::Cursor, &i, "deny", "tidak", None);
        assert_eq!(c["permission"], "deny");
        assert_eq!(c["agent_message"], "tidak");
        let c = output(ClientKind::Cursor, &i, "allow", "ok", rtk);
        assert_eq!(c["updated_input"]["command"], "rtk git status");

        let p = output(ClientKind::Copilot, &i, "deny", "tidak", None);
        assert_eq!(p["permissionDecision"], "deny");
        assert_eq!(p["permissionDecisionReason"], "tidak");
        let p = output(ClientKind::Copilot, &i, "allow", "ok", rtk);
        assert_eq!(p["modifiedArgs"]["command"], "rtk git status");

        let o = output(ClientKind::Opencode, &i, "allow", "ok", rtk);
        assert_eq!(
            (o["decision"].as_str(), o["command"].as_str()),
            (Some("allow"), rtk)
        );
        assert!(
            output(ClientKind::Opencode, &i, "deny", "x", None)
                .get("command")
                .is_none()
        );
    }

    #[test]
    fn session_context_uses_each_client_dialect() {
        assert_eq!(session_output(ClientKind::Claude, "halo"), "halo");
        assert_eq!(session_output(ClientKind::Opencode, "halo"), "halo");
        for c in [ClientKind::Codex, ClientKind::Gemini] {
            let v: Value = serde_json::from_str(&session_output(c, "halo")).unwrap();
            assert_eq!(v["hookSpecificOutput"]["hookEventName"], "SessionStart");
            assert_eq!(v["hookSpecificOutput"]["additionalContext"], "halo");
        }
        let v: Value = serde_json::from_str(&session_output(ClientKind::Cursor, "halo")).unwrap();
        assert_eq!(v["additional_context"], "halo");
        let v: Value = serde_json::from_str(&session_output(ClientKind::Copilot, "halo")).unwrap();
        assert_eq!(v["additionalContext"], "halo");
    }

    #[test]
    fn session_context_prefers_job_then_project() {
        let j = job();
        let text = session_context(Some(&j), None).unwrap();
        assert!(text.contains("KSR-1") && text.contains("Development"));
        let p = ProjectFile {
            project: config::ProjectSection {
                key: "KSR".into(),
                server: String::new(),
            },
            test: config::TestSection::default(),
        };
        assert!(
            session_context(None, Some(&p))
                .unwrap()
                .contains("project KSR")
        );
        assert_eq!(session_context(None, None), None);
    }
}
