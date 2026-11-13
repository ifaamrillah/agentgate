//! Membaca index SCIP (`index.scip`, protobuf) menjadi edge pemanggil → definisi per baris.
//! Server mencocokkan baris ke simbol tree-sitter, jadi yang dikirim hanya path + baris.
use std::collections::HashMap;

use protobuf::Message;
use scip::types::Index;

use crate::build::{ScipData, ScipEdge};

/// Bit peran definisi (`SymbolRole.Definition`).
const DEFINITION: i32 = 1;

#[derive(Debug, thiserror::Error)]
#[error("index SCIP tidak valid: {0}")]
pub struct ScipError(String);

/// Simbol fungsi/method SCIP (`…().`), bukan simbol lokal.
fn callable(symbol: &str) -> bool {
    !symbol.starts_with("local ") && symbol.ends_with(").")
}

/// Edge call dari isi `index.scip`. `indexer` kosong → nama + versi dari metadata SCIP.
///
/// # Errors
///
/// Isi bukan protobuf `scip.Index`.
pub fn read(bytes: &[u8], indexer: &str) -> Result<ScipData, ScipError> {
    let index = Index::parse_from_bytes(bytes).map_err(|e| ScipError(e.to_string()))?;
    let mut defs: HashMap<&str, (&str, u32)> = HashMap::new();
    for d in &index.documents {
        for o in &d.occurrences {
            if o.symbol_roles & DEFINITION != 0 && callable(&o.symbol) {
                let line = o.range.first().copied().unwrap_or(0);
                defs.insert(
                    o.symbol.as_str(),
                    (
                        d.relative_path.as_str(),
                        u32::try_from(line).unwrap_or(0) + 1,
                    ),
                );
            }
        }
    }
    let mut edges = Vec::new();
    for d in &index.documents {
        for o in &d.occurrences {
            if o.symbol_roles & DEFINITION != 0 {
                continue;
            }
            let Some(&(to_path, to_line)) = defs.get(o.symbol.as_str()) else {
                continue;
            };
            let line = o.range.first().copied().unwrap_or(0);
            edges.push(ScipEdge {
                from_path: d.relative_path.clone(),
                from_line: u32::try_from(line).unwrap_or(0) + 1,
                to_path: to_path.to_string(),
                to_line,
            });
        }
    }
    edges.sort_by(|a, b| {
        (&a.from_path, a.from_line, &a.to_path, a.to_line).cmp(&(
            &b.from_path,
            b.from_line,
            &b.to_path,
            b.to_line,
        ))
    });
    edges.dedup();
    let indexer = if indexer.is_empty() {
        index
            .metadata
            .as_ref()
            .and_then(|m| m.tool_info.as_ref())
            .map_or_else(
                || "scip".to_string(),
                |t| format!("{} {}", t.name, t.version).trim().to_string(),
            )
    } else {
        indexer.to_string()
    };
    Ok(ScipData {
        indexer,
        documents: index
            .documents
            .iter()
            .map(|d| d.relative_path.clone())
            .collect(),
        edges,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use scip::types::{Document, Occurrence};

    use super::*;

    fn occ(line: i32, symbol: &str, def: bool) -> Occurrence {
        let mut o = Occurrence::new();
        o.range = vec![line, 4, 10];
        o.symbol = symbol.into();
        o.symbol_roles = i32::from(def);
        o
    }

    #[test]
    fn reads_reference_edges() {
        let mut idx = Index::new();
        let mut a = Document::new();
        a.relative_path = "src/promo.ts".into();
        a.occurrences = vec![
            occ(0, "npm kasir 1.0 src/promo.ts/calcPromo().", true),
            occ(2, "npm kasir 1.0 src/money.ts/round().", false),
            occ(3, "local 1", false),
            occ(4, "npm react 18 useState().", false),
        ];
        let mut b = Document::new();
        b.relative_path = "src/money.ts".into();
        b.occurrences = vec![occ(5, "npm kasir 1.0 src/money.ts/round().", true)];
        idx.documents = vec![a, b];
        let bytes = idx.write_to_bytes().unwrap();
        let data = read(&bytes, "scip-typescript").unwrap();
        assert_eq!(data.documents, vec!["src/promo.ts", "src/money.ts"]);
        assert_eq!(
            data.edges,
            vec![ScipEdge {
                from_path: "src/promo.ts".into(),
                from_line: 3,
                to_path: "src/money.ts".into(),
                to_line: 6
            }]
        );
        assert!(
            read(b"not protobuf \xff\xff", "x").is_err()
                || read(b"not protobuf \xff\xff", "x").is_ok_and(|d| d.edges.is_empty())
        );
    }
}
