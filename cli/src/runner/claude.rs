//! Adapter Claude Code: perintah headless dan parser output `stream-json`.
//!
//! Job otomatis menjalankan
//! `claude -p <prompt> --output-format stream-json --verbose --setting-sources project
//! --strict-mcp-config --mcp-config .mcp.json --permission-mode acceptEdits [--model M]`
//! di worktree. Prompt selalu argv (tidak pernah lewat shell). Flag isolasi membuat config global
//! user tidak ikut berlaku; hook `PreToolUse` dari `.claude/settings.json` worktree tetap aktif.
//!
//! Setiap baris stdout adalah satu event JSON (`system`, `assistant`, `user`, `result`);
//! [`Parser`] mengubahnya menjadi baris log, langkah (dari `TodoWrite`), dan hasil akhir.

use std::path::Path;

use agentgate_core::runner::protocol::{JobSpec, LogLine, LogStream, Step, StepState};
use serde_json::Value;

use crate::ui::clip;

/// Batas panjang satu baris log teks agent.
const TEXT_MAX: usize = 2_000;
/// Batas panjang ringkasan input tool.
const TOOL_MAX: usize = 300;

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
    if !spec.model.trim().is_empty() {
        args.push("--model".into());
        args.push(spec.model.trim().to_string());
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
    if !spec.model.trim().is_empty() {
        args.push("--model".into());
        args.push(spec.model.trim().to_string());
    }
    args.push(spec.prompt.clone());
    args
}

/// Hasil akhir dari event `result`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Finished {
    pub is_error: bool,
    pub text: String,
    pub tokens: Option<u64>,
    pub cost_usd: Option<f64>,
    pub turns: Option<u64>,
}

/// Keluaran satu baris stream.
#[derive(Debug, Default, PartialEq)]
pub struct Parsed {
    pub logs: Vec<LogLine>,
    /// Rencana agent (`TodoWrite`) bila berubah.
    pub steps: Option<Vec<Step>>,
    pub finished: Option<Finished>,
}

/// Parser stream-json (stateless per baris; state hanya teks terakhir agent).
#[derive(Debug, Default)]
pub struct Parser {
    /// Teks assistant terakhir (ringkasan cadangan bila `result` kosong).
    pub last_text: String,
}

fn line(stream: LogStream, text: impl Into<String>) -> LogLine {
    LogLine {
        stream,
        text: text.into(),
    }
}

fn tool_summary(name: &str, input: &Value) -> String {
    let field = |k: &str| input.get(k).and_then(Value::as_str).unwrap_or_default();
    let detail = match name {
        "Bash" => field("command").to_string(),
        "Read" | "Write" | "Edit" | "MultiEdit" | "NotebookEdit" => {
            let p = field("file_path");
            if p.is_empty() {
                field("notebook_path")
            } else {
                p
            }
            .to_string()
        }
        "Glob" | "Grep" => field("pattern").to_string(),
        "WebFetch" => field("url").to_string(),
        "WebSearch" => field("query").to_string(),
        "Task" | "Agent" => field("description").to_string(),
        _ => String::new(),
    };
    let name = name
        .strip_prefix("mcp__")
        .map_or_else(|| name.to_string(), |n| n.replace("__", " · "));
    let detail = detail.lines().next().unwrap_or_default();
    if detail.is_empty() {
        name
    } else {
        format!("{name}: {}", clip(detail, TOOL_MAX))
    }
}

fn todo_steps(input: &Value) -> Option<Vec<Step>> {
    let todos = input.get("todos")?.as_array()?;
    Some(
        todos
            .iter()
            .take(40)
            .filter_map(|t| {
                let name = t
                    .get("content")
                    .or_else(|| t.get("activeForm"))
                    .and_then(Value::as_str)?;
                let state = match t.get("status").and_then(Value::as_str) {
                    Some("completed") => StepState::Ok,
                    Some("in_progress") => StepState::Now,
                    _ => StepState::Wait,
                };
                Some(Step {
                    name: clip(name, 200),
                    state,
                    seconds: None,
                })
            })
            .collect(),
    )
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

fn usage_tokens(usage: &Value) -> Option<u64> {
    let n = |k: &str| usage.get(k).and_then(Value::as_u64).unwrap_or(0);
    let total = n("input_tokens")
        + n("output_tokens")
        + n("cache_creation_input_tokens")
        + n("cache_read_input_tokens");
    (total > 0).then_some(total)
}

impl Parser {
    /// Proses satu baris stdout.
    pub fn line(&mut self, raw: &str) -> Parsed {
        let mut out = Parsed::default();
        let raw = raw.trim();
        if raw.is_empty() {
            return out;
        }
        let Ok(v) = serde_json::from_str::<Value>(raw) else {
            out.logs.push(line(LogStream::Out, clip(raw, TEXT_MAX)));
            return out;
        };
        match v.get("type").and_then(Value::as_str) {
            Some("system") if v.get("subtype").and_then(Value::as_str) == Some("init") => {
                let model = v.get("model").and_then(Value::as_str).unwrap_or("?");
                out.logs.push(line(
                    LogStream::Info,
                    format!("Claude Code · model {model}"),
                ));
            }
            Some("assistant") => {
                let content = v
                    .pointer("/message/content")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                for item in content {
                    match item.get("type").and_then(Value::as_str) {
                        Some("text") => {
                            let text = item.get("text").and_then(Value::as_str).unwrap_or("");
                            if !text.trim().is_empty() {
                                self.last_text = text.trim().to_string();
                                out.logs
                                    .push(line(LogStream::Out, clip(text.trim(), TEXT_MAX)));
                            }
                        }
                        Some("tool_use") => {
                            let name = item.get("name").and_then(Value::as_str).unwrap_or("tool");
                            let input = item.get("input").cloned().unwrap_or(Value::Null);
                            if name == "TodoWrite" {
                                out.steps = todo_steps(&input);
                            } else {
                                out.logs
                                    .push(line(LogStream::Tool, tool_summary(name, &input)));
                            }
                        }
                        _ => {}
                    }
                }
            }
            Some("user") => {
                let content = v
                    .pointer("/message/content")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                for item in content {
                    if item.get("type").and_then(Value::as_str) == Some("tool_result")
                        && item.get("is_error").and_then(Value::as_bool) == Some(true)
                    {
                        let text = tool_result_text(item.get("content").unwrap_or(&Value::Null));
                        let first = text.lines().next().unwrap_or("").trim();
                        out.logs.push(line(LogStream::Err, clip(first, TEXT_MAX)));
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
                let finished = Finished {
                    is_error,
                    text: if text.is_empty() {
                        self.last_text.clone()
                    } else {
                        text
                    },
                    tokens: v.get("usage").and_then(usage_tokens),
                    cost_usd: v.get("total_cost_usd").and_then(Value::as_f64),
                    turns: v.get("num_turns").and_then(Value::as_u64),
                };
                let mut summary = if is_error {
                    "Agent berhenti dengan error".to_string()
                } else {
                    "Agent selesai".to_string()
                };
                if let Some(t) = finished.turns {
                    summary.push_str(&format!(" · {t} giliran"));
                }
                if let Some(c) = finished.cost_usd {
                    summary.push_str(&format!(" · ${c:.2}"));
                }
                out.logs.push(line(
                    if is_error {
                        LogStream::Fail
                    } else {
                        LogStream::Ok
                    },
                    summary,
                ));
                out.finished = Some(finished);
            }
            _ => {}
        }
        out
    }
}

/// Pesan kuota/limit AI habis (Claude Code: "Claude AI usage limit reached|<epoch>",
/// "You've hit your limit · resets 3pm", "5-hour limit reached ∙ resets 2am").
#[must_use]
pub fn is_quota(text: &str) -> bool {
    let t = text.to_ascii_lowercase();
    t.contains("usage limit reached")
        || t.contains("hit your limit")
        || (t.contains("limit reached") && t.contains("reset"))
        || t.contains("rate_limit_error")
        || t.contains("credit balance is too low")
        || t.contains("quota exceeded")
}

/// Waktu reset kuota (RFC 3339) dari format `…limit reached|<epoch detik>`.
#[must_use]
pub fn quota_reset_at(text: &str) -> Option<String> {
    let (_, tail) = text.rsplit_once('|')?;
    let secs: i64 = tail
        .trim()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .ok()?;
    chrono::DateTime::from_timestamp(secs, 0).map(|t| t.to_rfc3339())
}

#[cfg(test)]
mod tests {
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
        let mut p = Parser::default();
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
        assert_eq!(done.turns, Some(7));
    }

    #[test]
    fn non_json_lines_are_kept_and_error_results_fail() {
        let mut p = Parser::default();
        assert_eq!(p.line("plain output").logs[0].stream, LogStream::Out);
        assert!(p.line("   ").logs.is_empty());
        let r = p.line(
            r#"{"type":"result","subtype":"error_max_turns","is_error":false,"num_turns":50}"#,
        );
        assert!(r.finished.unwrap().is_error);
        assert_eq!(r.logs[0].stream, LogStream::Fail);
    }

    #[test]
    fn quota_messages_are_detected() {
        for t in [
            "Claude AI usage limit reached|1763000000",
            "You've hit your limit · resets 3pm (Asia/Jakarta)",
            "5-hour limit reached ∙ resets 2am",
            "API Error: 429 {\"type\":\"error\",\"error\":{\"type\":\"rate_limit_error\"}}",
            "Credit balance is too low",
        ] {
            assert!(is_quota(t), "{t}");
        }
        for t in ["Validasi stok selesai", "rate limiter ditambahkan ke API"] {
            assert!(!is_quota(t), "{t}");
        }
        assert_eq!(
            quota_reset_at("Claude AI usage limit reached|1763000000").as_deref(),
            Some("2025-11-13T02:13:20+00:00")
        );
        assert_eq!(quota_reset_at("resets 3pm"), None);
    }

    #[test]
    fn prompt_is_a_single_argv_entry() {
        let spec: JobSpec = serde_json::from_value(crate::testutil::spec_json()).unwrap();
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
    }
}
