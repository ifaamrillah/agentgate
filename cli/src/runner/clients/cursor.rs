//! Adapter Cursor CLI (binary `agent`, alias lama `cursor-agent`).
//!
//! Job otomatis: `agent -p --workspace <wt> --trust --force --approve-mcps --output-format
//! stream-json [--model M] <prompt>`. Tanpa `--force`, `-p` hanya mengusulkan perubahan; tanpa
//! `--trust`, config project (`.cursor/mcp.json`, `.cursor/hooks.json`) diabaikan.
//!
//! - Isolasi: tidak ada flag untuk mengabaikan `~/.cursor/` (MCP, hooks, rules user) maupun hook
//!   `~/.claude/settings.json` (Third-Party Imports); config user tetap berlaku.
//! - Effort: Cursor memilih effort lewat slug model (`…-thinking-max`); effort persona diabaikan.
//! - Output `stream-json`: `system`/`user`/`assistant`/`result` berbentuk sama dengan Claude Code
//!   (diparse [`super::claude::Parser`]), ditambah `tool_call` (`started`/`completed`,
//!   `tool_call.<jenis>ToolCall.args`). Nama field token di `result` belum terverifikasi; dibaca
//!   toleran lewat [`super::usage_from`]. Cursor tidak melaporkan biaya.

use std::path::Path;

use agentgate_core::runner::protocol::JobSpec;
use serde_json::Value;

use super::{
    Parsed, StreamParser, claude, error_line, error_text, model, positional, todo_steps,
    tool_detail, tool_line,
};

/// Argumen headless.
#[must_use]
pub fn auto_args(spec: &JobSpec, wt: &Path) -> Vec<String> {
    let mut args = vec![
        "-p".to_string(),
        "--workspace".into(),
        wt.display().to_string(),
        "--trust".into(),
        "--force".into(),
        "--approve-mcps".into(),
        "--output-format".into(),
        "stream-json".into(),
    ];
    if let Some(m) = model(spec) {
        args.extend(["--model".into(), m.to_string()]);
    }
    args.push(positional(&spec.prompt));
    args
}

/// Sesi interaktif: `agent [--model M] <prompt>`.
#[must_use]
pub fn interactive_args(spec: &JobSpec) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(m) = model(spec) {
        args.extend(["--model".into(), m.to_string()]);
    }
    args.push(positional(&spec.prompt));
    args
}

/// Parser stream-json Cursor.
#[derive(Debug)]
pub struct Parser {
    inner: claude::Parser,
}

impl Default for Parser {
    fn default() -> Self {
        Self {
            inner: claude::Parser::new("Cursor"),
        }
    }
}

/// `{"shellToolCall": {"args": {...}, "result": {...}}}` → (`shell`, isi).
fn tool_call(v: &Value) -> Option<(String, &Value)> {
    let (key, body) = v.get("tool_call")?.as_object()?.iter().next()?;
    let name = key.strip_suffix("ToolCall").unwrap_or(key);
    Some((name.to_string(), body))
}

impl StreamParser for Parser {
    fn line(&mut self, raw: &str) -> Parsed {
        let Ok(v) = serde_json::from_str::<Value>(raw.trim()) else {
            return self.inner.line(raw);
        };
        if v.get("type").and_then(Value::as_str) != Some("tool_call") {
            return self.inner.line(raw);
        }
        let mut out = Parsed::default();
        let Some((name, body)) = tool_call(&v) else {
            return out;
        };
        let args = body.get("args").cloned().unwrap_or(Value::Null);
        match v.get("subtype").and_then(Value::as_str) {
            Some("started") if name.to_ascii_lowercase().contains("todo") => {
                out.steps = args
                    .get("todos")
                    .and_then(Value::as_array)
                    .map(|t| todo_steps(t));
            }
            Some("started") => {
                let label = if name == "mcp" {
                    args.get("toolName")
                        .or_else(|| args.get("name"))
                        .and_then(Value::as_str)
                        .unwrap_or("mcp")
                        .to_string()
                } else {
                    name
                };
                out.logs.push(tool_line(&label, &tool_detail(&args)));
            }
            Some("completed") => {
                let result = body.get("result").cloned().unwrap_or(Value::Null);
                let failed = ["failure", "error", "rejected"]
                    .iter()
                    .find_map(|k| result.get(k));
                if let Some(f) = failed {
                    let text = f
                        .get("stderr")
                        .and_then(Value::as_str)
                        .filter(|s| !s.trim().is_empty())
                        .map_or_else(|| error_text(f), str::to_string);
                    out.logs.push(error_line(&text));
                }
            }
            _ => {}
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use agentgate_core::runner::protocol::{LogStream, StepState, TokenUsage};

    use super::{super::tests, *};

    /// Fixture `agent -p --output-format stream-json` (2026.10).
    pub const SAMPLE: &[&str] = &[
        r#"{"type":"system","subtype":"init","apiKeySource":"login","cwd":"/wt","session_id":"c-1","model":"Claude 4.5 Sonnet","permissionMode":"default"}"#,
        r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"Kerjakan KSR-26"}]},"session_id":"c-1"}"#,
        r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Saya muat konteks tiket."}]},"session_id":"c-1"}"#,
        r#"{"type":"tool_call","subtype":"started","call_id":"1","tool_call":{"updateTodosToolCall":{"args":{"todos":[{"content":"Tambah validasi","status":"TODO_STATUS_IN_PROGRESS"}]}}},"session_id":"c-1"}"#,
        r#"{"type":"tool_call","subtype":"started","call_id":"2","tool_call":{"mcpToolCall":{"args":{"name":"agentgate-ag_context","toolName":"ag_context","providerIdentifier":"agentgate","args":{"ticket":"KSR-26"}}}},"session_id":"c-1"}"#,
        r#"{"type":"tool_call","subtype":"started","call_id":"3","tool_call":{"shellToolCall":{"args":{"command":"cargo test -p kasir","workingDirectory":""}}},"session_id":"c-1"}"#,
        r#"{"type":"tool_call","subtype":"completed","call_id":"3","tool_call":{"shellToolCall":{"args":{"command":"cargo test -p kasir"},"result":{"failure":{"command":"cargo test -p kasir","exitCode":101,"stdout":"","stderr":"error[E0425]: cannot find value `x`"}}}},"session_id":"c-1"}"#,
        r#"{"type":"tool_call","subtype":"started","call_id":"4","tool_call":{"editToolCall":{"args":{"path":"src/stok.rs"}}},"session_id":"c-1"}"#,
        r#"{"type":"tool_call","subtype":"completed","call_id":"4","tool_call":{"editToolCall":{"args":{"path":"src/stok.rs"},"result":{"success":{"linesAdded":3}}}},"session_id":"c-1"}"#,
        r#"{"type":"result","subtype":"success","duration_ms":8123,"duration_api_ms":8123,"is_error":false,"result":"Validasi stok selesai.","session_id":"c-1","request_id":"r-1","usage":{"inputTokens":900,"outputTokens":120,"cacheReadTokens":400,"cacheWriteTokens":0}}"#,
    ];

    #[test]
    fn stream_json_becomes_logs_steps_and_result() {
        let parsed = tests::run(&mut Parser::default(), SAMPLE);
        assert_eq!(
            tests::logs(&parsed),
            [
                (LogStream::Info, "Cursor · model Claude 4.5 Sonnet".into()),
                (LogStream::Out, "Saya muat konteks tiket.".into()),
                (LogStream::Tool, "ag_context".into()),
                (LogStream::Tool, "shell: cargo test -p kasir".into()),
                (LogStream::Err, "error[E0425]: cannot find value `x`".into()),
                (LogStream::Tool, "edit: src/stok.rs".into()),
                (LogStream::Ok, "Agent selesai".into()),
            ]
        );
        assert_eq!(parsed[3].steps.clone().unwrap()[0].state, StepState::Now);
        let done = tests::finished(&parsed);
        assert_eq!(done.text, "Validasi stok selesai.");
        assert_eq!(
            done.usage,
            Some(TokenUsage {
                input: 900,
                output: 120,
                cache_read: 400,
                cache_write: 0
            })
        );
    }

    #[test]
    fn headless_args_trust_force_and_approve_mcp() {
        let spec = tests::spec();
        let args = auto_args(&spec, Path::new("/wt"));
        assert_eq!(
            args[..8],
            [
                "-p",
                "--workspace",
                "/wt",
                "--trust",
                "--force",
                "--approve-mcps",
                "--output-format",
                "stream-json"
            ]
        );
        assert!(
            args.windows(2)
                .any(|w| w == ["--model", "claude-sonnet-4-5"])
        );
        assert_eq!(args.last(), Some(&spec.prompt));
        assert_eq!(interactive_args(&spec).last(), Some(&spec.prompt));
    }
}
