//! Setiap kunci terjemahan yang dipakai template dan kode punya entri di Fluent.

use std::{collections::HashSet, path::Path};

use agentgate_server::views::errors::MESSAGE_KEYS;

fn fluent_keys() -> HashSet<String> {
    let src = std::fs::read_to_string("assets/i18n/id/main.ftl").unwrap();
    src.lines()
        .filter_map(|l| l.split_once(" = ").map(|(k, _)| k.trim().to_string()))
        .filter(|k| !k.starts_with('#') && !k.starts_with(' '))
        .collect()
}

fn template_keys(dir: &Path, out: &mut Vec<(String, String)>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            template_keys(&path, out);
        } else if path.extension().is_some_and(|e| e == "html") {
            let src = std::fs::read_to_string(&path).unwrap();
            for part in src.split("t(key=\"").skip(1) {
                if let Some(end) = part.find('"') {
                    out.push((path.display().to_string(), part[..end].to_string()));
                }
            }
        }
    }
}

#[test]
fn every_literal_template_key_has_a_translation() {
    let keys = fluent_keys();
    let mut used = Vec::new();
    template_keys(Path::new("assets/views"), &mut used);
    assert!(!used.is_empty());
    let missing: Vec<_> = used.iter().filter(|(_, k)| !keys.contains(k)).collect();
    assert!(missing.is_empty(), "kunci tanpa terjemahan: {missing:?}");
}

#[test]
fn every_error_message_key_has_title_and_body() {
    let keys = fluent_keys();
    for key in MESSAGE_KEYS {
        for suffix in ["-title", "-body"] {
            assert!(keys.contains(&format!("{key}{suffix}")), "{key}{suffix}");
        }
    }
}
