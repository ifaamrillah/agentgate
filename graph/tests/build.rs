#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Resolusi call lintas file + keyakinan, pewarisan, import, dead code, modul, dampak.
use agentgate_graph::{
    analysis::{Adjacency, Direction, changed_nodes, impact, parse_diff},
    build::{Confidence, EdgeKind, RepoGraph, ScipData, ScipEdge, SourceFile, build},
    extract,
};

fn repo(files: &[(&str, &str)]) -> (Vec<SourceFile>, RepoGraph) {
    let src: Vec<SourceFile> = files
        .iter()
        .map(|(p, s)| SourceFile {
            path: (*p).to_string(),
            facts: extract(p, s).unwrap(),
        })
        .collect();
    let g = build(&src, None);
    (src, g)
}

fn label(g: &RepoGraph, n: u32) -> String {
    g.nodes[n as usize].qualified.clone()
}

fn edges(g: &RepoGraph, kind: EdgeKind) -> Vec<String> {
    let mut v: Vec<String> = g
        .edges
        .iter()
        .filter(|e| e.kind == kind)
        .map(|e| {
            let c = if e.confidence == Confidence::Certain {
                "!"
            } else {
                "?"
            };
            format!("{} -{c}> {}", label(g, e.from), label(g, e.to))
        })
        .collect();
    v.sort();
    v
}

fn node(g: &RepoGraph, q: &str) -> u32 {
    g.nodes
        .iter()
        .position(|n| n.qualified == q)
        .unwrap_or_else(|| panic!("{q}")) as u32
}

#[test]
fn typescript_resolution() {
    let (_, g) = repo(&[
        (
            "web/src/cart/promo.ts",
            "import { roundRupiah } from './money';\nimport axios from 'axios';\nimport * as api from '../api/promoApi';\nexport function calcPromo(x: number) { return roundRupiah(x) + api.validate(x) + helper(x) + axios.get('/x'); }\nfunction helper(x: number) { return format(x); }\nfunction legacy() { return 1; }\nexport class Cart extends Base { total() { return this.sum() + calcPromo(1); } sum() { return 0; } }\n",
        ),
        (
            "web/src/cart/money.ts",
            "export function roundRupiah(x: number) { return x; }\nexport function format(x: number) { return x; }\n",
        ),
        (
            "web/src/api/promoApi.ts",
            "export function validate(x: number) { return x; }\n",
        ),
        ("web/src/base.ts", "export class Base {}\n"),
        (
            "web/src/other/money.ts",
            "export function format(x: number) { return x; }\n",
        ),
    ]);
    assert_eq!(
        edges(&g, EdgeKind::Calls),
        vec![
            "Cart.total -!> Cart.sum",
            "Cart.total -!> calcPromo",
            "calcPromo -!> helper",
            "calcPromo -!> roundRupiah",
            "calcPromo -!> validate",
            "helper -?> format",
        ]
    );
    // `format` ada di dua file dan tidak diimpor: dipilih yang satu direktori, sebagai dugaan.
    assert_eq!(g.stats.ambiguous, 0);
    assert!(g.stats.external >= 1, "axios.get = luar repo");
    assert_eq!(edges(&g, EdgeKind::Inherits), vec!["Cart -?> Base"]);
    assert_eq!(
        edges(&g, EdgeKind::Imports),
        vec![
            "web/src/cart/promo.ts -!> web/src/api/promoApi.ts",
            "web/src/cart/promo.ts -!> web/src/cart/money.ts"
        ]
    );
    let legacy = &g.nodes[node(&g, "legacy") as usize];
    assert_eq!(legacy.dead, Some(Confidence::Certain));
    let other = g
        .nodes
        .iter()
        .find(|n| n.name == "format" && n.module == "web/other")
        .unwrap();
    assert_eq!(other.dead, Some(Confidence::Inferred), "exported");
    assert_eq!(g.nodes[node(&g, "calcPromo") as usize].dead, None);
    assert_eq!(g.nodes[node(&g, "calcPromo") as usize].module, "web/cart");
    assert!(
        g.modules
            .iter()
            .any(|m| m.name == "web/cart" && m.central == Some(node(&g, "calcPromo")))
    );
}

#[test]
fn go_package_and_receiver_resolution() {
    let (_, g) = repo(&[
        (
            "internal/promo/handler.go",
            "package promo\nimport m \"github.com/acme/api/internal/money\"\ntype Handler struct{}\nfunc (h *Handler) Validate() { calc(); m.Round(1); h.log() }\nfunc (h *Handler) log() {}\n",
        ),
        (
            "internal/promo/calc.go",
            "package promo\nfunc calc() int { return 1 }\n",
        ),
        (
            "internal/money/round.go",
            "package money\nfunc Round(x int) int { return x }\n",
        ),
    ]);
    assert_eq!(
        edges(&g, EdgeKind::Calls),
        vec![
            "Handler.Validate -!> Handler.log",
            "Handler.Validate -!> Round",
            "Handler.Validate -!> calc"
        ]
    );
}

#[test]
fn python_relative_imports_and_self() {
    let (_, g) = repo(&[
        (
            "app/promo.py",
            "from .money import round_rupiah\nimport requests\nclass Promo:\n    def run(self):\n        return self.calc() + round_rupiah(1) + requests.get('x')\n    def calc(self):\n        return 1\n",
        ),
        ("app/money.py", "def round_rupiah(x):\n    return x\n"),
        (
            "tests/test_promo.py",
            "from app.promo import Promo\ndef test_run():\n    Promo().run()\n",
        ),
    ]);
    assert_eq!(
        edges(&g, EdgeKind::Calls),
        vec![
            "Promo.run -!> Promo.calc",
            "Promo.run -!> round_rupiah",
            "test_run -!> Promo",
            "test_run -?> Promo.run"
        ]
    );
    assert!(g.nodes[node(&g, "test_run") as usize].test);
}

#[test]
fn rust_impl_and_scip_override() {
    let files = [
        (
            "src/promo.rs",
            "use crate::money::round;\npub struct Promo;\nimpl Promo {\n    pub fn new() -> Self { Self }\n    pub fn apply(&self) -> i64 { round(1) + Self::new().x() }\n    fn x(&self) -> i64 { 1 }\n}\n",
        ),
        ("src/money.rs", "pub fn round(x: i64) -> i64 { x }\n"),
    ];
    let (src, g) = repo(&files);
    assert_eq!(
        edges(&g, EdgeKind::Calls),
        vec![
            "Promo.apply -!> Promo.new",
            "Promo.apply -!> round",
            "Promo.apply -?> Promo.x"
        ]
    );
    // SCIP menggantikan call tree-sitter untuk dokumen yang dicakupnya.
    let scip = ScipData {
        indexer: "rust-analyzer".into(),
        documents: vec!["src/promo.rs".into()],
        edges: vec![
            ScipEdge {
                from_path: "src/promo.rs".into(),
                from_line: 5,
                to_path: "src/promo.rs".into(),
                to_line: 6,
            },
            ScipEdge {
                from_path: "src/promo.rs".into(),
                from_line: 5,
                to_path: "src/money.rs".into(),
                to_line: 1,
            },
        ],
    };
    let g2 = build(&src, Some(&scip));
    assert_eq!(
        edges(&g2, EdgeKind::Calls),
        vec!["Promo.apply -!> Promo.x", "Promo.apply -!> round"]
    );
    assert_eq!(g2.stats.scip_edges, 2);
}

#[test]
fn routes_handlers_and_impact_from_diff() {
    let (src, g) = repo(&[
        (
            "api/routes.js",
            "const { create } = require('./payment');\nrouter.post('/v1/pay', create);\n",
        ),
        (
            "api/payment.js",
            "function create(req) { return settle(req); }\nfunction settle(r) { return r; }\nmodule.exports = { create };\n",
        ),
        (
            "api/payment.test.js",
            "const { create } = require('./payment');\ntest('x', () => create({}));\n",
        ),
    ]);
    assert_eq!(
        edges(&g, EdgeKind::Handles),
        vec!["POST /v1/pay -!> create"]
    );
    assert_eq!(
        g.nodes[node(&g, "create") as usize].dead,
        None,
        "handler bukan dead code"
    );
    let diff = "--- a/api/payment.js\n+++ b/api/payment.js\n@@ -2,1 +2,1 @@\n-x\n+y\n";
    let changed = changed_nodes(&g, &src, &parse_diff(diff));
    assert_eq!(
        changed.iter().map(|&n| label(&g, n)).collect::<Vec<_>>(),
        vec!["settle"]
    );
    let adj = Adjacency::of(&g);
    let imp = impact(&g, &adj, &changed, 3);
    let affected: Vec<String> = imp
        .affected
        .iter()
        .map(|h| format!("{}@{}", label(&g, h.node), h.depth))
        .collect();
    assert_eq!(affected, vec!["create@1", "POST /v1/pay@2"]);
    assert_eq!(imp.tests.len(), 1);
    let callees = adj.trace(&[node(&g, "create")], Direction::Callees, 2, 10);
    assert_eq!(callees.len(), 1);
}

#[test]
fn rust_crate_paths_and_macros() {
    let (_, g) = repo(&[
        (
            "server/src/services/graph/mod.rs",
            "pub mod ingest;\npub fn ensure_row() -> i32 { 1 }\n",
        ),
        (
            "server/src/services/graph/ingest.rs",
            "use super::ensure_row;\npub fn start() -> i32 { ensure_row() }\n",
        ),
        (
            "server/src/controllers/graph.rs",
            "use crate::services::graph::{self as svc, ingest};\nuse crate::views::graph as gv;\npub fn reindex() { svc::ensure_row(); crate::services::graph::ensure_row(); ingest::start(); }\npub fn page() { json!({ \"n\": gv::num(1) }); }\n",
        ),
        (
            "server/src/views/graph.rs",
            "pub fn num(n: i64) -> i64 { n }\n",
        ),
        ("graph/src/query.rs", "fn other() -> i64 { num(2) }\n"),
    ]);
    let calls = edges(&g, EdgeKind::Calls);
    for want in [
        "start -!> ensure_row",
        "reindex -!> ensure_row",
        "reindex -!> start",
        "page -!> num",
    ] {
        assert!(calls.contains(&want.to_string()), "{want}: {calls:?}");
    }
    // Crate lain (tanpa import) tidak menebak fungsi `num` milik server.
    assert!(!calls.iter().any(|c| c.starts_with("other")), "{calls:?}");
}
