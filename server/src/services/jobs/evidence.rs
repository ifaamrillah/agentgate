//! Evidence bertanda tangan runner (spec "Evidence … dihasilkan dan ditandatangani oleh runner,
//! bukan sekadar klaim AI").
//!
//! Server memverifikasi: isi cocok dengan `sha256`, tanda tangan Ed25519 atas payload kanonik
//! valid untuk kunci publik **runner yang memegang job**, ukuran ≤ [`MAX_BYTES`], tipe berkas
//! sesuai jenisnya. Berkas disimpan di storage (`evidence/<job>/<id>`), metadata di
//! `job_evidence` bersama stage tiket saat itu. Gate `tests_passed`/`screenshot_exists` membaca
//! evidence sejak tiket masuk stage ([`gate_status`]).

use std::path::Path;

use agentgate_core::runner::{
    protocol::{EvidenceKind, EvidenceUpload, LogStream},
    sha256_hex,
    signing::{evidence_payload, verify},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use loco_rs::storage::Storage;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr,
    EntityTrait, QueryFilter, QueryOrder,
};
use uuid::Uuid;

use super::log;
use crate::{
    models::{job_evidence, jobs, runners, tickets, workspaces::WorkspaceId},
    services::{mcp::Session, tickets::TicketFailure},
};

pub const MAX_BYTES: usize = 10 * 1024 * 1024;
pub const NAME_MAX: usize = 120;
pub const SIGNATURE_INVALID: &str = "evidence-signature-invalid";
pub const CONTENT_INVALID: &str = "evidence-content-invalid";
pub const TOO_LARGE: &str = "evidence-too-large";
pub const TYPE_INVALID: &str = "evidence-type-invalid";
pub const ERROR_CODES: &[&str] = &[SIGNATURE_INVALID, CONTENT_INVALID, TOO_LARGE, TYPE_INVALID];

fn bad(code: &'static str) -> TicketFailure {
    TicketFailure::field("evidence", code)
}

/// Tipe berkas yang diterima per jenis evidence.
fn type_allowed(kind: EvidenceKind, content_type: &str, data: &[u8]) -> bool {
    match kind {
        EvidenceKind::Screenshot => match content_type {
            "image/png" => data.starts_with(b"\x89PNG\r\n\x1a\n"),
            "image/jpeg" => data.starts_with(&[0xFF, 0xD8, 0xFF]),
            "image/webp" => data.len() > 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP",
            _ => false,
        },
        EvidenceKind::TestReport | EvidenceKind::Log => {
            matches!(
                content_type,
                "text/plain" | "application/xml" | "application/json"
            ) && std::str::from_utf8(data).is_ok()
        }
    }
}

/// Simpan evidence dari runner pemegang job.
///
/// # Errors
///
/// `evidence-*` (422), error database/storage.
pub async fn upload(
    db: &DatabaseConnection,
    storage: &Storage,
    session: &Session,
    runner: &runners::Model,
    job: &jobs::Model,
    up: &EvidenceUpload,
) -> Result<job_evidence::Model, TicketFailure> {
    let wid = session.workspace.workspace_id();
    let data = STANDARD
        .decode(up.content.trim())
        .map_err(|_| bad(CONTENT_INVALID))?;
    if data.is_empty() {
        return Err(bad(CONTENT_INVALID));
    }
    if data.len() > MAX_BYTES {
        return Err(bad(TOO_LARGE));
    }
    if sha256_hex(&data) != up.sha256.trim().to_ascii_lowercase() {
        return Err(bad(CONTENT_INVALID));
    }
    let name: String = up
        .name
        .trim()
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_control())
        .take(NAME_MAX)
        .collect();
    if name.is_empty() {
        return Err(bad(CONTENT_INVALID));
    }
    let payload = evidence_payload(job.id, up.kind, &up.name, &up.sha256, &up.summary);
    if !verify(&runner.public_key, &payload, &up.signature) {
        return Err(bad(SIGNATURE_INVALID));
    }
    let content_type = up.content_type.trim().to_ascii_lowercase();
    if !type_allowed(up.kind, &content_type, &data) {
        return Err(bad(TYPE_INVALID));
    }
    let ticket_id = job.ticket_id.ok_or(TicketFailure::NotFound)?;
    let ticket = tickets::Model::find_in(db, wid, ticket_id)
        .await?
        .ok_or(TicketFailure::NotFound)?;
    let id = Uuid::now_v7();
    let key = format!("evidence/{}/{id}", job.id);
    storage
        .upload(Path::new(&key), &axum::body::Bytes::from(data.clone()))
        .await
        .map_err(|e| TicketFailure::Db(DbErr::Custom(format!("storage: {e}"))))?;
    let to_i32 = |v: Option<u32>| v.and_then(|n| i32::try_from(n).ok());
    let row = job_evidence::ActiveModel {
        id: Set(id),
        workspace_id: Set(wid.get()),
        job_id: Set(job.id),
        ticket_id: Set(ticket.id),
        // Stage tiket saat evidence masuk (job @agent bisa berjalan di stage mana pun).
        stage_key: Set(ticket.stage_key.clone()),
        runner_id: Set(Some(runner.id)),
        kind: Set(up.kind.as_str().into()),
        name: Set(name.clone()),
        content_type: Set(content_type),
        size_bytes: Set(i32::try_from(data.len()).unwrap_or(i32::MAX)),
        sha256: Set(up.sha256.trim().to_ascii_lowercase()),
        storage_key: Set(key.clone()),
        passed: Set(up.summary.passed),
        total: Set(to_i32(up.summary.total)),
        failed: Set(to_i32(up.summary.failed)),
        note: Set(up.summary.note.chars().take(500).collect()),
        signature: Set(up.signature.trim().to_string()),
        ..Default::default()
    }
    .insert(db)
    .await;
    let row = match row {
        Ok(r) => r,
        Err(e) => {
            if let Err(err) = storage.delete(Path::new(&key)).await {
                tracing::warn!(error = %err, "evidence cleanup failed");
            }
            return Err(e.into());
        }
    };
    let line = match (up.kind, up.summary.passed) {
        (EvidenceKind::TestReport, Some(true)) => {
            format!("→ evidence {name} (lulus, ditandatangani runner)")
        }
        (EvidenceKind::TestReport, _) => {
            format!("→ evidence {name} (gagal, ditandatangani runner)")
        }
        _ => format!("→ evidence {name} (ditandatangani runner)"),
    };
    log(db, wid, job.id, LogStream::Tool, &line).await?;
    Ok(row)
}

/// Evidence tiket di stage `stage_key` sejak tiket masuk stage itu, terbaru dulu.
///
/// # Errors
///
/// Error database.
pub async fn of_stage<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    ticket: &tickets::Model,
) -> Result<Vec<job_evidence::Model>, DbErr> {
    job_evidence::Entity::find()
        .filter(job_evidence::Column::WorkspaceId.eq(wid.get()))
        .filter(job_evidence::Column::TicketId.eq(ticket.id))
        .filter(job_evidence::Column::StageKey.eq(ticket.stage_key.clone()))
        .filter(job_evidence::Column::CreatedAt.gte(ticket.stage_entered_at))
        .order_by_desc(job_evidence::Column::CreatedAt)
        .all(db)
        .await
}

/// Evidence satu job, terbaru dulu.
///
/// # Errors
///
/// Error database.
pub async fn of_job<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    job_id: Uuid,
) -> Result<Vec<job_evidence::Model>, DbErr> {
    job_evidence::Entity::find()
        .filter(job_evidence::Column::WorkspaceId.eq(wid.get()))
        .filter(job_evidence::Column::JobId.eq(job_id))
        .order_by_desc(job_evidence::Column::CreatedAt)
        .all(db)
        .await
}

/// Gate evidence stage sekarang: (`tests_passed`, `screenshot_exists`), dari evidence runner
/// atau bukti manual yang sudah disetujui.
/// Hasil test = laporan test **terbaru** sejak masuk stage (tes gagal setelah lulus = belum).
///
/// # Errors
///
/// Error database.
pub async fn gate_status<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    ticket: &tickets::Model,
) -> Result<(bool, bool), DbErr> {
    let all = of_stage(db, wid, ticket).await?;
    let tests = all
        .iter()
        .find(|e| e.kind() == Some(EvidenceKind::TestReport))
        .is_some_and(|e| e.passed == Some(true));
    let shots = all
        .iter()
        .any(|e| e.kind() == Some(EvidenceKind::Screenshot));
    // Bukti manual yang disetujui approver stage juga memenuhi gate (R1).
    let (manual_tests, manual_shots) =
        crate::services::tickets::manual_evidence::approved_gates(db, wid, ticket).await?;
    Ok((tests || manual_tests, shots || manual_shots))
}
