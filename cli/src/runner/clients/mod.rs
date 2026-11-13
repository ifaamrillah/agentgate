//! Adapter client AI untuk runner: satu modul per client (spec "Runner & eksekusi job").
//!
//! Setiap adapter memberi:
//!
//! - **program**: nama binary bawaan, diganti env `AGENTGATE_<CLIENT>_BIN` (mis.
//!   `AGENTGATE_CODEX_BIN`). Cursor mencari `agent` lalu `cursor-agent` di PATH.
//! - **argumen headless** ([`ClientKind::auto`]): prompt selalu satu argumen argv atau stdin,
//!   tidak pernah lewat shell; flag trust/isolasi/auto-approve per client; model + effort persona.
//! - **argumen interaktif** ([`ClientKind::interactive`]) untuk `agentgate job attach`.
//! - **parser output** ([`StreamParser`]): JSONL client → baris log, langkah, hasil akhir, dan
//!   pemakaian token ([`TokenUsage`]). Field yang belum terverifikasi dibaca toleran (beberapa
//!   nama alternatif; tidak ada = `None`).
//!
//! **Isolasi best effort.** Hanya Claude Code (`--setting-sources project --strict-mcp-config`)
//! dan Codex (`--ignore-user-config`) bisa mengabaikan config global user tanpa mengganti home
//! dir. Client lain tetap membaca config global user (MCP, hook, rules pribadi); mengganti home
//! dir memutus login berbasis file, jadi runner tidak melakukannya.
//!
//! Kuota habis dideteksi dari teks log/stderr/hasil ([`is_quota`]) untuk semua client.

pub mod claude;
pub mod codex;
pub mod copilot;
pub mod cursor;
pub mod gemini;
pub mod opencode;

use std::path::Path;

use agentgate_core::runner::protocol::{JobSpec, LogLine, LogStream, Step, StepState, TokenUsage};
use clap::ValueEnum;
use serde_json::Value;

use crate::ui::clip;

/// Batas panjang satu baris log teks agent.
pub const TEXT_MAX: usize = 2_000;
/// Batas panjang ringkasan input tool.
pub const TOOL_MAX: usize = 300;
/// Batas jumlah langkah rencana agent.
const STEPS_MAX: usize = 40;

/// Client AI yang didukung runner (kunci = [`agentgate_core::persona::CLIENTS`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, ValueEnum)]
pub enum ClientKind {
    Claude,
    Codex,
    Gemini,
    Cursor,
    Opencode,
    Copilot,
}

/// Perintah client yang siap dijalankan (program ditentukan [`ClientKind::program`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Invocation {
    pub args: Vec<String>,
    /// Env tambahan untuk proses client.
    pub env: Vec<(&'static str, String)>,
    /// Isi stdin (prompt), bila client membaca prompt dari stdin.
    pub stdin: Option<String>,
}

impl Invocation {
    #[must_use]
    pub fn args(args: Vec<String>) -> Self {
        Self {
            args,
            ..Self::default()
        }
    }
}

impl ClientKind {
    /// Kunci client (`claude`, `codex`, …), sama dengan nama nilai `--client`.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Gemini => "gemini",
            Self::Cursor => "cursor",
            Self::Opencode => "opencode",
            Self::Copilot => "copilot",
        }
    }

    /// Client dengan kunci `key` (tanpa beda huruf besar/kecil).
    #[must_use]
    pub fn parse(key: &str) -> Option<Self> {
        <Self as ValueEnum>::from_str(key.trim(), true).ok()
    }

    /// Semua client.
    #[must_use]
    pub fn all() -> &'static [Self] {
        Self::value_variants()
    }

    /// Nama tampilan dari katalog client.
    #[must_use]
    pub fn name(self) -> &'static str {
        agentgate_core::persona::client(self.key()).map_or("?", |c| c.name)
    }

    /// Env pengganti program (`AGENTGATE_CODEX_BIN`).
    #[must_use]
    pub fn bin_env(self) -> String {
        format!("AGENTGATE_{}_BIN", self.key().to_ascii_uppercase())
    }

    /// Nama binary yang dicari di PATH, urut prioritas.
    #[must_use]
    pub fn programs(self) -> &'static [&'static str] {
        match self {
            Self::Claude => &["claude"],
            Self::Codex => &["codex"],
            Self::Gemini => &["gemini"],
            // Docs Cursor memakai `agent`; alias lama `cursor-agent` belum pasti dihapus.
            Self::Cursor => &["agent", "cursor-agent"],
            Self::Opencode => &["opencode"],
            Self::Copilot => &["copilot"],
        }
    }

    /// Program client: env pengganti, lalu binary pertama yang ada di PATH, lalu nama bawaan.
    #[must_use]
    pub fn program(self) -> String {
        if let Some(bin) = std::env::var(self.bin_env())
            .ok()
            .filter(|s| !s.trim().is_empty())
        {
            return bin;
        }
        let names = self.programs();
        names
            .iter()
            .find(|n| crate::hook::which(n).is_some())
            .or_else(|| names.first())
            .map_or_else(|| self.key().to_string(), |n| (*n).to_string())
    }

    /// Perintah headless untuk job otomatis di worktree `wt`.
    #[must_use]
    pub fn auto(self, spec: &JobSpec, wt: &Path) -> Invocation {
        match self {
            Self::Claude => Invocation::args(claude::auto_args(spec)),
            Self::Codex => codex::auto(spec, wt),
            Self::Gemini => gemini::auto(spec),
            Self::Cursor => Invocation::args(cursor::auto_args(spec, wt)),
            Self::Opencode => Invocation::args(opencode::auto_args(spec)),
            Self::Copilot => copilot::auto(spec, wt),
        }
    }

    /// Perintah sesi interaktif (`agentgate job attach`): prompt sebagai pesan pertama.
    #[must_use]
    pub fn interactive(self, spec: &JobSpec, wt: &Path) -> Vec<String> {
        match self {
            Self::Claude => claude::interactive_args(spec, wt),
            Self::Codex => codex::interactive_args(spec),
            Self::Gemini => gemini::interactive_args(spec),
            Self::Cursor => cursor::interactive_args(spec),
            Self::Opencode => opencode::interactive_args(spec),
            Self::Copilot => copilot::interactive_args(spec),
        }
    }

    /// Parser output headless client ini.
    #[must_use]
    pub fn parser(self) -> Box<dyn StreamParser> {
        match self {
            Self::Claude => Box::new(claude::Parser::new("Claude Code")),
            Self::Codex => Box::new(codex::Parser::default()),
            Self::Gemini => Box::new(gemini::Parser::default()),
            Self::Cursor => Box::new(cursor::Parser::default()),
            Self::Opencode => Box::new(opencode::Parser::default()),
            Self::Copilot => Box::new(copilot::Parser::default()),
        }
    }
}

/// Hasil akhir satu sesi client.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Finished {
    pub is_error: bool,
    pub text: String,
    pub tokens: Option<u64>,
    pub usage: Option<TokenUsage>,
    pub cost_usd: Option<f64>,
    pub turns: Option<u64>,
}

impl Finished {
    /// Baris log ringkasan (`Agent selesai · 7 giliran · $0.42`).
    #[must_use]
    pub fn log(&self) -> LogLine {
        let mut summary = if self.is_error {
            "Agent berhenti dengan error".to_string()
        } else {
            "Agent selesai".to_string()
        };
        if let Some(t) = self.turns {
            summary.push_str(&format!(" · {t} giliran"));
        }
        if let Some(c) = self.cost_usd {
            summary.push_str(&format!(" · ${c:.2}"));
        }
        line(
            if self.is_error {
                LogStream::Fail
            } else {
                LogStream::Ok
            },
            summary,
        )
    }
}

/// Keluaran satu baris stream.
#[derive(Debug, Default, PartialEq)]
pub struct Parsed {
    pub logs: Vec<LogLine>,
    /// Rencana agent (todo list) bila berubah.
    pub steps: Option<Vec<Step>>,
    pub finished: Option<Finished>,
}

/// Parser output JSONL satu client (satu instance per job).
pub trait StreamParser: Send {
    /// Proses satu baris stdout.
    fn line(&mut self, raw: &str) -> Parsed;

    /// Stdout ditutup. Client tanpa event hasil akhir (OpenCode, Copilot) menyusun hasilnya
    /// dari state; bawaannya tidak ada.
    fn eof(&mut self) -> Parsed {
        Parsed::default()
    }
}

/// Satu baris log.
#[must_use]
pub fn line(stream: LogStream, text: impl Into<String>) -> LogLine {
    LogLine {
        stream,
        text: text.into(),
    }
}

/// Baris teks agent (dipotong), atau tidak ada bila kosong.
#[must_use]
pub fn text_line(text: &str) -> Option<LogLine> {
    let text = text.trim();
    (!text.is_empty()).then(|| line(LogStream::Out, clip(text, TEXT_MAX)))
}

/// Baris pertama teks error tool.
#[must_use]
pub fn error_line(text: &str) -> LogLine {
    let first = text.lines().map(str::trim).find(|l| !l.is_empty());
    line(
        LogStream::Err,
        clip(first.unwrap_or("tool gagal"), TEXT_MAX),
    )
}

/// Baris JSON yang tidak bisa diparse: tetap dicatat sebagai output.
#[must_use]
pub fn raw_line(raw: &str) -> Option<LogLine> {
    let raw = raw.trim();
    (!raw.is_empty()).then(|| line(LogStream::Out, clip(raw, TEXT_MAX)))
}

/// Ringkasan pemanggilan tool: `nama: detail` (detail = perintah/path/pola, baris pertama).
#[must_use]
pub fn tool_line(name: &str, detail: &str) -> LogLine {
    let name = name
        .strip_prefix("mcp__")
        .map_or_else(|| name.to_string(), |n| n.replace("__", " · "));
    let detail = detail.lines().next().unwrap_or_default().trim();
    let text = if detail.is_empty() {
        name
    } else {
        format!("{name}: {}", clip(detail, TOOL_MAX))
    };
    line(LogStream::Tool, text)
}

/// Detail input tool dari nama field umum (perintah, path, pola, url, query, deskripsi).
#[must_use]
pub fn tool_detail(input: &Value) -> String {
    const KEYS: &[&str] = &[
        "command",
        "cmd",
        "file_path",
        "filePath",
        "path",
        "notebook_path",
        "pattern",
        "url",
        "query",
        "description",
    ];
    if let Some(s) = input.as_str() {
        return s.to_string();
    }
    let field = KEYS
        .iter()
        .find_map(|k| input.get(k))
        .cloned()
        .unwrap_or(Value::Null);
    match field {
        Value::String(s) => s,
        // Codex/Copilot kadang memberi perintah sebagai argv.
        Value::Array(items) => items
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(" "),
        _ => String::new(),
    }
}

/// Status todo → keadaan langkah (`completed`, `in_progress`, `TODO_STATUS_COMPLETED`, …).
#[must_use]
pub fn step_state(status: &str) -> StepState {
    let s = status.to_ascii_lowercase();
    if s.contains("complete") || s == "done" {
        StepState::Ok
    } else if s.contains("progress") || s == "active" {
        StepState::Now
    } else if s.contains("cancel") || s.contains("fail") {
        StepState::Fail
    } else {
        StepState::Wait
    }
}

/// Langkah dari daftar todo: setiap item punya nama (`content`/`description`/`text`/`title`) dan
/// status (`status` teks atau `completed` boolean).
#[must_use]
pub fn todo_steps(items: &[Value]) -> Vec<Step> {
    items
        .iter()
        .take(STEPS_MAX)
        .filter_map(|t| {
            let name = ["content", "description", "text", "title", "activeForm"]
                .iter()
                .find_map(|k| t.get(k).and_then(Value::as_str))
                .filter(|n| !n.trim().is_empty())?;
            let state = match (t.get("status").and_then(Value::as_str), t.get("completed")) {
                (Some(s), _) => step_state(s),
                (None, Some(Value::Bool(true))) => StepState::Ok,
                _ => StepState::Wait,
            };
            Some(Step {
                name: clip(name.trim(), 200),
                state,
                seconds: None,
            })
        })
        .collect()
}

/// Angka pertama yang ada dari beberapa nama field (`a.b` = bertingkat).
fn first_u64(v: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|k| {
        let mut cur = v;
        for part in k.split('.') {
            cur = cur.get(part)?;
        }
        cur.as_u64()
    })
}

/// Pemakaian token dari objek usage dengan nama field client mana pun:
/// Claude (`input_tokens`, `cache_read_input_tokens`, …), camelCase (`inputTokens`,
/// `cacheReadTokens`), Gemini (`prompt`/`candidates`/`cached`), OpenCode (`input`, `output`,
/// `cache.read`). `cached_input` = token input yang sudah termasuk di `input` (Codex, Gemini):
/// dikurangkan supaya total tidak dihitung dua kali.
#[must_use]
pub fn usage_from(v: &Value) -> Option<TokenUsage> {
    let input = first_u64(
        v,
        &[
            "input_tokens",
            "inputTokens",
            "prompt_tokens",
            "promptTokens",
            "input",
            "prompt",
        ],
    )
    .unwrap_or(0);
    let output = first_u64(
        v,
        &[
            "output_tokens",
            "outputTokens",
            "completion_tokens",
            "completionTokens",
            "output",
            "candidates",
        ],
    )
    .unwrap_or(0);
    let cache_read = first_u64(
        v,
        &[
            "cache_read_input_tokens",
            "cacheReadInputTokens",
            "cacheReadTokens",
            "cache_read_tokens",
            "cache.read",
        ],
    );
    let cached_input = first_u64(v, &["cached_input_tokens", "cached", "cachedInputTokens"]);
    let cache_write = first_u64(
        v,
        &[
            "cache_creation_input_tokens",
            "cacheCreationInputTokens",
            "cacheWriteTokens",
            "cache_write_tokens",
            "cache.write",
        ],
    )
    .unwrap_or(0);
    let u = match (cache_read, cached_input) {
        (Some(read), _) => TokenUsage {
            input,
            output,
            cache_read: read,
            cache_write,
        },
        (None, Some(cached)) => TokenUsage {
            input: input.saturating_sub(cached),
            output,
            cache_read: cached.min(input),
            cache_write,
        },
        (None, None) => TokenUsage {
            input,
            output,
            cache_read: 0,
            cache_write,
        },
    };
    (u.total() > 0).then_some(u)
}

/// Jumlahkan dua usage (sesi multi-langkah).
#[must_use]
pub fn add_usage(a: Option<TokenUsage>, b: Option<TokenUsage>) -> Option<TokenUsage> {
    match (a, b) {
        (Some(a), Some(b)) => Some(TokenUsage {
            input: a.input.saturating_add(b.input),
            output: a.output.saturating_add(b.output),
            cache_read: a.cache_read.saturating_add(b.cache_read),
            cache_write: a.cache_write.saturating_add(b.cache_write),
        }),
        (a, b) => a.or(b),
    }
}

/// Teks pesan error dari nilai JSON (string, `{message}`, `{data: {message}}`, `{error: …}`).
#[must_use]
pub fn error_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Object(_) => ["message", "data.message", "error.message", "error", "name"]
            .iter()
            .find_map(|k| {
                let mut cur = v;
                for part in k.split('.') {
                    cur = cur.get(part)?;
                }
                cur.as_str().map(str::to_string)
            })
            .unwrap_or_else(|| v.to_string()),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Prompt sebagai argumen posisional: diawali `-` → diberi spasi supaya tidak dibaca sebagai
/// flag oleh parser argumen client.
#[must_use]
pub fn positional(prompt: &str) -> String {
    if prompt.starts_with('-') {
        format!(" {prompt}")
    } else {
        prompt.to_string()
    }
}

/// Model persona (kosong = default client).
#[must_use]
pub fn model(spec: &JobSpec) -> Option<&str> {
    Some(spec.model.trim()).filter(|m| !m.is_empty())
}

/// Effort persona (kosong = default client).
#[must_use]
pub fn effort(spec: &JobSpec) -> Option<&str> {
    Some(spec.effort.trim()).filter(|m| !m.is_empty())
}

/// Pesan kuota/limit AI habis dari client mana pun:
/// Claude Code ("Claude AI usage limit reached|<epoch>", "You've hit your limit · resets 3pm"),
/// Codex/Cursor ("You've hit your usage limit"), Gemini (`RESOURCE_EXHAUSTED`, "Quota exceeded"),
/// Copilot ("exceeded your premium request allowance"), API OpenAI (`insufficient_quota`).
#[must_use]
pub fn is_quota(text: &str) -> bool {
    let t = text.to_ascii_lowercase();
    t.contains("usage limit reached")
        || t.contains("hit your limit")
        || t.contains("hit your usage limit")
        || (t.contains("limit reached") && t.contains("reset"))
        || t.contains("rate_limit_error")
        || t.contains("credit balance is too low")
        || t.contains("quota exceeded")
        || t.contains("resource_exhausted")
        || t.contains("insufficient_quota")
        || t.contains("premium request allowance")
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
pub(crate) mod tests {
    use super::*;

    /// Jalankan semua baris fixture lewat parser, lalu `eof`.
    pub fn run(parser: &mut dyn StreamParser, lines: &[&str]) -> Vec<Parsed> {
        let mut out: Vec<Parsed> = lines.iter().map(|l| parser.line(l)).collect();
        out.push(parser.eof());
        out
    }

    pub fn logs(parsed: &[Parsed]) -> Vec<(LogStream, String)> {
        parsed
            .iter()
            .flat_map(|p| p.logs.iter().map(|l| (l.stream, l.text.clone())))
            .collect()
    }

    pub fn finished(parsed: &[Parsed]) -> Finished {
        parsed
            .iter()
            .rev()
            .find_map(|p| p.finished.clone())
            .expect("hasil akhir")
    }

    pub fn spec() -> JobSpec {
        serde_json::from_value(crate::testutil::spec_json()).unwrap()
    }

    #[test]
    fn keys_match_cli_values_and_catalog() {
        for c in ClientKind::all() {
            let value = c.to_possible_value().unwrap();
            assert_eq!(value.get_name(), c.key());
            assert_eq!(ClientKind::parse(c.key()), Some(*c));
            assert!(agentgate_core::persona::client(c.key()).is_some(), "{c:?}");
        }
        assert_eq!(
            ClientKind::all().len(),
            agentgate_core::persona::CLIENTS.len()
        );
        assert_eq!(ClientKind::parse(" Codex "), Some(ClientKind::Codex));
        assert_eq!(ClientKind::parse("vim"), None);
        assert_eq!(ClientKind::Copilot.bin_env(), "AGENTGATE_COPILOT_BIN");
        assert_eq!(ClientKind::Cursor.programs(), ["agent", "cursor-agent"]);
    }

    #[test]
    fn prompt_never_becomes_a_flag() {
        assert_eq!(positional("--yolo"), " --yolo");
        assert_eq!(positional("Kerjakan"), "Kerjakan");
        let spec = spec();
        let wt = Path::new("/wt");
        for c in ClientKind::all() {
            let inv = c.auto(&spec, wt);
            let carried = inv.stdin.as_deref() == Some(spec.prompt.as_str())
                || inv.args.iter().any(|a| a == &spec.prompt);
            assert!(carried, "{c:?}: {inv:?}");
            assert!(
                c.interactive(&spec, wt).iter().any(|a| a == &spec.prompt),
                "{c:?}"
            );
        }
    }

    #[test]
    fn usage_is_read_from_any_field_naming() {
        let u = |v: Value| usage_from(&v);
        assert_eq!(
            u(
                serde_json::json!({"input_tokens": 10, "output_tokens": 5, "cache_read_input_tokens": 3, "cache_creation_input_tokens": 2})
            ),
            Some(TokenUsage {
                input: 10,
                output: 5,
                cache_read: 3,
                cache_write: 2
            })
        );
        // Codex: input sudah termasuk cache.
        assert_eq!(
            u(
                serde_json::json!({"input_tokens": 100, "cached_input_tokens": 60, "output_tokens": 7})
            ),
            Some(TokenUsage {
                input: 40,
                output: 7,
                cache_read: 60,
                cache_write: 0
            })
        );
        assert_eq!(
            u(serde_json::json!({"inputTokens": 4, "outputTokens": 2, "cacheReadTokens": 1})),
            Some(TokenUsage {
                input: 4,
                output: 2,
                cache_read: 1,
                cache_write: 0
            })
        );
        assert_eq!(
            u(
                serde_json::json!({"input": 4, "output": 2, "reasoning": 9, "cache": {"read": 1, "write": 3}})
            ),
            Some(TokenUsage {
                input: 4,
                output: 2,
                cache_read: 1,
                cache_write: 3
            })
        );
        assert_eq!(u(serde_json::json!({"foo": 1})), None);
        assert_eq!(
            add_usage(
                Some(TokenUsage {
                    input: 1,
                    ..TokenUsage::default()
                }),
                Some(TokenUsage {
                    output: 2,
                    ..TokenUsage::default()
                })
            ),
            Some(TokenUsage {
                input: 1,
                output: 2,
                ..TokenUsage::default()
            })
        );
    }

    #[test]
    fn todo_items_become_steps() {
        let items: Vec<Value> = serde_json::from_str(
            r#"[{"content":"a","status":"completed"},{"description":"b","status":"in_progress"},
                {"text":"c","completed":false},{"text":"d","completed":true},
                {"content":"e","status":"TODO_STATUS_CANCELLED"},{"status":"pending"}]"#,
        )
        .unwrap();
        let steps = todo_steps(&items);
        assert_eq!(
            steps
                .iter()
                .map(|s| (s.name.as_str(), s.state))
                .collect::<Vec<_>>(),
            [
                ("a", StepState::Ok),
                ("b", StepState::Now),
                ("c", StepState::Wait),
                ("d", StepState::Ok),
                ("e", StepState::Fail)
            ]
        );
    }

    #[test]
    fn quota_messages_are_detected() {
        for t in [
            "Claude AI usage limit reached|1763000000",
            "You've hit your limit · resets 3pm (Asia/Jakarta)",
            "5-hour limit reached ∙ resets 2am",
            "API Error: 429 {\"type\":\"error\",\"error\":{\"type\":\"rate_limit_error\"}}",
            "Credit balance is too low",
            "You've hit your usage limit. Upgrade to Pro or try again at 3:15 PM.",
            "[API Error: RESOURCE_EXHAUSTED: Quota exceeded for quota metric]",
            "You have exceeded your premium request allowance.",
            "error: insufficient_quota",
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
}
