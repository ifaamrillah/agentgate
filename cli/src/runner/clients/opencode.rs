//! Adapter OpenCode.
//!
//! Job otomatis: `opencode run --standalone --auto --format json [-m provider/model[#effort]]
//! <prompt>` di worktree. `--standalone` wajib: tanpa itu OpenCode V2 memakai server latar
//! bersama per user yang tidak membaca env/cwd job. `--auto` menyetujui semua izin yang tidak
//! ditolak eksplisit; aksi berbahaya dibatasi plugin `.opencode/plugins/agentgate.ts`.
//!
//! - Isolasi: config global (`~/.config/opencode`) tetap berlaku; mengganti
//!   `OPENCODE_CONFIG_DIR` tidak dilakukan supaya provider/model user tetap tersedia.
//! - Effort: varian model (`-m provider/model#high`), hanya bila model persona diisi.
//! - Output `--format json`: JSONL `{type, timestamp, sessionID, part}` dengan `type` `text`,
//!   `tool_use`, `step_start`, `step_finish` (`part.tokens`, `part.cost`), `error`. Tidak ada
//!   event hasil akhir; hasil disusun saat stdout ditutup ([`StreamParser::eof`]).

use agentgate_core::runner::protocol::{JobSpec, LogStream, TokenUsage};
use serde_json::Value;

use super::{
    Finished, Parsed, StreamParser, add_usage, effort, error_line, error_text, line, model,
    positional, raw_line, text_line, todo_steps, tool_detail, tool_line, usage_from,
};

/// Model + varian effort (`anthropic/claude-sonnet-4-5#high`).
fn model_arg(spec: &JobSpec) -> Option<String> {
    let m = model(spec)?;
    Some(match effort(spec) {
        Some(e) if !m.contains('#') => format!("{m}#{e}"),
        _ => m.to_string(),
    })
}

/// Argumen headless.
#[must_use]
pub fn auto_args(spec: &JobSpec) -> Vec<String> {
    let mut args = vec![
        "run".to_string(),
        "--standalone".into(),
        "--auto".into(),
        "--format".into(),
        "json".into(),
    ];
    if let Some(m) = model_arg(spec) {
        args.extend(["-m".into(), m]);
    }
    args.push(positional(&spec.prompt));
    args
}

/// Sesi interaktif: `opencode [-m M] --prompt <prompt>`.
#[must_use]
pub fn interactive_args(spec: &JobSpec) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(m) = model_arg(spec) {
        args.extend(["-m".into(), m]);
    }
    args.extend(["--prompt".into(), spec.prompt.clone()]);
    args
}

/// Parser `--format json`.
#[derive(Debug, Default)]
pub struct Parser {
    last_text: String,
    usage: Option<TokenUsage>,
    cost: Option<f64>,
    steps: u64,
    error: Option<String>,
}

impl StreamParser for Parser {
    fn line(&mut self, raw: &str) -> Parsed {
        let mut out = Parsed::default();
        let Ok(v) = serde_json::from_str::<Value>(raw.trim()) else {
            out.logs.extend(raw_line(raw));
            return out;
        };
        let part = v.get("part").cloned().unwrap_or(Value::Null);
        match v.get("type").and_then(Value::as_str) {
            Some("text") => {
                let text = part.get("text").and_then(Value::as_str).unwrap_or_default();
                if let Some(l) = text_line(text) {
                    self.last_text = text.trim().to_string();
                    out.logs.push(l);
                }
            }
            Some("tool_use") => {
                let tool = part.get("tool").and_then(Value::as_str).unwrap_or("tool");
                let state = part.get("state").cloned().unwrap_or(Value::Null);
                let input = state.get("input").cloned().unwrap_or(Value::Null);
                if tool == "todowrite" {
                    out.steps = input
                        .get("todos")
                        .and_then(Value::as_array)
                        .map(|t| todo_steps(t));
                } else {
                    out.logs.push(tool_line(tool, &tool_detail(&input)));
                }
                if state.get("status").and_then(Value::as_str) == Some("error") {
                    let msg = state.get("error").map(error_text).unwrap_or_default();
                    out.logs.push(error_line(&msg));
                }
            }
            Some("step_finish") => {
                self.steps += 1;
                self.usage = add_usage(self.usage, part.get("tokens").and_then(usage_from));
                if let Some(c) = part.get("cost").and_then(Value::as_f64) {
                    self.cost = Some(self.cost.unwrap_or(0.0) + c);
                }
            }
            Some("error") => {
                let msg = error_text(v.get("error").unwrap_or(&v));
                out.logs.push(line(LogStream::Err, msg.clone()));
                self.error = Some(msg);
            }
            _ => {}
        }
        out
    }

    fn eof(&mut self) -> Parsed {
        let mut out = Parsed::default();
        if self.steps == 0 && self.error.is_none() && self.last_text.is_empty() {
            return out;
        }
        let finished = Finished {
            is_error: self.error.is_some(),
            text: self.error.clone().unwrap_or_else(|| self.last_text.clone()),
            tokens: self.usage.map(|u| u.total()),
            usage: self.usage,
            cost_usd: self.cost,
            turns: (self.steps > 0).then_some(self.steps),
        };
        out.logs.push(finished.log());
        out.finished = Some(finished);
        out
    }
}

#[cfg(test)]
mod tests {
    use agentgate_core::runner::protocol::StepState;

    use super::{super::tests, *};

    /// Fixture `opencode run --format json` (V2).
    pub const SAMPLE: &[&str] = &[
        r#"{"type":"step_start","timestamp":1763000000000,"sessionID":"ses_1","part":{"id":"prt_1","type":"step-start"}}"#,
        r#"{"type":"text","timestamp":1763000000100,"sessionID":"ses_1","part":{"id":"prt_2","type":"text","text":"Saya muat konteks tiket."}}"#,
        r#"{"type":"tool_use","timestamp":1763000000200,"sessionID":"ses_1","part":{"id":"prt_3","type":"tool","tool":"todowrite","callID":"c0","state":{"status":"completed","input":{"todos":[{"content":"Tambah validasi","status":"completed","priority":"high","id":"1"}]}}}}"#,
        r#"{"type":"tool_use","timestamp":1763000000300,"sessionID":"ses_1","part":{"id":"prt_4","type":"tool","tool":"bash","callID":"c1","state":{"status":"error","input":{"command":"cargo test -p kasir","description":"tes"},"error":"error[E0425]: cannot find value `x`"}}}"#,
        r#"{"type":"step_finish","timestamp":1763000000400,"sessionID":"ses_1","part":{"id":"prt_5","type":"step-finish","reason":"tool-calls","cost":0.01,"tokens":{"input":100,"output":20,"reasoning":0,"cache":{"read":50,"write":10}}}}"#,
        r#"{"type":"tool_use","timestamp":1763000000500,"sessionID":"ses_1","part":{"id":"prt_6","type":"tool","tool":"edit","callID":"c2","state":{"status":"completed","input":{"filePath":"src/stok.rs"},"output":""}}}"#,
        r#"{"type":"text","timestamp":1763000000600,"sessionID":"ses_1","part":{"id":"prt_7","type":"text","text":"Validasi stok selesai."}}"#,
        r#"{"type":"step_finish","timestamp":1763000000700,"sessionID":"ses_1","part":{"id":"prt_8","type":"step-finish","reason":"stop","cost":0.02,"tokens":{"input":200,"output":30,"reasoning":5,"cache":{"read":0,"write":0}}}}"#,
    ];

    #[test]
    fn jsonl_becomes_logs_steps_and_result_at_eof() {
        let parsed = tests::run(&mut Parser::default(), SAMPLE);
        assert_eq!(
            tests::logs(&parsed),
            [
                (LogStream::Out, "Saya muat konteks tiket.".into()),
                (LogStream::Tool, "bash: cargo test -p kasir".into()),
                (LogStream::Err, "error[E0425]: cannot find value `x`".into()),
                (LogStream::Tool, "edit: src/stok.rs".into()),
                (LogStream::Out, "Validasi stok selesai.".into()),
                (LogStream::Ok, "Agent selesai · 2 giliran · $0.03".into()),
            ]
        );
        assert_eq!(parsed[2].steps.clone().unwrap()[0].state, StepState::Ok);
        assert!(parsed[..SAMPLE.len()].iter().all(|p| p.finished.is_none()));
        let done = tests::finished(&parsed);
        assert!(!done.is_error);
        assert_eq!(done.text, "Validasi stok selesai.");
        assert_eq!(
            done.usage,
            Some(TokenUsage {
                input: 300,
                output: 50,
                cache_read: 50,
                cache_write: 10
            })
        );
        assert!((done.cost_usd.unwrap() - 0.03).abs() < 1e-9);
    }

    #[test]
    fn error_event_fails_and_empty_stream_has_no_result() {
        let parsed = tests::run(
            &mut Parser::default(),
            &[
                r#"{"type":"error","timestamp":1,"sessionID":"s","error":{"name":"APIError","data":{"message":"Quota exceeded"}}}"#,
            ],
        );
        let done = tests::finished(&parsed);
        assert!(done.is_error);
        assert_eq!(done.text, "Quota exceeded");
        assert!(Parser::default().eof().finished.is_none());
    }

    #[test]
    fn headless_args_are_standalone_and_auto() {
        let mut spec = tests::spec();
        spec.model = "anthropic/claude-sonnet-4-5".into();
        spec.effort = "high".into();
        let args = auto_args(&spec);
        assert_eq!(
            args[..5],
            ["run", "--standalone", "--auto", "--format", "json"]
        );
        assert!(
            args.windows(2)
                .any(|w| w == ["-m", "anthropic/claude-sonnet-4-5#high"])
        );
        assert_eq!(args.last(), Some(&spec.prompt));
        spec.model.clear();
        assert!(!auto_args(&spec).contains(&"-m".to_string()));
        assert_eq!(
            interactive_args(&spec),
            ["--prompt".to_string(), spec.prompt.clone()]
        );
    }
}
