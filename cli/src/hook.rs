//! Hook Claude Code (`.claude/settings.json` memanggil `agentgate hook <event>`).
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
};

/// Batas tunggu jawaban izin (detik); `AGENTGATE_PERMISSION_TIMEOUT` menggantinya.
const PERMISSION_TIMEOUT: u64 = 30 * 60;
/// Interval cek status izin (detik).
const PERMISSION_POLL: u64 = 3;

/// Input hook dari Claude Code (stdin).
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
    fn command(&self) -> Option<&str> {
        (self.tool_name == "Bash")
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

/// JSON output hook `PreToolUse` untuk keputusan `decision` (`allow`/`deny`/`ask`).
#[must_use]
pub fn output(input: &HookInput, decision: &str, reason: &str, rewritten: Option<&str>) -> Value {
    let mut specific = json!({
        "hookEventName": "PreToolUse",
        "permissionDecision": decision,
        "permissionDecisionReason": reason,
    });
    if let Some(cmd) = rewritten {
        let mut updated = input.tool_input.clone();
        if let Some(obj) = updated.as_object_mut() {
            obj.insert("command".into(), Value::String(cmd.to_string()));
            specific["updatedInput"] = updated;
        }
    }
    json!({ "hookSpecificOutput": specific })
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

fn read_input() -> HookInput {
    let mut raw = String::new();
    let _ = std::io::stdin().read_to_string(&mut raw);
    serde_json::from_str(&raw).unwrap_or_default()
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

/// `agentgate hook pre-tool`. Tidak pernah gagal: di dalam job, error = tolak (fail closed).
pub async fn pre_tool(paths: &Paths) {
    let input = read_input();
    let job = job_from_env(paths);
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

/// `agentgate hook session-start`.
pub fn session_start(paths: &Paths) {
    let input = read_input();
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
        let _ = writeln!(stdout, "{text}");
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
        let v = output(&i, "allow", "ok", Some("rtk git status"));
        let s = &v["hookSpecificOutput"];
        assert_eq!(s["hookEventName"], "PreToolUse");
        assert_eq!(s["permissionDecision"], "allow");
        assert_eq!(s["permissionDecisionReason"], "ok");
        assert_eq!(s["updatedInput"]["command"], "rtk git status");
        assert_eq!(s["updatedInput"]["description"], "x");
        let v = output(&i, "deny", "tidak", None);
        assert_eq!(v["hookSpecificOutput"]["permissionDecision"], "deny");
        assert!(v["hookSpecificOutput"].get("updatedInput").is_none());
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
