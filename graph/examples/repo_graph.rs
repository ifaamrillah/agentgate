//! Benchmark code graph pada repo nyata (dipakai `docs/benchmarks/2026-11-13-code-graph.md`):
//!
//! ```text
//! cargo run --release -p agentgate-graph --example repo_graph -- <root> [nama_fungsi ...]
//! ```
//!
//! Mencetak waktu walk/parse/build, jumlah file/simbol/relasi, porsi relasi pasti, lalu untuk
//! tiap nama fungsi: pemanggil langsung (`path:qualified`, keyakinan).
use std::{path::PathBuf, time::Instant};

use agentgate_graph::{
    analysis::{Adjacency, Direction},
    build::{self, SourceFile},
    extract,
};

fn main() {
    let mut args = std::env::args().skip(1);
    let root = PathBuf::from(args.next().unwrap_or_else(|| ".".into()));
    let names: Vec<String> = args.collect();

    let t = Instant::now();
    let entries = agentgate_graph::walk::walk(&root);
    let walk_ms = t.elapsed().as_millis();

    let t = Instant::now();
    let mut files = Vec::new();
    let mut failed = 0;
    for e in &entries {
        let Some(content) = &e.content else { continue };
        match extract::extract(&e.path, content) {
            Ok(facts) => files.push(SourceFile {
                path: e.path.clone(),
                facts,
            }),
            Err(_) => failed += 1,
        }
    }
    let parse_ms = t.elapsed().as_millis();

    let t = Instant::now();
    let g = build::build(&files, None);
    let build_ms = t.elapsed().as_millis();

    let symbols = g.nodes.iter().filter(|n| n.symbol.is_some()).count();
    let s = g.stats;
    println!(
        "files={} parsed={} failed={failed} symbols={symbols} edges={} calls={} certain={} inferred={} external={} ambiguous={}",
        entries.len(),
        files.len(),
        g.edges.len(),
        s.calls,
        s.certain,
        s.inferred,
        s.external,
        s.ambiguous
    );
    println!("walk_ms={walk_ms} parse_ms={parse_ms} build_ms={build_ms}");

    let adj = Adjacency::of(&g);
    for name in &names {
        let targets: Vec<u32> = g
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.name == *name && n.symbol.is_some())
            .filter_map(|(i, _)| u32::try_from(i).ok())
            .collect();
        println!("## {name} ({} definisi)", targets.len());
        for t in targets {
            let def = &g.nodes[t as usize];
            println!("  def {}:{}", files[def.file as usize].path, def.qualified);
            for (c, conf) in adj.neighbours(t, Direction::Callers) {
                let n = &g.nodes[*c as usize];
                println!(
                    "    <- {}:{} ({conf:?})",
                    files[n.file as usize].path, n.qualified
                );
            }
        }
    }
}
