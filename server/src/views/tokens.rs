//! Halaman Penghemat token (M10): format angka token ("4,8 jt", "412 rb"), biaya USD dari laporan
//! client, daftar fitur penghemat (wajib/opsional, terukur/tidak), grafik minggu, batang per
//! tiket, dan baris tabel per job. Teks lewat kunci Fluent (`ui-tokens.ftl`).
use std::collections::HashMap;

use serde::Serialize;

use super::{
    charts::{self, Stack},
    runner::PersonaMark,
    tickets::ticket_href,
};
use crate::models::{
    personas::PersonaLabels,
    token_savings::{JobUsage, SavingFeature, TicketUsage, TokenSaver, Week},
};

/// Kunci flash setelah toggle disimpan.
pub const FLASH_SAVED: &str = "tokens-saved";

/// Kunci teks yang dipakai dari Rust (kontrak i18n).
pub const MESSAGE_KEYS: &[&str] = &[
    FLASH_SAVED,
    "tokens-stat-saved",
    "tokens-stat-saved-hint",
    "tokens-stat-saved-cost",
    "tokens-stat-saved-cost-hint",
    "tokens-stat-used",
    "tokens-stat-used-hint",
    "tokens-stat-cost",
    "tokens-stat-cost-hint",
    "tokens-feat-code-graph",
    "tokens-feat-rtk",
    "tokens-feat-mcp-rules",
    "tokens-feat-handoff",
    "tokens-feat-layered",
    "tokens-feat-stable-prefix",
    "tokens-feat-concise",
    "tokens-feat-persona-models",
    "tokens-feat-code-graph-desc",
    "tokens-feat-rtk-desc",
    "tokens-feat-mcp-rules-desc",
    "tokens-feat-handoff-desc",
    "tokens-feat-layered-desc",
    "tokens-feat-stable-prefix-desc",
    "tokens-feat-concise-desc",
    "tokens-feat-persona-models-desc",
    "tokens-panel-summary",
    "tokens-panel-summary-desc",
    "tokens-panel-features",
    "tokens-panel-features-desc",
    "tokens-panel-tickets",
    "tokens-panel-tickets-desc",
    "tokens-panel-jobs",
    "tokens-panel-jobs-desc",
];

/// Panel subpage: (kunci, label, ikon, deskripsi).
pub const PANELS: &[(&str, &str, &str, &str)] = &[
    (
        "ringkasan",
        "tokens-panel-summary",
        "chart-bar",
        "tokens-panel-summary-desc",
    ),
    (
        "fitur",
        "tokens-panel-features",
        "adjustments-horizontal",
        "tokens-panel-features-desc",
    ),
    (
        "tiket",
        "tokens-panel-tickets",
        "ticket",
        "tokens-panel-tickets-desc",
    ),
    (
        "job",
        "tokens-panel-jobs",
        "terminal-2",
        "tokens-panel-jobs-desc",
    ),
];

#[allow(clippy::cast_precision_loss)]
fn one_decimal(v: f64) -> String {
    let s = format!("{v:.1}");
    s.trim_end_matches('0')
        .trim_end_matches('.')
        .replace('.', ",")
}

/// Jumlah token ringkas gaya Indonesia: `950`, `412 rb`, `4,8 jt`, `1,2 M`.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn short(n: i64) -> String {
    let v = n.max(0) as f64;
    if v >= 1e9 {
        format!("{} M", one_decimal(v / 1e9))
    } else if v >= 1e6 {
        format!("{} jt", one_decimal(v / 1e6))
    } else if v >= 1e3 {
        format!(
            "{} rb",
            one_decimal(v / 1e3).split(',').next().unwrap_or("0")
        )
    } else {
        format!("{v}")
    }
}

/// Biaya dari mikro-USD: `$0,04`, `$12,30`, `$1.204`.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn usd(micros: i64) -> String {
    let v = micros.max(0) as f64 / 1e6;
    if v >= 1000.0 {
        let whole = format!("{v:.0}");
        let mut out = String::new();
        for (i, c) in whole.chars().enumerate() {
            if i > 0 && (whole.len() - i) % 3 == 0 {
                out.push('.');
            }
            out.push(c);
        }
        format!("${out}")
    } else {
        format!("${}", format!("{v:.2}").replace('.', ","))
    }
}

/// Persen bulat `part / whole` (0 bila `whole` 0).
#[must_use]
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
pub fn percent(part: i64, whole: i64) -> i64 {
    if whole <= 0 {
        0
    } else {
        ((part as f64 / whole as f64) * 100.0).round() as i64
    }
}

/// Satu fitur penghemat di panel "Fitur penghemat".
#[derive(Debug, Clone, Serialize)]
pub struct FeatureView {
    pub key: &'static str,
    /// Kunci Fluent `tokens-feat-<key>` dan `tokens-feat-<key>-desc`.
    pub label_key: String,
    pub desc_key: String,
    pub required: bool,
    pub on: bool,
    /// Token dihemat 30 hari (`None` = tidak terukur server).
    pub saved: Option<String>,
    /// Nama field form untuk fitur opsional.
    pub field: Option<&'static str>,
}

fn measured(f: SavingFeature) -> &'static str {
    match f {
        SavingFeature::CodeGraph => "code_graph",
        SavingFeature::Layered => "layered",
        SavingFeature::McpRules => "mcp_rules",
        SavingFeature::Handoff => "handoff",
    }
}

/// Urutan fitur sesuai spec: wajib (code graph, RTK, aturan lewat MCP, serah terima ringkas,
/// respons berlapis, prefix stabil) lalu opsional (gaya ringkas, model per persona).
#[must_use]
pub fn features(saved: &HashMap<SavingFeature, i64>, saver: TokenSaver) -> Vec<FeatureView> {
    let by_key = |k: &str| {
        saved
            .iter()
            .find(|(f, _)| measured(**f) == k)
            .map(|(_, v)| short(*v))
    };
    let row = |key: &'static str, required: bool, on: bool, field: Option<&'static str>| {
        let slug = key.replace('_', "-");
        FeatureView {
            key,
            label_key: format!("tokens-feat-{slug}"),
            desc_key: format!("tokens-feat-{slug}-desc"),
            required,
            on,
            saved: by_key(key),
            field,
        }
    };
    vec![
        row("code_graph", true, true, None),
        row("rtk", true, true, None),
        row("mcp_rules", true, true, None),
        row("handoff", true, true, None),
        row("layered", true, true, None),
        row("stable_prefix", true, true, None),
        row("concise", false, saver.concise, Some("concise")),
        row(
            "persona_models",
            false,
            saver.persona_models,
            Some("persona_models"),
        ),
    ]
}

/// Fitur terukur dengan penghematan terbesar (kunci label, token) untuk banner ringkasan.
#[must_use]
pub fn top_feature(saved: &HashMap<SavingFeature, i64>) -> Option<(String, String)> {
    saved
        .iter()
        .filter(|(_, v)| **v > 0)
        .max_by_key(|(f, v)| (**v, measured(**f)))
        .map(|(f, v)| {
            (
                format!("tokens-feat-{}", measured(*f).replace('_', "-")),
                short(*v),
            )
        })
}

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "Mei", "Jun", "Jul", "Agu", "Sep", "Okt", "Nov", "Des",
];

fn day_month(d: chrono::NaiveDate) -> String {
    use chrono::Datelike;
    format!("{} {}", d.day(), MONTHS[d.month0() as usize])
}

/// Grafik token per minggu (terpakai + dihemat, bertumpuk). Satuan sumbu menyesuaikan nilai
/// terbesar (rb/jt).
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn week_chart(weeks: &[Week]) -> String {
    let max = weeks.iter().map(|w| w.used + w.saved).max().unwrap_or(0);
    let (div, unit) = if max >= 1_000_000 {
        (1e6, " jt")
    } else {
        (1e3, " rb")
    };
    let labels: Vec<String> = weeks.iter().map(|w| day_month(w.start)).collect();
    let tips: Vec<String> = weeks
        .iter()
        .map(|w| {
            format!(
                "Minggu {}\nTerpakai: {} token\nDihemat: {} token",
                day_month(w.start),
                short(w.used),
                short(w.saved)
            )
        })
        .collect();
    charts::stacked_bars(
        "Token per minggu: terpakai dan dihemat",
        &labels,
        &[
            Stack {
                tone: "accent",
                soft: false,
                values: weeks.iter().map(|w| w.used as f64 / div).collect(),
            },
            Stack {
                tone: "accent",
                soft: true,
                values: weeks.iter().map(|w| w.saved as f64 / div).collect(),
            },
        ],
        &tips,
        unit,
        680.0,
    )
}

/// Satu baris batang per tiket: lebar trek relatif terhadap tiket terbesar, segmen dalam persen.
#[derive(Debug, Clone, Serialize)]
pub struct TicketBar {
    pub code: String,
    pub title: String,
    pub href: String,
    pub width: i64,
    pub input_pct: i64,
    pub output_pct: i64,
    pub cache_pct: i64,
    pub input: String,
    pub output: String,
    pub cache: String,
    pub total: String,
    pub cost: String,
    pub jobs: i64,
}

#[must_use]
pub fn ticket_bars(slug: &str, rows: &[TicketUsage]) -> Vec<TicketBar> {
    let max = rows.iter().map(|r| r.total).max().unwrap_or(0);
    rows.iter()
        .map(|r| {
            // Client yang hanya melapor total: seluruh batang dihitung input.
            let parts = r.input + r.output + r.cache;
            let (input, output, cache) = if parts == 0 {
                (r.total, 0, 0)
            } else {
                (r.input, r.output, r.cache)
            };
            let sum = (input + output + cache).max(1);
            TicketBar {
                code: r.code.clone(),
                title: r.title.clone(),
                href: ticket_href(slug, &r.code),
                width: percent(r.total, max).max(4),
                input_pct: percent(input, sum),
                output_pct: percent(output, sum),
                cache_pct: percent(cache, sum),
                input: short(input),
                output: short(output),
                cache: short(cache),
                total: short(r.total),
                cost: usd(r.cost_micros),
                jobs: r.jobs,
            }
        })
        .collect()
}

/// Satu baris tabel per job.
#[derive(Debug, Clone, Serialize)]
pub struct JobRow {
    pub id: String,
    pub short_id: String,
    pub href: String,
    pub code: Option<String>,
    pub ticket_href: Option<String>,
    pub persona: PersonaMark,
    pub client: String,
    pub running: bool,
    pub failed: bool,
    pub input: String,
    pub output: String,
    pub cache: String,
    pub saved: String,
    pub cost: String,
}

fn opt_short(v: Option<i64>) -> String {
    v.map_or_else(|| "–".into(), short)
}

#[must_use]
pub fn job_rows(slug: &str, rows: &[JobUsage], labels: &PersonaLabels) -> Vec<JobRow> {
    rows.iter()
        .map(|j| {
            let client = match (j.client.as_str(), j.model.as_str()) {
                ("", "") => String::new(),
                (c, "") => c.to_string(),
                ("", m) => m.to_string(),
                (c, m) => format!("{c} · {m}"),
            };
            let id = j.id.to_string();
            JobRow {
                short_id: id
                    .chars()
                    .rev()
                    .take(6)
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect(),
                href: format!("/w/{slug}/jobs/{id}"),
                id,
                ticket_href: j.code.as_deref().map(|c| ticket_href(slug, c)),
                code: j.code.clone(),
                persona: PersonaMark::of(&j.persona, labels),
                client,
                running: matches!(j.status.as_str(), "claimed" | "running"),
                failed: j.status == "failed",
                input: opt_short(j.input),
                output: opt_short(j.output),
                cache: opt_short(j.cache),
                saved: if j.saved > 0 {
                    short(j.saved)
                } else {
                    "–".into()
                },
                cost: j.cost_micros.map_or_else(|| "–".into(), usd),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_numbers_read_like_the_prototype() {
        assert_eq!(short(950), "950");
        assert_eq!(short(412_300), "412 rb");
        assert_eq!(short(4_800_000), "4,8 jt");
        assert_eq!(short(2_000_000), "2 jt");
        assert_eq!(short(-5), "0");
    }

    #[test]
    fn usd_uses_indonesian_separators() {
        assert_eq!(usd(41_200), "$0,04");
        assert_eq!(usd(12_300_000), "$12,30");
        assert_eq!(usd(1_204_000_000), "$1.204");
    }

    #[test]
    fn features_lock_required_and_mark_unmeasured() {
        let mut saved = HashMap::new();
        saved.insert(SavingFeature::CodeGraph, 1_900_000);
        let f = features(
            &saved,
            TokenSaver {
                concise: false,
                persona_models: true,
            },
        );
        assert_eq!(f.len(), 8);
        assert_eq!(f[0].saved.as_deref(), Some("1,9 jt"));
        assert!(f[1].saved.is_none(), "RTK tidak terukur server");
        assert!(
            f.iter()
                .filter(|x| x.required)
                .all(|x| x.on && x.field.is_none())
        );
        let concise = f.iter().find(|x| x.key == "concise").unwrap();
        assert!(!concise.on && !concise.required);
        assert_eq!(
            top_feature(&saved),
            Some(("tokens-feat-code-graph".into(), "1,9 jt".into()))
        );
    }

    #[test]
    fn ticket_bars_scale_to_largest() {
        let rows = vec![
            TicketUsage {
                code: "KSR-1".into(),
                title: "A".into(),
                input: 50,
                output: 25,
                cache: 25,
                total: 100,
                cost_micros: 0,
                jobs: 1,
            },
            TicketUsage {
                code: "KSR-2".into(),
                title: "B".into(),
                input: 0,
                output: 0,
                cache: 0,
                total: 50,
                cost_micros: 0,
                jobs: 1,
            },
        ];
        let bars = ticket_bars("acme", &rows);
        assert_eq!(bars[0].width, 100);
        assert_eq!(bars[0].input_pct, 50);
        assert_eq!(bars[1].width, 50);
        assert_eq!(bars[1].input_pct, 100);
    }
}
