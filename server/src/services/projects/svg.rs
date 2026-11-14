//! Sanitasi logo SVG (R1, keputusan pemilik: "SVG didukung dengan sanitasi + CSP ketat").
//!
//! Pendekatan **allowlist**, bukan blocklist: dokumen dibaca ulang lewat parser XML streaming dan
//! hanya elemen/atribut presentasi SVG yang dikenal yang ditulis kembali. Yang dibuang:
//! - elemen di luar daftar beserta seluruh isinya (`script`, `foreignObject`, `iframe`, `image`,
//!   `a`, `animate`/`set` yang bisa mengubah `href`, elemen ber-namespace lain, dst.);
//! - atribut di luar daftar, termasuk semua event handler `on*`;
//! - `href`/`xlink:href` yang bukan rujukan internal `#id` (jadi `use` eksternal hilang; `use`
//!   tanpa href internal dibuang seluruhnya);
//! - nilai berisi `url(...)` yang bukan `url(#id)`, `javascript:`, `expression(`, `@import`;
//! - DOCTYPE/deklarasi entity (cegah XXE dan billion laughs), processing instruction, komentar,
//!   entity bernama selain lima entity XML bawaan.
//!
//! Hasil sanitasi yang disimpan, bukan berkas asli. Lapisan kedua ada saat menyajikan: CSP
//! `default-src 'none'; style-src 'unsafe-inline'` + `nosniff` (controllers::projects::logo).

use std::fmt::Write;

use quick_xml::{
    Reader,
    events::{BytesStart, Event},
};

/// Elemen SVG yang dipertahankan (nama lokal, peka huruf besar seperti SVG).
const ELEMENTS: &[&str] = &[
    "svg",
    "g",
    "defs",
    "title",
    "desc",
    "symbol",
    "use",
    "path",
    "rect",
    "circle",
    "ellipse",
    "line",
    "polyline",
    "polygon",
    "text",
    "tspan",
    "linearGradient",
    "radialGradient",
    "stop",
    "clipPath",
    "mask",
    "pattern",
    "style",
    "filter",
    "feGaussianBlur",
    "feOffset",
    "feBlend",
    "feColorMatrix",
    "feMerge",
    "feMergeNode",
    "feFlood",
    "feComposite",
    "feDropShadow",
];

/// Atribut yang dipertahankan (nama lengkap, termasuk prefiks `xlink:`/`xmlns:`).
const ATTRIBUTES: &[&str] = &[
    "xmlns",
    "xmlns:xlink",
    "version",
    "id",
    "class",
    "style",
    "transform",
    "d",
    "x",
    "y",
    "x1",
    "y1",
    "x2",
    "y2",
    "cx",
    "cy",
    "r",
    "rx",
    "ry",
    "fx",
    "fy",
    "fr",
    "dx",
    "dy",
    "width",
    "height",
    "viewBox",
    "preserveAspectRatio",
    "points",
    "pathLength",
    "fill",
    "fill-opacity",
    "fill-rule",
    "clip-rule",
    "clip-path",
    "clipPathUnits",
    "mask",
    "maskUnits",
    "maskContentUnits",
    "stroke",
    "stroke-width",
    "stroke-linecap",
    "stroke-linejoin",
    "stroke-miterlimit",
    "stroke-dasharray",
    "stroke-dashoffset",
    "stroke-opacity",
    "opacity",
    "color",
    "display",
    "visibility",
    "offset",
    "stop-color",
    "stop-opacity",
    "gradientUnits",
    "gradientTransform",
    "spreadMethod",
    "patternUnits",
    "patternContentUnits",
    "patternTransform",
    "font-family",
    "font-size",
    "font-weight",
    "font-style",
    "text-anchor",
    "dominant-baseline",
    "letter-spacing",
    "filter",
    "filterUnits",
    "primitiveUnits",
    "in",
    "in2",
    "result",
    "stdDeviation",
    "mode",
    "values",
    "type",
    "operator",
    "k1",
    "k2",
    "k3",
    "k4",
    "flood-color",
    "flood-opacity",
    "href",
    "xlink:href",
];

const SVG_NS: &str = "http://www.w3.org/2000/svg";
const XLINK_NS: &str = "http://www.w3.org/1999/xlink";
/// Kedalaman elemen maksimal (cegah dokumen patologis).
const MAX_DEPTH: usize = 64;

/// SVG ditolak sanitasi (bukan SVG, rusak, atau terlalu dalam).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotSvg;

/// Berkas ini tampak seperti dokumen SVG (dipakai sebelum sanitasi untuk memilih jalur).
#[must_use]
pub fn looks_like_svg(data: &[u8]) -> bool {
    let head = &data[..data.len().min(1024)];
    std::str::from_utf8(head)
        .map(|s| s.to_ascii_lowercase().contains("<svg"))
        .unwrap_or(false)
}

/// Nilai berbahaya: skema skrip, CSS aktif, atau `url()` ke luar dokumen.
fn value_is_unsafe(value: &str) -> bool {
    let v: String = value
        .chars()
        .filter(|c| !c.is_whitespace() && !c.is_control())
        .collect::<String>()
        .to_ascii_lowercase();
    if v.contains("javascript:")
        || v.contains("vbscript:")
        || v.contains("data:")
        || v.contains("expression(")
        || v.contains("@import")
        || v.contains("behavior:")
        || v.contains("-moz-binding")
        || v.contains('<')
    {
        return true;
    }
    // Setiap url(...) harus rujukan internal url(#id).
    let mut rest = v.as_str();
    while let Some(i) = rest.find("url(") {
        let after = rest[i + 4..].trim_start_matches(['\'', '"']);
        if !after.starts_with('#') {
            return true;
        }
        rest = &rest[i + 4..];
    }
    false
}

fn escape_attr(v: &str) -> String {
    v.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn escape_text(v: &str) -> String {
    v.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Tulis tag pembuka elemen yang diizinkan; `None` = elemen dibuang (beserta isinya).
fn open_tag(e: &BytesStart<'_>, root: bool) -> Option<String> {
    let qname = AsRef::<str>::as_ref(&e.name()).to_string();
    // Elemen ber-prefiks namespace (mis. `svg:path`, `sodipodi:namedview`) tidak dikenal.
    if qname.contains(':') || !ELEMENTS.contains(&qname.as_str()) {
        return None;
    }
    let mut out = format!("<{qname}");
    let mut has_ns = false;
    let mut internal_href = false;
    for attr in e.attributes().with_checks(true) {
        let attr = attr.ok()?;
        let key = AsRef::<str>::as_ref(&attr.key).to_string();
        if !ATTRIBUTES.contains(&key.as_str()) {
            continue;
        }
        let value = attr
            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
            .ok()?
            .to_string();
        match key.as_str() {
            "xmlns" => {
                if value != SVG_NS {
                    continue;
                }
                has_ns = true;
            }
            "xmlns:xlink" if value != XLINK_NS => continue,
            "href" | "xlink:href" => {
                if !value.trim().starts_with('#') || value_is_unsafe(&value) {
                    continue;
                }
                internal_href = true;
            }
            _ if value_is_unsafe(&value) => continue,
            _ => {}
        }
        let _ = write!(out, " {key}=\"{}\"", escape_attr(&value));
    }
    // `use` hanya bermakna dengan rujukan internal; tanpa itu (eksternal dibuang) → buang.
    if qname == "use" && !internal_href {
        return None;
    }
    if root && !has_ns {
        let _ = write!(out, " xmlns=\"{SVG_NS}\"");
    }
    Some(out)
}

/// Sanitasi dokumen SVG. Hasilnya UTF-8 dengan root `<svg>`.
///
/// # Errors
///
/// [`NotSvg`]: bukan UTF-8, XML rusak, root bukan `svg`, terlalu dalam, atau kosong.
pub fn sanitize(data: &[u8]) -> Result<Vec<u8>, NotSvg> {
    let text = std::str::from_utf8(data).map_err(|_| NotSvg)?;
    let mut reader = Reader::from_str(text);
    reader.config_mut().check_end_names = true;
    let mut out = String::with_capacity(data.len());
    // Stack elemen terbuka: `Some(nama)` ditulis, `None` sedang dibuang.
    let mut stack: Vec<Option<String>> = Vec::new();
    let mut seen_root = false;
    let mut root_closed = false;
    loop {
        let event = reader.read_event().map_err(|_| NotSvg)?;
        let skipping = stack.iter().any(Option::is_none);
        let in_style = stack.last().and_then(|s| s.as_deref()) == Some("style");
        match event {
            Event::Start(e) | Event::Empty(e) if root_closed => {
                let _ = e;
                return Err(NotSvg);
            }
            Event::Start(e) => {
                if stack.len() >= MAX_DEPTH {
                    return Err(NotSvg);
                }
                let root = !seen_root;
                if root && AsRef::<str>::as_ref(&e.name()) != "svg" {
                    return Err(NotSvg);
                }
                seen_root = true;
                let tag = if skipping { None } else { open_tag(&e, root) };
                if root && tag.is_none() {
                    return Err(NotSvg);
                }
                if let Some(t) = &tag {
                    out.push_str(t);
                    out.push('>');
                }
                let name = AsRef::<str>::as_ref(&e.name()).to_string();
                stack.push(tag.map(|_| name));
            }
            Event::Empty(e) => {
                let root = !seen_root;
                if root && AsRef::<str>::as_ref(&e.name()) != "svg" {
                    return Err(NotSvg);
                }
                seen_root = true;
                if !skipping && let Some(t) = open_tag(&e, root) {
                    out.push_str(&t);
                    out.push_str("/>");
                }
                if root {
                    root_closed = true;
                }
            }
            Event::End(_) => {
                if let Some(Some(name)) = stack.pop() {
                    let _ = write!(out, "</{name}>");
                }
                if stack.is_empty() && seen_root {
                    root_closed = true;
                }
            }
            Event::Text(t) => {
                if !skipping && !stack.is_empty() {
                    let raw: &str = t.as_ref();
                    if in_style && value_is_unsafe(raw) {
                        // CSS aktif/eksternal di <style>: isi dibuang, elemen tetap.
                    } else {
                        // Teks mentah dari parser sudah ter-escape (entity dipisah jadi GeneralRef).
                        out.push_str(raw);
                    }
                }
            }
            Event::CData(c) => {
                if !skipping && in_style {
                    let raw: &str = c.as_ref();
                    if !value_is_unsafe(raw) {
                        out.push_str(&escape_text(raw));
                    }
                }
            }
            Event::GeneralRef(r) => {
                if !skipping && !stack.is_empty() {
                    let name: &str = r.as_ref();
                    if r.is_char_ref() {
                        if let Ok(Some(c)) = r.resolve_char_ref() {
                            out.push_str(&escape_text(&c.to_string()));
                        }
                    } else if matches!(name, "lt" | "gt" | "amp" | "quot" | "apos") {
                        let _ = write!(out, "&{name};");
                    }
                }
            }
            // Deklarasi XML, DOCTYPE (beserta entity-nya), PI, komentar: tidak ditulis.
            Event::Decl(_) | Event::DocType(_) | Event::PI(_) | Event::Comment(_) => {}
            Event::Eof => break,
        }
    }
    if !seen_root || !stack.is_empty() {
        return Err(NotSvg);
    }
    Ok(out.into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clean(s: &str) -> String {
        String::from_utf8(sanitize(s.as_bytes()).expect("svg valid")).unwrap()
    }

    #[test]
    fn keeps_plain_shapes_and_adds_namespace() {
        let out = clean(
            r##"<svg viewBox="0 0 10 10"><rect width="10" height="10" fill="#4f46e5"/><path d="M0 0L10 10"/></svg>"##,
        );
        assert_eq!(
            out,
            r##"<svg viewBox="0 0 10 10" xmlns="http://www.w3.org/2000/svg"><rect width="10" height="10" fill="#4f46e5"/><path d="M0 0L10 10"/></svg>"##
        );
    }

    #[test]
    fn strips_scripts_handlers_foreign_object_and_external_refs() {
        let out = clean(
            r##"<?xml version="1.0"?><!DOCTYPE svg [<!ENTITY x "boom">]>
<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" onload="alert(1)">
<script>alert(1)</script><script xlink:href="https://evil.test/x.js"/>
<foreignObject><div xmlns="http://www.w3.org/1999/xhtml"><img src="x" onerror="alert(1)"/></div></foreignObject>
<a href="javascript:alert(1)"><text>klik</text></a>
<use href="https://evil.test/sprite.svg#a"/><use xlink:href="#ok"/>
<image href="https://evil.test/x.png"/>
<rect fill="url(https://evil.test/p)" style="background:url(//evil.test)" onclick="x()"/>
<circle r="1" fill="url(#grad)"/>
<animate attributeName="href" to="javascript:alert(1)"/>
<style>@import url(https://evil.test/a.css);</style>
<text>&x; &amp; &#65;</text>
<!-- komentar --></svg>"##,
        );
        for bad in [
            "script",
            "onload",
            "onerror",
            "onclick",
            "foreignObject",
            "javascript",
            "evil.test",
            "<a",
            "<image",
            "animate",
            "@import",
            "boom",
            "komentar",
            "DOCTYPE",
        ] {
            assert!(!out.contains(bad), "{bad} lolos: {out}");
        }
        assert!(out.contains(r##"<use xlink:href="#ok"/>"##), "{out}");
        assert!(out.contains(r##"fill="url(#grad)""##), "{out}");
        assert!(out.contains("<text> &amp; A</text>"), "{out}");
        assert!(out.contains("<rect/>"), "{out}");
    }

    #[test]
    fn rejects_non_svg_root_broken_xml_and_trailing_elements() {
        for bad in [
            "<html><svg/></html>",
            "<svg><g></svg>",
            "bukan xml",
            "",
            "<svg/><script>alert(1)</script>",
            "<svg:svg xmlns:svg='http://www.w3.org/2000/svg'/>",
        ] {
            assert_eq!(sanitize(bad.as_bytes()), Err(NotSvg), "{bad}");
        }
        assert!(sanitize(&[0xff, 0xfe, b'<']).is_err());
    }

    #[test]
    fn detects_svg_by_content() {
        assert!(looks_like_svg(b"<?xml version='1.0'?>\n<svg xmlns='x'/>"));
        assert!(looks_like_svg(b"<SVG/>"));
        assert!(!looks_like_svg(b"\x89PNG\r\n\x1a\n"));
    }
}
