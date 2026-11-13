//! Adapter Claude Code: perintah headless dan parser output `stream-json`.
//!
//! Job otomatis menjalankan
//! `claude -p <prompt> --output-format stream-json --verbose --setting-sources project
//! --strict-mcp-config --mcp-config .mcp.json --permission-mode acceptEdits [--model M]
//! [--effort E]` di worktree. Flag isolasi membuat config global user tidak ikut berlaku; hook
//! `PreToolUse` dari `.claude/settings.json` worktree tetap aktif.
//!
//! Setiap baris stdout adalah satu event JSON (`system`, `assistant`, `user`, `result`);
//! [`Parser`] mengubahnya menjadi baris log, langkah (dari `TodoWrite`), dan hasil akhir. Cursor
//! memakai bentuk event yang sama untuk `system`/`assistant`/`result`, jadi parser ini juga
//! dipakai adapter Cursor.

use std::path::Path;

use agentgate_core::runner::protocol::{JobSpec, LogStream};
use serde_json::Value;

use super::{
    Finished, Parsed, StreamParser, effort, error_line, line, model, raw_line, text_line,
    todo_steps, tool_detail, tool_line, usage_from,
};

/// Argumen `claude` untuk job otomatis.
#[must_use]
pub fn auto_args(spec: &JobSpec) -> Vec<String> {
    let mut args = vec![
        "-p".to_string(),
        spec.prompt.clone(),
        "--output-format".into(),
        "stream-json".into(),
        "--verbose".into(),
        "--setting-sources".into(),
        "project".into(),
        "--strict-mcp-config".into(),
        "--mcp-config".into(),
        ".mcp.json".into(),
        "--permission-mode".into(),
        "acceptEdits".into(),
    ];
    if let Some(m) = model(spec) {
        args.extend(["--model".into(), m.to_string()]);
    }
    if let Some(e) = effort(spec) {
        args.extend(["--effort".into(), e.to_string()]);
    }
    args
}

/// Argumen `claude` untuk sesi interaktif (`agentgate job attach`): prompt sebagai pesan
/// pertama, config MCP project.
#[must_use]
pub fn interactive_args(spec: &JobSpec, worktree: &Path) -> Vec<String> {
    let mut args = Vec::new();
    if worktree.join(".mcp.json").is_file() {
        args.push("--mcp-config".into());
        args.push(".mcp.json".into());
    }
    if let Some(m) = model(spec) {
        args.extend(["--model".into(), m.to_string()]);
    }
    args.push(spec.prompt.clone());
    args
}

/// Parser stream-json (stateless per baris; state hanya teks terakhir agent).
#[derive(Debug)]
pub struct Parser {
    /// Nama client di baris log awal.
    label: &'static str,
    /// Teks assistant terakhir (ringkasan cadangan bila `result` kosong).
    pub last_text: String,
}

impl Parser {
    #[must_use]
    pub fn new(label: &'static str) -> Self {
        Self {
            label,
            last_text: String::new(),
        }
    }
}

fn tool_result_text(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(items) => items
            .iter()
            .filter_map(|i| i.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn content(v: &Value) -> Vec<Value> {
    v.pointer("/message/content")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

impl StreamParser for Parser {
    fn line(&mut self, raw: &str) -> Parsed {
        let mut out = Parsed::default();
        let Ok(v) = serde_json::from_str::<Value>(raw.trim()) else {
            out.logs.extend(raw_line(raw));
            return out;
        };
        match v.get("type").and_then(Value::as_str) {
            Some("system") if v.get("subtype").and_then(Value::as_str) == Some("init") => {
                let model = v.get("model").and_then(Value::as_str).unwrap_or("?");
                out.logs.push(line(
                    LogStream::Info,
                    format!("{} · model {model}", self.label),
                ));
            }
            Some("assistant") => {
                for item in content(&v) {
                    match item.get("type").and_then(Value::as_str) {
                        Some("text") => {
                            let text = item.get("text").and_then(Value::as_str).unwrap_or("");
                            if let Some(l) = text_line(text) {
                                self.last_text = text.trim().to_string();
                                out.logs.push(l);
                            }
                        }
                        Some("tool_use") => {
                            let name = item.get("name").and_then(Value::as_str).unwrap_or("tool");
                            let input = item.get("input").cloned().unwrap_or(Value::Null);
                            if name == "TodoWrite" {
                                out.steps = input
                                    .get("todos")
                                    .and_then(Value::as_array)
                                    .map(|t| todo_steps(t));
                            } else {
                                out.logs.push(tool_line(name, &tool_detail(&input)));
                            }
                        }
                        _ => {}
                    }
                }
            }
            Some("user") => {
                for item in content(&v) {
                    if item.get("type").and_then(Value::as_str) == Some("tool_result")
                        && item.get("is_error").and_then(Value::as_bool) == Some(true)
                    {
                        let text = tool_result_text(item.get("content").unwrap_or(&Value::Null));
                        out.logs.push(error_line(&text));
                    }
                }
            }
            Some("result") => {
                let is_error = v.get("is_error").and_then(Value::as_bool).unwrap_or(false)
                    || v.get("subtype")
                        .and_then(Value::as_str)
                        .is_some_and(|s| s != "success");
                let text = v
                    .get("result")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                let usage = v.get("usage").and_then(usage_from);
                let finished = Finished {
                    is_error,
                    text: if text.is_empty() {
                        self.last_text.clone()
                    } else {
                        text
                    },
                    tokens: usage.map(|u| u.total()),
                    usage,
                    cost_usd: v.get("total_cost_usd").and_then(Value::as_f64),
                    turns: v.get("num_turns").and_then(Value::as_u64),
                };
                out.logs.push(finished.log());
                out.finished = Some(finished);
            }
            _ => {}
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use agentgate_core::runner::protocol::{StepState, TokenUsage};

    use super::*;

    const SAMPLE: &[&str] = &[
        r#"{"type":"system","subtype":"init","cwd":"/wt","session_id":"s1","tools":["Bash","Edit"],"mcp_servers":[{"name":"agentgate","status":"connected"}],"model":"claude-sonnet-4-5","permissionMode":"acceptEdits"}"#,
        r#"{"type":"assistant","message":{"id":"m1","type":"message","role":"assistant","model":"claude-sonnet-4-5","content":[{"type":"text","text":"Saya muat konteks tiket dulu."},{"type":"tool_use","id":"t1","name":"mcp__agentgate__ag_context","input":{"ticket":"KSR-26"}}],"usage":{"input_tokens":10,"output_tokens":5}},"session_id":"s1"}"#,
        r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t2","name":"TodoWrite","input":{"todos":[{"content":"Baca kode stok","status":"completed","activeForm":"Membaca"},{"content":"Tambah validasi","status":"in_progress","activeForm":"Menambah"},{"content":"Tulis tes","status":"pending","activeForm":"Menulis"}]}}]}}"#,
        r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t3","name":"Bash","input":{"command":"cargo test -p kasir\necho done","description":"Run tests"}}]}}"#,
        r#"{"type":"user","message":{"role":"user","content":[{"tool_use_id":"t3","type":"tool_result","content":"error[E0425]: cannot find value `x`\nmore","is_error":true}]}}"#,
        r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t4","name":"Edit","input":{"file_path":"src/stok.rs","old_string":"a","new_string":"b"}}]}}"#,
        r#"{"type":"user","message":{"content":[{"tool_use_id":"t4","type":"tool_result","content":[{"type":"text","text":"ok"}]}]}}"#,
        r#"{"type":"result","subtype":"success","is_error":false,"duration_ms":81234,"num_turns":7,"result":"Validasi stok ditambahkan dan tes lulus.","session_id":"s1","total_cost_usd":0.4213,"usage":{"input_tokens":1200,"cache_creation_input_tokens":300,"cache_read_input_tokens":5000,"output_tokens":800}}"#,
    ];

    #[test]
    fn stream_json_becomes_log_lines_steps_and_result() {
        let mut p = Parser::new("Claude Code");
        let parsed: Vec<Parsed> = SAMPLE.iter().map(|l| p.line(l)).collect();
        let logs: Vec<(LogStream, String)> = parsed
            .iter()
            .flat_map(|x| x.logs.iter().map(|l| (l.stream, l.text.clone())))
            .collect();
        assert_eq!(
            logs,
            [
                (
                    LogStream::Info,
                    "Claude Code · model claude-sonnet-4-5".into()
                ),
                (LogStream::Out, "Saya muat konteks tiket dulu.".into()),
                (LogStream::Tool, "agentgate · ag_context".into()),
                (LogStream::Tool, "Bash: cargo test -p kasir".into()),
                (LogStream::Err, "error[E0425]: cannot find value `x`".into()),
                (LogStream::Tool, "Edit: src/stok.rs".into()),
                (LogStream::Ok, "Agent selesai · 7 giliran · $0.42".into()),
            ]
        );
        let steps = parsed[2].steps.clone().unwrap();
        assert_eq!(
            steps.iter().map(|s| s.state).collect::<Vec<_>>(),
            [StepState::Ok, StepState::Now, StepState::Wait]
        );
        assert_eq!(steps[1].name, "Tambah validasi");
        let done = parsed[7].finished.clone().unwrap();
        assert!(!done.is_error);
        assert_eq!(done.text, "Validasi stok ditambahkan dan tes lulus.");
        assert_eq!(done.tokens, Some(7300));
        assert_eq!(
            done.usage,
            Some(TokenUsage {
                input: 1200,
                output: 800,
                cache_read: 5000,
                cache_write: 300,
            })
        );
        assert_eq!(done.turns, Some(7));
    }

    #[test]
    fn non_json_lines_are_kept_and_error_results_fail() {
        let mut p = Parser::new("Claude Code");
        assert_eq!(p.line("plain output").logs[0].stream, LogStream::Out);
        assert!(p.line("   ").logs.is_empty());
        let r = p.line(
            r#"{"type":"result","subtype":"error_max_turns","is_error":false,"num_turns":50}"#,
        );
        assert!(r.finished.unwrap().is_error);
        assert_eq!(r.logs[0].stream, LogStream::Fail);
    }

    #[test]
    fn prompt_is_a_single_argv_entry() {
        let mut spec = super::super::tests::spec();
        spec.effort = "high".into();
        let args = auto_args(&spec);
        assert_eq!(args[0], "-p");
        assert_eq!(args[1], spec.prompt);
        assert!(
            args.windows(2)
                .any(|w| w == ["--model", "claude-sonnet-4-5"])
        );
        assert!(
            args.windows(2)
                .any(|w| w == ["--setting-sources", "project"])
        );
        assert!(args.windows(2).any(|w| w == ["--effort", "high"]));
    }
}
