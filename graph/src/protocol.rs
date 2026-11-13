//! Protokol CLI ↔ server untuk index code graph (`/api/cli/graph/*`). Alur inkremental:
//! `manifest` (path + sha semua file) → server menjawab file mana yang perlu dikirim → `upload`
//! per batch → `finish` (statistik git) → server build graph. SCIP dikirim terpisah lewat `scip`.
use serde::{Deserialize, Serialize};

use crate::{build::ScipData, history::GitStats, walk::SkipReason};

/// Batas file per batch upload (isi maks. 1 MB per file).
pub const UPLOAD_BATCH: usize = 50;

/// Satu file di manifest. `sha` kosong untuk file dilewati tanpa dibaca (rahasia, terlalu besar).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestFile {
    pub path: String,
    #[serde(default)]
    pub sha: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestRequest {
    pub project: String,
    pub repo: String,
    #[serde(default)]
    pub commit: String,
    #[serde(default)]
    pub branch: String,
    /// Index penuh: semua file dikirim ulang walau sha sama.
    #[serde(default)]
    pub full: bool,
    pub files: Vec<ManifestFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestResponse {
    /// Path yang perlu di-upload (baru atau berubah).
    pub need: Vec<String>,
    /// File yang dihapus dari index karena tidak ada lagi di repo.
    pub removed: u32,
    /// File yang tidak berubah sejak index terakhir.
    pub unchanged: u32,
}

/// Satu file upload. File dilewati dikirim tanpa isi beserta alasannya (rahasia tidak pernah
/// dikirim isinya).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UploadFile {
    pub path: String,
    #[serde(default)]
    pub sha: String,
    #[serde(default)]
    pub bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skip: Option<SkipReason>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UploadRequest {
    pub project: String,
    pub repo: String,
    pub files: Vec<UploadFile>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct UploadResponse {
    pub parsed: u32,
    pub skipped: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinishRequest {
    pub project: String,
    pub repo: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git: Option<GitStats>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScipRequest {
    pub project: String,
    pub repo: String,
    pub scip: ScipData,
}

/// Ringkasan index sesudah build (juga jawaban `status`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexSummary {
    pub repo: String,
    pub status: String,
    #[serde(default)]
    pub commit: String,
    pub files_indexed: i32,
    pub files_skipped: i32,
    pub symbols: i32,
    pub edges: i32,
    pub edges_certain: i32,
    pub links: i32,
    #[serde(default)]
    pub scip_indexer: String,
    #[serde(default)]
    pub indexed_at: Option<String>,
    #[serde(default)]
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusRequest {
    pub project: String,
}

/// Kata pencarian sebuah identifier: pecahan camelCase/snake_case/kebab plus bentuk utuhnya,
/// huruf kecil (`getUserById` → `get user by id getuserbyid`).
#[must_use]
pub fn words(parts: &[&str]) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut push = |w: String| {
        if !w.is_empty() && !out.contains(&w) {
            out.push(w);
        }
    };
    for part in parts {
        for token in part.split(|c: char| !c.is_alphanumeric()) {
            if token.is_empty() {
                continue;
            }
            push(token.to_lowercase());
            let chars: Vec<char> = token.chars().collect();
            let mut cur = String::new();
            for (i, &c) in chars.iter().enumerate() {
                let boundary = i > 0
                    && ((c.is_uppercase()
                        && (chars[i - 1].is_lowercase()
                            || chars.get(i + 1).is_some_and(|n| n.is_lowercase())))
                        || (c.is_ascii_digit() != chars[i - 1].is_ascii_digit()));
                if boundary {
                    push(std::mem::take(&mut cur).to_lowercase());
                }
                cur.push(c);
            }
            push(cur.to_lowercase());
        }
    }
    out.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_split_identifiers() {
        assert_eq!(words(&["getUserById"]), "getuserbyid get user by id");
        assert_eq!(
            words(&["HTTPServer", "parse_v2"]),
            "httpserver http server parse v2 v 2"
        );
        assert_eq!(words(&["src/stock/reserve.ts"]), "src stock reserve ts");
    }
}
