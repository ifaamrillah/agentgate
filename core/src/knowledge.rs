//! Format file knowledge di repo (`.agentgate/knowledge/<slug>.md`), dipakai bersama oleh CLI
//! (`agentgate knowledge pull|sync`) dan server (PR knowledge, R2) supaya keduanya menulis isi
//! yang sama persis.

/// Folder knowledge di repo.
pub const DIR: &str = ".agentgate/knowledge";

/// Path file dokumen `slug` relatif terhadap akar repo.
#[must_use]
pub fn path(slug: &str) -> String {
    format!("{DIR}/{slug}.md")
}

/// Isi file `.md`: front matter (judul, jenis, kelompok, versi server) lalu isi dokumen.
#[must_use]
pub fn render(title: &str, type_key: &str, group: &str, version: i32, body: &str) -> String {
    format!(
        "---\ntitle: {}\ntype: {}\ngroup: {}\nversion: {}\n---\n\n{}\n",
        title.replace('\n', " "),
        type_key,
        group,
        version,
        body.trim_end()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_front_matter_and_path() {
        assert_eq!(path("api"), ".agentgate/knowledge/api.md");
        assert_eq!(
            render("API\nv2", "api", "backend", 3, "# Isi\n\n"),
            "---\ntitle: API v2\ntype: api\ngroup: backend\nversion: 3\n---\n\n# Isi\n"
        );
    }
}
