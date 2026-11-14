//! Pesan flash: disimpan di sesi oleh satu POST, dibaca sekali oleh GET berikutnya
//! (`controllers::flash`), dan dirender komponen `flash` di layout. Isinya kunci Fluent,
//! bukan kalimat.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FlashKind {
    Info,
    Success,
    /// Peringatan (aksi berhasil sebagian, mis. rilis tersimpan tetapi git tag tidak dibuat).
    Warn,
    Error,
}

/// Variabel `flash` di layout: `kind` (`info|success|error`, varian komponen `flash`) dan
/// `key` (kunci Fluent pesannya).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Flash {
    pub kind: FlashKind,
    pub key: String,
    /// Kalimat yang sudah diterjemahkan dengan argumen (mis. nama kolom dan batas WIP, R3);
    /// bila ada, layout memakainya menggantikan `key`.
    #[serde(default)]
    pub text: Option<String>,
}

impl Flash {
    #[must_use]
    pub fn success(key: &str) -> Self {
        Self {
            kind: FlashKind::Success,
            key: key.to_string(),
            text: None,
        }
    }

    #[must_use]
    pub fn warn(key: &str) -> Self {
        Self {
            kind: FlashKind::Warn,
            key: key.to_string(),
            text: None,
        }
    }

    #[must_use]
    pub fn error(key: &str) -> Self {
        Self {
            kind: FlashKind::Error,
            key: key.to_string(),
            text: None,
        }
    }

    /// Error dengan kalimat jadi (`key` tetap disimpan sebagai kode untuk tes/penanda).
    #[must_use]
    pub fn error_text(key: &str, text: String) -> Self {
        Self {
            kind: FlashKind::Error,
            key: key.to_string(),
            text: Some(text),
        }
    }
}
