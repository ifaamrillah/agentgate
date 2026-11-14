//! Adapter Codex CLI.
//!
//! Job otomatis: `codex exec --json --ignore-user-config -s workspace-write -a never
//! --dangerously-bypass-hook-trust -C <wt> -c mcp_servers.agentgate.url="<mcp>" [-m M]
//! [-c model_reasoning_effort="E"] -` dengan prompt di stdin.
//!
//! - `--ignore-user-config` melewati `~/.codex/config.toml`; login tetap dibaca dari
//!   `CODEX_HOME`.
//! - `.codex/` project hanya dimuat Codex bila folder trusted, dan trust disimpan di config user.
//!   Belum terverifikasi apakah layer project tetap dimuat bersama `--ignore-user-config`, jadi
//!   server MCP juga dioper lewat `-c` (berlaku tanpa trust). Hook `.codex/hooks.json` butuh
//!   `--dangerously-bypass-hook-trust` di headless; bila layer project tidak dimuat, hook tidak
//!   jalan dan pembatas yang tersisa adalah sandbox `workspace-write` (tanpa jaringan).
//! - Output `--json`: JSONL `thread.started`, `turn.started`, `item.started|updated|completed`
//!   (`agent_message`, `command_execution`, `file_change`, `mcp_tool_call`, `todo_list`, …),
//!   `turn.completed` (`usage`), `turn.failed`, `error`. Codex tidak melaporkan biaya.

use std::path::Path;

use agentgate_core::runner::protocol::{JobSpec, LogStream};
use serde_json::Value;

use super::{
    Finished, Invocation, Parsed, StreamParser, add_usage, effort, error_line, error_text, line,
    model, positional, raw_line, text_line, todo_steps, tool_line, usage_from,
};
use agentgate_core::runner::protocol::TokenUsage;

/// Perintah headless (prompt lewat stdin `-`).
#[must_use]
pub fn auto(spec: &JobSpec, wt: &Path) -> Invocation {
    let mut args: Vec<String> = [
        "exec",
        "--json",
        "--ignore-user-config",
        "-s",
        "workspace-write",
        "-a",
        "never",
        "--dangerously-bypass-hook-trust",
        "-C",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    args.push(wt.display().to_string());
    if !spec.mcp_url.trim().is_empty() {
        args.push("-c".into());
        args.push(format!(
            "mcp_servers.agentgate.url={}",
            toml_string(spec.mcp_url.trim())
        ));
    }
    push_model(&mut args, spec);
    args.push("-".into());
    Invocation {
        args,
        env: Vec::new(),
        stdin: Some(spec.prompt.clone()),
    }
}

/// Nilai string TOML untuk `-c key=value` (escape JSON sah sebagai basic string TOML).
fn toml_string(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

fn push_model(args: &mut Vec<String>, spec: &JobSpec) {
    if let Some(m) = model(spec) {
        args.extend(["-m".into(), m.to_string()]);
    }
    if let Some(e) = effort(spec) {
        args.extend([
            "-c".into(),
            format!("model_reasoning_effort={}", toml_string(e)),
        ]);
    }
}

/// Sesi interaktif: `codex [-m M] <prompt>` di worktree.
#[must_use]
pub fn interactive_args(spec: &JobSpec) -> Vec<String> {
    let mut args = Vec::new();
    push_model(&mut args, spec);
    args.push(positional(&spec.prompt));
    args
}

/// Parser JSONL `codex exec --json`.
#[derive(Debug, Default)]
pub struct Parser {
    last_text: String,
    usage: Option<TokenUsage>,
    turns: u64,
}

impl Parser {
    fn finished(&self, is_error: bool, text: String) -> Finished {
        Finished {
            is_error,
            text,
            tokens: self.usage.map(|u| u.total()),
            usage: self.usage,
            cost_usd: None,
            turns: (self.turns > 0).then_some(self.turns),
        }
    }

    fn item(&mut self, event: &str, item: &Value, out: &mut Parsed) {
        let kind = item.get("type").and_then(Value::as_str).unwrap_or_default();
        let done = event == "item.completed";
        match kind {
            "agent_message" if done => {
                let text = item.get("text").and_then(Value::as_str).unwrap_or_default();
                if let Some(l) = text_line(text) {
                    self.last_text = text.trim().to_string();
                    out.logs.push(l);
                }
            }
            "command_execution" if event == "item.started" => {
                let cmd = item.get("command").and_then(Value::as_str).unwrap_or("");
                out.logs.push(tool_line("shell", cmd));
            }
            "command_execution" if done => {
                let failed = item
                    .get("exit_code")
                    .and_then(Value::as_i64)
                    .is_some_and(|c| c != 0)
                    || item.get("status").and_then(Value::as_str) == Some("failed");
                if failed {
                    let output = item
                        .get("aggregated_output")
                        .and_then(Value::as_str)
                        .unwrap_or("perintah gagal");
                    out.logs.push(error_line(output));
                }
            }
            "file_change" if done => {
                for change in item
                    .get("changes")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    let path = change.get("path").and_then(Value::as_str).unwrap_or("");
                    let verb = change.get("kind").and_then(Value::as_str).unwrap_or("edit");
                    out.logs.push(tool_line(verb, path));
                }
            }
            "mcp_tool_call" if event == "item.started" => {
                let server = item.get("server").and_then(Value::as_str).unwrap_or("mcp");
                let tool = item.get("tool").and_then(Value::as_str).unwrap_or("tool");
                out.logs.push(tool_line(&format!("{server} · {tool}"), ""));
            }
            "web_search" if event == "item.started" => {
                let q = item.get("query").and_then(Value::as_str).unwrap_or("");
                out.logs.push(tool_line("web_search", q));
            }
            "todo_list" => {
                if let Some(items) = item.get("items").and_then(Value::as_array) {
                    out.steps = Some(todo_steps(items));
                }
            }
            "error" if done => {
                let msg = item.get("message").and_then(Value::as_str).unwrap_or("");
                out.logs.push(line(LogStream::Err, msg.to_string()));
            }
            _ => {}
        }
    }
}

impl StreamParser for Parser {
    fn line(&mut self, raw: &str) -> Parsed {
        let mut out = Parsed::default();
        let Ok(v) = serde_json::from_str::<Value>(raw.trim()) else {
            out.logs.extend(raw_line(raw));
            return out;
        };
        let event = v.get("type").and_then(Value::as_str).unwrap_or_default();
        match event {
            "thread.started" => {
                out.logs
                    .push(line(LogStream::Info, "Codex CLI · sesi dimulai"));
            }
            "item.started" | "item.updated" | "item.completed" => {
                if let Some(item) = v.get("item") {
                    self.item(event, item, &mut out);
                }
            }
            "turn.completed" => {
                self.turns += 1;
                self.usage = add_usage(self.usage, v.get("usage").and_then(usage_from));
                let finished = self.finished(false, self.last_text.clone());
                out.logs.push(finished.log());
                out.finished = Some(finished);
            }
            "turn.failed" => {
                self.turns += 1;
                let msg = error_text(v.get("error").unwrap_or(&Value::Null));
                let finished = self.finished(true, msg);
                out.logs.push(line(LogStream::Err, finished.text.clone()));
                out.logs.push(finished.log());
                out.finished = Some(finished);
            }
            "error" => {
                let msg = error_text(v.get("message").unwrap_or(&v));
                out.logs.push(line(LogStream::Err, msg));
            }
            _ => {}
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use agentgate_core::runner::protocol::StepState;

    use super::{super::tests, *};

    /// Fixture JSONL `codex exec --json` (0.16x).
    pub const SAMPLE: &[&str] = &[
        r#"{"type":"thread.started","thread_id":"0199a213-81c0-7800-8aa1-bbab2a035a53"}"#,
        r#"{"type":"turn.started"}"#,
        r#"{"type":"item.completed","item":{"id":"item_0","type":"reasoning","text":"**Membaca konteks**"}}"#,
        r#"{"type":"item.started","item":{"id":"item_1","type":"mcp_tool_call","server":"agentgate","tool":"ag_context","status":"in_progress"}}"#,
        r#"{"type":"item.started","item":{"id":"item_2","type":"todo_list","items":[{"text":"Baca kode stok","completed":true},{"text":"Tambah validasi","completed":false}]}}"#,
        r#"{"type":"item.started","item":{"id":"item_3","type":"command_execution","command":"bash -lc 'cargo test -p kasir'","aggregated_output":"","exit_code":null,"status":"in_progress"}}"#,
        r#"{"type":"item.completed","item":{"id":"item_3","type":"command_execution","command":"bash -lc 'cargo test -p kasir'","aggregated_output":"error[E0425]: cannot find value `x`\n","exit_code":101,"status":"failed"}}"#,
        r#"{"type":"item.completed","item":{"id":"item_4","type":"file_change","changes":[{"path":"src/stok.rs","kind":"update"}],"status":"completed"}}"#,
        r#"{"type":"item.completed","item":{"id":"item_5","type":"agent_message","text":"Validasi stok ditambahkan."}}"#,
        r#"{"type":"turn.completed","usage":{"input_tokens":24763,"cached_input_tokens":24448,"output_tokens":122,"reasoning_output_tokens":64}}"#,
    ];

    #[test]
    fn jsonl_becomes_logs_steps_and_result() {
        let parsed = tests::run(&mut Parser::default(), SAMPLE);
        assert_eq!(
            tests::logs(&parsed),
            [
                (LogStream::Info, "Codex CLI · sesi dimulai".into()),
                (LogStream::Tool, "agentgate · ag_context".into()),
                (
                    LogStream::Tool,
                    "shell: bash -lc 'cargo test -p kasir'".into()
                ),
                (LogStream::Err, "error[E0425]: cannot find value `x`".into()),
                (LogStream::Tool, "update: src/stok.rs".into()),
                (LogStream::Out, "Validasi stok ditambahkan.".into()),
                (LogStream::Ok, "Agent selesai · 1 giliran".into()),
            ]
        );
        let steps = parsed[4].steps.clone().unwrap();
        assert_eq!(
            steps.iter().map(|s| s.state).collect::<Vec<_>>(),
            [StepState::Ok, StepState::Wait]
        );
        let done = tests::finished(&parsed);
        assert!(!done.is_error);
        assert_eq!(done.text, "Validasi stok ditambahkan.");
        assert_eq!(
            done.usage,
            Some(TokenUsage {
                input: 315,
                output: 122,
                cache_read: 24448,
                cache_write: 0
            })
        );
        assert_eq!(done.tokens, Some(24885));
        assert_eq!(done.cost_usd, None);
    }

    #[test]
    fn failed_turn_is_an_error_with_the_message() {
        let parsed = tests::run(
            &mut Parser::default(),
            &[
                r#"{"type":"error","message":"stream disconnected"}"#,
                r#"{"type":"turn.failed","error":{"message":"You've hit your usage limit. Try again at 3:15 PM."}}"#,
                "bukan json",
            ],
        );
        let done = tests::finished(&parsed);
        assert!(done.is_error);
        assert!(super::super::is_quota(&done.text));
        let logs = tests::logs(&parsed);
        assert_eq!(logs[0], (LogStream::Err, "stream disconnected".into()));
        assert_eq!(logs.last().unwrap().0, LogStream::Out);
    }

    #[test]
    fn headless_args_trust_isolate_and_read_prompt_from_stdin() {
        let mut spec = tests::spec();
        spec.model = "gpt-5-codex".into();
        spec.effort = "high".into();
        let inv = auto(&spec, Path::new("/s/wt/kasir"));
        let args = inv.args.join(" ");
        assert!(
            args.starts_with(
                "exec --json --ignore-user-config -s workspace-write -a never \
                 --dangerously-bypass-hook-trust -C /s/wt/kasir"
            ),
            "{args}"
        );
        assert!(
            inv.args
                .windows(2)
                .any(|w| w == ["-c", "mcp_servers.agentgate.url=\"http://x/mcp\""])
        );
        assert!(inv.args.windows(2).any(|w| w == ["-m", "gpt-5-codex"]));
        assert!(
            inv.args
                .windows(2)
                .any(|w| w == ["-c", "model_reasoning_effort=\"high\""])
        );
        assert_eq!(inv.args.last().map(String::as_str), Some("-"));
        assert_eq!(inv.stdin.as_deref(), Some(spec.prompt.as_str()));
        assert!(!inv.args.contains(&spec.prompt));
        assert_eq!(interactive_args(&spec).last(), Some(&spec.prompt));
    }
}
