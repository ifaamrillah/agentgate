//! Bukti gate manual di kartu stage tiket (R1): unggah (multipart), setujui/tolak, dan unduh.
//! Aturan ada di `services::tickets::manual_evidence`; hasil aksi berupa flash + redirect ke
//! detail tiket.
use axum::{
    body::Body,
    extract::Multipart,
    http::{HeaderValue, header},
};
use loco_rs::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::acting;
use crate::{
    controllers::{
        extractors::{RequestId, TicketAccess},
        flash,
    },
    models::{
        manual_evidence::{self, ManualGate, ManualKind, ManualStatus},
        permission::Permission,
    },
    services::tickets::{
        TicketFailure,
        manual_evidence::{self as svc, Submission},
    },
    views::{flash::Flash, tickets::ticket_href},
};

pub const FLASH_SUBMITTED: &str = "manual-evidence-flash-submitted";
pub const FLASH_APPROVED: &str = "manual-evidence-flash-approved";
pub const FLASH_REJECTED: &str = "manual-evidence-flash-rejected";
/// Kunci teks yang dipakai dari Rust (kontrak i18n).
pub const MESSAGE_KEYS: &[&str] = &[FLASH_SUBMITTED, FLASH_APPROVED, FLASH_REJECTED];

/// Batas body unggah: berkas + ruang untuk field form.
pub const BODY_LIMIT: usize = svc::MAX_BYTES + 512 * 1024;

/// Satu bukti manual di kartu stage.
#[derive(Debug, Clone, Serialize)]
pub struct ManualEvidenceView {
    pub id: Uuid,
    pub name: String,
    pub kind: String,
    pub icon: &'static str,
    pub gate_key: String,
    pub status: String,
    pub status_key: String,
    pub status_variant: &'static str,
    pub body: String,
    pub note: String,
    pub decision_note: String,
    pub submitter: String,
    pub at: String,
    /// Unduh/buka berkas (screenshot/log); `None` untuk bukti teks.
    pub href: Option<String>,
    pub image: bool,
    /// Form setujui/tolak (approver, bukti menunggu).
    pub approve_action: Option<String>,
    pub reject_action: Option<String>,
}

/// Bangun daftar bukti manual stage sekarang untuk kartu stage.
///
/// # Errors
///
/// Error database.
pub(super) async fn views(
    db: &sea_orm::DatabaseConnection,
    access: &TicketAccess,
    can_decide: bool,
) -> Result<Vec<ManualEvidenceView>> {
    let tenant = &access.access.tenant;
    let t = &access.ticket;
    let rows = svc::of_stage(db, tenant.workspace_id(), t).await?;
    let users = super::users_by_id(
        db,
        rows.iter()
            .filter_map(|r| r.submitted_by)
            .collect::<Vec<_>>(),
    )
    .await?;
    let base = format!("{}/evidence", ticket_href(&tenant.workspace.slug, &t.code));
    Ok(rows
        .iter()
        .map(|r| {
            let status = r.status();
            let pending = status == ManualStatus::Pending;
            let kind = r.kind().unwrap_or(ManualKind::Text);
            ManualEvidenceView {
                id: r.id,
                name: r.name.clone(),
                kind: r.kind.clone(),
                icon: match kind {
                    ManualKind::Screenshot => "photo",
                    ManualKind::Log => "file-text",
                    ManualKind::Text => "message-2",
                },
                gate_key: format!("workflow-gate-{}", r.gate.replace('_', "-")),
                status: r.status.clone(),
                status_key: format!("manual-evidence-status-{}", r.status),
                status_variant: match status {
                    ManualStatus::Pending => "gate",
                    ManualStatus::Approved => "success",
                    ManualStatus::Rejected => "destructive",
                },
                body: r.body.clone(),
                note: r.note.clone(),
                decision_note: r.decision_note.clone(),
                submitter: r
                    .submitted_by
                    .and_then(|u| users.iter().find(|x| x.id == u))
                    .map(|u| u.name.clone())
                    .unwrap_or_default(),
                at: crate::views::time::wib_datetime(&r.created_at),
                href: r.storage_key.as_ref().map(|_| format!("{base}/{}", r.id)),
                image: kind == ManualKind::Screenshot,
                approve_action: (can_decide && pending).then(|| format!("{base}/{}/approve", r.id)),
                reject_action: (can_decide && pending).then(|| format!("{base}/{}/reject", r.id)),
            }
        })
        .collect())
}

/// Pilihan gate di form unggah: `(nilai, kunci label)`.
#[must_use]
pub fn gate_options(gates: &[ManualGate]) -> Vec<serde_json::Value> {
    gates
        .iter()
        .map(|g| {
            let v = sea_orm::ActiveEnum::to_value(g);
            serde_json::json!({ "value": v, "label_key": format!("workflow-gate-{}", v.replace('_', "-")) })
        })
        .collect()
}

fn redirect_flash(session: Session, access: &TicketAccess, message: &Flash) -> Result<Response> {
    let session = flash::set(session, message)?;
    let to = ticket_href(&access.access.tenant.workspace.slug, &access.ticket.code);
    Ok((session, format::redirect(&to)?).into_response())
}

fn first_code(e: &loco_rs::validation::ModelValidationErrors) -> String {
    e.errors
        .values()
        .flat_map(|es| es.iter().map(|e| e.code.to_string()))
        .next()
        .unwrap_or_else(|| "error-generic-title".into())
}

fn outcome<T>(
    session: Session,
    access: &TicketAccess,
    r: std::result::Result<T, TicketFailure>,
    ok: &str,
) -> Result<Response> {
    match r {
        Ok(_) => redirect_flash(session, access, &Flash::success(ok)),
        Err(TicketFailure::Invalid(errors)) => {
            redirect_flash(session, access, &Flash::error(&first_code(&errors)))
        }
        Err(e) => Err(e.into()),
    }
}

/// Field form unggah. Body rusak/terpotong (melebihi batas) → `BadRequest`.
async fn read_form(mut multipart: Multipart) -> Result<Submission> {
    let mut out = Submission::default();
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| Error::BadRequest("multipart".into()))?
    {
        let name = field.name().unwrap_or_default().to_string();
        match name.as_str() {
            "file" => {
                let filename = field.file_name().unwrap_or_default().to_string();
                let data = field
                    .bytes()
                    .await
                    .map_err(|_| Error::BadRequest("multipart".into()))?;
                if !data.is_empty() {
                    out.file = Some((filename, data));
                }
            }
            "gate" | "kind" | "text" | "note" => {
                let value = field
                    .text()
                    .await
                    .map_err(|_| Error::BadRequest("multipart".into()))?;
                match name.as_str() {
                    "gate" => out.gate = value,
                    "kind" => out.kind = value,
                    "text" => out.text = value,
                    _ => out.note = value,
                }
            }
            _ => {}
        }
    }
    Ok(out)
}

/// `TicketAccess` + cek `EditProject` sebelum body dibaca (tanpa izin → 404).
#[debug_handler]
pub(super) async fn submit(
    State(ctx): State<AppContext>,
    access: TicketAccess,
    session: Session,
    RequestId(request_id): RequestId,
    multipart: Multipart,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    let input = match read_form(multipart).await {
        Ok(i) => i,
        Err(Error::BadRequest(_)) => {
            return redirect_flash(session, &access, &Flash::error(svc::TOO_LARGE));
        }
        Err(e) => return Err(e),
    };
    let r = svc::submit(
        &ctx.db,
        &ctx.storage,
        &acting(&access.access.tenant, request_id),
        access.ticket.id,
        input,
    )
    .await;
    outcome(session, &access, r, FLASH_SUBMITTED)
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct DecideForm {
    #[serde(default)]
    note: String,
}

#[debug_handler]
pub(super) async fn approve(
    State(ctx): State<AppContext>,
    access: TicketAccess,
    session: Session,
    RequestId(request_id): RequestId,
    Path((_, _, id)): Path<(String, String, Uuid)>,
    Form(form): Form<DecideForm>,
) -> Result<Response> {
    let r = svc::decide(
        &ctx.db,
        &acting(&access.access.tenant, request_id),
        access.ticket.id,
        id,
        true,
        &form.note,
    )
    .await;
    outcome(session, &access, r, FLASH_APPROVED)
}

#[debug_handler]
pub(super) async fn reject(
    State(ctx): State<AppContext>,
    access: TicketAccess,
    session: Session,
    RequestId(request_id): RequestId,
    Path((_, _, id)): Path<(String, String, Uuid)>,
    Form(form): Form<DecideForm>,
) -> Result<Response> {
    let r = svc::decide(
        &ctx.db,
        &acting(&access.access.tenant, request_id),
        access.ticket.id,
        id,
        false,
        &form.note,
    )
    .await;
    outcome(session, &access, r, FLASH_REJECTED)
}

/// Buka berkas bukti manual (siapa pun yang bisa membuka tiket). Screenshot `inline`, log
/// `attachment` sebagai `text/plain`; selalu `nosniff` + `no-store`.
#[debug_handler]
pub(super) async fn download(
    State(ctx): State<AppContext>,
    access: TicketAccess,
    Path((_, _, id)): Path<(String, String, Uuid)>,
) -> Result<Response> {
    let row: manual_evidence::Model = svc::find_of_ticket(
        &ctx.db,
        access.access.tenant.workspace_id(),
        access.ticket.id,
        id,
    )
    .await?
    .ok_or(Error::NotFound)?;
    let key = row.storage_key.clone().ok_or(Error::NotFound)?;
    let data: Vec<u8> = ctx
        .storage
        .download(std::path::Path::new(&key))
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "bukti manual tidak ada di storage");
            Error::NotFound
        })?;
    let image = row.kind() == Some(ManualKind::Screenshot);
    let mut res = Response::new(Body::from(data));
    let headers = res.headers_mut();
    let content_type = if image {
        row.content_type.as_str()
    } else {
        "text/plain; charset=utf-8"
    };
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(content_type)
            .unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static(if image { "inline" } else { "attachment" }),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(res)
}
