//! Analisis di atas [`RepoGraph`] di memori: trace pemanggil/dipanggil, dampak dari simbol atau
//! unified diff. Server memakai versi SQL yang sama bentuknya; ini dipakai benchmark, query, dan tes.
use std::collections::{HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::build::{Confidence, EdgeKind, RepoGraph, SourceFile};

/// Arah trace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Callers,
    Callees,
}

/// Satu langkah hasil trace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hop {
    pub node: u32,
    pub depth: u8,
    /// Keyakinan terlemah sepanjang jalur.
    pub confidence: Confidence,
}

/// Adjacency call (`calls` + `handles`).
#[derive(Debug, Clone, Default)]
pub struct Adjacency {
    out: HashMap<u32, Vec<(u32, Confidence)>>,
    inc: HashMap<u32, Vec<(u32, Confidence)>>,
}

impl Adjacency {
    #[must_use]
    pub fn of(g: &RepoGraph) -> Self {
        let mut a = Self::default();
        for e in &g.edges {
            if matches!(e.kind, EdgeKind::Calls | EdgeKind::Handles) && e.from != e.to {
                a.out.entry(e.from).or_default().push((e.to, e.confidence));
                a.inc.entry(e.to).or_default().push((e.from, e.confidence));
            }
        }
        a
    }

    #[must_use]
    pub fn neighbours(&self, n: u32, dir: Direction) -> &[(u32, Confidence)] {
        let m = match dir {
            Direction::Callers => &self.inc,
            Direction::Callees => &self.out,
        };
        m.get(&n).map_or(&[], Vec::as_slice)
    }

    /// BFS dari `start` sampai `depth` (maks. `limit` node).
    #[must_use]
    pub fn trace(&self, start: &[u32], dir: Direction, depth: u8, limit: usize) -> Vec<Hop> {
        let mut seen: HashSet<u32> = start.iter().copied().collect();
        let mut q: VecDeque<(u32, u8, Confidence)> =
            start.iter().map(|&s| (s, 0, Confidence::Certain)).collect();
        let mut out = Vec::new();
        while let Some((n, d, c)) = q.pop_front() {
            if d >= depth {
                continue;
            }
            for &(m, ec) in self.neighbours(n, dir) {
                if out.len() >= limit {
                    return out;
                }
                if seen.insert(m) {
                    let conf = if c == Confidence::Certain && ec == Confidence::Certain {
                        Confidence::Certain
                    } else {
                        Confidence::Inferred
                    };
                    out.push(Hop {
                        node: m,
                        depth: d + 1,
                        confidence: conf,
                    });
                    q.push_back((m, d + 1, conf));
                }
            }
        }
        out
    }
}

/// Rentang baris yang berubah per file (sisi baru; file dihapus memakai sisi lama).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileChange {
    pub path: String,
    pub ranges: Vec<(u32, u32)>,
    pub deleted: bool,
}

/// Parse unified diff (`git diff`).
#[must_use]
pub fn parse_diff(diff: &str) -> Vec<FileChange> {
    let mut out: Vec<FileChange> = Vec::new();
    let mut old_path = String::new();
    for line in diff.lines() {
        if let Some(p) = line.strip_prefix("--- ") {
            old_path = p.trim().trim_start_matches("a/").to_string();
        } else if let Some(p) = line.strip_prefix("+++ ") {
            let p = p.trim();
            let (path, deleted) = if p == "/dev/null" {
                (old_path.clone(), true)
            } else {
                (p.trim_start_matches("b/").to_string(), false)
            };
            out.push(FileChange {
                path,
                ranges: Vec::new(),
                deleted,
            });
        } else if let Some(h) = line.strip_prefix("@@ ")
            && let Some(cur) = out.last_mut()
        {
            // @@ -a,b +c,d @@
            let mut parts = h.split_whitespace();
            let old = parts.next().unwrap_or("");
            let new = parts.next().unwrap_or("");
            let spec = if cur.deleted {
                old.trim_start_matches('-')
            } else {
                new.trim_start_matches('+')
            };
            let (start, len) = spec.split_once(',').map_or((spec, "1"), |(a, b)| (a, b));
            let start: u32 = start.parse().unwrap_or(0);
            let len: u32 = len.parse().unwrap_or(1);
            let end = start + len.saturating_sub(1);
            cur.ranges.push((start.max(1), end.max(start.max(1))));
        }
    }
    out
}

/// Simbol (indeks node) yang rentangnya bersinggungan dengan perubahan.
#[must_use]
pub fn changed_nodes(g: &RepoGraph, files: &[SourceFile], changes: &[FileChange]) -> Vec<u32> {
    let file_ix: HashMap<&str, u32> = files
        .iter()
        .enumerate()
        .map(|(i, f)| (f.path.as_str(), i as u32))
        .collect();
    let mut out = Vec::new();
    for c in changes {
        let Some(&fi) = file_ix.get(c.path.as_str()) else {
            continue;
        };
        // Simbol terdalam per rentang.
        for &(a, b) in &c.ranges {
            let hit = g
                .nodes
                .iter()
                .enumerate()
                .filter(|(_, n)| {
                    n.file == fi
                        && n.symbol.is_some()
                        && n.kind.is_callable()
                        && n.start <= b
                        && a <= n.end
                })
                .map(|(i, _)| i as u32);
            for h in hit {
                if !out.contains(&h) {
                    out.push(h);
                }
            }
        }
    }
    out
}

/// Ringkasan dampak.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Impact {
    pub changed: Vec<u32>,
    pub affected: Vec<Hop>,
    pub tests: Vec<u32>,
}

/// Dampak perubahan `changed`: pemanggil transitif (kedalaman ≤ `depth`), dipisah tes.
#[must_use]
pub fn impact(g: &RepoGraph, adj: &Adjacency, changed: &[u32], depth: u8) -> Impact {
    let hops = adj.trace(changed, Direction::Callers, depth, 500);
    let (tests, affected): (Vec<Hop>, Vec<Hop>) = hops
        .into_iter()
        .partition(|h| g.nodes[h.node as usize].test);
    Impact {
        changed: changed.to_vec(),
        affected,
        tests: tests.into_iter().map(|h| h.node).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_ranges() {
        let d = "diff --git a/src/a.ts b/src/a.ts\n--- a/src/a.ts\n+++ b/src/a.ts\n@@ -10,2 +10,3 @@ fn\n+x\n@@ -40 +41 @@\n-y\n+z\ndiff --git a/old.go b/old.go\n--- a/old.go\n+++ /dev/null\n@@ -1,5 +0,0 @@\n";
        let c = parse_diff(d);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].path, "src/a.ts");
        assert_eq!(c[0].ranges, vec![(10, 12), (41, 41)]);
        assert_eq!(
            (c[1].path.as_str(), c[1].deleted, c[1].ranges.clone()),
            ("old.go", true, vec![(1, 5)])
        );
    }
}
