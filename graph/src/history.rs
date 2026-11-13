//! Riwayat git → hotspot, coupling (file yang selalu berubah bersamaan), dan pemilik kode per
//! modul. Masukan = keluaran [`GIT_LOG_ARGS`].
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::build::module_of;

/// Jendela riwayat (hari).
pub const WINDOW_DAYS: u32 = 90;

/// Argumen `git log` yang keluarannya dibaca [`parse`].
pub const GIT_LOG_ARGS: &[&str] = &[
    "log",
    "--since=90.days",
    "--no-merges",
    "--format=@@%H%x09%an%x09%ae%x09%ct",
    "--numstat",
];

/// Commit dengan file sebanyak ini atau lebih (refactor massal, rename) tidak dihitung coupling.
const MAX_COUPLING_FILES: usize = 30;
const TOP: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hotspot {
    pub path: String,
    pub commits: u32,
    /// Baris ditambah + dihapus.
    pub churn: u32,
    pub top_author: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Coupling {
    pub a: String,
    pub b: String,
    pub together: u32,
    /// `together / min(commit a, commit b)` dalam persen.
    pub percent: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Owner {
    pub module: String,
    pub author: String,
    pub email: String,
    /// Persen commit modul oleh pemilik.
    pub percent: u32,
    pub commits: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitStats {
    pub window_days: u32,
    pub commits: u32,
    pub hotspots: Vec<Hotspot>,
    pub coupling: Vec<Coupling>,
    pub owners: Vec<Owner>,
}

struct Commit {
    author: String,
    email: String,
    files: Vec<(String, u32)>,
}

/// Parse keluaran `git log` (format [`GIT_LOG_ARGS`]). File yang tidak lolos `keep` (mis. bukan
/// kode) diabaikan.
#[must_use]
pub fn parse(log: &str, keep: impl Fn(&str) -> bool) -> GitStats {
    let mut commits: Vec<Commit> = Vec::new();
    for line in log.lines() {
        if let Some(h) = line.strip_prefix("@@") {
            let mut p = h.split('\t');
            let _sha = p.next();
            let author = p.next().unwrap_or("").to_string();
            let email = p.next().unwrap_or("").to_string();
            commits.push(Commit {
                author,
                email,
                files: Vec::new(),
            });
            continue;
        }
        let mut p = line.split('\t');
        let (Some(a), Some(d), Some(path)) = (p.next(), p.next(), p.next()) else {
            continue;
        };
        // Rename `a => b` / `{a => b}`: pakai path baru.
        let path = rename_target(path);
        if !keep(&path) {
            continue;
        }
        let churn = a.parse::<u32>().unwrap_or(0) + d.parse::<u32>().unwrap_or(0);
        if let Some(c) = commits.last_mut() {
            c.files.push((path, churn));
        }
    }
    let mut per_file: HashMap<&str, (u32, u32, HashMap<&str, u32>)> = HashMap::new();
    let mut per_module: HashMap<String, HashMap<(&str, &str), u32>> = HashMap::new();
    let mut pairs: HashMap<(&str, &str), u32> = HashMap::new();
    for c in &commits {
        let mut modules: Vec<String> = Vec::new();
        for (f, churn) in &c.files {
            let e = per_file.entry(f.as_str()).or_default();
            e.0 += 1;
            e.1 += churn;
            *e.2.entry(c.author.as_str()).or_default() += 1;
            let m = module_of(f);
            if !modules.contains(&m) {
                modules.push(m);
            }
        }
        for m in modules {
            *per_module
                .entry(m)
                .or_default()
                .entry((c.author.as_str(), c.email.as_str()))
                .or_default() += 1;
        }
        if c.files.len() < MAX_COUPLING_FILES {
            let mut fs: Vec<&str> = c.files.iter().map(|(f, _)| f.as_str()).collect();
            fs.sort_unstable();
            fs.dedup();
            for i in 0..fs.len() {
                for j in i + 1..fs.len() {
                    *pairs.entry((fs[i], fs[j])).or_default() += 1;
                }
            }
        }
    }
    let mut hotspots: Vec<Hotspot> = per_file
        .iter()
        .map(|(p, (n, churn, authors))| Hotspot {
            path: (*p).to_string(),
            commits: *n,
            churn: *churn,
            top_author: authors
                .iter()
                .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))
                .map(|(a, _)| (*a).to_string())
                .unwrap_or_default(),
        })
        .collect();
    hotspots.sort_by(|a, b| {
        b.commits
            .cmp(&a.commits)
            .then(b.churn.cmp(&a.churn))
            .then(a.path.cmp(&b.path))
    });
    hotspots.truncate(TOP);
    let mut coupling: Vec<Coupling> = pairs
        .into_iter()
        .filter(|(_, n)| *n >= 3)
        .map(|((a, b), n)| {
            let min = per_file
                .get(a)
                .map_or(1, |x| x.0)
                .min(per_file.get(b).map_or(1, |x| x.0))
                .max(1);
            Coupling {
                a: a.to_string(),
                b: b.to_string(),
                together: n,
                percent: n * 100 / min,
            }
        })
        .collect();
    coupling.sort_by(|x, y| {
        y.percent
            .cmp(&x.percent)
            .then(y.together.cmp(&x.together))
            .then(x.a.cmp(&y.a))
    });
    coupling.truncate(TOP);
    let mut owners: Vec<Owner> = per_module
        .into_iter()
        .filter_map(|(m, authors)| {
            let total: u32 = authors.values().sum();
            let ((author, email), n) = authors
                .into_iter()
                .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))?;
            Some(Owner {
                module: m,
                author: author.to_string(),
                email: email.to_string(),
                percent: n * 100 / total.max(1),
                commits: total,
            })
        })
        .collect();
    owners.sort_by(|a, b| b.commits.cmp(&a.commits).then(a.module.cmp(&b.module)));
    owners.truncate(TOP);
    GitStats {
        window_days: WINDOW_DAYS,
        commits: u32::try_from(commits.len()).unwrap_or(u32::MAX),
        hotspots,
        coupling,
        owners,
    }
}

fn rename_target(path: &str) -> String {
    if let (Some(o), Some(c)) = (path.find('{'), path.find('}')) {
        let inner = &path[o + 1..c];
        let new = inner.split(" => ").nth(1).unwrap_or(inner);
        let joined = format!("{}{}{}", &path[..o], new, &path[c + 1..]);
        return joined.replace("//", "/");
    }
    path.split(" => ").last().unwrap_or(path).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotspots_coupling_owners() {
        let mut log = String::new();
        for i in 0..4 {
            log.push_str(&format!(
                "@@{i}\tDewi\tdewi@x.id\t1\n\n10\t2\tsrc/cart/promo.ts\n1\t1\tsrc/cart/totals.ts\n"
            ));
        }
        log.push_str("@@9\tAdi\tadi@x.id\t1\n\n5\t0\tsrc/cart/promo.ts\n-\t-\tlogo.png\n3\t3\tsrc/{old => cart}/rename.ts\n");
        let s = parse(&log, |p| !p.ends_with(".png"));
        assert_eq!(s.commits, 5);
        assert_eq!(
            s.hotspots[0],
            Hotspot {
                path: "src/cart/promo.ts".into(),
                commits: 5,
                churn: 53,
                top_author: "Dewi".into()
            }
        );
        assert!(s.hotspots.iter().any(|h| h.path == "src/cart/rename.ts"));
        assert_eq!(
            s.coupling,
            vec![Coupling {
                a: "src/cart/promo.ts".into(),
                b: "src/cart/totals.ts".into(),
                together: 4,
                percent: 100
            }]
        );
        assert_eq!(
            s.owners,
            vec![Owner {
                module: "cart".into(),
                author: "Dewi".into(),
                email: "dewi@x.id".into(),
                percent: 80,
                commits: 5
            }]
        );
    }
}
