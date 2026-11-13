//! Graph viewer (M10): SVG satu level zoom dari `agentgate_graph::viewer::Graph` (sudah
//! di-layout), warna layer/bahasa, kelas status node (terpilih, disentuh tiket, terdampak, luar,
//! kode mati, redup), dan tautan node. Setiap node adalah `<a>` SVG (pilih node tanpa JS);
//! `data-drill` dipakai `graph-viewer.js` untuk klik ganda zoom masuk. Teks sudah di-escape.
use std::{collections::HashSet, fmt::Write as _};

use agentgate_graph::viewer::{Graph, Layer, NODE_H, NODE_W, edge_path};
use serde::Serialize;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn clip(s: &str, max: usize) -> String {
    if s.chars().count() > max {
        format!("{}…", s.chars().take(max - 1).collect::<String>())
    } else {
        s.to_string()
    }
}

/// Warna layer (sama dengan prototype; jelas di tema terang dan gelap).
#[must_use]
pub fn layer_color(l: Layer) -> &'static str {
    match l {
        Layer::Ui => "#db2777",
        Layer::Logic => "#5b5bd6",
        Layer::Api => "#0891b2",
        Layer::Data => "#16a34a",
        Layer::Test => "#c2410c",
        Layer::Ext => "#64748b",
        Layer::Doc => "#a16207",
    }
}

/// Kunci Fluent label layer.
#[must_use]
pub fn layer_key(l: Layer) -> String {
    format!("gv-layer-{}", l.as_str())
}

/// Warna titik bahasa.
#[must_use]
pub fn lang_color(lang: &str) -> &'static str {
    match lang.to_lowercase().as_str() {
        "typescript" | "tsx" => "#3178c6",
        "javascript" | "jsx" => "#e0b50f",
        "go" => "#00add8",
        "rust" => "#dea584",
        "python" => "#3572a5",
        "java" => "#b07219",
        "php" => "#4f5d95",
        "c#" | "csharp" | "c_sharp" => "#178600",
        "kotlin" => "#a97bff",
        "dart" => "#00b4ab",
        "sql" => "#e38c00",
        _ => "#8d8d99",
    }
}

/// Sorotan yang aktif di kanvas.
#[derive(Debug, Default)]
pub struct Marks<'a> {
    pub selected: Option<&'a str>,
    pub touched: HashSet<String>,
    /// `Some` = analisis dampak aktif (node terdampak).
    pub impact: Option<HashSet<String>>,
}

/// SVG kanvas. `select_href(id)` = tautan memilih node; `drill_href(query)` = tautan zoom masuk.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn svg(
    graph: &Graph,
    marks: &Marks<'_>,
    aria: &str,
    select_href: &dyn Fn(&str) -> String,
    drill_href: &dyn Fn(&str) -> String,
) -> String {
    let highlight = !marks.touched.is_empty();
    let impact = marks.impact.as_ref();
    let in_impact = |id: &str| {
        impact.is_some_and(|set| set.contains(id))
            || (impact.is_some() && marks.selected == Some(id))
    };
    let mut out = String::new();
    let _ = write!(
        out,
        r#"<svg class="app-gv-svg" viewBox="0 0 {w:.0} {h:.0}" width="{w:.0}" height="{h:.0}" role="img" aria-label="{a}" data-gv-svg><defs>"#,
        w = graph.width,
        h = graph.height,
        a = esc(aria),
    );
    for (id, class) in [
        ("gv-a", ""),
        ("gv-ac", " data-tone=\"accent\""),
        ("gv-aw", " data-tone=\"warn\""),
    ] {
        let _ = write!(
            out,
            r#"<marker id="{id}" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path class="app-gv-mk"{class} d="M0,1 L9,5 L0,9 z"/></marker>"#
        );
    }
    out.push_str("</defs><g data-gv-layer>");
    for e in &graph.edges {
        let (Some(a), Some(b)) = (graph.node(&e.from), graph.node(&e.to)) else {
            continue;
        };
        let (d, lx, ly) = edge_path(a, b);
        let hl = highlight && marks.touched.contains(&e.from) && marks.touched.contains(&e.to);
        let im = impact.is_some() && in_impact(&e.from) && in_impact(&e.to);
        let dim = if impact.is_some() {
            !im
        } else {
            highlight && !hl
        };
        let mut attrs = String::new();
        if !e.certain {
            attrs.push_str(" data-guess");
        }
        if hl {
            attrs.push_str(" data-hl");
        }
        if im {
            attrs.push_str(" data-impact");
        }
        if dim {
            attrs.push_str(" data-dim");
        }
        let marker = if hl {
            "gv-ac"
        } else if im {
            "gv-aw"
        } else {
            "gv-a"
        };
        let _ = write!(
            out,
            r#"<path class="app-gv-e"{attrs} d="{d}" marker-end="url(#{marker})"/>"#
        );
        if !e.label.is_empty() && !dim {
            let _ = write!(
                out,
                r#"<text class="app-gv-el" x="{lx:.0}" y="{y:.0}" text-anchor="middle">{l}</text>"#,
                y = ly - 4.0,
                l = esc(&e.label),
            );
        }
    }
    for n in &graph.nodes {
        let selected = marks.selected == Some(n.id.as_str());
        let tk = marks.touched.contains(&n.id);
        let im = impact.is_some_and(|s| s.contains(&n.id));
        let root = impact.is_some() && selected;
        let dim = if impact.is_some() {
            !im && !root
        } else {
            highlight && !tk && !selected
        };
        let mut attrs = String::new();
        for (on, a) in [
            (selected, " data-sel"),
            (tk, " data-tk"),
            (im, " data-impact"),
            (root, " data-root"),
            (dim, " data-dim"),
            (n.dead, " data-dead"),
            (n.out, " data-out"),
        ] {
            if on {
                attrs.push_str(a);
            }
        }
        let drill = n.drill.as_deref().map(drill_href);
        if let Some(h) = &drill {
            let _ = write!(attrs, r#" data-drill="{}""#, esc(h));
        }
        let _ = write!(
            out,
            r#"<a class="app-gv-n" href="{href}" data-node="{id}"{attrs} aria-label="{al}"><g transform="translate({x:.1},{y:.1})"><title>{tt}</title><rect class="bg" width="{NODE_W}" height="{NODE_H}" rx="9"/><rect x="0" y="9" width="3" height="{sh}" rx="1.5" fill="{col}"/><text class="t" x="14" y="21">{label}</text><text class="s" x="14" y="37">{sub}</text>"#,
            href = esc(&select_href(&n.id)),
            id = esc(&n.id),
            al = esc(&format!("{} · {}", n.label, n.sub)),
            x = n.x - NODE_W / 2.0,
            y = n.y - NODE_H / 2.0,
            tt = esc(&if n.path.is_empty() {
                n.label.clone()
            } else {
                n.path.clone()
            }),
            sh = NODE_H - 18.0,
            col = layer_color(n.layer),
            label = esc(&clip(&n.label, 21)),
            sub = esc(&clip(&n.sub, 25)),
        );
        if !n.lang.is_empty() {
            let _ = write!(
                out,
                r#"<circle cx="{cx}" cy="13" r="3.5" fill="{c}"/>"#,
                cx = NODE_W - 12.0,
                c = lang_color(&n.lang),
            );
        }
        if drill.is_some() {
            let _ = write!(
                out,
                r#"<g class="drill" transform="translate({x},{y})"><rect width="14" height="14" rx="4"/><path d="M4 7h6M7 4v6"/></g>"#,
                x = NODE_W - 20.0,
                y = NODE_H - 20.0,
            );
        }
        out.push_str("</g></a>");
    }
    out.push_str("</g></svg>");
    out
}

/// Satu tombol filter layer/bahasa (tautan GET yang membalik filter).
#[derive(Debug, Clone, Serialize)]
pub struct FilterChip {
    pub key: String,
    /// Kunci Fluent (layer) atau teks (bahasa).
    pub label_key: Option<String>,
    pub label: String,
    pub color: &'static str,
    pub on: bool,
    pub href: String,
}

#[cfg(test)]
mod tests {
    use agentgate_graph::viewer::{Edge, Node};

    use super::*;

    #[test]
    fn svg_escapes_and_marks_nodes() {
        let mut a = Node::new("f:a<b", "a<b.ts", Layer::Ui);
        a.drill = Some("repo=x&file=a".into());
        a.lang = "TypeScript".into();
        let b = Node::new("f:c", "c.ts", Layer::Logic);
        let mut g = Graph::new(
            vec![a, b],
            vec![Edge {
                from: "f:a<b".into(),
                to: "f:c".into(),
                certain: false,
                label: "2".into(),
            }],
        );
        g.layout();
        let marks = Marks {
            selected: Some("f:c"),
            touched: ["f:a<b".to_string()].into_iter().collect(),
            impact: None,
        };
        let svg = svg(&g, &marks, "Graph", &|id| format!("?sel={id}"), &|q| {
            format!("?{q}")
        });
        assert!(svg.contains("a&lt;b.ts"));
        assert!(!svg.contains("a<b.ts"));
        assert!(svg.contains("data-guess"));
        assert!(svg.contains(r#"data-drill="?repo=x&amp;file=a""#));
        assert!(svg.contains("data-sel"));
        assert!(svg.contains("data-tk"));
        assert!(svg.contains("#3178c6"));
    }
}
