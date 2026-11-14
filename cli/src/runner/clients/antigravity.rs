//! Adapter Antigravity CLI (binary `agy`), pengganti Gemini CLI.
//!
//! Job otomatis: `agy -p <prompt> --output-format stream-json --dangerously-skip-permissions
//! [--model M] [--effort E]` dengan env `AGY_CLI_DISABLE_AUTO_UPDATE=true` (self-updater latar
//! memakai lock bersama; job paralel tidak boleh saling menunggu). Sumber:
//! <https://antigravity.google/docs/cli/headless/>.
//!
//! - Izin: headless tidak punya prompt; aksi yang butuh persetujuan di-*soft-deny*, jadi semua
//!   tool disetujui flag dan aksi berbahaya dibatasi hook `PreToolUse` `.agents/hooks.json`
//!   (`deny` = blok keras).
//! - Isolasi: tidak ada flag untuk melewati config global (`~/.gemini/config/`: MCP, hook, rules);
//!   config user tetap berlaku. Trust folder disimpan di `trustedWorkspaces` user dan tidak punya
//!   flag; perilaku headless di worktree yang belum trusted belum terverifikasi.
//! - Effort: `--effort low|medium|high|xhigh|max` (tergantung model; slug model juga bisa memuat
//!   effort, mis. `gemini-4-carbon-high`).
//! - Output `stream-json`: NDJSON `{event, <event>: {...}}` dengan `init`, `step_update`
//!   (`step_type` `agent_response`/`tool`/…, `text_delta`, `tool_info`, `usage` per langkah), dan
//!   `result` (`status`, `response`, `error`, `usage` kumulatif). Tidak ada tool todo, jadi tidak
//!   ada langkah rencana; tidak ada biaya.

use std::collections::BTreeSet;

use agentgate_core::runner::protocol::{JobSpec, LogStream, TokenUsage};
use serde_json::Value;

use super::{
    Finished, Invocation, Parsed, StreamParser, add_usage, effort, error_line, error_text, line,
    model, positional, raw_line, text_line, tool_line, usage_from,
};

/// Env yang mematikan self-updater latar Antigravity CLI.
pub const NO_UPDATE_ENV: &str = "AGY_CLI_DISABLE_AUTO_UPDATE";

fn push_model(args: &mut Vec<String>, spec: &JobSpec) {
    if let Some(m) = model(spec) {
        args.extend(["--model".into(), m.to_string()]);
    }
    if let Some(e) = effort(spec) {
        args.extend(["--effort".into(), e.to_string()]);
    }
}

/// Perintah headless.
#[must_use]
pub fn auto(spec: &JobSpec) -> Invocation {
    let mut args = vec![
        "-p".to_string(),
        positional(&spec.prompt),
        "--output-format".into(),
        "stream-json".into(),
        "--dangerously-skip-permissions".into(),
    ];
    push_model(&mut args, spec);
    Invocation {
        args,
        env: vec![(NO_UPDATE_ENV, "true".into())],
        stdin: None,
    }
}

/// Sesi interaktif: `agy [--model M] [--effort E] -i <prompt>`.
#[must_use]
pub fn interactive_args(spec: &JobSpec) -> Vec<String> {
    let mut args = Vec::new();
    push_model(&mut args, spec);
    args.extend(["-i".into(), positional(&spec.prompt)]);
    args
}

/// Detail argumen tool Antigravity (nama field PascalCase per tool).
fn tool_detail(params: &Value) -> String {
    const KEYS: &[&str] = &[
        "CommandLine",
        "TargetFile",
        "AbsolutePath",
        "DirectoryPath",
        "SearchPath",
        "SearchDirectory",
        "Query",
        "Pattern",
        "Url",
        "query",
        "Prompt",
    ];
    KEYS.iter()
        .find_map(|k| params.get(k).and_then(Value::as_str))
        .unwrap_or_default()
        .to_string()
}

/// Nama + detail tool. MCP lazy (`call_mcp_tool`) → `server · tool`.
fn tool_text(name: &str, params: &Value) -> (String, String) {
    if name == "call_mcp_tool" {
        let server = params.get("ServerName").and_then(Value::as_str);
        let tool = params.get("ToolName").and_then(Value::as_str);
        if let (Some(s), Some(t)) = (server, tool) {
            return (format!("{s} · {t}"), String::new());
        }
    }
    (name.to_string(), tool_detail(params))
}

/// Parser `--output-format stream-json`.
#[derive(Debug, Default)]
pub struct Parser {
    /// Potongan teks `agent_response` yang belum dicatat.
    pending: String,
    last_text: String,
    /// Langkah tool yang sudah dicatat (satu baris per langkah walau ada `ACTIVE` + `DONE`).
    logged: BTreeSet<u64>,
    /// Usage per langkah (cadangan bila `result` tanpa `usage`).
    step_usage: Option<TokenUsage>,
}

impl Parser {
    fn flush(&mut self, out: &mut Parsed) {
        let text = std::mem::take(&mut self.pending);
        if let Some(l) = text_line(&text) {
            self.last_text = text.trim().to_string();
            out.logs.push(l);
        }
    }

    fn step(&mut self, s: &Value, out: &mut Parsed) {
        let kind = s
            .get("step_type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let done = s.get("state").and_then(Value::as_str) == Some("DONE");
        if done {
            self.step_usage = add_usage(self.step_usage, s.get("usage").and_then(usage_from));
        }
        if kind == "agent_response" {
            if let Some(t) = s.get("text_delta").and_then(Value::as_str) {
                self.pending.push_str(t);
            }
            if done {
                self.flush(out);
            }
            return;
        }
        self.flush(out);
        if kind != "tool" {
            return;
        }
        let info = s.get("tool_info").cloned().unwrap_or(Value::Null);
        let name = info
            .get("name")
            .or_else(|| s.get("tool_name"))
            .and_then(Value::as_str)
            .unwrap_or("tool");
        let index = s.get("step_index").and_then(Value::as_u64);
        if index.is_none_or(|i| self.logged.insert(i)) {
            let params = info.get("parameters").cloned().unwrap_or(Value::Null);
            let (name, detail) = tool_text(name, &params);
            out.logs.push(tool_line(&name, &detail));
        }
        if let Some(err) = info.get("error").filter(|e| !e.is_null()) {
            out.logs.push(error_line(&error_text(err)));
        }
    }

    fn result(&mut self, r: &Value, out: &mut Parsed) {
        self.flush(out);
        let status = r.get("status").and_then(Value::as_str).unwrap_or_default();
        let is_error = !status.eq_ignore_ascii_case("SUCCESS");
        let usage = r.get("usage").and_then(usage_from).or(self.step_usage);
        let response = r
            .get("response")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map_or_else(|| self.last_text.clone(), str::to_string);
        let text = if is_error {
            let msg = r.get("error").map(error_text).unwrap_or_default();
            if msg.trim().is_empty() {
                format!("Antigravity CLI berhenti dengan status {status}")
            } else {
                msg
            }
        } else {
            response
        };
        let finished = Finished {
            is_error,
            text,
            tokens: usage.map(|u| u.total()),
            usage,
            cost_usd: None,
            turns: None,
        };
        out.logs.push(finished.log());
        out.finished = Some(finished);
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
        let event = v.get("event").and_then(Value::as_str).unwrap_or_default();
        let body = v.get(event).cloned().unwrap_or(Value::Null);
        match event {
            "init" => {
                let model = body
                    .get("model")
                    .and_then(Value::as_str)
                    .unwrap_or("bawaan");
                out.logs.push(line(
                    LogStream::Info,
                    format!("Antigravity CLI · model {model}"),
                ));
            }
            "step_update" => self.step(&body, &mut out),
            "result" => self.result(&body, &mut out),
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
    use super::{super::tests, *};

    /// Fixture `agy -p … --output-format stream-json`, bentuk dari contoh docs headless.
    pub const SAMPLE: &[&str] = &[
        r#"{"event":"init","conversation_id":"c1","init":{"cwd":"/wt","tools":["run_command","write_to_file"],"permission_mode":"always-proceed","model":"gemini-3.8-flash-medium"}}"#,
        r#"{"event":"step_update","step_update":{"conversation_id":"c1","step_index":0,"state":"DONE","step_type":"user_input"}}"#,
        r#"{"event":"step_update","step_update":{"conversation_id":"c1","step_index":1,"state":"ACTIVE","step_type":"agent_response","text_delta":"Saya muat "}}"#,
        r#"{"event":"step_update","step_update":{"conversation_id":"c1","step_index":1,"state":"DONE","step_type":"agent_response","text_delta":"konteks tiket.\n","usage":{"input_tokens":100,"output_tokens":10,"thinking_tokens":4,"cache_read_tokens":0,"total_tokens":110}}}"#,
        r#"{"event":"step_update","step_update":{"conversation_id":"c1","step_index":2,"state":"DONE","step_type":"tool","tool_name":"call_mcp_tool","tool_info":{"name":"call_mcp_tool","parameters":{"ServerName":"agentgate","ToolName":"ag_context","Arguments":{"ticket":"KSR-26"}},"output":"{}"}}}"#,
        r#"{"event":"step_update","step_update":{"conversation_id":"c1","step_index":3,"state":"ACTIVE","step_type":"tool","tool_name":"run_command","tool_info":{"name":"run_command","parameters":{"CommandLine":"cargo test -p kasir","Cwd":"/wt"}}}}"#,
        r#"{"event":"step_update","step_update":{"conversation_id":"c1","step_index":3,"state":"DONE","step_type":"tool","tool_name":"run_command","duration_seconds":7.1,"tool_info":{"name":"run_command","parameters":{"CommandLine":"cargo test -p kasir","Cwd":"/wt"},"output":"","error":{"type":"exit","message":"exit status 101"}}}}"#,
        r#"{"event":"step_update","step_update":{"conversation_id":"c1","step_index":4,"state":"DONE","step_type":"checkpoint","duration_seconds":0.5}}"#,
        r#"{"event":"step_update","step_update":{"conversation_id":"c1","step_index":5,"state":"DONE","step_type":"agent_response","text_delta":"Validasi stok selesai.\n"}}"#,
        r#"{"event":"result","result":{"conversation_id":"c1","status":"SUCCESS","response":"Validasi stok selesai.\n","duration_seconds":11.2,"num_turns":1,"usage":{"input_tokens":1000,"output_tokens":300,"thinking_tokens":120,"cache_read_tokens":200,"total_tokens":1300}}}"#,
    ];

    #[test]
    fn stream_json_becomes_logs_and_result() {
        let parsed = tests::run(&mut Parser::default(), SAMPLE);
        assert_eq!(
            tests::logs(&parsed),
            [
                (
                    LogStream::Info,
                    "Antigravity CLI · model gemini-3.8-flash-medium".into()
                ),
                (LogStream::Out, "Saya muat konteks tiket.".into()),
                (LogStream::Tool, "agentgate · ag_context".into()),
                (LogStream::Tool, "run_command: cargo test -p kasir".into()),
                (LogStream::Err, "exit status 101".into()),
                (LogStream::Out, "Validasi stok selesai.".into()),
                (LogStream::Ok, "Agent selesai".into()),
            ]
        );
        assert!(parsed.iter().all(|p| p.steps.is_none()));
        let done = tests::finished(&parsed);
        assert!(!done.is_error);
        assert_eq!(done.text, "Validasi stok selesai.");
        // `cache_read_tokens` terpisah dari `input_tokens` (total docs = input + output).
        assert_eq!(
            done.usage,
            Some(TokenUsage {
                input: 1000,
                output: 300,
                cache_read: 200,
                cache_write: 0
            })
        );
        assert_eq!(done.tokens, Some(1500));
    }

    #[test]
    fn error_result_and_quota() {
        let parsed = tests::run(
            &mut Parser::default(),
            &[
                r#"{"event":"result","result":{"conversation_id":"c","status":"ERROR","response":"","error":"HTTP 429 RESOURCE_EXHAUSTED: Resource has been exhausted (e.g. check quota).","duration_seconds":0,"num_turns":0,"usage":{"input_tokens":0,"output_tokens":0,"thinking_tokens":0,"cache_read_tokens":0,"total_tokens":0}}}"#,
            ],
        );
        let done = tests::finished(&parsed);
        assert!(done.is_error);
        assert!(super::super::is_quota(&done.text));
        assert_eq!(done.usage, None);
        // Status selain SUCCESS tanpa pesan error (mis. WAITING) tetap gagal dengan alasan jelas.
        let parsed = tests::run(
            &mut Parser::default(),
            &[r#"{"event":"result","result":{"status":"WAITING","response":""}}"#],
        );
        let done = tests::finished(&parsed);
        assert!(done.is_error);
        assert!(done.text.contains("WAITING"), "{}", done.text);
    }

    #[test]
    fn step_usage_is_the_fallback_and_raw_lines_are_kept() {
        let parsed = tests::run(
            &mut Parser::default(),
            &[
                "Peringatan: update tersedia",
                r#"{"event":"step_update","step_update":{"step_index":1,"state":"DONE","step_type":"agent_response","text_delta":"ok","usage":{"input_tokens":5,"output_tokens":2}}}"#,
                r#"{"event":"step_update","step_update":{"step_index":2,"state":"DONE","step_type":"checkpoint","usage":{"input_tokens":1,"output_tokens":1}}}"#,
                r#"{"event":"result","result":{"status":"SUCCESS","response":""}}"#,
            ],
        );
        assert_eq!(
            tests::logs(&parsed)[0],
            (LogStream::Out, "Peringatan: update tersedia".into())
        );
        let done = tests::finished(&parsed);
        assert_eq!(done.text, "ok");
        assert_eq!(
            done.usage,
            Some(TokenUsage {
                input: 6,
                output: 3,
                ..TokenUsage::default()
            })
        );
    }

    #[test]
    fn headless_args_skip_permissions_and_pass_model_effort() {
        let mut spec = tests::spec();
        spec.effort = "high".into();
        let inv = auto(&spec);
        assert_eq!(inv.args[..2], ["-p".to_string(), spec.prompt.clone()]);
        for w in [
            ["--output-format", "stream-json"],
            ["--model", "claude-sonnet-4-5"],
            ["--effort", "high"],
        ] {
            assert!(inv.args.windows(2).any(|x| x == w), "{w:?}");
        }
        assert!(
            inv.args
                .contains(&"--dangerously-skip-permissions".to_string())
        );
        assert_eq!(inv.env, [(NO_UPDATE_ENV, "true".to_string())]);
        assert_eq!(inv.stdin, None);
        let i = interactive_args(&spec);
        assert_eq!(i[i.len() - 2..], ["-i".to_string(), spec.prompt.clone()]);
        assert!(i.windows(2).any(|x| x == ["--effort", "high"]));
        spec.prompt = "--yolo".into();
        assert_eq!(auto(&spec).args[1], " --yolo");
    }
}
