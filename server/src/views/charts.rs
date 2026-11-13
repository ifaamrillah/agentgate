//! Grafik SVG server-side (laporan M9): garis (burndown/burnup), batang berpasangan (velocity),
//! area bertumpuk (cumulative flow), dan sebar (kalibrasi). Keluaran berupa markup SVG yang
//! sudah di-escape, dirender di template dengan `| safe`. Warna lewat `data-c` (token di
//! `components/planning.css`), tooltip lewat `<title>` (tanpa JS, aturan CSP).
use std::fmt::Write as _;

const H: f64 = 250.0;
const PL: f64 = 40.0;
const PR: f64 = 14.0;
const PT: f64 = 14.0;
const PB: f64 = 28.0;

/// Warna seri (nama token, lihat `.app-chart [data-c]`).
pub const TONES: &[&str] = &["accent", "info", "success", "warn", "danger", "ai", "muted"];

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn num(v: f64) -> String {
    let s = format!("{v:.1}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Angka untuk label (gaya Indonesia, koma desimal).
#[must_use]
pub fn label(v: f64) -> String {
    num(v).replace('.', ",")
}

/// Satu seri garis.
#[derive(Debug, Clone)]
pub struct Series {
    pub tone: &'static str,
    pub values: Vec<Option<f64>>,
    pub dash: bool,
    pub area: bool,
}

/// Garis vertikal penanda (mis. "Hari ini").
#[derive(Debug, Clone)]
pub struct Mark {
    pub index: usize,
    pub tone: &'static str,
    pub label: String,
}

/// Langkah sumbu y yang rapi untuk nilai maksimum `max` (sekitar 5 garis).
#[must_use]
pub fn nice_max(max: f64) -> (f64, f64) {
    let max = max.max(1.0);
    let raw = max / 5.0;
    let pow = 10f64.powf(raw.log10().floor());
    let step = [1.0, 2.0, 2.5, 5.0, 10.0]
        .iter()
        .map(|m| m * pow)
        .find(|s| *s >= raw)
        .unwrap_or(10.0 * pow);
    ((max / step).ceil() * step, step)
}

struct Frame {
    n: usize,
    w: f64,
    y_max: f64,
}

impl Frame {
    #[allow(clippy::cast_precision_loss)]
    fn x(&self, i: usize) -> f64 {
        if self.n <= 1 {
            PL + (self.w - PL - PR) / 2.0
        } else {
            PL + i as f64 * (self.w - PL - PR) / (self.n - 1) as f64
        }
    }
    fn y(&self, v: f64) -> f64 {
        PT + (H - PT - PB) * (1.0 - v / self.y_max)
    }
    fn grid(&self, step: f64, labels: &[String], out: &mut String) {
        let mut v = 0.0;
        while v <= self.y_max + f64::EPSILON {
            let y = self.y(v);
            let _ = write!(
                out,
                r#"<line class="gl" x1="{PL}" x2="{x2}" y1="{y:.1}" y2="{y:.1}"/><text x="{tx}" y="{ty:.1}" text-anchor="end">{l}</text>"#,
                x2 = self.w - PR,
                tx = PL - 8.0,
                ty = y + 4.0,
                l = label(v),
            );
            v += step;
        }
        let last = labels.len().saturating_sub(1);
        for (i, l) in labels.iter().enumerate() {
            if !l.is_empty() {
                // Label pertama/terakhir rata ke dalam agar tidak terpotong tepi SVG.
                let anchor = match i {
                    0 => "start",
                    _ if i == last => "end",
                    _ => "middle",
                };
                let _ = write!(
                    out,
                    r#"<text x="{x:.1}" y="{y}" text-anchor="{anchor}">{l}</text>"#,
                    x = self.x(i),
                    y = H - 8.0,
                    l = esc(l),
                );
            }
        }
    }
}

fn svg(w: f64, aria: &str, body: &str) -> String {
    format!(
        r#"<div class="app-chart-w"><svg class="app-chart" viewBox="0 0 {w} {H}" role="img" aria-label="{a}">{body}</svg></div>"#,
        a = esc(aria)
    )
}

fn path(points: &[(f64, f64)]) -> String {
    points
        .iter()
        .enumerate()
        .map(|(i, (x, y))| format!("{}{x:.1},{y:.1}", if i == 0 { "M" } else { "L" }))
        .collect()
}

/// Grafik garis. `tips[i]` = isi tooltip kolom ke-i (baris dipisah `\n`).
#[must_use]
pub fn line_chart(
    aria: &str,
    labels: &[String],
    series: &[Series],
    marks: &[Mark],
    tips: &[String],
    width: f64,
) -> String {
    let max = series
        .iter()
        .flat_map(|s| s.values.iter().flatten())
        .copied()
        .fold(0.0, f64::max);
    let (y_max, step) = nice_max(max);
    let f = Frame {
        n: labels.len(),
        w: width,
        y_max,
    };
    let mut out = String::new();
    f.grid(step, labels, &mut out);
    for m in marks {
        let x = f.x(m.index);
        let _ = write!(
            out,
            r#"<g data-c="{t}"><line class="mk" x1="{x:.1}" x2="{x:.1}" y1="{PT}" y2="{y2}"/><text class="mk-l" x="{tx:.1}" y="{ty}">{l}</text></g>"#,
            t = m.tone,
            y2 = H - PB,
            tx = x + 4.0,
            ty = PT + 10.0,
            l = esc(&m.label),
        );
    }
    for s in series {
        let pts: Vec<(f64, f64)> = s
            .values
            .iter()
            .enumerate()
            .filter_map(|(i, v)| v.map(|v| (f.x(i), f.y(v))))
            .collect();
        if pts.is_empty() {
            continue;
        }
        let _ = write!(out, r#"<g data-c="{}">"#, s.tone);
        if s.area {
            let (first, last) = (pts[0], pts[pts.len() - 1]);
            let _ = write!(
                out,
                r#"<path class="ar" d="{}L{:.1},{y0:.1}L{:.1},{y0:.1}Z"/>"#,
                path(&pts),
                last.0,
                first.0,
                y0 = f.y(0.0)
            );
        }
        let _ = write!(
            out,
            r#"<path class="ln" d="{}"{}/>"#,
            path(&pts),
            if s.dash { " data-dash" } else { "" }
        );
        if !s.dash {
            for (x, y) in &pts {
                let _ = write!(
                    out,
                    r#"<circle class="pt" cx="{x:.1}" cy="{y:.1}" r="2.6"/>"#
                );
            }
        }
        out.push_str("</g>");
    }
    #[allow(clippy::cast_precision_loss)]
    let col = (width - PL - PR) / (labels.len().max(2) - 1) as f64;
    for (i, tip) in tips.iter().enumerate() {
        let _ = write!(
            out,
            r#"<rect class="hit" x="{x:.1}" y="{PT}" width="{col:.1}" height="{h}"><title>{t}</title></rect>"#,
            x = f.x(i) - col / 2.0,
            h = H - PT - PB,
            t = esc(tip),
        );
    }
    svg(width, aria, &out)
}

/// Batang berpasangan per kelompok: (label, komitmen, selesai `None` = berjalan), garis rata-rata.
#[must_use]
pub fn pair_bars(
    aria: &str,
    groups: &[(String, f64, Option<f64>)],
    average: Option<f64>,
    running: &str,
    width: f64,
) -> String {
    let max = groups
        .iter()
        .map(|g| g.1.max(g.2.unwrap_or(0.0)))
        .fold(average.unwrap_or(0.0), f64::max);
    let (y_max, step) = nice_max(max);
    let f = Frame {
        n: 2,
        w: width,
        y_max,
    };
    let mut out = String::new();
    f.grid(step, &[], &mut out);
    #[allow(clippy::cast_precision_loss)]
    let gx = (width - PL - PR) / groups.len().max(1) as f64;
    let bw = (gx / 2.0 - 8.0).clamp(8.0, 44.0);
    let y0 = f.y(0.0);
    #[allow(clippy::cast_precision_loss)]
    for (i, (name, commit, done)) in groups.iter().enumerate() {
        let cx = PL + gx * i as f64 + gx / 2.0;
        let _ = write!(
            out,
            r#"<g data-c="muted"><rect class="bx" data-soft x="{x:.1}" y="{y:.1}" width="{bw:.1}" height="{h:.1}" rx="3"><title>{t}</title></rect></g>"#,
            x = cx - bw - 2.0,
            y = f.y(*commit),
            h = y0 - f.y(*commit),
            t = esc(&format!("{name}\nKomitmen: {} poin", label(*commit))),
        );
        match done {
            Some(d) => {
                let _ = write!(
                    out,
                    r#"<g data-c="accent"><rect class="bx" x="{x:.1}" y="{y:.1}" width="{bw:.1}" height="{h:.1}" rx="3"><title>{t}</title></rect></g>"#,
                    x = cx + 2.0,
                    y = f.y(*d),
                    h = y0 - f.y(*d),
                    t = esc(&format!("{name}\nSelesai: {} poin", label(*d))),
                );
            }
            None => {
                let _ = write!(
                    out,
                    r#"<text x="{x:.1}" y="{y:.1}" text-anchor="middle">{r}</text>"#,
                    x = cx + 2.0 + bw / 2.0,
                    y = y0 - 8.0,
                    r = esc(running),
                );
            }
        }
        let _ = write!(
            out,
            r#"<text x="{cx:.1}" y="{y}" text-anchor="middle">{n}</text>"#,
            y = H - 8.0,
            n = esc(name),
        );
    }
    if let Some(a) = average {
        let y = f.y(a);
        let _ = write!(
            out,
            r#"<g data-c="warn"><line class="mk" x1="{PL}" x2="{x2}" y1="{y:.1}" y2="{y:.1}"/><text class="mk-l" x="{x2}" y="{ty:.1}" text-anchor="end">{l}</text></g>"#,
            x2 = width - PR,
            ty = y - 5.0,
            l = label(a),
        );
    }
    svg(width, aria, &out)
}

/// Area bertumpuk: `layers` dari bawah ke atas (nama, tone, nilai per label).
#[must_use]
pub fn stacked_area(
    aria: &str,
    labels: &[String],
    layers: &[(String, &'static str, Vec<f64>)],
    width: f64,
) -> String {
    let n = labels.len();
    let totals: Vec<f64> = (0..n)
        .map(|i| {
            layers
                .iter()
                .map(|l| l.2.get(i).copied().unwrap_or(0.0))
                .sum()
        })
        .collect();
    let (y_max, step) = nice_max(totals.iter().copied().fold(0.0, f64::max));
    let f = Frame { n, w: width, y_max };
    let mut out = String::new();
    f.grid(step, labels, &mut out);
    let mut base = vec![0.0; n];
    for (name, tone, values) in layers {
        let lo = base.clone();
        for (i, b) in base.iter_mut().enumerate() {
            *b += values.get(i).copied().unwrap_or(0.0);
        }
        let top: Vec<(f64, f64)> = base
            .iter()
            .enumerate()
            .map(|(i, v)| (f.x(i), f.y(*v)))
            .collect();
        let bottom: String = lo
            .iter()
            .enumerate()
            .rev()
            .map(|(i, v)| format!("L{:.1},{:.1}", f.x(i), f.y(*v)))
            .collect();
        let _ = write!(
            out,
            r#"<g data-c="{tone}"><path class="ar" data-solid d="{}{bottom}Z"><title>{t}</title></path></g>"#,
            path(&top),
            t = esc(name),
        );
    }
    #[allow(clippy::cast_precision_loss)]
    let col = (width - PL - PR) / (n.max(2) - 1) as f64;
    for (i, l) in labels.iter().enumerate() {
        let tip =
            std::iter::once(l.clone())
                .chain(layers.iter().rev().map(|(name, _, v)| {
                    format!("{name}: {}", label(v.get(i).copied().unwrap_or(0.0)))
                }))
                .collect::<Vec<_>>()
                .join("\n");
        let _ = write!(
            out,
            r#"<rect class="hit" x="{x:.1}" y="{PT}" width="{col:.1}" height="{h}"><title>{t}</title></rect>"#,
            x = f.x(i) - col / 2.0,
            h = H - PT - PB,
            t = esc(&tip),
        );
    }
    svg(width, aria, &out)
}

/// Sebar estimasi (x) vs aktual (y) dengan garis tepat; titik di atas 115% berwarna peringatan.
#[must_use]
pub fn scatter(aria: &str, points: &[(f64, f64, String)], x_label: &str, y_label: &str) -> String {
    let max = points.iter().map(|p| p.0.max(p.1)).fold(1.0, f64::max);
    let (m, step) = nice_max(max);
    let size = 260.0;
    let left = 40.0;
    let top = 14.0;
    let plot = size - left - 10.0;
    let sx = |v: f64| left + v / m * plot;
    let sy = |v: f64| top + plot - v / m * plot;
    let mut out = String::new();
    let mut v = 0.0;
    while v <= m + f64::EPSILON {
        let _ = write!(
            out,
            r#"<line class="gl" x1="{x1:.1}" x2="{x2:.1}" y1="{y:.1}" y2="{y:.1}"/><text x="{tx:.1}" y="{ty:.1}" text-anchor="end">{l}</text><text x="{x:.1}" y="{by:.1}" text-anchor="middle">{l}</text>"#,
            x1 = sx(0.0),
            x2 = sx(m),
            y = sy(v),
            tx = left - 6.0,
            ty = sy(v) + 4.0,
            x = sx(v),
            by = top + plot + 16.0,
            l = label(v),
        );
        v += step;
    }
    let _ = write!(
        out,
        r#"<g data-c="muted"><line class="mk" x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}"/></g>"#,
        sx(0.0),
        sy(0.0),
        sx(m),
        sy(m)
    );
    for (e, a, code) in points {
        let tone = if *a > e * 1.15 { "warn" } else { "accent" };
        let _ = write!(
            out,
            r#"<g data-c="{tone}"><circle class="bx" cx="{:.1}" cy="{:.1}" r="4.5"><title>{}</title></circle></g>"#,
            sx(*e),
            sy(*a),
            esc(&format!(
                "{code}\nEstimasi {} jam\nAktual {} jam",
                label(*e),
                label(*a)
            )),
        );
    }
    let h = size + 24.0;
    let _ = write!(
        out,
        r#"<text x="{:.1}" y="{:.1}" text-anchor="middle">{}</text><text x="{left}" y="{:.1}">{}</text>"#,
        left + plot / 2.0,
        h - 2.0,
        esc(x_label),
        top - 4.0,
        esc(y_label)
    );
    format!(
        r#"<div class="app-chart-w"><svg class="app-chart" data-square viewBox="0 0 {size} {h}" role="img" aria-label="{a}">{out}</svg></div>"#,
        a = esc(aria)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axis_steps_are_round() {
        assert_eq!(nice_max(26.0), (30.0, 10.0));
        assert_eq!(nice_max(8.0), (8.0, 2.0));
        assert_eq!(nice_max(0.0), (1.0, 0.2));
    }

    #[test]
    fn charts_escape_text_and_draw_series() {
        let svg = line_chart(
            "Burndown",
            &["1".into(), "2<".into()],
            &[Series {
                tone: "accent",
                values: vec![Some(5.0), Some(2.0)],
                dash: false,
                area: true,
            }],
            &[Mark {
                index: 1,
                tone: "danger",
                label: "Hari ini".into(),
            }],
            &["a\"b".into(), "c".into()],
            600.0,
        );
        assert!(svg.contains("2&lt;"));
        assert!(svg.contains("a&quot;b"));
        assert!(svg.contains(r#"class="ln""#));
        assert!(!svg.contains("<script"));
        let bars = pair_bars(
            "V",
            &[("S<1".into(), 8.0, Some(5.0))],
            Some(5.0),
            "berjalan",
            600.0,
        );
        assert!(bars.contains("S&lt;1"));
        let cfd = stacked_area(
            "C",
            &["a".into(), "b".into()],
            &[("Dev".into(), "info", vec![1.0, 2.0])],
            600.0,
        );
        assert!(cfd.contains("Dev: 2"));
        let sc = scatter("K", &[(2.0, 4.0, "KSR-1".into())], "estimasi", "aktual");
        assert!(sc.contains(r#"data-c="warn""#));
    }
}
