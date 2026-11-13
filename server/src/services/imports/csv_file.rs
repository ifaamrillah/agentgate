//! Baca berkas CSV unggahan: ukuran, UTF-8 (BOM dibuang), deteksi pemisah `,`/`;` dari baris
//! header, dan batas baris. Murni (tanpa database), dites unit.
use crate::models::ticket_imports::{Delimiter, MAX_BYTES, MAX_ROWS};

pub const CSV_EMPTY: &str = "csv-empty";
pub const CSV_TOO_LARGE: &str = "csv-too-large";
pub const CSV_NOT_UTF8: &str = "csv-not-utf8";
pub const CSV_INVALID: &str = "csv-invalid";
pub const CSV_TOO_MANY_ROWS: &str = "csv-too-many-rows";
pub const CSV_NO_ROWS: &str = "csv-no-rows";
pub const CSV_NOT_CSV: &str = "csv-not-csv";

/// Isi berkas yang sudah dibaca.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsvFile {
    pub delimiter: Delimiter,
    pub headers: Vec<String>,
    /// Baris data; panjang setiap baris disamakan dengan header (kurang = kosong).
    pub rows: Vec<Vec<String>>,
}

/// Teks UTF-8 tanpa BOM; selain itu [`CSV_NOT_UTF8`].
///
/// # Errors
///
/// Kode error (kunci Fluent).
pub fn decode(bytes: &[u8]) -> Result<&str, &'static str> {
    if bytes.is_empty() {
        return Err(CSV_EMPTY);
    }
    if bytes.len() > MAX_BYTES {
        return Err(CSV_TOO_LARGE);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| CSV_NOT_UTF8)?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    // Berkas biner yang kebetulan UTF-8 valid (mis. berisi NUL) bukan CSV.
    if text.contains('\0') {
        return Err(CSV_NOT_CSV);
    }
    Ok(text)
}

/// Pemisah dari baris pertama: yang lebih banyak muncul di luar tanda kutip (seri = koma).
#[must_use]
pub fn detect_delimiter(text: &str) -> Delimiter {
    let (mut comma, mut semi, mut quoted) = (0usize, 0usize, false);
    for c in text.chars() {
        match c {
            '"' => quoted = !quoted,
            '\n' | '\r' if !quoted => break,
            ',' if !quoted => comma += 1,
            ';' if !quoted => semi += 1,
            _ => {}
        }
    }
    if semi > comma {
        Delimiter::Semicolon
    } else {
        Delimiter::Comma
    }
}

/// Parse seluruh berkas.
///
/// # Errors
///
/// Kode error (kunci Fluent).
pub fn parse(bytes: &[u8]) -> Result<CsvFile, &'static str> {
    let text = decode(bytes)?;
    let delimiter = detect_delimiter(text);
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter.byte())
        .flexible(true)
        .has_headers(true)
        .from_reader(text.as_bytes());
    let headers: Vec<String> = reader
        .headers()
        .map_err(|_| CSV_INVALID)?
        .iter()
        .map(|h| h.trim().to_string())
        .collect();
    if headers.iter().all(String::is_empty) {
        return Err(CSV_EMPTY);
    }
    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record.map_err(|_| CSV_INVALID)?;
        if record.iter().all(|v| v.trim().is_empty()) {
            continue;
        }
        if rows.len() == MAX_ROWS {
            return Err(CSV_TOO_MANY_ROWS);
        }
        let mut row: Vec<String> = record.iter().map(str::to_string).collect();
        row.resize(headers.len(), String::new());
        rows.push(row);
    }
    if rows.is_empty() {
        return Err(CSV_NO_ROWS);
    }
    Ok(CsvFile {
        delimiter,
        headers,
        rows,
    })
}

/// Tulis baris CSV (koma, kutip bila perlu).
#[must_use]
pub fn write(rows: &[Vec<String>]) -> String {
    let mut w = csv::WriterBuilder::new().from_writer(Vec::new());
    for row in rows {
        // Menulis ke Vec tidak pernah gagal I/O.
        if w.write_record(row).is_err() {
            break;
        }
    }
    String::from_utf8(w.into_inner().unwrap_or_default()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_semicolon_and_comma() {
        assert_eq!(detect_delimiter("a;b;c\n1,2;3"), Delimiter::Semicolon);
        assert_eq!(detect_delimiter("\"a;b\",c\n"), Delimiter::Comma);
        assert_eq!(detect_delimiter("judul\n"), Delimiter::Comma);
    }

    #[test]
    fn parses_quoted_multiline_and_pads_rows() {
        let f = parse("\u{feff}Summary;Description;Labels\n\"Bayar; QRIS\";\"baris 1\nbaris 2\"\nTanpa deskripsi\n\n".as_bytes()).unwrap();
        assert_eq!(f.delimiter, Delimiter::Semicolon);
        assert_eq!(f.headers, ["Summary", "Description", "Labels"]);
        assert_eq!(f.rows.len(), 2);
        assert_eq!(f.rows[0][0], "Bayar; QRIS");
        assert_eq!(f.rows[0][1], "baris 1\nbaris 2");
        assert_eq!(f.rows[1], ["Tanpa deskripsi", "", ""]);
    }

    #[test]
    fn rejects_non_utf8_empty_and_headers_only() {
        assert_eq!(parse(&[0x53, 0x75, 0xe9, 0x0a]), Err(CSV_NOT_UTF8));
        assert_eq!(parse(b""), Err(CSV_EMPTY));
        assert_eq!(parse(b"Summary\n"), Err(CSV_NO_ROWS));
        assert_eq!(parse(b"a\0b\n1\n"), Err(CSV_NOT_CSV));
    }

    #[test]
    fn writes_quoted_fields() {
        let out = write(&[vec!["a,b".into(), "c".into()]]);
        assert_eq!(out, "\"a,b\",c\n");
    }
}
