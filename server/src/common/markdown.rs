//! Render Markdown (GFM) menjadi HTML yang aman ditampilkan: deskripsi tiket, komentar, artifact,
//! ringkasan job, dan dokumen knowledge (M8).
//!
//! Dua lapis pertahanan:
//! 1. HTML mentah di Markdown tidak pernah diteruskan: blok/inline HTML diubah menjadi teks biasa.
//! 2. Hasil render tetap melewati allowlist `ammonia` (tanpa `style`, atribut `on*`, skema
//!    `javascript:`); tautan diberi `rel="noopener noreferrer nofollow"`.
//!
//! Blok kode ```` ```mermaid ```` dirender `<pre class="app-mermaid" data-mermaid>` berisi kode
//! sumbernya; `static/js/markdown.js` menggantinya dengan SVG Mermaid (lazy, `securityLevel:
//! strict`). Tanpa JS, kode diagram tetap terbaca. Heading diberi `id` (`md-<slug>`) untuk outline.
use std::collections::{HashMap, HashSet};

use pulldown_cmark::{CodeBlockKind, CowStr, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// Satu heading untuk outline dokumen.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    /// Id anchor (`md-…`), sama dengan id di HTML hasil [`render`].
    pub id: String,
}

fn options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_FOOTNOTES
}

fn level_num(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn slug(text: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in text.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            out.push(c);
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    let out = out.trim_end_matches('-');
    if out.is_empty() {
        "bagian".to_string()
    } else {
        out.chars().take(60).collect()
    }
}

/// Heading dokumen (urut kemunculan) beserta id unik.
#[must_use]
pub fn outline(src: &str) -> Vec<Heading> {
    let mut out = Vec::new();
    let mut current: Option<(u8, String)> = None;
    let mut seen: HashMap<String, usize> = HashMap::new();
    for ev in Parser::new_ext(src, options()) {
        match ev {
            Event::Start(Tag::Heading { level, .. }) => current = Some((level_num(level), String::new())),
            Event::Text(t) | Event::Code(t) => {
                if let Some((_, buf)) = current.as_mut() {
                    buf.push_str(&t);
                }
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((level, text)) = current.take() {
                    let base = slug(&text);
                    let n = seen.entry(base.clone()).or_insert(0);
                    *n += 1;
                    let id = if *n == 1 {
                        format!("md-{base}")
                    } else {
                        format!("md-{base}-{n}")
                    };
                    out.push(Heading {
                        level,
                        text: text.trim().to_string(),
                        id,
                    });
                }
            }
            _ => {}
        }
    }
    out
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

fn sanitizer() -> ammonia::Builder<'static> {
    let mut b = ammonia::Builder::default();
    b.add_tags(["input", "del", "sup", "section"])
        .add_tag_attributes("input", ["type", "checked", "disabled"])
        .add_tag_attributes("pre", ["class", "data-mermaid"])
        .add_tag_attributes("code", ["class"])
        .add_tag_attributes("li", ["class"])
        .add_tag_attributes("ul", ["class"])
        .add_tag_attributes("a", ["id"])
        .add_tag_attributes("sup", ["class", "id"])
        .add_tag_attributes("div", ["class", "id"])
        .add_tag_attributes("th", ["align"])
        .add_tag_attributes("td", ["align"]);
    for h in ["h1", "h2", "h3", "h4", "h5", "h6"] {
        b.add_tag_attributes(h, ["id"]);
    }
    b.link_rel(Some("noopener noreferrer nofollow"));
    b
}

/// Render `src` (Markdown) menjadi HTML tersanitasi.
#[must_use]
pub fn render(src: &str) -> String {
    if src.trim().is_empty() {
        return String::new();
    }
    let ids: Vec<String> = outline(src).into_iter().map(|h| h.id).collect();
    let mut ids = ids.into_iter();
    let mut in_mermaid = false;
    let events = Parser::new_ext(src, options()).filter_map(|ev| match ev {
        // HTML mentah tidak pernah diteruskan.
        Event::Html(h) | Event::InlineHtml(h) => Some(Event::Text(h)),
        Event::Start(Tag::Heading {
            level,
            classes,
            attrs,
            ..
        }) => Some(Event::Start(Tag::Heading {
            level,
            id: ids.next().map(CowStr::from),
            classes,
            attrs,
        })),
        Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang)))
            if lang.split_whitespace().next() == Some("mermaid") =>
        {
            in_mermaid = true;
            Some(Event::Html(CowStr::from(
                "<pre class=\"app-mermaid\" data-mermaid=\"\">",
            )))
        }
        Event::Text(t) if in_mermaid => Some(Event::Html(CowStr::from(escape(&t)))),
        Event::End(TagEnd::CodeBlock) if in_mermaid => {
            in_mermaid = false;
            Some(Event::Html(CowStr::from("</pre>")))
        }
        // Tabel: pulldown menulis `style="text-align"`; style dibuang sanitizer, jadi biarkan.
        other => Some(other),
    });
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, events);
    let allowed: HashSet<&str> = ["http", "https", "mailto"].into_iter().collect();
    sanitizer().url_schemes(allowed).clean(&html).to_string()
}

/// Teks polos ringkas dari Markdown (untuk cuplikan daftar), maksimal `max` karakter.
#[must_use]
pub fn plain(src: &str, max: usize) -> String {
    let mut out = String::new();
    for ev in Parser::new_ext(src, options()) {
        match ev {
            Event::Text(t) | Event::Code(t) => {
                if !out.is_empty() && !out.ends_with(' ') {
                    out.push(' ');
                }
                out.push_str(t.trim());
            }
            Event::SoftBreak | Event::HardBreak => out.push(' '),
            _ => {}
        }
        if out.chars().count() > max {
            break;
        }
    }
    let out: String = out.split_whitespace().collect::<Vec<_>>().join(" ");
    if out.chars().count() > max {
        let mut cut: String = out.chars().take(max.saturating_sub(1)).collect();
        cut.push('…');
        cut
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_gfm() {
        let html = render("# Judul\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n- [x] selesai\n- [ ] belum\n\n~~lama~~ **tebal**");
        assert!(html.contains("<h1 id=\"md-judul\">Judul</h1>"), "{html}");
        assert!(html.contains("<table>"));
        assert!(html.contains("type=\"checkbox\""));
        assert!(html.contains("checked"));
        assert!(html.contains("<del>lama</del>"));
        assert!(html.contains("<strong>tebal</strong>"));
    }

    #[test]
    fn raw_html_is_text() {
        let html = render("halo <script>alert(1)</script>\n\n<img src=x onerror=alert(1)>");
        assert!(!html.contains("<script"), "{html}");
        assert!(!html.contains("<img"), "{html}");
        assert!(html.contains("&lt;script&gt;"), "{html}");
    }

    #[test]
    fn dangerous_links_removed() {
        let html = render("[klik](javascript:alert(1)) [ok](https://contoh.id) ![x](javascript:alert(2))");
        assert!(!html.contains("javascript:"), "{html}");
        assert!(html.contains("href=\"https://contoh.id\""));
        assert!(html.contains("rel=\"noopener noreferrer nofollow\""));
    }

    #[test]
    fn relative_links_kept() {
        let html = render("[doc](/w/acme/p/KSR/knowledge/api)");
        assert!(html.contains("href=\"/w/acme/p/KSR/knowledge/api\""), "{html}");
    }

    #[test]
    fn mermaid_block_marked() {
        let html = render("```mermaid\nflowchart LR\n  a-->b\n```");
        assert!(html.contains("<pre class=\"app-mermaid\" data-mermaid=\"\">"), "{html}");
        assert!(html.contains("a--&gt;b"), "{html}");
    }

    #[test]
    fn code_block_keeps_language() {
        let html = render("```go\nfunc main() {}\n```");
        assert!(html.contains("<code class=\"language-go\">"), "{html}");
    }

    #[test]
    fn mermaid_escapes_html() {
        let html = render("```mermaid\n<script>x</script>\n```");
        assert!(!html.contains("<script"), "{html}");
    }

    #[test]
    fn outline_ids_unique() {
        let o = outline("## Alur\n\ntext\n\n## Alur\n\n### Detail `kode`");
        assert_eq!(o.len(), 3);
        assert_eq!(o[0].id, "md-alur");
        assert_eq!(o[1].id, "md-alur-2");
        assert_eq!(o[2].text, "Detail kode");
        let html = render("## Alur\n\n## Alur");
        assert!(html.contains("id=\"md-alur-2\""), "{html}");
    }

    #[test]
    fn plain_truncates() {
        assert_eq!(plain("## Judul\n\nIsi **tebal** di sini", 100), "Judul Isi tebal di sini");
        assert_eq!(plain("abcdefghij", 5), "abcd…");
    }
}
