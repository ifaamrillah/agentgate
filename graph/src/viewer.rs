//! Model graph viewer (M10): node + relasi satu level zoom (Project → Repo → Modul → File →
//! Fungsi), tebakan layer node dari path, pemangkasan ke node berderajat tertinggi, layout
//! berlapis kiri→kanan (rank longest-path, urut barycenter), dan analisis dampak (penutupan
//! pemanggil). Murni tanpa I/O supaya server (query) dan view (SVG) memakai bentuk yang sama.
use std::collections::{HashMap, HashSet, VecDeque};

use serde::Serialize;

/// Lebar dan tinggi kotak node (px, koordinat SVG).
pub const NODE_W: f64 = 164.0;
pub const NODE_H: f64 = 50.0;
const COL_GAP: f64 = 76.0;
const ROW_GAP: f64 = 30.0;
const PAD: f64 = 32.0;
/// Tinggi kolom maksimal sebelum dipecah menjadi kolom tambahan.
const COL_MAX: usize = 7;
/// Batas node per level (sisanya diringkas "+N lainnya").
pub const NODES_MAX: usize = 40;

/// Level zoom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Project,
    Repo,
    Module,
    File,
    Symbol,
}

impl Level {
    pub const ALL: [Self; 5] = [
        Self::Project,
        Self::Repo,
        Self::Module,
        Self::File,
        Self::Symbol,
    ];

    #[must_use]
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|l| *l == self).unwrap_or(0)
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Repo => "repo",
            Self::Module => "module",
            Self::File => "file",
            Self::Symbol => "symbol",
        }
    }
}

/// Layer arsitektur node (warna garis kiri node dan filter).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Layer {
    Ui,
    Logic,
    Api,
    Data,
    Test,
    Ext,
    Doc,
}

impl Layer {
    pub const ALL: [Self; 7] = [
        Self::Ui,
        Self::Logic,
        Self::Api,
        Self::Data,
        Self::Test,
        Self::Ext,
        Self::Doc,
    ];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ui => "ui",
            Self::Logic => "logic",
            Self::Api => "api",
            Self::Data => "data",
            Self::Test => "test",
            Self::Ext => "ext",
            Self::Doc => "doc",
        }
    }

    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|l| l.as_str() == s)
    }
}

const UI_DIRS: &[&str] = &[
    "components",
    "component",
    "pages",
    "page",
    "views",
    "view",
    "ui",
    "screens",
    "screen",
    "templates",
    "layouts",
    "widgets",
    "frontend",
    "web",
    "app",
    "styles",
];
const API_DIRS: &[&str] = &[
    "api",
    "routes",
    "router",
    "routers",
    "controllers",
    "controller",
    "handlers",
    "handler",
    "http",
    "server",
    "endpoints",
    "rest",
    "graphql",
    "grpc",
    "backend",
];
const DATA_DIRS: &[&str] = &[
    "models",
    "model",
    "db",
    "database",
    "migrations",
    "migration",
    "repository",
    "repositories",
    "entities",
    "entity",
    "schema",
    "schemas",
    "dao",
    "store",
    "stores",
    "sql",
];
const UI_EXT: &[&str] = &[
    ".tsx", ".jsx", ".vue", ".svelte", ".html", ".css", ".scss", ".dart",
];

fn segments(path: &str) -> Vec<String> {
    path.to_lowercase()
        .split(['/', '\\', '-', '_', '.'])
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// Tebak layer dari path (file/modul/nama repo). `test` = simbol/file ditandai test oleh index.
#[must_use]
pub fn guess_layer(path: &str, test: bool) -> Layer {
    let lower = path.to_lowercase();
    let segs = segments(path);
    let has = |dirs: &[&str]| segs.iter().any(|s| dirs.contains(&s.as_str()));
    if test
        || has(&["test", "tests", "spec", "specs", "__tests__", "e2e"])
        || lower.contains(".test.")
        || lower.contains(".spec.")
        || lower.ends_with("_test.go")
    {
        Layer::Test
    } else if lower.ends_with(".sql") || has(DATA_DIRS) {
        Layer::Data
    } else if has(API_DIRS) {
        Layer::Api
    } else if UI_EXT.iter().any(|e| lower.ends_with(e)) || has(UI_DIRS) {
        Layer::Ui
    } else {
        Layer::Logic
    }
}

/// Satu node.
#[derive(Debug, Clone, Serialize)]
pub struct Node {
    /// Unik di level: `r:<repo>`, `m:<modul>`, `f:<path>`, `s:<uuid>`, `d:<slug>`, `x:<n>`.
    pub id: String,
    pub label: String,
    pub sub: String,
    pub layer: Layer,
    /// Bahasa (kosong untuk dokumen/eksternal).
    pub lang: String,
    /// Kunci Fluent jenis node (`gv-kind-repo`, …).
    pub kind_key: String,
    pub path: String,
    /// Node di luar lingkup level (tetangga), tampil pucat.
    pub out: bool,
    pub dead: bool,
    /// Bobot untuk pemangkasan (derajat atau ukuran).
    pub weight: i64,
    /// Query string tujuan zoom masuk (tanpa `?`); `None` = level terdalam.
    pub drill: Option<String>,
    /// Id simbol (halaman Lihat kode).
    pub symbol: Option<String>,
    pub x: f64,
    pub y: f64,
}

impl Node {
    #[must_use]
    pub fn new(id: impl Into<String>, label: impl Into<String>, layer: Layer) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            sub: String::new(),
            layer,
            lang: String::new(),
            kind_key: String::new(),
            path: String::new(),
            out: false,
            dead: false,
            weight: 0,
            drill: None,
            symbol: None,
            x: 0.0,
            y: 0.0,
        }
    }
}

/// Satu relasi berarah (`from` memanggil/memakai `to`).
#[derive(Debug, Clone, Serialize)]
pub struct Edge {
    pub from: String,
    pub to: String,
    /// `false` = dugaan (garis putus).
    pub certain: bool,
    pub label: String,
}

/// Graph satu level setelah pemangkasan dan layout.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    /// Node yang tidak ditampilkan karena batas.
    pub more: usize,
    pub width: f64,
    pub height: f64,
}

impl Graph {
    /// Gabungkan relasi kembar (dari, ke) dan buang relasi ke diri sendiri / node tak dikenal.
    #[must_use]
    pub fn new(nodes: Vec<Node>, edges: Vec<Edge>) -> Self {
        let ids: HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
        let mut seen: HashMap<(String, String), usize> = HashMap::new();
        let mut out: Vec<Edge> = Vec::new();
        for e in edges {
            if e.from == e.to || !ids.contains(e.from.as_str()) || !ids.contains(e.to.as_str()) {
                continue;
            }
            match seen.get(&(e.from.clone(), e.to.clone())) {
                Some(&i) => out[i].certain |= e.certain,
                None => {
                    seen.insert((e.from.clone(), e.to.clone()), out.len());
                    out.push(e);
                }
            }
        }
        Self {
            nodes,
            edges: out,
            ..Self::default()
        }
    }

    /// Simpan paling banyak `max` node: `keep` (node terpilih) selalu, lalu bobot + derajat
    /// tertinggi.
    pub fn prune(&mut self, max: usize, keep: &[&str]) {
        if self.nodes.len() <= max {
            return;
        }
        let mut degree: HashMap<&str, i64> = HashMap::new();
        for e in &self.edges {
            *degree.entry(e.from.as_str()).or_default() += 1;
            *degree.entry(e.to.as_str()).or_default() += 1;
        }
        let mut order: Vec<(bool, i64, String)> = self
            .nodes
            .iter()
            .map(|n| {
                (
                    keep.contains(&n.id.as_str()),
                    n.weight + degree.get(n.id.as_str()).copied().unwrap_or(0),
                    n.id.clone(),
                )
            })
            .collect();
        order.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)).then(a.2.cmp(&b.2)));
        let kept: HashSet<String> = order.into_iter().take(max).map(|o| o.2).collect();
        self.more += self.nodes.len() - kept.len();
        self.nodes.retain(|n| kept.contains(&n.id));
        self.edges
            .retain(|e| kept.contains(&e.from) && kept.contains(&e.to));
    }

    /// Buang node yang layer/bahasanya disembunyikan filter.
    pub fn filter(&mut self, hidden_layers: &[Layer], hidden_langs: &[String]) {
        self.nodes
            .retain(|n| !hidden_layers.contains(&n.layer) && !hidden_langs.contains(&n.lang));
        let ids: HashSet<String> = self.nodes.iter().map(|n| n.id.clone()).collect();
        self.edges
            .retain(|e| ids.contains(&e.from) && ids.contains(&e.to));
    }

    #[must_use]
    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Node yang memanggil/memakai `id` (relasi masuk).
    #[must_use]
    pub fn incoming(&self, id: &str) -> Vec<(&Node, bool)> {
        self.edges
            .iter()
            .filter(|e| e.to == id)
            .filter_map(|e| self.node(&e.from).map(|n| (n, e.certain)))
            .collect()
    }

    /// Node yang dipanggil/dipakai `id` (relasi keluar).
    #[must_use]
    pub fn outgoing(&self, id: &str) -> Vec<(&Node, bool)> {
        self.edges
            .iter()
            .filter(|e| e.from == id)
            .filter_map(|e| self.node(&e.to).map(|n| (n, e.certain)))
            .collect()
    }

    /// Analisis dampak: semua node yang (langsung/tidak langsung) bergantung pada `id`
    /// (penutupan pemanggil), tanpa `id` sendiri.
    #[must_use]
    pub fn impact(&self, id: &str) -> HashSet<String> {
        let mut seen: HashSet<String> = HashSet::new();
        let mut queue: VecDeque<&str> = VecDeque::from([id]);
        while let Some(cur) = queue.pop_front() {
            for e in self.edges.iter().filter(|e| e.to == cur) {
                if e.from != id && seen.insert(e.from.clone()) {
                    queue.push_back(e.from.as_str());
                }
            }
        }
        seen
    }

    /// Layout berlapis kiri→kanan: rank = jalur terpanjang dari sumber (siklus diputus urutan
    /// DFS), urutan dalam kolom = barycenter pendahulu (dua sapuan), kolom tinggi dipecah.
    #[allow(clippy::cast_precision_loss)]
    pub fn layout(&mut self) {
        let n = self.nodes.len();
        if n == 0 {
            self.width = NODE_W + 2.0 * PAD;
            self.height = NODE_H + 2.0 * PAD;
            return;
        }
        let index: HashMap<&str, usize> = self
            .nodes
            .iter()
            .enumerate()
            .map(|(i, nd)| (nd.id.as_str(), i))
            .collect();
        let mut succ: Vec<Vec<usize>> = vec![Vec::new(); n];
        for e in &self.edges {
            if let (Some(&a), Some(&b)) = (index.get(e.from.as_str()), index.get(e.to.as_str())) {
                succ[a].push(b);
            }
        }
        // Buang relasi balik (siklus) lewat DFS berurutan.
        let mut state = vec![0u8; n];
        let mut dag: Vec<Vec<usize>> = vec![Vec::new(); n];
        for start in 0..n {
            if state[start] != 0 {
                continue;
            }
            let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
            state[start] = 1;
            while let Some(&mut (v, ref mut i)) = stack.last_mut() {
                if *i < succ[v].len() {
                    let w = succ[v][*i];
                    *i += 1;
                    match state[w] {
                        0 => {
                            dag[v].push(w);
                            state[w] = 1;
                            stack.push((w, 0));
                        }
                        2 => dag[v].push(w),
                        _ => {} // relasi balik
                    }
                } else {
                    state[v] = 2;
                    stack.pop();
                }
            }
        }
        // Rank jalur terpanjang (urutan topologis Kahn).
        let mut indeg = vec![0usize; n];
        for outs in &dag {
            for &w in outs {
                indeg[w] += 1;
            }
        }
        let mut rank = vec![0usize; n];
        let mut queue: VecDeque<usize> = (0..n).filter(|&v| indeg[v] == 0).collect();
        while let Some(v) = queue.pop_front() {
            for &w in &dag[v] {
                rank[w] = rank[w].max(rank[v] + 1);
                indeg[w] -= 1;
                if indeg[w] == 0 {
                    queue.push_back(w);
                }
            }
        }
        let ranks = rank.iter().max().copied().unwrap_or(0) + 1;
        let mut cols: Vec<Vec<usize>> = vec![Vec::new(); ranks];
        for v in 0..n {
            cols[rank[v]].push(v);
        }
        let mut pred: Vec<Vec<usize>> = vec![Vec::new(); n];
        for (v, outs) in dag.iter().enumerate() {
            for &w in outs {
                pred[w].push(v);
            }
        }
        let mut pos = vec![0.0f64; n];
        for col in &cols {
            for (i, &v) in col.iter().enumerate() {
                pos[v] = i as f64;
            }
        }
        for _ in 0..2 {
            for col in cols.iter_mut().skip(1) {
                let bary = |v: usize| -> f64 {
                    if pred[v].is_empty() {
                        pos[v]
                    } else {
                        pred[v].iter().map(|&p| pos[p]).sum::<f64>() / pred[v].len() as f64
                    }
                };
                let mut keyed: Vec<(f64, usize)> = col.iter().map(|&v| (bary(v), v)).collect();
                keyed.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
                *col = keyed.into_iter().map(|k| k.1).collect();
                for (i, &v) in col.iter().enumerate() {
                    pos[v] = i as f64;
                }
            }
        }
        // Pecah kolom tinggi; kolom dipusatkan vertikal.
        let mut slots: Vec<Vec<usize>> = Vec::new();
        for col in cols {
            for chunk in col.chunks(COL_MAX) {
                slots.push(chunk.to_vec());
            }
        }
        let tallest = slots.iter().map(Vec::len).max().unwrap_or(1);
        let full = tallest as f64 * (NODE_H + ROW_GAP) - ROW_GAP;
        for (c, slot) in slots.iter().enumerate() {
            let h = slot.len() as f64 * (NODE_H + ROW_GAP) - ROW_GAP;
            let top = PAD + (full - h) / 2.0;
            for (i, &v) in slot.iter().enumerate() {
                self.nodes[v].x = PAD + c as f64 * (NODE_W + COL_GAP) + NODE_W / 2.0;
                self.nodes[v].y = top + i as f64 * (NODE_H + ROW_GAP) + NODE_H / 2.0;
            }
        }
        self.width = 2.0 * PAD + slots.len() as f64 * (NODE_W + COL_GAP) - COL_GAP;
        self.height = 2.0 * PAD + full;
    }
}

/// Jalur kurva relasi `a → b` (dari sisi kanan ke kiri bila maju, lengkung bawah bila mundur
/// atau satu kolom) dan titik label.
#[must_use]
pub fn edge_path(a: &Node, b: &Node) -> (String, f64, f64) {
    let (hw, hh) = (NODE_W / 2.0, NODE_H / 2.0);
    if b.x - a.x > NODE_W / 2.0 {
        let (x1, y1, x2, y2) = (a.x + hw, a.y, b.x - hw - 4.0, b.y);
        let mx = (x1 + x2) / 2.0;
        (
            format!("M{x1:.1},{y1:.1} C{mx:.1},{y1:.1} {mx:.1},{y2:.1} {x2:.1},{y2:.1}"),
            mx,
            (y1 + y2) / 2.0,
        )
    } else if a.x - b.x > NODE_W / 2.0 {
        let (x1, y1, x2, y2) = (a.x - hw, a.y, b.x + hw + 4.0, b.y);
        let mx = (x1 + x2) / 2.0;
        (
            format!("M{x1:.1},{y1:.1} C{mx:.1},{y1:.1} {mx:.1},{y2:.1} {x2:.1},{y2:.1}"),
            mx,
            (y1 + y2) / 2.0,
        )
    } else {
        // Satu kolom: dari bawah/atas node, melengkung ke kanan.
        let down = b.y > a.y;
        let (y1, y2) = if down {
            (a.y + hh, b.y - hh - 4.0)
        } else {
            (a.y - hh, b.y + hh + 4.0)
        };
        let bend = a.x + hw * 0.9;
        (
            format!(
                "M{x1:.1},{y1:.1} C{bend:.1},{y1:.1} {bend:.1},{y2:.1} {x2:.1},{y2:.1}",
                x1 = a.x + 20.0,
                x2 = b.x + 20.0,
            ),
            bend,
            (y1 + y2) / 2.0,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(ids: &[&str], edges: &[(&str, &str)]) -> Graph {
        Graph::new(
            ids.iter()
                .map(|i| Node::new(*i, *i, Layer::Logic))
                .collect(),
            edges
                .iter()
                .map(|(a, b)| Edge {
                    from: (*a).into(),
                    to: (*b).into(),
                    certain: true,
                    label: String::new(),
                })
                .collect(),
        )
    }

    #[test]
    fn layers_are_guessed_from_paths() {
        assert_eq!(
            guess_layer("src/features/cart/CartPage.tsx", false),
            Layer::Ui
        );
        assert_eq!(guess_layer("internal/handlers/order.go", false), Layer::Api);
        assert_eq!(guess_layer("migrations/001_init.sql", false), Layer::Data);
        assert_eq!(guess_layer("src/cart/promo.test.ts", false), Layer::Test);
        assert_eq!(guess_layer("src/cart/promo.ts", false), Layer::Logic);
        assert_eq!(guess_layer("src/cart/promo.ts", true), Layer::Test);
    }

    #[test]
    fn layout_flows_left_to_right_and_survives_cycles() {
        let mut graph = g(
            &["a", "b", "c"],
            &[("a", "b"), ("b", "c"), ("c", "a"), ("a", "a")],
        );
        assert_eq!(graph.edges.len(), 3, "relasi ke diri sendiri dibuang");
        graph.layout();
        let x = |id: &str| graph.node(id).unwrap().x;
        assert!(x("a") < x("b") && x("b") < x("c"));
        assert!(graph.width > 3.0 * NODE_W);
    }

    #[test]
    fn tall_columns_split_and_nodes_never_overlap() {
        let ids: Vec<String> = (0..20).map(|i| format!("n{i}")).collect();
        let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        let mut graph = g(&refs, &[]);
        graph.layout();
        for a in &graph.nodes {
            for b in &graph.nodes {
                if a.id != b.id {
                    assert!(
                        (a.x - b.x).abs() >= NODE_W || (a.y - b.y).abs() >= NODE_H,
                        "{} tumpang tindih {}",
                        a.id,
                        b.id
                    );
                }
            }
        }
    }

    #[test]
    fn impact_is_the_caller_closure() {
        let graph = g(
            &["ui", "svc", "repo", "other"],
            &[("ui", "svc"), ("svc", "repo"), ("other", "ui")],
        );
        let hit = graph.impact("repo");
        assert_eq!(
            hit,
            ["svc", "ui", "other"]
                .iter()
                .map(|s| (*s).to_string())
                .collect()
        );
        assert!(graph.impact("other").is_empty());
    }

    #[test]
    fn prune_keeps_selected_and_busiest() {
        let mut graph = g(&["a", "b", "c", "d"], &[("a", "b"), ("c", "b")]);
        graph.prune(2, &["d"]);
        let ids: Vec<&str> = graph.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, vec!["b", "d"]);
        assert_eq!(graph.more, 2);
        assert!(graph.edges.is_empty());
    }
}
