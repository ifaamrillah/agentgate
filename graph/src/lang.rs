//! Bahasa yang dikenal dan grammar tree-sitter-nya.
use serde::{Deserialize, Serialize};

/// Bahasa file yang di-index. `Manifest` = file paket (package.json, go.mod, …) yang tidak diparse
/// tree-sitter tetapi dibaca untuk relasi shared package lintas repo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lang {
    TypeScript,
    Tsx,
    JavaScript,
    Python,
    Go,
    Rust,
    Java,
    Kotlin,
    Php,
    CSharp,
    Dart,
    Manifest,
}

impl Lang {
    pub const ALL: [Self; 12] = [
        Self::TypeScript,
        Self::Tsx,
        Self::JavaScript,
        Self::Python,
        Self::Go,
        Self::Rust,
        Self::Java,
        Self::Kotlin,
        Self::Php,
        Self::CSharp,
        Self::Dart,
        Self::Manifest,
    ];

    /// Bahasa dari path file (ekstensi atau nama file manifest).
    #[must_use]
    pub fn detect(path: &str) -> Option<Self> {
        let name = path.rsplit('/').next().unwrap_or(path);
        if matches!(
            name,
            "package.json" | "go.mod" | "Cargo.toml" | "composer.json" | "pubspec.yaml"
        ) {
            return Some(Self::Manifest);
        }
        let ext = name.rsplit_once('.')?.1.to_ascii_lowercase();
        Some(match ext.as_str() {
            "ts" | "mts" | "cts" => Self::TypeScript,
            "tsx" => Self::Tsx,
            "js" | "jsx" | "mjs" | "cjs" => Self::JavaScript,
            "py" | "pyi" => Self::Python,
            "go" => Self::Go,
            "rs" => Self::Rust,
            "java" => Self::Java,
            "kt" | "kts" => Self::Kotlin,
            "php" => Self::Php,
            "cs" => Self::CSharp,
            "dart" => Self::Dart,
            _ => return None,
        })
    }

    /// Kunci stabil (disimpan di DB).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TypeScript => "typescript",
            Self::Tsx => "tsx",
            Self::JavaScript => "javascript",
            Self::Python => "python",
            Self::Go => "go",
            Self::Rust => "rust",
            Self::Java => "java",
            Self::Kotlin => "kotlin",
            Self::Php => "php",
            Self::CSharp => "csharp",
            Self::Dart => "dart",
            Self::Manifest => "manifest",
        }
    }

    /// Nama tampilan.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::TypeScript | Self::Tsx => "TypeScript",
            Self::JavaScript => "JavaScript",
            Self::Python => "Python",
            Self::Go => "Go",
            Self::Rust => "Rust",
            Self::Java => "Java",
            Self::Kotlin => "Kotlin",
            Self::Php => "PHP",
            Self::CSharp => "C#",
            Self::Dart => "Dart",
            Self::Manifest => "Manifest",
        }
    }

    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|l| l.as_str() == s)
    }

    /// Grammar tree-sitter (`None` untuk manifest).
    #[must_use]
    pub fn grammar(self) -> Option<tree_sitter::Language> {
        Some(match self {
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Self::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
            Self::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Self::Python => tree_sitter_python::LANGUAGE.into(),
            Self::Go => tree_sitter_go::LANGUAGE.into(),
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
            Self::Java => tree_sitter_java::LANGUAGE.into(),
            Self::Kotlin => tree_sitter_kotlin_ng::LANGUAGE.into(),
            Self::Php => tree_sitter_php::LANGUAGE_PHP.into(),
            Self::CSharp => tree_sitter_c_sharp::LANGUAGE.into(),
            Self::Dart => tree_sitter_dart::LANGUAGE.into(),
            Self::Manifest => return None,
        })
    }

    /// Query definisi/call/import/pewarisan (`queries/*.scm`, ditulis per grammar).
    #[must_use]
    pub const fn query(self) -> &'static str {
        match self {
            Self::TypeScript | Self::Tsx => include_str!("queries/typescript.scm"),
            Self::JavaScript => include_str!("queries/javascript.scm"),
            Self::Python => include_str!("queries/python.scm"),
            Self::Go => include_str!("queries/go.scm"),
            Self::Rust => include_str!("queries/rust.scm"),
            Self::Java => include_str!("queries/java.scm"),
            Self::Kotlin => include_str!("queries/kotlin.scm"),
            Self::Php => include_str!("queries/php.scm"),
            Self::CSharp => include_str!("queries/csharp.scm"),
            Self::Dart => include_str!("queries/dart.scm"),
            Self::Manifest => "",
        }
    }

    /// Paket = direktori (Go, Java, Kotlin, C#, Dart): simbol satu direktori saling terlihat tanpa
    /// import.
    #[must_use]
    pub const fn package_is_dir(self) -> bool {
        matches!(
            self,
            Self::Go | Self::Java | Self::Kotlin | Self::CSharp | Self::Dart
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_priority_languages_and_manifests() {
        assert_eq!(Lang::detect("src/a.ts"), Some(Lang::TypeScript));
        assert_eq!(Lang::detect("src/A.tsx"), Some(Lang::Tsx));
        assert_eq!(Lang::detect("x/y.MJS"), Some(Lang::JavaScript));
        assert_eq!(Lang::detect("app.py"), Some(Lang::Python));
        assert_eq!(Lang::detect("main.go"), Some(Lang::Go));
        assert_eq!(Lang::detect("lib.rs"), Some(Lang::Rust));
        assert_eq!(Lang::detect("A.java"), Some(Lang::Java));
        assert_eq!(Lang::detect("A.kt"), Some(Lang::Kotlin));
        assert_eq!(Lang::detect("index.php"), Some(Lang::Php));
        assert_eq!(Lang::detect("A.cs"), Some(Lang::CSharp));
        assert_eq!(Lang::detect("main.dart"), Some(Lang::Dart));
        assert_eq!(Lang::detect("web/package.json"), Some(Lang::Manifest));
        assert_eq!(Lang::detect("README.md"), None);
        assert_eq!(Lang::detect("Makefile"), None);
    }

    #[test]
    fn every_grammar_loads_and_every_query_compiles() {
        for lang in Lang::ALL {
            assert_eq!(Lang::parse(lang.as_str()), Some(lang));
            let Some(grammar) = lang.grammar() else {
                continue;
            };
            let mut parser = tree_sitter::Parser::new();
            assert!(parser.set_language(&grammar).is_ok(), "{lang:?}");
            if let Err(e) = tree_sitter::Query::new(&grammar, lang.query()) {
                panic!("{lang:?}: {e}");
            }
        }
    }
}
