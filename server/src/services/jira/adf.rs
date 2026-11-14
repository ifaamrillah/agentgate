//! Atlassian Document Format (deskripsi dan komentar Jira Cloud REST v3) ↔ teks Markdown
//! sederhana Agent Gate. Konversi sengaja kecil: paragraf, judul, daftar berbutir/bernomor,
//! blok kode, kutipan, baris baru, mention, dan tanda inline (tebal, miring, kode, tautan) saat
//! membaca. Saat menulis, teks inline dikirim apa adanya (tanpa mengurai tanda Markdown), jadi
//! `to_text(from_text(t))` stabil setelah satu putaran: sinkron dua arah tidak berayun.

use serde_json::{Value, json};

/// ADF → teks Markdown sederhana. Nilai bukan dokumen ADF (mis. string dari Data Center) →
/// apa adanya.
#[must_use]
pub fn to_text(doc: &Value) -> String {
    match doc {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Object(_) => blocks(doc.get("content")).join("\n\n").trim().to_string(),
        _ => String::new(),
    }
}

fn children(v: Option<&Value>) -> &[Value] {
    v.and_then(Value::as_array).map_or(&[], Vec::as_slice)
}

fn blocks(content: Option<&Value>) -> Vec<String> {
    children(content).iter().filter_map(block).collect()
}

fn block(node: &Value) -> Option<String> {
    let kind = node.get("type").and_then(Value::as_str).unwrap_or_default();
    let content = node.get("content");
    Some(match kind {
        "paragraph" => inline(content),
        "heading" => {
            let level = node
                .pointer("/attrs/level")
                .and_then(Value::as_u64)
                .unwrap_or(1)
                .clamp(1, 6);
            format!(
                "{} {}",
                "#".repeat(usize::try_from(level).unwrap_or(1)),
                inline(content)
            )
        }
        "bulletList" | "orderedList" => {
            let ordered = kind == "orderedList";
            children(content)
                .iter()
                .enumerate()
                .map(|(i, item)| {
                    let text = blocks(item.get("content")).join("\n");
                    if ordered {
                        format!("{}. {text}", i + 1)
                    } else {
                        format!("- {text}")
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
        "codeBlock" => format!("```\n{}\n```", plain(content)),
        "blockquote" => blocks(content)
            .join("\n\n")
            .lines()
            .map(|l| format!("> {l}"))
            .collect::<Vec<_>>()
            .join("\n"),
        "rule" => "---".to_string(),
        "mediaSingle" | "mediaGroup" => return None,
        _ => {
            let text = inline(content);
            if text.is_empty() {
                return None;
            }
            text
        }
    })
}

fn plain(content: Option<&Value>) -> String {
    children(content)
        .iter()
        .map(|n| n.get("text").and_then(Value::as_str).unwrap_or_default())
        .collect()
}

fn inline(content: Option<&Value>) -> String {
    let mut out = String::new();
    for node in children(content) {
        match node.get("type").and_then(Value::as_str).unwrap_or_default() {
            "text" => {
                let mut text = node
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                for mark in children(node.get("marks")) {
                    text = match mark.get("type").and_then(Value::as_str).unwrap_or_default() {
                        "strong" => format!("**{text}**"),
                        "em" => format!("*{text}*"),
                        "code" => format!("`{text}`"),
                        "strike" => format!("~~{text}~~"),
                        "link" => {
                            let href = mark
                                .pointer("/attrs/href")
                                .and_then(Value::as_str)
                                .unwrap_or_default();
                            format!("[{text}]({href})")
                        }
                        _ => text,
                    };
                }
                out.push_str(&text);
            }
            "hardBreak" => out.push('\n'),
            "mention" => {
                let name = node
                    .pointer("/attrs/text")
                    .and_then(Value::as_str)
                    .unwrap_or("@?");
                if name.starts_with('@') {
                    out.push_str(name);
                } else {
                    out.push('@');
                    out.push_str(name);
                }
            }
            "emoji" => out.push_str(
                node.pointer("/attrs/text")
                    .or_else(|| node.pointer("/attrs/shortName"))
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            ),
            "inlineCard" => out.push_str(
                node.pointer("/attrs/url")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            ),
            _ => out.push_str(&inline(node.get("content"))),
        }
    }
    out
}

/// Teks → dokumen ADF (paragraf per blok yang dipisah baris kosong; daftar `- `/`1. `, judul
/// `#`, blok kode ```` ``` ````; baris dalam paragraf menjadi `hardBreak`).
#[must_use]
pub fn from_text(text: &str) -> Value {
    let mut content = Vec::new();
    let normalized = text.replace("\r\n", "\n");
    let mut lines = normalized.lines().peekable();
    let mut para: Vec<&str> = Vec::new();
    let flush = |para: &mut Vec<&str>, content: &mut Vec<Value>| {
        if !para.is_empty() {
            content.push(paragraph(para));
            para.clear();
        }
    };
    while let Some(line) = lines.next() {
        let trimmed = line.trim_end();
        if trimmed.trim().is_empty() {
            flush(&mut para, &mut content);
            continue;
        }
        if trimmed.trim_start().starts_with("```") {
            flush(&mut para, &mut content);
            let mut code = Vec::new();
            for l in lines.by_ref() {
                if l.trim_start().starts_with("```") {
                    break;
                }
                code.push(l);
            }
            let text = code.join("\n");
            content.push(if text.is_empty() {
                json!({ "type": "codeBlock" })
            } else {
                json!({ "type": "codeBlock", "content": [{ "type": "text", "text": text }] })
            });
            continue;
        }
        if para.is_empty() {
            if let Some((level, rest)) = heading(trimmed) {
                content.push(json!({
                    "type": "heading", "attrs": { "level": level },
                    "content": text_nodes(rest),
                }));
                continue;
            }
            if let Some(kind) = list_kind(trimmed) {
                let mut items = vec![list_item(trimmed)];
                while let Some(next) = lines.peek() {
                    if list_kind(next.trim_end()) == Some(kind) {
                        items.push(list_item(next.trim_end()));
                        lines.next();
                    } else {
                        break;
                    }
                }
                content.push(json!({ "type": kind, "content": items }));
                continue;
            }
        }
        para.push(trimmed);
    }
    flush(&mut para, &mut content);
    json!({ "type": "doc", "version": 1, "content": content })
}

fn heading(line: &str) -> Option<(usize, &str)> {
    let hashes = line.chars().take_while(|c| *c == '#').count();
    if (1..=6).contains(&hashes) {
        line[hashes..]
            .strip_prefix(' ')
            .map(|rest| (hashes, rest.trim()))
    } else {
        None
    }
}

fn list_kind(line: &str) -> Option<&'static str> {
    let l = line.trim_start();
    if l.starts_with("- ") || l.starts_with("* ") {
        return Some("bulletList");
    }
    let digits = l.chars().take_while(char::is_ascii_digit).count();
    (digits > 0 && l[digits..].starts_with(". ")).then_some("orderedList")
}

fn list_item(line: &str) -> Value {
    let l = line.trim_start();
    let text = if l.starts_with("- ") || l.starts_with("* ") {
        &l[2..]
    } else {
        let digits = l.chars().take_while(char::is_ascii_digit).count();
        &l[digits + 2..]
    };
    json!({ "type": "listItem", "content": [{ "type": "paragraph", "content": text_nodes(text) }] })
}

fn text_nodes(text: &str) -> Vec<Value> {
    if text.is_empty() {
        Vec::new()
    } else {
        vec![json!({ "type": "text", "text": text })]
    }
}

fn paragraph(lines: &[&str]) -> Value {
    let mut nodes = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        if i > 0 {
            nodes.push(json!({ "type": "hardBreak" }));
        }
        nodes.extend(text_nodes(l));
    }
    json!({ "type": "paragraph", "content": nodes })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_common_nodes() {
        let doc = json!({ "type": "doc", "version": 1, "content": [
            { "type": "heading", "attrs": { "level": 2 }, "content": [{ "type": "text", "text": "Latar" }] },
            { "type": "paragraph", "content": [
                { "type": "text", "text": "Bayar " },
                { "type": "text", "text": "QRIS", "marks": [{ "type": "strong" }] },
                { "type": "hardBreak" },
                { "type": "mention", "attrs": { "text": "@Dewi" } },
            ]},
            { "type": "bulletList", "content": [
                { "type": "listItem", "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "a" }] }] },
                { "type": "listItem", "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "b" }] }] },
            ]},
            { "type": "codeBlock", "content": [{ "type": "text", "text": "x = 1" }] },
        ]});
        assert_eq!(
            to_text(&doc),
            "## Latar\n\nBayar **QRIS**\n@Dewi\n\n- a\n- b\n\n```\nx = 1\n```"
        );
    }

    #[test]
    fn text_round_trip_is_stable() {
        for text in [
            "Satu paragraf",
            "Baris satu\nbaris dua\n\nParagraf dua",
            "# Judul\n\n- a\n- b\n\n1. x\n2. y\n\n```\nkode\n```",
            "Pakai **tebal** dan `kode`",
        ] {
            assert_eq!(to_text(&from_text(text)), text, "{text}");
        }
        assert_eq!(to_text(&from_text("")), "");
    }

    #[test]
    fn plain_strings_pass_through() {
        assert_eq!(to_text(&json!("teks wiki")), "teks wiki");
        assert_eq!(to_text(&Value::Null), "");
    }
}
