//! Adapter GitHub Copilot CLI.
//!
//! Job otomatis: `copilot -p <prompt> -C <wt> --trust-folder --allow-all-tools --no-ask-user
//! --output-format json [--model M] [--reasoning-effort E]` dengan env
//! `GITHUB_COPILOT_PROMPT_MODE_WORKSPACE_MCP=true` (tanpa itu MCP workspace `.mcp.json` tidak
//! dimuat di mode `-p`). Aksi berbahaya dibatasi hook `.github/hooks/agentgate.json`.
//!
//! - Isolasi: `~/.copilot` (MCP, hook, agent user) tetap berlaku; mengganti `COPILOT_HOME`
//!   memutus login. Copilot juga menjalankan hook `.claude/settings.json`; hook itu memanggil
//!   `agentgate hook … --client claude` yang diam saat job berjalan dengan client lain.
//! - Output `--output-format json` (JSONL) **belum terverifikasi**: dibaca sebagai event sesi
//!   Copilot (`{type, data}`): `assistant.message[_delta]`, `tool.execution_start|complete`,
//!   `assistant.usage`, `session.error`, dan record hasil akhir (`result`). Field yang tidak
//!   dikenal diabaikan; bila tidak ada record hasil, hasil disusun saat stdout ditutup.

use std::path::Path;

use agentgate_core::runner::protocol::{JobSpec, LogStream, TokenUsage};
use serde_json::Value;

use super::{
    Finished, Invocation, Parsed, StreamParser, add_usage, effort, error_line, error_text, line,
    model, raw_line, text_line, todo_steps, tool_detail, tool_line, usage_from,
};

/// Perintah headless.
#[must_use]
pub fn auto(spec: &JobSpec, wt: &Path) -> Invocation {
    let mut args = vec![
        "-p".to_string(),
        spec.prompt.clone(),
        "-C".into(),
        wt.display().to_string(),
        "--trust-folder".into(),
        "--allow-all-tools".into(),
        "--no-ask-user".into(),
        "--output-format".into(),
        "json".into(),
    ];
    push_model(&mut args, spec);
    Invocation {
        args,
        env: vec![("GITHUB_COPILOT_PROMPT_MODE_WORKSPACE_MCP", "true".into())],
        stdin: None,
    }
}

fn push_model(args: &mut Vec<String>, spec: &JobSpec) {
    if let Some(m) = model(spec) {
        args.extend(["--model".into(), m.to_string()]);
    }
    if let Some(e) = effort(spec) {
        args.extend(["--reasoning-effort".into(), e.to_string()]);
    }
}

/// Sesi interaktif: `copilot [--model M] -i <prompt>`.
#[must_use]
pub fn interactive_args(spec: &JobSpec) -> Vec<String> {
    let mut args = Vec::new();
    push_model(&mut args, spec);
    args.extend(["-i".into(), spec.prompt.clone()]);
    args
}

/// Parser JSONL Copilot.
#[derive(Debug, Default)]
pub struct Parser {
    pending: String,
    last_text: String,
    usage: Option<TokenUsage>,
    cost: Option<f64>,
    error: Option<String>,
    seen: bool,
    done: bool,
}

/// Data event: `data` bila ada, selain itu event itu sendiri.
fn data(v: &Value) -> &Value {
    v.get("data").filter(|d| d.is_object()).unwrap_or(v)
}

fn str_of<'a>(v: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|k| v.get(k).and_then(Value::as_str))
}

impl Parser {
    fn flush(&mut self, out: &mut Parsed) {
        let text = std::mem::take(&mut self.pending);
        if let Some(l) = text_line(&text) {
            self.last_text = text.trim().to_string();
            out.logs.push(l);
        }
    }

    fn finish(
        &mut self,
        is_error: bool,
        text: Option<String>,
        usage: Option<TokenUsage>,
    ) -> Finished {
        self.done = true;
        let usage = usage.or(self.usage);
        Finished {
            is_error: is_error || self.error.is_some(),
            text: text
                .filter(|t| !t.trim().is_empty())
                .or_else(|| self.error.clone())
                .unwrap_or_else(|| self.last_text.clone()),
            tokens: usage.map(|u| u.total()),
            usage,
            cost_usd: self.cost,
            turns: None,
        }
    }
}

impl StreamParser for Parser {
    fn line(&mut self, raw: &str) -> Parsed {
        let mut out = Parsed::default();
        let Ok(v) = serde_json::from_str::<Value>(raw.trim()) else {
            self.flush(&mut out);
            out.logs.extend(raw_line(raw));
            return out;
        };
        self.seen = true;
        let kind = v.get("type").and_then(Value::as_str).unwrap_or_default();
        let d = data(&v);
        if kind == "assistant.message_delta" {
            self.pending
                .push_str(str_of(d, &["deltaContent", "content"]).unwrap_or_default());
            return out;
        }
        self.flush(&mut out);
        match kind {
            "session.start" => {
                let model = str_of(d, &["selectedModel", "model"]).unwrap_or("?");
                out.logs.push(line(
                    LogStream::Info,
                    format!("Copilot CLI · model {model}"),
                ));
            }
            "assistant.message" => {
                // Pesan lengkap menggantikan delta yang sudah terkumpul.
                self.pending.clear();
                if let Some(l) = text_line(str_of(d, &["content", "text"]).unwrap_or_default()) {
                    self.last_text = l.text.clone();
                    out.logs.push(l);
                }
            }
            "tool.execution_start" => {
                let name = str_of(d, &["toolName", "name"]).unwrap_or("tool");
                let args = d
                    .get("arguments")
                    .or_else(|| d.get("toolArgs"))
                    .cloned()
                    .unwrap_or(Value::Null);
                // Argumen kadang berupa string JSON.
                let args = match args {
                    Value::String(s) => serde_json::from_str(&s).unwrap_or(Value::String(s)),
                    other => other,
                };
                if name.contains("todo") {
                    out.steps = args
                        .get("todos")
                        .and_then(Value::as_array)
                        .map(|t| todo_steps(t));
                } else {
                    out.logs.push(tool_line(name, &tool_detail(&args)));
                }
            }
            "tool.execution_complete"
                if d.get("success").and_then(Value::as_bool) == Some(false) =>
            {
                out.logs.push(error_line(&error_text(
                    d.get("error").unwrap_or(&Value::Null),
                )));
            }
            "assistant.usage" => {
                self.usage = add_usage(self.usage, usage_from(d));
                if let Some(c) = d.get("cost").and_then(Value::as_f64) {
                    self.cost = Some(self.cost.unwrap_or(0.0) + c);
                }
            }
            "session.error" | "error" => {
                let msg = error_text(d);
                out.logs.push(line(LogStream::Err, msg.clone()));
                self.error = Some(msg);
            }
            "result" => {
                let failed = d.get("is_error").and_then(Value::as_bool) == Some(true)
                    || d.get("success").and_then(Value::as_bool) == Some(false)
                    || d.get("exitCode")
                        .and_then(Value::as_i64)
                        .is_some_and(|c| c != 0)
                    || str_of(d, &["status"]).is_some_and(|s| s != "success" && s != "ok");
                let text = str_of(d, &["result", "content", "message"])
                    .map(str::to_string)
                    .or_else(|| d.get("error").map(error_text));
                let usage = d.get("usage").and_then(usage_from);
                let finished = self.finish(failed, text, usage);
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
        if self.seen && !self.done {
            let finished = self.finish(false, None, None);
            out.logs.push(finished.log());
            out.finished = Some(finished);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use agentgate_core::runner::protocol::StepState;

    use super::{super::tests, *};

    /// Fixture `copilot -p --output-format json` (1.0.10x, bentuk event sesi Copilot).
    pub const SAMPLE: &[&str] = &[
        r#"{"type":"session.start","data":{"sessionId":"s-1","selectedModel":"claude-sonnet-4.5"}}"#,
        r#"{"type":"assistant.message_delta","data":{"messageId":"m1","deltaContent":"Saya muat "}}"#,
        r#"{"type":"assistant.message_delta","data":{"messageId":"m1","deltaContent":"konteks tiket."}}"#,
        r#"{"type":"tool.execution_start","data":{"toolCallId":"t0","toolName":"update_todo","arguments":{"todos":[{"content":"Tambah validasi","status":"in_progress"}]}}}"#,
        r#"{"type":"tool.execution_start","data":{"toolCallId":"t1","toolName":"bash","arguments":"{\"command\":\"cargo test -p kasir\",\"description\":\"tes\"}"}}"#,
        r#"{"type":"tool.execution_complete","data":{"toolCallId":"t1","success":false,"error":{"message":"error[E0425]: cannot find value `x`"}}}"#,
        r#"{"type":"assistant.usage","data":{"model":"claude-sonnet-4.5","inputTokens":800,"outputTokens":90,"cacheReadTokens":300,"cacheWriteTokens":20,"cost":1}}"#,
        r#"{"type":"assistant.message","data":{"messageId":"m2","content":"Validasi stok selesai."}}"#,
        r#"{"type":"result","data":{"exitCode":0,"sessionId":"s-1"}}"#,
    ];

    #[test]
    fn jsonl_becomes_logs_steps_and_result() {
        let parsed = tests::run(&mut Parser::default(), SAMPLE);
        assert_eq!(
            tests::logs(&parsed),
            [
                (
                    LogStream::Info,
                    "Copilot CLI · model claude-sonnet-4.5".into()
                ),
                (LogStream::Out, "Saya muat konteks tiket.".into()),
                (LogStream::Tool, "bash: cargo test -p kasir".into()),
                (LogStream::Err, "error[E0425]: cannot find value `x`".into()),
                (LogStream::Out, "Validasi stok selesai.".into()),
                (LogStream::Ok, "Agent selesai · $1.00".into()),
            ]
        );
        assert_eq!(parsed[3].steps.clone().unwrap()[0].state, StepState::Now);
        let done = tests::finished(&parsed);
        assert!(!done.is_error);
        assert_eq!(done.text, "Validasi stok selesai.");
        assert_eq!(
            done.usage,
            Some(TokenUsage {
                input: 800,
                output: 90,
                cache_read: 300,
                cache_write: 20
            })
        );
        // Tanpa record hasil akhir: hasil disusun saat stdout ditutup.
        let parsed = tests::run(&mut Parser::default(), &SAMPLE[..8]);
        assert_eq!(tests::finished(&parsed).text, "Validasi stok selesai.");
    }

    #[test]
    fn session_error_fails_the_job() {
        let parsed = tests::run(
            &mut Parser::default(),
            &[
                r#"{"type":"session.error","data":{"errorType":"quota","message":"You have exceeded your premium request allowance."}}"#,
                r#"{"type":"result","data":{"exitCode":1}}"#,
            ],
        );
        let done = tests::finished(&parsed);
        assert!(done.is_error);
        assert!(super::super::is_quota(&done.text));
        assert!(Parser::default().eof().finished.is_none());
    }

    #[test]
    fn headless_args_trust_folder_and_enable_workspace_mcp() {
        let mut spec = tests::spec();
        spec.effort = "high".into();
        let inv = auto(&spec, Path::new("/wt"));
        assert_eq!(
            inv.args[..4],
            [
                "-p".to_string(),
                spec.prompt.clone(),
                "-C".into(),
                "/wt".into()
            ]
        );
        for flag in ["--trust-folder", "--allow-all-tools", "--no-ask-user"] {
            assert!(inv.args.contains(&flag.to_string()), "{flag}");
        }
        assert!(
            inv.args
                .windows(2)
                .any(|w| w == ["--output-format", "json"])
        );
        assert!(
            inv.args
                .windows(2)
                .any(|w| w == ["--reasoning-effort", "high"])
        );
        assert_eq!(
            inv.env,
            [(
                "GITHUB_COPILOT_PROMPT_MODE_WORKSPACE_MCP",
                "true".to_string()
            )]
        );
        assert_eq!(
            interactive_args(&spec)[4..],
            ["-i".to_string(), spec.prompt.clone()]
        );
    }
}
