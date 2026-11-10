//! Ikon SVG inline. Semua file `assets/icons/*.svg` dibaca sekali saat boot; template
//! memanggil `icon(name="trash", class="size-4")`. Nama yang tidak dikenal adalah error,
//! supaya ikon salah ketik gagal di tes, bukan tampil kosong di produksi.

use std::{collections::HashMap, path::Path};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum IconError {
    #[error("cannot read icon directory {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("icon file {0} is not an <svg> element")]
    NotSvg(String),
}

/// Satu ikon: atribut `<svg>` asli (tanpa class/width/height) dan isi elemennya.
#[derive(Debug, Clone)]
struct Icon {
    attrs: String,
    inner: String,
}

#[derive(Debug, Clone, Default)]
pub struct IconSet {
    icons: HashMap<String, Icon>,
}

impl IconSet {
    /// Muat semua `*.svg` di `dir` (datar, tanpa subfolder). Nama ikon = nama file tanpa ekstensi.
    pub fn load(dir: &Path) -> Result<Self, IconError> {
        let io = |source| IconError::Io {
            path: dir.display().to_string(),
            source,
        };
        let mut icons = HashMap::new();
        for entry in std::fs::read_dir(dir).map_err(io)? {
            let path = entry.map_err(io)?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("svg") {
                continue;
            }
            let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let source = std::fs::read_to_string(&path).map_err(io)?;
            let icon =
                parse_svg(&source).ok_or_else(|| IconError::NotSvg(path.display().to_string()))?;
            icons.insert(name.to_string(), icon);
        }
        Ok(Self { icons })
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.icons.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.icons.is_empty()
    }

    /// SVG siap sisip, atau `None` bila nama tidak dikenal. Ikon selalu dekoratif
    /// (`aria-hidden`); tombol yang hanya berisi ikon memberi `aria-label` sendiri.
    #[must_use]
    pub fn render(&self, name: &str, class: &str) -> Option<String> {
        let icon = self.icons.get(name)?;
        let class = escape_attr(class);
        Some(format!(
            r#"<svg {} class="{class}" aria-hidden="true" focusable="false">{}</svg>"#,
            icon.attrs, icon.inner
        ))
    }
}

/// Pisahkan tag pembuka `<svg ...>` dari isinya, buang atribut yang diatur pemanggil.
fn parse_svg(source: &str) -> Option<Icon> {
    let start = source.find("<svg")?;
    let open_end = start + source[start..].find('>')?;
    let close = source.rfind("</svg>")?;
    let attrs = parse_attrs(&source[start + 4..open_end])
        .into_iter()
        .filter(|(name, _)| !matches!(name.as_str(), "class" | "width" | "height"))
        .map(|(name, value)| format!(r#"{name}="{value}""#))
        .collect::<Vec<_>>()
        .join(" ");
    let inner = source[open_end + 1..close]
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<String>();
    Some(Icon { attrs, inner })
}

/// Atribut `name="value"` dari isi tag pembuka. Nilai boleh berisi spasi.
fn parse_attrs(s: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = s.trim();
    while let Some(eq) = rest.find('=') {
        let name = rest[..eq].trim().to_string();
        let after = rest[eq + 1..].trim_start();
        let Some(after) = after.strip_prefix('"') else {
            break;
        };
        let Some(end) = after.find('"') else {
            break;
        };
        out.push((name, after[..end].to_string()));
        rest = after[end + 1..].trim_start();
    }
    out
}

fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHECK: &str = r#"<svg
  xmlns="http://www.w3.org/2000/svg"
  width="24"
  height="24"
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  class="icon icon-tabler"
>
  <path d="M5 12l5 5l10 -10" />
</svg>"#;

    #[test]
    fn render_replaces_class_and_size_and_hides_from_screen_readers() {
        let mut set = IconSet::default();
        set.icons.insert("check".into(), parse_svg(CHECK).unwrap());
        let svg = set.render("check", "size-4").unwrap();
        assert_eq!(
            svg,
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" class="size-4" aria-hidden="true" focusable="false"><path d="M5 12l5 5l10 -10" /></svg>"#
        );
    }

    #[test]
    fn unknown_icon_is_none() {
        assert!(IconSet::default().render("nope", "").is_none());
    }

    #[test]
    fn class_is_escaped() {
        let mut set = IconSet::default();
        set.icons.insert("check".into(), parse_svg(CHECK).unwrap());
        let svg = set.render("check", r#"a" onload="x"#).unwrap();
        assert!(svg.contains(r#"class="a&quot; onload=&quot;x""#));
    }
}
