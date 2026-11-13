//! File manifest paket → nama paket + dependensi (relasi shared package lintas repo).
use crate::facts::PackageFact;

/// Baca manifest (`package.json`, `go.mod`, `Cargo.toml`, `composer.json`, `pubspec.yaml`).
#[must_use]
pub fn parse(path: &str, source: &str) -> Option<PackageFact> {
    let name = path.rsplit('/').next().unwrap_or(path);
    match name {
        "package.json" | "composer.json" => {
            let v: serde_json::Value = serde_json::from_str(source).ok()?;
            let pkg = v.get("name")?.as_str()?.to_string();
            let mut deps = Vec::new();
            for key in [
                "dependencies",
                "devDependencies",
                "peerDependencies",
                "require",
                "require-dev",
            ] {
                if let Some(obj) = v.get(key).and_then(|d| d.as_object()) {
                    deps.extend(obj.keys().cloned());
                }
            }
            Some(finish(pkg, deps))
        }
        "go.mod" => {
            let mut pkg = String::new();
            let mut deps = Vec::new();
            let mut in_req = false;
            for l in source.lines() {
                let l = l.trim();
                if let Some(m) = l.strip_prefix("module ") {
                    pkg = m.trim().to_string();
                } else if l.starts_with("require (") {
                    in_req = true;
                } else if in_req && l == ")" {
                    in_req = false;
                } else if let Some(r) = l.strip_prefix("require ") {
                    deps.extend(r.split_whitespace().next().map(str::to_string));
                } else if in_req {
                    deps.extend(l.split_whitespace().next().map(str::to_string));
                }
            }
            (!pkg.is_empty()).then(|| finish(pkg, deps))
        }
        "Cargo.toml" => {
            let mut pkg = String::new();
            let mut deps = Vec::new();
            let mut section = String::new();
            for l in source.lines() {
                let l = l.trim();
                if l.starts_with('[') {
                    section = l.trim_matches(['[', ']']).to_string();
                    continue;
                }
                let Some((k, v)) = l.split_once('=') else {
                    continue;
                };
                let k = k.trim();
                if section == "package" && k == "name" {
                    pkg = v.trim().trim_matches('"').to_string();
                } else if section.ends_with("dependencies") && !k.is_empty() && !k.contains('.') {
                    deps.push(k.to_string());
                }
            }
            (!pkg.is_empty()).then(|| finish(pkg, deps))
        }
        "pubspec.yaml" => {
            let mut pkg = String::new();
            let mut deps = Vec::new();
            let mut in_deps = false;
            for l in source.lines() {
                if l.trim().is_empty() || l.trim_start().starts_with('#') {
                    continue;
                }
                let indented = l.starts_with(' ');
                if !indented {
                    in_deps = l.starts_with("dependencies:") || l.starts_with("dev_dependencies:");
                    if let Some(n) = l.strip_prefix("name:") {
                        pkg = n.trim().to_string();
                    }
                } else if in_deps
                    && l.starts_with("  ")
                    && !l.starts_with("   ")
                    && let Some((k, _)) = l.trim().split_once(':')
                {
                    deps.push(k.to_string());
                }
            }
            (!pkg.is_empty()).then(|| finish(pkg, deps))
        }
        _ => None,
    }
}

fn finish(name: String, mut deps: Vec<String>) -> PackageFact {
    deps.sort();
    deps.dedup();
    PackageFact { name, deps }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_each_manifest_kind() {
        let p = parse(
            "web/package.json",
            r#"{"name":"@acme/web","dependencies":{"@acme/printer":"1","react":"18"}}"#,
        )
        .unwrap_or_else(|| PackageFact {
            name: String::new(),
            deps: vec![],
        });
        assert_eq!(p.name, "@acme/web");
        assert_eq!(p.deps, vec!["@acme/printer", "react"]);
        let g = parse(
            "go.mod",
            "module github.com/acme/api\n\nrequire (\n\tgithub.com/acme/shared v1.0.0\n)\nrequire golang.org/x/text v0.1\n",
        );
        assert_eq!(
            g.map(|g| (g.name, g.deps)),
            Some((
                "github.com/acme/api".into(),
                vec!["github.com/acme/shared".into(), "golang.org/x/text".into()]
            ))
        );
        let c = parse(
            "Cargo.toml",
            "[package]\nname = \"acme-core\"\n\n[dependencies]\nserde = \"1\"\nacme-shared = { path = \"../s\" }\n",
        );
        assert_eq!(
            c.map(|c| (c.name, c.deps)),
            Some((
                "acme-core".into(),
                vec!["acme-shared".into(), "serde".into()]
            ))
        );
        let d = parse(
            "pubspec.yaml",
            "name: kasir_app\ndependencies:\n  flutter:\n    sdk: flutter\n  kasir_core: ^1.0.0\n",
        );
        assert_eq!(
            d.map(|d| (d.name, d.deps)),
            Some((
                "kasir_app".into(),
                vec!["flutter".into(), "kasir_core".into()]
            ))
        );
        assert!(parse("package.json", "{oops").is_none());
    }
}
