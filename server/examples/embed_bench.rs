//! Benchmark embedding lokal code graph (R2): waktu muat/unduh model, throughput embed simbol,
//! dan latensi embed query. Simbol diambil dari repo ini sendiri (walk + tree-sitter, teks sama
//! dengan `services::graph::embed::symbol_text`). Hasil: docs/benchmarks/2026-11-14-embedding.md.
//!
//! ```sh
//! ORT_DYLIB_PATH=/workspace/tmp/ort/onnxruntime-linux-x64-1.24.1/lib/libonnxruntime.so.1 \
//!   cargo run --release -p agentgate-server --features semantic --example embed_bench -- \
//!   multilingual-e5-small 2000
//! ```
//! Argumen: MODEL (kunci `graph.embedding.model`) dan jumlah simbol maks (default 2000).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::cast_precision_loss)]

use std::{path::Path, time::Instant};

use agentgate_server::{common::settings::EmbeddingSettings, services::graph::embed};

fn pct(sorted: &[f64], p: f64) -> f64 {
    sorted[((sorted.len() as f64 - 1.0) * p).round() as usize]
}

fn main() {
    let model = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "multilingual-e5-small".into());
    let max: usize = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(2000);
    let settings = EmbeddingSettings {
        model: model.clone(),
        cache_dir: "/workspace/tmp/agentgate-models".into(),
        ..EmbeddingSettings::default()
    };
    let cached = settings.dir().exists();

    let t = Instant::now();
    let e = embed::load_blocking(&settings).expect("model dimuat");
    let load = t.elapsed().as_secs_f64();

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let mut texts = Vec::new();
    'outer: for entry in agentgate_graph::walk::walk(root) {
        let Some(src) = entry.content else { continue };
        let Ok(facts) = agentgate_graph::extract(&entry.path, &src) else {
            continue;
        };
        let lines: Vec<&str> = src.lines().collect();
        for s in facts.symbols {
            let start = s.start.saturating_sub(1) as usize;
            let end = (s.end as usize)
                .min(start + embed::CODE_LINES as usize)
                .min(lines.len());
            let code = lines
                .get(start..end)
                .map(|l| l.join("\n"))
                .unwrap_or_default();
            texts.push(embed::symbol_text(
                s.kind.as_str(),
                &s.name,
                &s.signature,
                &s.doc,
                &entry.path,
                &code
                    .chars()
                    .take(embed::CODE_MAX as usize)
                    .collect::<String>(),
            ));
            if texts.len() >= max {
                break 'outer;
            }
        }
    }
    texts.sort_by_key(String::len);
    let t = Instant::now();
    let vectors = e.passages(&texts, 64).expect("embed");
    let index = t.elapsed().as_secs_f64();
    assert_eq!(vectors.len(), texts.len());

    let queries = [
        "potong stok saat checkout",
        "verifikasi tanda tangan webhook",
        "hitung ulang relasi lintas repo",
        "kirim email undangan anggota",
        "token instalasi github app",
        "parse git log jadi hotspot",
        "rate limit login per ip",
        "buat pull request knowledge",
        "refund order supervisor",
        "encrypt secret totp",
    ];
    let mut lat = Vec::new();
    for _ in 0..5 {
        for q in queries {
            let t = Instant::now();
            e.query(q).unwrap();
            lat.push(t.elapsed().as_secs_f64() * 1000.0);
        }
    }
    lat.sort_by(f64::total_cmp);
    println!(
        "model={model} dim={} cached_before={cached}",
        vectors[0].len()
    );
    let avg = texts.iter().map(String::len).sum::<usize>() / texts.len();
    println!("load_s={load:.2} avg_chars={avg}");
    println!(
        "symbols={} index_s={index:.2} per_symbol_ms={:.2} symbols_per_s={:.1}",
        texts.len(),
        index * 1000.0 / texts.len() as f64,
        texts.len() as f64 / index
    );
    // Contoh kualitas: 3 simbol teratas per query (kosinus; vektor sudah dinormalisasi model).
    for q in queries {
        let qv = e.query(q).unwrap();
        let mut scored: Vec<(f32, &str)> = vectors
            .iter()
            .zip(&texts)
            .map(|(v, t)| {
                (
                    v.iter().zip(&qv).map(|(a, b)| a * b).sum::<f32>(),
                    t.lines().next().unwrap_or(""),
                )
            })
            .collect();
        scored.sort_by(|a, b| b.0.total_cmp(&a.0));
        let top: Vec<String> = scored
            .iter()
            .take(3)
            .map(|(_, n)| (*n).to_string())
            .collect();
        println!("top \"{q}\": {}", top.join(" | "));
    }
    println!(
        "query_ms p50={:.1} p95={:.1} max={:.1}",
        pct(&lat, 0.5),
        pct(&lat, 0.95),
        lat[lat.len() - 1]
    );
}
