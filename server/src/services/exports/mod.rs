//! Ekspor data workspace (M11, spec "Ekspor data: seluruh data workspace bisa diekspor ke
//! JSON + Markdown"). Hanya Owner/Admin (role bawaan, [`can_export`]).
//!
//! - [`request`]: baris `queued` + audit `export.requested` di satu transaksi, lalu enqueue
//!   worker setelah commit (wajib terjadi: gagal enqueue disapu `export_sweep`).
//! - [`run`] (worker): claim → [`archive::build`] → upload ZIP ke storage
//!   (`exports/<id>/<uuid>.zip`) → `ready` (7 hari) + notifikasi penerima di satu transaksi.
//!   Gagal → `failed` + notifikasi; tombol Ulangi ([`retry`]).
//! - [`downloaded`]: audit `export.downloaded` (unduhan lewat POST: GET tidak menulis audit).
//! - [`sweep`] dan [`expire`]: sapuan baris tertahan dan pembersihan berkas kedaluwarsa.
//!
//! Utang: ZIP disusun di memori (driver storage Loco hanya menerima isi utuh); ekspor dengan
//! lampiran sangat besar butuh upload multipart bertahap.
pub mod archive;

use std::path::Path;

use chrono::{DateTime, Utc};
use loco_rs::app::AppContext;
use sea_orm::{ConnectionTrait, DbErr, EntityTrait, TransactionTrait};
use uuid::Uuid;

use crate::{
    models::{
        data_exports::{
            self, ExportPart, ExportStatus, FAILED_ATTEMPTS, FAILED_BUILD, FAILED_STORAGE,
            MAX_ATTEMPTS, NewExport, PARTS_REQUIRED, PROJECTS_INVALID, RECIPIENT_INVALID,
        },
        notifications::{self, NewNotification, NotificationKind},
        projects, roles, workspace_members,
        workspaces::{self, WorkspaceId},
    },
    services::{
        audit::{self, AuditAction, AuditEntry, AuditSubject},
        tickets::TicketFailure,
        workspaces::Acting,
    },
};

/// Kode error (tes kelengkapan Fluent).
pub const ERROR_CODES: &[&str] = &[
    PARTS_REQUIRED,
    RECIPIENT_INVALID,
    PROJECTS_INVALID,
    FAILED_STORAGE,
    FAILED_BUILD,
    FAILED_ATTEMPTS,
];

/// Satu-satunya aturan izin ekspor: role bawaan Owner atau Admin (spec "Hanya Owner dan Admin
/// yang bisa mengekspor"). Juga aturan penerima link.
#[must_use]
pub fn can_export(role: &roles::Model) -> bool {
    role.is_admin_everywhere()
}

/// Isian form Ekspor baru (mentah).
#[derive(Debug, Clone, Default)]
pub struct ExportForm {
    pub parts: Vec<String>,
    /// Id project; kosong atau berisi `all` = semua project.
    pub projects: Vec<String>,
    pub recipient: String,
}

/// Anggota Owner/Admin workspace (pilihan "Kirim link ke").
///
/// # Errors
///
/// Error database.
pub async fn recipients<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
) -> Result<Vec<crate::models::users::Model>, DbErr> {
    Ok(workspace_members::Model::list_in(db, wid)
        .await?
        .into_iter()
        .filter(|(_, _, role)| can_export(role))
        .map(|(_, user, _)| user)
        .collect())
}

/// Validasi + simpan permintaan, audit, lalu enqueue (setelah commit).
///
/// # Errors
///
/// `Invalid` (`parts`/`projects`/`recipient`), `NotFound` (bukan Owner/Admin), error database.
pub async fn request(
    ctx: &AppContext,
    acting: &Acting<'_>,
    form: &ExportForm,
) -> Result<data_exports::Model, TicketFailure> {
    let wid = acting.workspace_id();
    let txn = ctx.db.begin().await?;
    let (_, role) = workspace_members::Model::find_with_role(&txn, wid, acting.user.id)
        .await?
        .ok_or(TicketFailure::NotFound)?;
    if !can_export(&role) {
        return Err(TicketFailure::NotFound);
    }
    let mut parts: Vec<ExportPart> = Vec::new();
    for p in form.parts.iter().filter_map(|p| ExportPart::parse(p)) {
        if !parts.contains(&p) {
            parts.push(p);
        }
    }
    if parts.is_empty() {
        return Err(TicketFailure::field("parts", PARTS_REQUIRED));
    }
    let mut project_ids: Vec<Uuid> = Vec::new();
    if !form.projects.iter().any(|p| p.trim() == "all") {
        for raw in form
            .projects
            .iter()
            .map(|p| p.trim())
            .filter(|p| !p.is_empty())
        {
            let id = Uuid::parse_str(raw)
                .map_err(|_| TicketFailure::field("projects", PROJECTS_INVALID))?;
            projects::Model::find_in(&txn, wid, id)
                .await?
                .ok_or_else(|| TicketFailure::field("projects", PROJECTS_INVALID))?;
            if !project_ids.contains(&id) {
                project_ids.push(id);
            }
        }
    }
    let recipient = Uuid::parse_str(form.recipient.trim())
        .ok()
        .ok_or_else(|| TicketFailure::field("recipient", RECIPIENT_INVALID))?;
    match workspace_members::Model::find_with_role(&txn, wid, recipient).await? {
        Some((_, r)) if can_export(&r) => {}
        _ => return Err(TicketFailure::field("recipient", RECIPIENT_INVALID)),
    }
    let row = data_exports::ActiveModel::create(
        &txn,
        wid,
        NewExport {
            requested_by: acting.user.id,
            recipient_id: recipient,
            parts: parts.clone(),
            project_ids: project_ids.clone(),
        },
    )
    .await?;
    audit::record(
        &txn,
        AuditEntry {
            workspace_id: wid,
            actor_id: Some(acting.user.id),
            action: AuditAction::ExportRequested,
            subject: (AuditSubject::DataExport, row.id),
            changes: serde_json::json!({
                "parts": parts.iter().map(|p| p.key()).collect::<Vec<_>>(),
                "project_ids": project_ids,
                "recipient_id": recipient,
            }),
            request_id: acting.request_id.clone(),
        },
    )
    .await?;
    txn.commit().await?;
    enqueue(ctx, row.id).await;
    Ok(data_exports::Model::find_in(&ctx.db, wid, row.id)
        .await?
        .unwrap_or(row))
}

/// Ulangi ekspor yang gagal (Owner/Admin).
///
/// # Errors
///
/// `NotFound` (bukan milik workspace / bukan `failed`), error database.
pub async fn retry(ctx: &AppContext, acting: &Acting<'_>, id: Uuid) -> Result<(), TicketFailure> {
    if !can_export(acting.role) {
        return Err(TicketFailure::NotFound);
    }
    if !data_exports::ActiveModel::requeue_failed(&ctx.db, acting.workspace_id(), id).await? {
        return Err(TicketFailure::NotFound);
    }
    enqueue(ctx, id).await;
    Ok(())
}

/// Enqueue worker ekspor; gagal hanya dicatat (baris `queued` disapu `export_sweep`).
pub async fn enqueue(ctx: &AppContext, id: Uuid) {
    use loco_rs::bgworker::BackgroundWorker;
    if let Err(e) = crate::workers::data_export::DataExportWorker::perform_later(
        ctx,
        crate::workers::data_export::DataExportArgs { export_id: id },
    )
    .await
    {
        tracing::error!(export_id = %id, error = %e, "enqueue data export failed");
    }
}

/// Berkas siap diunduh: isi ZIP + nama berkas, dan audit `export.downloaded` (commit sebelum
/// isi dikirim).
///
/// # Errors
///
/// `NotFound` (bukan milik workspace, belum siap, kedaluwarsa, berkas hilang), error database.
pub async fn download(
    ctx: &AppContext,
    acting: &Acting<'_>,
    id: Uuid,
) -> Result<(String, Vec<u8>), TicketFailure> {
    if !can_export(acting.role) {
        return Err(TicketFailure::NotFound);
    }
    let wid = acting.workspace_id();
    let row = data_exports::Model::find_in(&ctx.db, wid, id)
        .await?
        .filter(|r| r.downloadable(Utc::now()))
        .ok_or(TicketFailure::NotFound)?;
    let key = row.storage_key.clone().unwrap_or_default();
    let data: Vec<u8> = ctx.storage.download(Path::new(&key)).await.map_err(|e| {
        tracing::error!(export_id = %id, error = %e, "berkas ekspor tidak ada di storage");
        TicketFailure::NotFound
    })?;
    let txn = ctx.db.begin().await?;
    audit::record(
        &txn,
        AuditEntry {
            workspace_id: wid,
            actor_id: Some(acting.user.id),
            action: AuditAction::ExportDownloaded,
            subject: (AuditSubject::DataExport, id),
            changes: serde_json::json!({ "size_bytes": row.size_bytes }),
            request_id: acting.request_id.clone(),
        },
    )
    .await?;
    txn.commit().await?;
    Ok((row.filename(&acting.workspace.slug), data))
}

async fn notify<C: ConnectionTrait>(
    db: &C,
    row: &data_exports::Model,
    wid: WorkspaceId,
    kind: NotificationKind,
) -> Result<(), DbErr> {
    if let Some(user_id) = row.recipient_id {
        notifications::ActiveModel::notify(
            db,
            wid,
            NewNotification {
                user_id,
                actor_id: row.requested_by,
                kind,
                ticket_id: None,
                comment_id: None,
                project_id: None,
            },
        )
        .await?;
    }
    Ok(())
}

/// Tandai gagal + notifikasi penerima (satu transaksi).
async fn fail(
    ctx: &AppContext,
    row: &data_exports::Model,
    wid: WorkspaceId,
    code: &str,
) -> Result<(), DbErr> {
    let txn = ctx.db.begin().await?;
    if data_exports::ActiveModel::fail(&txn, row.id, code).await? {
        notify(&txn, row, wid, NotificationKind::ExportFailed).await?;
    }
    txn.commit().await
}

/// Worker: susun ZIP ekspor `id`.
///
/// # Errors
///
/// Error database (job diulang). Kegagalan menyusun/menyimpan berkas menjadi status `failed`.
pub async fn run(ctx: &AppContext, id: Uuid) -> Result<(), DbErr> {
    let db = &ctx.db;
    let Some(row) = data_exports::Entity::find_by_id(id).one(db).await? else {
        return Ok(());
    };
    let Some(workspace) = workspaces::Entity::find_by_id(row.workspace_id)
        .one(db)
        .await?
    else {
        return Ok(());
    };
    let wid = workspace.workspace_id();
    if !data_exports::ActiveModel::claim(db, id).await? {
        return Ok(());
    }
    let built = archive::build(ctx, &workspace, &row).await;
    let bytes = match built {
        Ok(b) => b,
        Err(e) => {
            tracing::error!(export_id = %id, error = %e, "data export build failed");
            return fail(ctx, &row, wid, FAILED_BUILD).await;
        }
    };
    let key = format!("exports/{id}/{}.zip", Uuid::now_v7());
    if let Err(e) = ctx
        .storage
        .upload(Path::new(&key), &axum::body::Bytes::from(bytes.clone()))
        .await
    {
        tracing::error!(export_id = %id, error = %e, "data export upload failed");
        return fail(ctx, &row, wid, FAILED_STORAGE).await;
    }
    let size = i64::try_from(bytes.len()).unwrap_or(i64::MAX);
    let txn = db.begin().await?;
    if data_exports::ActiveModel::finish(&txn, id, &key, size).await? {
        notify(&txn, &row, wid, NotificationKind::ExportReady).await?;
        txn.commit().await?;
    } else {
        txn.rollback().await?;
        crate::services::tickets::attachments::remove_file_quietly(&ctx.storage, &key).await;
    }
    Ok(())
}

/// Sapuan (task `export_sweep`): ekspor tertahan di-enqueue ulang (paling banyak
/// [`MAX_ATTEMPTS`] percobaan, lalu `failed`). Kembalikan jumlah yang disapu.
///
/// # Errors
///
/// Error database.
pub async fn sweep(ctx: &AppContext, now: DateTime<Utc>) -> Result<u64, DbErr> {
    let mut n = 0;
    for row in data_exports::Model::stuck(&ctx.db, now).await? {
        let Some(ws) = workspaces::Entity::find_by_id(row.workspace_id)
            .one(&ctx.db)
            .await?
        else {
            continue;
        };
        if row.status() == ExportStatus::Running && row.attempts >= MAX_ATTEMPTS {
            fail(ctx, &row, ws.workspace_id(), FAILED_ATTEMPTS).await?;
        } else if data_exports::ActiveModel::requeue_stuck(&ctx.db, &row).await? {
            enqueue(ctx, row.id).await;
        }
        n += 1;
    }
    Ok(n)
}

/// Pembersihan (task `cleanup`): ekspor `ready` yang lewat 7 hari → `expired`, berkasnya
/// dihapus setelah baris diperbarui. `apply` false = hanya menghitung.
///
/// # Errors
///
/// Error database.
pub async fn expire(ctx: &AppContext, now: DateTime<Utc>, apply: bool) -> Result<u64, DbErr> {
    let due = data_exports::Model::expired_due(&ctx.db, now).await?;
    if !apply {
        return Ok(due.len() as u64);
    }
    let mut n = 0;
    for row in due {
        if data_exports::ActiveModel::expire(&ctx.db, row.id).await? {
            if let Some(key) = &row.storage_key {
                crate::services::tickets::attachments::remove_file_quietly(&ctx.storage, key).await;
            }
            n += 1;
        }
    }
    Ok(n)
}
