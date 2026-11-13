//! Fakta per file hasil parsing (disimpan server sebagai jsonb, dibaca ulang saat graph dibangun
//! ulang tanpa parsing ulang). Baris 1-based.
use serde::{Deserialize, Serialize};

use crate::lang::Lang;

/// Jenis simbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    File,
    Function,
    Method,
    Class,
    Interface,
    Struct,
    Enum,
    Trait,
    Type,
    Module,
    Route,
}

impl SymbolKind {
    pub const ALL: [Self; 11] = [
        Self::File,
        Self::Function,
        Self::Method,
        Self::Class,
        Self::Interface,
        Self::Struct,
        Self::Enum,
        Self::Trait,
        Self::Type,
        Self::Module,
        Self::Route,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Function => "function",
            Self::Method => "method",
            Self::Class => "class",
            Self::Interface => "interface",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Trait => "trait",
            Self::Type => "type",
            Self::Module => "module",
            Self::Route => "route",
        }
    }

    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == s)
    }

    /// Bisa dipanggil (fungsi/method).
    #[must_use]
    pub const fn is_callable(self) -> bool {
        matches!(self, Self::Function | Self::Method)
    }

    /// Tipe (target `new X()` / pewarisan).
    #[must_use]
    pub const fn is_type(self) -> bool {
        matches!(
            self,
            Self::Class | Self::Interface | Self::Struct | Self::Enum | Self::Trait | Self::Type
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolFact {
    pub name: String,
    pub kind: SymbolKind,
    pub start: u32,
    pub end: u32,
    /// Indeks simbol pembungkus (kelas untuk method).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<u32>,
    /// Nama tipe pemilik di luar blok kelas (receiver Go, `impl` Rust).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub owner: String,
    pub exported: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub test: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub doc: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub signature: String,
    /// Kompleksitas siklomatik kasar (1 + cabang).
    #[serde(default)]
    pub complexity: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallFact {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub receiver: String,
    pub line: u32,
    /// Simbol pembungkus (`None` = level file).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportFact {
    pub source: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub names: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub alias: String,
    pub line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InheritFact {
    /// Nama tipe yang mewarisi (dicocokkan ke simbol saat build, bisa di file lain: `impl` Rust).
    pub owner: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<u32>,
    pub base: String,
    pub line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteFact {
    /// `GET`, `POST`, … atau `ANY`.
    pub method: String,
    /// Path ternormalisasi (`/v1/stock/:p`).
    pub path: String,
    pub line: u32,
    /// Fungsi handler (simbol file ini) bila diketahui.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handler: Option<u32>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub handler_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpFact {
    pub method: String,
    pub path: String,
    /// URL literal utuh (bukan template/variabel) → relasi pasti.
    pub exact: bool,
    /// Host URL absolut (kosong = relatif/template), mis. `api.xendit.co`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub host: String,
    pub line: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventFact {
    pub topic: String,
    pub publish: bool,
    pub line: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<u32>,
}

/// Paket yang didefinisikan file manifest beserta dependensinya.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageFact {
    pub name: String,
    pub deps: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileFacts {
    pub lang: Lang,
    pub lines: u32,
    #[serde(default)]
    pub symbols: Vec<SymbolFact>,
    #[serde(default)]
    pub calls: Vec<CallFact>,
    #[serde(default)]
    pub imports: Vec<ImportFact>,
    #[serde(default)]
    pub inherits: Vec<InheritFact>,
    #[serde(default)]
    pub routes: Vec<RouteFact>,
    #[serde(default)]
    pub http: Vec<HttpFact>,
    #[serde(default)]
    pub events: Vec<EventFact>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package: Option<PackageFact>,
    #[serde(default)]
    pub todos: u32,
    /// Jumlah node ERROR tree-sitter (0 = parsing bersih).
    #[serde(default)]
    pub errors: u32,
}

impl FileFacts {
    #[must_use]
    pub const fn empty(lang: Lang, lines: u32) -> Self {
        Self {
            lang,
            lines,
            symbols: Vec::new(),
            calls: Vec::new(),
            imports: Vec::new(),
            inherits: Vec::new(),
            routes: Vec::new(),
            http: Vec::new(),
            events: Vec::new(),
            package: None,
            todos: 0,
            errors: 0,
        }
    }
}
