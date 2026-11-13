//! Evidence yang dihasilkan runner sendiri (bukan klaim AI) dan ditandatangani Ed25519:
//! `test_report` dari menjalankan `[test] command`, `screenshot` dari
//! `.agentgate/evidence/*.png|jpg|jpeg|webp` di worktree.

use std::{fs, path::Path};

use agentgate_core::runner::{
    protocol::{EvidenceKind, EvidenceSummary, EvidenceUpload},
    sha256_hex,
    signing::{evidence_payload, sign},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use uuid::Uuid;

/// Batas ukuran evidence di server.
pub const MAX_BYTES: usize = 10 * 1024 * 1024;
/// Output tes yang dikirim (bagian akhir, tempat ringkasan biasanya berada).
pub const OUTPUT_MAX: usize = 256 * 1024;
/// Folder screenshot bukti di worktree.
pub const EVIDENCE_DIR: &str = ".agentgate/evidence";

/// Evidence bertanda tangan.
#[must_use]
pub fn signed(
    secret: &[u8; 32],
    job: Uuid,
    kind: EvidenceKind,
    name: &str,
    content_type: &str,
    data: &[u8],
    summary: EvidenceSummary,
) -> EvidenceUpload {
    let sha256 = sha256_hex(data);
    let signature = sign(
        secret,
        &evidence_payload(job, kind, name, &sha256, &summary),
    );
    EvidenceUpload {
        kind,
        name: name.to_string(),
        content_type: content_type.to_string(),
        content: STANDARD.encode(data),
        sha256,
        summary,
        signature,
    }
}

/// Ambil bagian akhir output (batas byte, di batas karakter UTF-8).
#[must_use]
pub fn tail(output: &str, max: usize) -> String {
    if output.len() <= max {
        return output.to_string();
    }
    let mut start = output.len() - max;
    while !output.is_char_boundary(start) {
        start += 1;
    }
    format!("…\n{}", &output[start..])
}

/// Jumlah tes (total, gagal) dari ringkasan runner tes umum: cargo/libtest
/// (`test result: ok. 3 passed; 1 failed`), Jest/Vitest (`Tests: 1 failed, 5 passed, 6 total`),
/// pytest (`== 2 failed, 10 passed in 1.2s ==`).
#[must_use]
pub fn count_tests(output: &str) -> (Option<u32>, Option<u32>) {
    let mut passed = 0u32;
    let mut failed = 0u32;
    let mut found = false;
    let num_before = |line: &str, word: &str| -> Option<u32> {
        let idx = line.rfind(word)?;
        line[..idx]
            .trim_end()
            .rsplit(|c: char| !c.is_ascii_digit())
            .next()
            .and_then(|n| n.parse().ok())
    };
    for line in output.lines() {
        let l = line.trim();
        let lower = l.to_ascii_lowercase();
        let is_summary = lower.starts_with("test result:")
            || lower.starts_with("tests:")
            || (lower.starts_with('=') && (lower.contains(" passed") || lower.contains(" failed")));
        if !is_summary {
            continue;
        }
        found = true;
        passed += num_before(&lower, " passed").unwrap_or(0);
        failed += num_before(&lower, " failed").unwrap_or(0);
    }
    if found {
        (Some(passed + failed), Some(failed))
    } else {
        (None, None)
    }
}

/// Evidence `test_report`.
#[must_use]
pub fn test_report(
    secret: &[u8; 32],
    job: Uuid,
    command: &str,
    success: bool,
    output: &str,
) -> EvidenceUpload {
    let (total, failed) = count_tests(output);
    let body = tail(output, OUTPUT_MAX);
    let body = if body.trim().is_empty() {
        format!("$ {command}\n(tidak ada output)\n")
    } else {
        format!("$ {command}\n{body}")
    };
    signed(
        secret,
        job,
        EvidenceKind::TestReport,
        "test-report.txt",
        "text/plain",
        body.as_bytes(),
        EvidenceSummary {
            passed: Some(success),
            total,
            failed,
            note: command.to_string(),
        },
    )
}

fn image_type(name: &str) -> Option<&'static str> {
    let ext = name.rsplit_once('.')?.1.to_ascii_lowercase();
    match ext.as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

/// Evidence `screenshot` dari folder bukti di `worktree` (urut nama). Berkas terlalu besar
/// dilewati (dikembalikan sebagai nama di daftar kedua).
#[must_use]
pub fn screenshots(
    secret: &[u8; 32],
    job: Uuid,
    worktree: &Path,
) -> (Vec<EvidenceUpload>, Vec<String>) {
    let dir = worktree.join(EVIDENCE_DIR);
    let Ok(entries) = fs::read_dir(&dir) else {
        return (Vec::new(), Vec::new());
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| image_type(n).is_some())
        .collect();
    names.sort();
    let mut out = Vec::new();
    let mut skipped = Vec::new();
    for name in names {
        let Some(ct) = image_type(&name) else {
            continue;
        };
        match fs::read(dir.join(&name)) {
            Ok(data) if !data.is_empty() && data.len() <= MAX_BYTES => {
                out.push(signed(
                    secret,
                    job,
                    EvidenceKind::Screenshot,
                    &name,
                    ct,
                    &data,
                    EvidenceSummary {
                        note: format!("{EVIDENCE_DIR}/{name}"),
                        ..EvidenceSummary::default()
                    },
                ));
            }
            _ => skipped.push(name),
        }
    }
    (out, skipped)
}

#[cfg(test)]
mod tests {
    use agentgate_core::runner::signing::{public_key_b64, verify};

    use super::*;
    use crate::testutil::temp_dir;

    const SECRET: [u8; 32] = [3; 32];

    fn verifies(up: &EvidenceUpload, job: Uuid) -> bool {
        verify(
            &public_key_b64(&SECRET),
            &evidence_payload(job, up.kind, &up.name, &up.sha256, &up.summary),
            &up.signature,
        )
    }

    #[test]
    fn test_report_is_signed_and_counts_tests() {
        let job = Uuid::new_v4();
        let out = "running 3 tests\ntest a ... ok\ntest result: ok. 3 passed; 0 failed; 0 ignored\n\
                   test result: FAILED. 1 passed; 1 failed; 0 ignored\n";
        let up = test_report(&SECRET, job, "cargo test", false, out);
        assert_eq!(up.kind, EvidenceKind::TestReport);
        assert_eq!(up.summary.passed, Some(false));
        assert_eq!(up.summary.total, Some(5));
        assert_eq!(up.summary.failed, Some(1));
        assert_eq!(up.summary.note, "cargo test");
        assert!(verifies(&up, job));
        assert!(!verifies(&up, Uuid::new_v4()));
        let body = STANDARD.decode(&up.content).unwrap();
        assert!(
            String::from_utf8(body)
                .unwrap()
                .starts_with("$ cargo test\n")
        );
    }

    #[test]
    fn test_counts_from_other_runners() {
        assert_eq!(
            count_tests("Tests:       1 failed, 5 passed, 6 total"),
            (Some(6), Some(1))
        );
        assert_eq!(
            count_tests("===== 2 failed, 10 passed in 1.20s ====="),
            (Some(12), Some(2))
        );
        assert_eq!(count_tests("ok"), (None, None));
    }

    #[test]
    fn output_tail_respects_char_boundaries() {
        let s = "ééééé";
        let t = tail(s, 3);
        assert!(t.starts_with('…'));
        assert!(t.ends_with('é'));
        assert_eq!(tail("abc", 10), "abc");
    }

    #[test]
    fn screenshots_are_collected_from_evidence_dir() {
        let wt = temp_dir("shots");
        let dir = wt.join(EVIDENCE_DIR);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("b.png"), b"\x89PNG\r\n\x1a\nxx").unwrap();
        fs::write(dir.join("a.JPG"), [0xFF, 0xD8, 0xFF, 1]).unwrap();
        fs::write(dir.join("notes.txt"), "x").unwrap();
        fs::write(dir.join("empty.webp"), b"").unwrap();
        let job = Uuid::new_v4();
        let (ups, skipped) = screenshots(&SECRET, job, &wt);
        assert_eq!(
            ups.iter()
                .map(|u| (u.name.as_str(), u.content_type.as_str()))
                .collect::<Vec<_>>(),
            [("a.JPG", "image/jpeg"), ("b.png", "image/png")]
        );
        assert_eq!(skipped, ["empty.webp"]);
        assert!(ups.iter().all(|u| verifies(u, job)));
    }
}
