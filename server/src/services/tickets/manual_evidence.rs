//! Bukti gate manual (R1, keputusan pemilik: "bukti manual boleh, berlabel manual, gate baru
//! terpenuhi setelah approver stage menyetujui").
//!
//! - Unggah: anggota dengan `EditProject` di stage aktif yang punya gate evidence
//!   (`tests_passed`, `screenshot_exists`). Jenis: screenshot (PNG/JPEG/WebP, dicek dari isi),
//!   log (teks UTF-8: `.log`/`.txt`/`.json`/`.xml`), atau teks langsung. `screenshot_exists` hanya
//!   menerima screenshot. Berkas di storage `manual-evidence/<ticket>/<id>`.
//! - Putuskan: approver stage (atau, bila stage tanpa approver, pemegang `EditProject`; aturan
//!   yang sama dengan approve/reject stage, `stages::may_decide`). Tolak wajib catatan. Hanya bukti
//!   stage sekarang yang masih menunggu.
//! - Gate (`jobs::evidence::gate_status`) dihitung dari evidence runner **atau** bukti manual
//!   `approved` sejak tiket masuk stage.
//!
//! Setiap langkah: audit (`evidence.*`), riwayat tiket (`evidence_submitted`,
//! `evidence_decided`), notifikasi approver (`evidence_review`). Efek storage: berkas diunggah
//! sebelum transaksi; transaksi gagal → berkas dihapus.

use std::path::Path;

use agentgate_core::workflow::Gate;
use axum::body::Bytes;
use loco_rs::storage::Storage;
use sea_orm::{
    ActiveEnum, ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait,
    DatabaseConnection, DbErr, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder,
};
use uuid::Uuid;

use super::{
    TicketFailure, Working,
    stages::{Flow, may_decide, notify_all},
};
use crate::{
    models::{
        manual_evidence::{self, ManualGate, ManualKind, ManualStatus},
        notifications::NotificationKind,
        permission::Permission,
        ticket_events::EventKind,
        ticket_stage_people, tickets,
        workspaces::WorkspaceId,
    },
    services::{
        audit::{self, AuditAction, AuditSubject},
        workspaces::Acting,
    },
};

pub const MAX_BYTES: usize = 10 * 1024 * 1024;
pub const TEXT_MAX: usize = 20_000;
pub const NAME_MAX: usize = 120;
pub const GATE_INVALID: &str = "manual-evidence-gate-invalid";
pub const KIND_INVALID: &str = "manual-evidence-kind-invalid";
pub const FILE_INVALID: &str = "manual-evidence-file-invalid";
pub const TOO_LARGE: &str = "manual-evidence-too-large";
pub const TEXT_REQUIRED: &str = "manual-evidence-text-required";
pub const NOT_PENDING: &str = "manual-evidence-not-pending";
pub const STAGE_NOT_ACTIVE: &str = "manual-evidence-stage-not-active";
/// Kode error yang dipakai dari Rust (kontrak i18n).
pub const ERROR_CODES: &[&str] = &[
    GATE_INVALID,
    KIND_INVALID,
    FILE_INVALID,
    TOO_LARGE,
    TEXT_REQUIRED,
    NOT_PENDING,
    STAGE_NOT_ACTIVE,
];

/// Isian form unggah bukti manual.
#[derive(Debug, Clone, Default)]
pub struct Submission {
    pub gate: String,
    pub kind: String,
    /// Berkas (screenshot/log). Kosong untuk jenis teks.
    pub file: Option<(String, Bytes)>,
    /// Isi bukti teks.
    pub text: String,
    pub note: String,
}

fn bad(field: &'static str, code: &'static str) -> TicketFailure {
    TicketFailure::field(field, code)
}

fn clean_name(raw: &str) -> String {
    raw.trim()
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_control())
        .take(NAME_MAX)
        .collect()
}

/// Content type screenshot dari isi berkas.
fn image_type(data: &[u8]) -> Option<&'static str> {
    crate::services::projects::logo::sniff(data)
}

/// Gate evidence stage sekarang yang bisa dipenuhi bukti manual.
#[must_use]
pub fn manual_gates(stage: &agentgate_core::workflow::Stage) -> Vec<ManualGate> {
    stage
        .gates
        .iter()
        .filter_map(|g| match g {
            Gate::TestsPassed => Some(ManualGate::TestsPassed),
            Gate::ScreenshotExists => Some(ManualGate::ScreenshotExists),
            _ => None,
        })
        .collect()
}

/// Validasi isian → (gate, jenis, nama, content type, data berkas, isi teks).
type Validated = (
    ManualGate,
    ManualKind,
    String,
    String,
    Option<Bytes>,
    String,
);

fn validate(input: &Submission, allowed: &[ManualGate]) -> Result<Validated, TicketFailure> {
    let gate = ManualGate::try_from_value(&input.gate.trim().to_string())
        .ok()
        .filter(|g| allowed.contains(g))
        .ok_or_else(|| bad("gate", GATE_INVALID))?;
    let kind = ManualKind::try_from_value(&input.kind.trim().to_string())
        .map_err(|_| bad("kind", KIND_INVALID))?;
    if gate == ManualGate::ScreenshotExists && kind != ManualKind::Screenshot {
        return Err(bad("kind", KIND_INVALID));
    }
    match kind {
        ManualKind::Text => {
            let text = input.text.trim().to_string();
            if text.is_empty() {
                return Err(bad("text", TEXT_REQUIRED));
            }
            if text.chars().count() > TEXT_MAX {
                return Err(bad("text", TOO_LARGE));
            }
            Ok((
                gate,
                kind,
                "Catatan teks".into(),
                "text/plain".into(),
                None,
                text,
            ))
        }
        ManualKind::Screenshot | ManualKind::Log => {
            let (raw_name, data) = input
                .file
                .as_ref()
                .filter(|(_, d)| !d.is_empty())
                .ok_or_else(|| bad("file", FILE_INVALID))?;
            if data.len() > MAX_BYTES {
                return Err(bad("file", TOO_LARGE));
            }
            let name = clean_name(raw_name);
            let name = if name.is_empty() {
                "bukti".to_string()
            } else {
                name
            };
            let content_type = if kind == ManualKind::Screenshot {
                image_type(data).ok_or_else(|| bad("file", FILE_INVALID))?
            } else {
                // Log: teks UTF-8 tanpa byte NUL; disajikan sebagai text/plain (nosniff).
                if std::str::from_utf8(data).is_err() || data.contains(&0) {
                    return Err(bad("file", FILE_INVALID));
                }
                "text/plain"
            };
            Ok((
                gate,
                kind,
                name,
                content_type.into(),
                Some(data.clone()),
                String::new(),
            ))
        }
    }
}

async fn remove_quietly(storage: &Storage, key: &str) {
    if let Err(e) = storage.delete(Path::new(key)).await {
        tracing::warn!(error = %e, key, "gagal menghapus berkas bukti manual");
    }
}

/// Unggah bukti manual untuk stage sekarang tiket.
///
/// # Errors
///
/// `Invalid` (kode `manual-evidence-*`), `NotFound` (tiket/izin), error database/storage.
pub async fn submit(
    db: &DatabaseConnection,
    storage: &Storage,
    acting: &Acting<'_>,
    ticket_id: Uuid,
    input: Submission,
) -> Result<manual_evidence::Model, TicketFailure> {
    let work = Working::begin(db, acting, ticket_id, Permission::EditProject, false).await?;
    if work.ticket.cancelled_at.is_some() {
        return Err(bad("gate", STAGE_NOT_ACTIVE));
    }
    let flow = Flow::load(&work.txn, work.wid, &work.ticket).await?;
    let stage = flow
        .rule(&work.ticket.stage_key)
        .cloned()
        .ok_or_else(|| bad("gate", GATE_INVALID))?;
    let (gate, kind, name, content_type, data, body) = validate(&input, &manual_gates(&stage))?;
    let note: String = input.note.trim().chars().take(500).collect();
    let id = Uuid::now_v7();
    let key = match &data {
        Some(bytes) => {
            let key = format!("manual-evidence/{}/{id}", work.ticket.id);
            storage
                .upload(Path::new(&key), bytes)
                .await
                .map_err(|e| TicketFailure::Db(DbErr::Custom(format!("storage: {e}"))))?;
            Some(key)
        }
        None => None,
    };
    let result = async {
        let row = manual_evidence::ActiveModel {
            id: Set(id),
            workspace_id: Set(work.wid.get()),
            ticket_id: Set(work.ticket.id),
            stage_key: Set(work.ticket.stage_key.clone()),
            gate: Set(sea_orm::ActiveEnum::to_value(&gate)),
            kind: Set(sea_orm::ActiveEnum::to_value(&kind)),
            name: Set(name.clone()),
            content_type: Set(content_type),
            size_bytes: Set(data
                .as_ref()
                .map_or(i32::try_from(body.len()).unwrap_or(i32::MAX), |d| {
                    i32::try_from(d.len()).unwrap_or(i32::MAX)
                })),
            storage_key: Set(key.clone()),
            body: Set(body),
            note: Set(note.clone()),
            submitted_by: Set(Some(acting.user.id)),
            status: Set(sea_orm::ActiveEnum::to_value(&ManualStatus::Pending)),
            ..Default::default()
        }
        .insert(&work.txn)
        .await?;
        work.event(
            acting.user.id,
            EventKind::EvidenceSubmitted,
            serde_json::json!({ "filename": name, "gate": row.gate, "note": note }),
        )
        .await?;
        audit::record(
            &work.txn,
            acting.actor().audit(
                AuditAction::EvidenceSubmitted,
                (AuditSubject::Evidence, row.id),
                serde_json::json!({
                    "ticket": work.ticket.code, "stage": row.stage_key, "gate": row.gate,
                    "kind": row.kind, "name": row.name,
                }),
            ),
        )
        .await?;
        let people = ticket_stage_people::Model::of_stage(
            &work.txn,
            work.wid,
            work.ticket.id,
            &work.ticket.stage_key,
        )
        .await?;
        notify_all(
            &work.txn,
            work.wid,
            acting.user.id,
            people.approvers,
            NotificationKind::EvidenceReview,
            work.ticket.id,
        )
        .await?;
        Ok::<_, DbErr>(row)
    }
    .await;
    match result {
        Ok(row) => match work.txn.commit().await {
            Ok(()) => Ok(row),
            Err(e) => {
                if let Some(k) = &key {
                    remove_quietly(storage, k).await;
                }
                Err(e.into())
            }
        },
        Err(e) => {
            if let Some(k) = &key {
                remove_quietly(storage, k).await;
            }
            Err(e.into())
        }
    }
}

/// Setujui (`approve = true`) atau tolak bukti manual yang menunggu di stage sekarang.
///
/// # Errors
///
/// `Invalid` (`manual-evidence-not-pending`, `ticket-note-required`, `ticket-not-approver`),
/// `NotFound`, error database.
pub async fn decide(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    ticket_id: Uuid,
    evidence_id: Uuid,
    approve: bool,
    note: &str,
) -> Result<manual_evidence::Model, TicketFailure> {
    let note = note.trim();
    if !approve && note.is_empty() {
        return Err(bad("note", super::stages::NOTE_REQUIRED));
    }
    let note: String = note.chars().take(500).collect();
    let work = Working::begin(db, acting, ticket_id, Permission::ViewProject, false).await?;
    let row = manual_evidence::Entity::find_by_id(evidence_id)
        .filter(manual_evidence::Column::WorkspaceId.eq(work.wid.get()))
        .filter(manual_evidence::Column::TicketId.eq(work.ticket.id))
        .one(&work.txn)
        .await?
        .ok_or(TicketFailure::NotFound)?;
    may_decide(
        &work.txn,
        work.wid,
        &work.ticket,
        acting.user.id,
        &work.access,
    )
    .await?;
    let current =
        row.stage_key == work.ticket.stage_key && row.created_at >= work.ticket.stage_entered_at;
    if row.status() != ManualStatus::Pending || !current {
        return Err(bad("evidence", NOT_PENDING));
    }
    let status = if approve {
        ManualStatus::Approved
    } else {
        ManualStatus::Rejected
    };
    let mut m = row.into_active_model();
    m.status = Set(sea_orm::ActiveEnum::to_value(&status));
    m.decided_by = Set(Some(acting.user.id));
    m.decided_at = Set(Some(chrono::Utc::now().into()));
    m.decision_note = Set(note.clone());
    let row = m.update(&work.txn).await?;
    let verdict = if approve { "disetujui" } else { "ditolak" };
    work.event(
        acting.user.id,
        EventKind::EvidenceDecided,
        serde_json::json!({
            "filename": format!("{} ({verdict})", row.name),
            "note": note,
        }),
    )
    .await?;
    audit::record(
        &work.txn,
        acting.actor().audit(
            if approve {
                AuditAction::EvidenceApproved
            } else {
                AuditAction::EvidenceRejected
            },
            (AuditSubject::Evidence, row.id),
            serde_json::json!({
                "ticket": work.ticket.code, "stage": row.stage_key, "gate": row.gate,
                "name": row.name, "note": note,
            }),
        ),
    )
    .await?;
    work.txn.commit().await?;
    Ok(row)
}

/// Bukti manual stage sekarang tiket (sejak masuk stage), terbaru dulu.
///
/// # Errors
///
/// Error database.
pub async fn of_stage<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    ticket: &tickets::Model,
) -> Result<Vec<manual_evidence::Model>, DbErr> {
    manual_evidence::Entity::find()
        .filter(manual_evidence::Column::WorkspaceId.eq(wid.get()))
        .filter(manual_evidence::Column::TicketId.eq(ticket.id))
        .filter(manual_evidence::Column::StageKey.eq(ticket.stage_key.clone()))
        .filter(manual_evidence::Column::CreatedAt.gte(ticket.stage_entered_at))
        .order_by_desc(manual_evidence::Column::CreatedAt)
        .order_by_desc(manual_evidence::Column::Id)
        .all(db)
        .await
}

/// Gate yang dipenuhi bukti manual yang sudah disetujui di stage sekarang:
/// (`tests_passed`, `screenshot_exists`).
///
/// # Errors
///
/// Error database.
pub async fn approved_gates<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    ticket: &tickets::Model,
) -> Result<(bool, bool), DbErr> {
    let rows = of_stage(db, wid, ticket).await?;
    let approved = |g: ManualGate| {
        rows.iter()
            .any(|r| r.status() == ManualStatus::Approved && r.gate() == Some(g))
    };
    Ok((
        approved(ManualGate::TestsPassed),
        approved(ManualGate::ScreenshotExists),
    ))
}

/// Satu bukti manual tiket (untuk unduh). `None` bila bukan milik tiket.
///
/// # Errors
///
/// Error database.
pub async fn find_of_ticket<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    ticket_id: Uuid,
    id: Uuid,
) -> Result<Option<manual_evidence::Model>, DbErr> {
    manual_evidence::Entity::find_by_id(id)
        .filter(manual_evidence::Column::WorkspaceId.eq(wid.get()))
        .filter(manual_evidence::Column::TicketId.eq(ticket_id))
        .one(db)
        .await
}
