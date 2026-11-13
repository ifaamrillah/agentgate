//! Adapter Gemini CLI.
//!
//! Job otomatis: `gemini -p <prompt> --approval-mode yolo -o stream-json --skip-trust [-m M]`
//! dengan env `GEMINI_CLI_TRUST_WORKSPACE=true`. Tanpa trust, Gemini mengabaikan
//! `.gemini/settings.json` (MCP, hook, `context.fileName`) dan headless gagal
//! (`FatalUntrustedWorkspaceError`); flag dan env dipasang keduanya karena versi Gemini berbeda
//! membaca salah satunya.
//!
//! - Isolasi: tidak ada cara melewati `~/.gemini/settings.json` tanpa mengganti home (yang
//!   memutus login Google), jadi config user tetap berlaku.
//! - Effort: Gemini tidak punya flag effort (thinking diatur lewat settings); effort persona
//!   diabaikan.
//! - Output `stream-json`: `init`, `message` (assistant bisa dipecah `delta: true`), `tool_use`,
//!   `tool_result`, `error`, `result` (`stats`). Sub-field `stats` belum terverifikasi; dibaca
//!   toleran (`input_tokens`/`output_tokens`/`cached` atau `models.<m>.tokens`).

use agentgate_core::runner::protocol::{JobSpec, LogStream, TokenUsage};
use serde_json::Value;

use super::{
    Finished, Invocation, Parsed, StreamParser, add_usage, error_line, error_text, line, model,
    raw_line, text_line, todo_steps, tool_detail, tool_line, usage_from,
};

/// Perintah headless.
#[must_use]
pub fn auto(spec: &JobSpec) -> Invocation {
    let mut args = vec![
        "-p".to_string(),
        spec.prompt.clone(),
        "--approval-mode".into(),
        "yolo".into(),
        "-o".into(),
        "stream-json".into(),
        "--skip-trust".into(),
    ];
    if let Some(m) = model(spec) {
        args.extend(["-m".into(), m.to_string()]);
    }
    Invocation {
        args,
        env: vec![("GEMINI_CLI_TRUST_WORKSPACE", "true".into())],
        stdin: None,
    }
}

/// Sesi interaktif: `gemini [-m M] -i <prompt>`.
#[must_use]
pub fn interactive_args(spec: &JobSpec) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(m) = model(spec) {
        args.extend(["-m".into(), m.to_string()]);
    }
    args.extend(["-i".into(), spec.prompt.clone()]);
    args
}

/// Parser `-o stream-json`.
#[derive(Debug, Default)]
pub struct Parser {
    /// Potongan pesan assistant (`delta: true`) yang belum dicatat.
    pending: String,
    last_text: String,
}

impl Parser {
    /// Catat pesan assistant yang terkumpul.
    fn flush(&mut self, out: &mut Parsed) {
        let text = std::mem::take(&mut self.pending);
        if let Some(l) = text_line(&text) {
            self.last_text = text.trim().to_string();
            out.logs.push(l);
        }
    }
}

/// Usage dari `stats`: field datar atau jumlah per model (`models.<m>.tokens`).
fn stats_usage(stats: &Value) -> Option<TokenUsage> {
    if let Some(u) = usage_from(stats) {
        return Some(u);
    }
    stats
        .get("models")
        .and_then(Value::as_object)?
        .values()
        .map(|m| m.get("tokens").and_then(usage_from))
        .fold(None, add_usage)
}

impl StreamParser for Parser {
    fn line(&mut self, raw: &str) -> Parsed {
        let mut out = Parsed::default();
        let Ok(v) = serde_json::from_str::<Value>(raw.trim()) else {
            self.flush(&mut out);
            out.logs.extend(raw_line(raw));
            return out;
        };
        let kind = v.get("type").and_then(Value::as_str).unwrap_or_default();
        let role = v.get("role").and_then(Value::as_str).unwrap_or_default();
        if kind == "message" && role == "assistant" {
            let text = v.get("content").and_then(Value::as_str).unwrap_or_default();
            self.pending.push_str(text);
            if v.get("delta").and_then(Value::as_bool) != Some(true) {
                self.flush(&mut out);
            }
            return out;
        }
        self.flush(&mut out);
        match kind {
            "init" => {
                let model = v.get("model").and_then(Value::as_str).unwrap_or("?");
                out.logs
                    .push(line(LogStream::Info, format!("Gemini CLI · model {model}")));
            }
            "tool_use" => {
                let name = v.get("tool_name").and_then(Value::as_str).unwrap_or("tool");
                let params = v.get("parameters").cloned().unwrap_or(Value::Null);
                if name == "write_todos" {
                    out.steps = params
                        .get("todos")
                        .and_then(Value::as_array)
                        .map(|t| todo_steps(t));
                } else {
                    out.logs.push(tool_line(name, &tool_detail(&params)));
                }
            }
            "tool_result" if v.get("status").and_then(Value::as_str) == Some("error") => {
                let msg = v
                    .get("error")
                    .map(error_text)
                    .or_else(|| v.get("output").and_then(Value::as_str).map(str::to_string))
                    .unwrap_or_default();
                out.logs.push(error_line(&msg));
            }
            "error" => {
                let warn = v.get("severity").and_then(Value::as_str) == Some("warning");
                let msg = error_text(v.get("message").unwrap_or(&v));
                out.logs.push(line(
                    if warn {
                        LogStream::Warn
                    } else {
                        LogStream::Err
                    },
                    msg,
                ));
            }
            "result" => {
                let is_error = v.get("status").and_then(Value::as_str) != Some("success")
                    || v.get("error").is_some_and(|e| !e.is_null());
                let usage = v.get("stats").and_then(stats_usage);
                let text = if is_error {
                    v.get("error").map(error_text).unwrap_or_default()
                } else {
                    String::new()
                };
                let finished = Finished {
                    is_error,
                    text: if text.is_empty() {
                        self.last_text.clone()
                    } else {
                        text
                    },
                    tokens: usage.map(|u| u.total()),
                    usage,
                    cost_usd: None,
                    turns: None,
                };
                out.logs.push(finished.log());
                out.finished = Some(finished);
            }
            _ => {}
        }
        out
    }

    fn eof(&mut self) -> Parsed {
        let mut out = Parsed::default();
        self.flush(&mut out);
        out
    }
}

#[cfg(test)]
mod tests {
    use agentgate_core::runner::protocol::StepState;

    use super::{super::tests, *};

    /// Fixture `gemini -o stream-json` (0.6x).
    pub const SAMPLE: &[&str] = &[
        r#"{"type":"init","timestamp":"2026-11-13T01:00:00.000Z","session_id":"c1","model":"gemini-2.5-pro"}"#,
        r#"{"type":"message","timestamp":"2026-11-13T01:00:00.100Z","role":"user","content":"Kerjakan KSR-26"}"#,
        r#"{"type":"message","timestamp":"2026-11-13T01:00:01.000Z","role":"assistant","content":"Saya muat ","delta":true}"#,
        r#"{"type":"message","timestamp":"2026-11-13T01:00:01.100Z","role":"assistant","content":"konteks tiket.","delta":true}"#,
        r#"{"type":"tool_use","timestamp":"2026-11-13T01:00:02.000Z","tool_name":"write_todos","tool_id":"t0","parameters":{"todos":[{"description":"Tambah validasi","status":"in_progress"}]}}"#,
        r#"{"type":"tool_use","timestamp":"2026-11-13T01:00:02.000Z","tool_name":"run_shell_command","tool_id":"t1","parameters":{"command":"cargo test -p kasir","description":"tes"}}"#,
        r#"{"type":"tool_result","timestamp":"2026-11-13T01:00:09.000Z","tool_id":"t1","status":"error","output":"","error":{"type":"exit","message":"Command exited with code 101"}}"#,
        r#"{"type":"error","timestamp":"2026-11-13T01:00:09.500Z","severity":"warning","message":"Loop terdeteksi"}"#,
        r#"{"type":"message","timestamp":"2026-11-13T01:00:10.000Z","role":"assistant","content":"Validasi stok selesai.","delta":true}"#,
        r#"{"type":"result","timestamp":"2026-11-13T01:00:11.000Z","status":"success","stats":{"total_tokens":1500,"input_tokens":1200,"output_tokens":300,"cached":200,"duration_ms":11000,"tool_calls":2}}"#,
    ];

    #[test]
    fn stream_json_becomes_logs_steps_and_result() {
        let parsed = tests::run(&mut Parser::default(), SAMPLE);
        assert_eq!(
            tests::logs(&parsed),
            [
                (LogStream::Info, "Gemini CLI · model gemini-2.5-pro".into()),
                (LogStream::Out, "Saya muat konteks tiket.".into()),
                (
                    LogStream::Tool,
                    "run_shell_command: cargo test -p kasir".into()
                ),
                (LogStream::Err, "Command exited with code 101".into()),
                (LogStream::Warn, "Loop terdeteksi".into()),
                (LogStream::Out, "Validasi stok selesai.".into()),
                (LogStream::Ok, "Agent selesai".into()),
            ]
        );
        assert_eq!(parsed[4].steps.clone().unwrap()[0].state, StepState::Now);
        let done = tests::finished(&parsed);
        assert!(!done.is_error);
        assert_eq!(done.text, "Validasi stok selesai.");
        assert_eq!(
            done.usage,
            Some(TokenUsage {
                input: 1000,
                output: 300,
                cache_read: 200,
                cache_write: 0
            })
        );
    }

    #[test]
    fn per_model_stats_and_error_result() {
        let parsed = tests::run(
            &mut Parser::default(),
            &[
                r#"{"type":"result","status":"error","error":{"type":"api","message":"[API Error: RESOURCE_EXHAUSTED]"},"stats":{"models":{"gemini-2.5-pro":{"tokens":{"prompt":50,"candidates":5,"cached":10,"total":55}},"gemini-2.5-flash":{"tokens":{"prompt":10,"candidates":1}}}}}"#,
            ],
        );
        let done = tests::finished(&parsed);
        assert!(done.is_error);
        assert!(super::super::is_quota(&done.text));
        assert_eq!(
            done.usage,
            Some(TokenUsage {
                input: 50,
                output: 6,
                cache_read: 10,
                cache_write: 0
            })
        );
    }

    #[test]
    fn headless_args_trust_the_worktree() {
        let mut spec = tests::spec();
        spec.effort = "high".into();
        let inv = auto(&spec);
        assert_eq!(inv.args[..2], ["-p".to_string(), spec.prompt.clone()]);
        for w in [
            ["--approval-mode", "yolo"],
            ["-o", "stream-json"],
            ["-m", "claude-sonnet-4-5"],
        ] {
            assert!(inv.args.windows(2).any(|x| x == w), "{w:?}");
        }
        assert!(inv.args.contains(&"--skip-trust".to_string()));
        assert!(!inv.args.iter().any(|a| a.contains("high")));
        assert_eq!(
            inv.env,
            [("GEMINI_CLI_TRUST_WORKSPACE", "true".to_string())]
        );
        assert_eq!(
            interactive_args(&spec)[2..],
            ["-i".to_string(), spec.prompt.clone()]
        );
    }
}
