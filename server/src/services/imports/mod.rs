//! Impor tiket dari CSV (M11, spec "Impor dari CSV juga didukung"). Wizard empat langkah, state
//! di baris `ticket_imports` (tanpa JS, setiap langkah form POST + PRG):
//!
//! 1. [`upload`]: berkas ≤ 20 MB, UTF-8, pemisah `,`/`;`, ≤ [`MAX_ROWS`] baris → storage
//!    (`imports/<id>/source.csv`) lalu baris `draft` dengan pemetaan otomatis.
//! 2. [`save_mapping`]: project tujuan (izin `EditProject`) + field per kolom (Judul wajib).
//! 3. [`preview`]: rencana per baris ([`plan`]), tanpa menulis apa pun.
//! 4. [`start`]: `draft → queued` lalu `workers::ticket_import` menjalankan [`run`] di luar
//!    request (5.000 baris × layanan buat tiket terlalu lama untuk satu request HTMX).
//!
//! **Efek samping "wajib terjadi"**: status di baris impor, worker meng-claim `queued`/`running`
//! dengan update bersyarat, dan setiap baris tiket dibuat di transaksinya sendiri bersama
//! kenaikan `processed` (job ulang melanjutkan dari sana, tidak menggandakan tiket). Sapuan
//! `export_sweep` meng-enqueue ulang baris yang tertahan.
//!
//! Tiket dibuat lewat `services::tickets::create::run_in` (validasi, kode, workflow, gate):
//! tiket baru masuk stage yang boleh jadi titik awal. Kode lama menjadi alias
//! (`ticket_codes`), `created_at` diisi dari kolom Tanggal dibuat. Notifikasi per tiket
//! (ditugaskan, mention) dihapus di transaksi yang sama: impor massal tidak membanjiri kotak
//! masuk; penerima melihat tiketnya di board.
pub mod csv_file;
pub mod fields;
pub mod plan;

use std::path::Path;

use chrono::{Duration, Utc};
use loco_rs::app::AppContext;
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::Set,
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter,
    TransactionTrait,
    sea_query::{Expr, ExprTrait},
};
use uuid::Uuid;

pub use crate::models::ticket_imports::{MAX_BYTES, MAX_ROWS};
use crate::{
    models::{
        notifications,
        permission::Permission,
        projects, ticket_attachments, ticket_codes,
        ticket_imports::{self, ImportField, ImportStatus, REPORT_RETENTION_DAYS, SkippedRow},
        tickets, users, workspace_members,
        workspaces::{self, WorkspaceId},
    },
    services::{
        audit::{self, AuditAction, AuditEntry, AuditSubject},
        tickets::{TicketFailure, access_in, create},
        workspaces::Acting,
    },
};

pub const PROJECT_REQUIRED: &str = "csv-project-required";
pub const TITLE_REQUIRED: &str = "csv-title-required";
pub const FIELD_TWICE: &str = "csv-field-twice";
pub const STORAGE_FAILED: &str = "csv-storage-failed";
pub const NOT_DRAFT: &str = "csv-not-draft";
pub const ERROR_ACCESS: &str = "csv-error-access";
pub const ERROR_SOURCE: &str = "csv-error-source";

/// Kode error yang dipakai service ini (tes kelengkapan Fluent).
pub const ERROR_CODES: &[&str] = &[
    PROJECT_REQUIRED,
    TITLE_REQUIRED,
    FIELD_TWICE,
    STORAGE_FAILED,
    NOT_DRAFT,
    ERROR_ACCESS,
    ERROR_SOURCE,
    csv_file::CSV_EMPTY,
    csv_file::CSV_TOO_LARGE,
    csv_file::CSV_NOT_UTF8,
    csv_file::CSV_INVALID,
    csv_file::CSV_TOO_MANY_ROWS,
    csv_file::CSV_NO_ROWS,
    csv_file::CSV_NOT_CSV,
];

/// Isi template CSV (Unduh template CSV).
pub const TEMPLATE: &str = "Issue key,Summary,Description,Issue Type,Status,Assignee,Priority,Story Points,Labels,Sprint,Due date,Created\n\
OLD-1,Bayar pakai QRIS,Pelanggan bisa membayar dengan QRIS di kasir.,Story,To Do,anda@contoh.id,High,5,pembayaran,,2026-12-01,2026-10-02\n";

fn storage_err(e: impl std::fmt::Display) -> TicketFailure {
    TicketFailure::Db(DbErr::Custom(format!("storage: {e}")))
}

fn source_key(id: Uuid) -> String {
    format!("imports/{id}/source.csv")
}

/// Langkah 1: simpan berkas dan buat draf dengan pemetaan otomatis.
///
/// # Errors
///
/// [`TicketFailure::Invalid`] (`file`: kode `csv-*`), error storage/database.
pub async fn upload(
    ctx: &AppContext,
    acting: &Acting<'_>,
    filename: &str,
    bytes: &[u8],
) -> Result<ticket_imports::Model, TicketFailure> {
    let file = csv_file::parse(bytes).map_err(|code| TicketFailure::field("file", code))?;
    let id = Uuid::now_v7();
    let key = source_key(id);
    ctx.storage
        .upload(Path::new(&key), &axum::body::Bytes::copy_from_slice(bytes))
        .await
        .map_err(storage_err)?;
    let mapping = fields::auto_map(&file.headers);
    let mut name = ticket_attachments::sanitize_filename(filename);
    if name.is_empty() {
        name = "import.csv".into();
    }
    let row = ticket_imports::ActiveModel {
        id: Set(id),
        workspace_id: Set(acting.workspace_id().get()),
        created_by: Set(Some(acting.user.id)),
        filename: Set(name.chars().take(255).collect()),
        size_bytes: Set(i64::try_from(bytes.len()).unwrap_or(i64::MAX)),
        source_key: Set(Some(key.clone())),
        delimiter: Set(sea_orm::ActiveEnum::to_value(&file.delimiter)),
        headers: Set(serde_json::to_value(&file.headers).unwrap_or_default()),
        mapping: Set(serde_json::to_value(&mapping).unwrap_or_default()),
        row_count: Set(i32::try_from(file.rows.len()).unwrap_or(i32::MAX)),
        status: Set(sea_orm::ActiveEnum::to_value(&ImportStatus::Draft)),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await;
    match row {
        Ok(row) => Ok(row),
        Err(e) => {
            crate::services::tickets::attachments::remove_file_quietly(&ctx.storage, &key).await;
            Err(e.into())
        }
    }
}

/// Impor milik workspace pelaku; `NotFound` selain itu.
///
/// # Errors
///
/// `NotFound`, error database.
pub async fn find<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    id: Uuid,
) -> Result<ticket_imports::Model, TicketFailure> {
    ticket_imports::Model::find_in(db, wid, id)
        .await?
        .ok_or(TicketFailure::NotFound)
}

/// Project tujuan yang boleh diisi pelaku (`EditProject`).
///
/// # Errors
///
/// `NotFound` (bukan project workspace atau tanpa izin), error database.
pub async fn target_project<C: ConnectionTrait>(
    db: &C,
    acting: &Acting<'_>,
    project_id: Uuid,
) -> Result<projects::Model, TicketFailure> {
    let project = projects::Model::find_in(db, acting.workspace_id(), project_id)
        .await?
        .filter(|p| p.archived_at.is_none())
        .ok_or(TicketFailure::NotFound)?;
    access_in(db, acting, project.id, Permission::EditProject).await?;
    Ok(project)
}

/// Validasi pemetaan (Judul wajib, field tunggal tidak dipakai dua kolom).
///
/// # Errors
///
/// [`TicketFailure::Invalid`] (`mapping`).
pub fn check_mapping(mapping: &[ImportField]) -> Result<(), TicketFailure> {
    if !mapping.contains(&ImportField::Title) {
        return Err(TicketFailure::field("mapping", TITLE_REQUIRED));
    }
    for (i, f) in mapping.iter().enumerate() {
        if f.single() && mapping[..i].contains(f) {
            return Err(TicketFailure::field("mapping", FIELD_TWICE));
        }
    }
    Ok(())
}

/// Langkah 2: project tujuan + pemetaan. `project_id` `None` = belum dipilih (422).
///
/// # Errors
///
/// [`TicketFailure::Invalid`] (`project`/`mapping`), `NotFound`, error database.
pub async fn save_mapping(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
    project_id: Option<Uuid>,
    mapping: Vec<ImportField>,
) -> Result<ticket_imports::Model, TicketFailure> {
    let row = find(db, acting.workspace_id(), id).await?;
    if !row.status().is_draft() {
        return Err(TicketFailure::field("mapping", NOT_DRAFT));
    }
    let mut mapping = mapping;
    mapping.resize(row.headers().len(), ImportField::Skip);
    let project = match project_id {
        Some(pid) => Some(target_project(db, acting, pid).await.map_err(|e| match e {
            TicketFailure::NotFound => TicketFailure::field("project", PROJECT_REQUIRED),
            e => e,
        })?),
        None => None,
    };
    // Simpan dulu (isian tidak hilang), lalu tolak bila belum lengkap.
    let mut am: ticket_imports::ActiveModel = row.into();
    am.mapping = Set(serde_json::to_value(&mapping).unwrap_or_default());
    if let Some(p) = &project {
        am.project_id = Set(Some(p.id));
    }
    let row = am.update(db).await?;
    if project.is_none() {
        return Err(TicketFailure::field("project", PROJECT_REQUIRED));
    }
    check_mapping(&mapping)?;
    Ok(row)
}

/// Hasil pratinjau.
#[derive(Debug)]
pub struct Preview {
    pub project: projects::Model,
    pub rows: Vec<plan::RowPlan>,
    pub ready: usize,
    pub warned: usize,
    pub skipped: usize,
}

async fn read_source(
    ctx: &AppContext,
    row: &ticket_imports::Model,
) -> Result<csv_file::CsvFile, TicketFailure> {
    let key = row.source_key.as_deref().ok_or(TicketFailure::NotFound)?;
    let bytes: Vec<u8> = ctx
        .storage
        .download(Path::new(key))
        .await
        .map_err(storage_err)?;
    csv_file::parse(&bytes).map_err(|code| TicketFailure::field("file", code))
}

fn alias_values(file: &csv_file::CsvFile, mapping: &[ImportField]) -> Vec<String> {
    mapping
        .iter()
        .position(|f| *f == ImportField::Alias)
        .map(|i| file.rows.iter().filter_map(|r| r.get(i).cloned()).collect())
        .unwrap_or_default()
}

/// Langkah 3: rencana semua baris (tanpa menulis).
///
/// # Errors
///
/// `NotFound` (bukan draf lengkap / project tanpa akses), error storage/database.
pub async fn preview(
    ctx: &AppContext,
    acting: &Acting<'_>,
    row: &ticket_imports::Model,
) -> Result<Preview, TicketFailure> {
    let project = target_project(
        &ctx.db,
        acting,
        row.project_id.ok_or(TicketFailure::NotFound)?,
    )
    .await?;
    let mapping = row.mapping();
    check_mapping(&mapping)?;
    let file = read_source(ctx, row).await?;
    let pctx = plan::Context::load(
        &ctx.db,
        acting.workspace_id(),
        &project,
        &alias_values(&file, &mapping),
    )
    .await?;
    let mut seen = plan::Seen::default();
    let rows: Vec<plan::RowPlan> = file
        .rows
        .iter()
        .enumerate()
        .map(|(i, values)| plan::plan_row(&pctx, &mut seen, &mapping, row_number(i), values))
        .collect();
    let count = |o: plan::Outcome| rows.iter().filter(|r| r.outcome == o).count();
    Ok(Preview {
        ready: count(plan::Outcome::Ready),
        warned: count(plan::Outcome::Warning),
        skipped: count(plan::Outcome::Skipped),
        project,
        rows,
    })
}

fn row_number(i: usize) -> u32 {
    u32::try_from(i + 2).unwrap_or(u32::MAX)
}

/// Langkah 4: antrekan impor (`draft → queued`), lalu worker (setelah commit, wajib terjadi).
///
/// # Errors
///
/// `Invalid` (belum lengkap / bukan draf), `NotFound`, error database.
pub async fn start(
    ctx: &AppContext,
    acting: &Acting<'_>,
    id: Uuid,
) -> Result<ticket_imports::Model, TicketFailure> {
    let row = find(&ctx.db, acting.workspace_id(), id).await?;
    if !row.status().is_draft() {
        return Err(TicketFailure::field("mapping", NOT_DRAFT));
    }
    target_project(
        &ctx.db,
        acting,
        row.project_id
            .ok_or_else(|| TicketFailure::field("project", PROJECT_REQUIRED))?,
    )
    .await?;
    check_mapping(&row.mapping())?;
    if !ticket_imports::ActiveModel::transition(
        &ctx.db,
        id,
        &[ImportStatus::Draft],
        ImportStatus::Queued,
    )
    .await?
    {
        return Err(TicketFailure::field("mapping", NOT_DRAFT));
    }
    enqueue(ctx, id).await;
    find(&ctx.db, acting.workspace_id(), id).await
}

/// Enqueue worker impor. Gagal enqueue hanya dicatat: baris `queued` disapu `export_sweep`.
pub async fn enqueue(ctx: &AppContext, id: Uuid) {
    use loco_rs::bgworker::BackgroundWorker;
    if let Err(e) = crate::workers::ticket_import::TicketImportWorker::perform_later(
        ctx,
        crate::workers::ticket_import::TicketImportArgs { import_id: id },
    )
    .await
    {
        tracing::error!(import_id = %id, error = %e, "enqueue ticket import failed");
    }
}

/// Batalkan draf: berkas dihapus setelah commit.
///
/// # Errors
///
/// `Invalid` (bukan draf), `NotFound`, error database.
pub async fn cancel(ctx: &AppContext, acting: &Acting<'_>, id: Uuid) -> Result<(), TicketFailure> {
    let row = find(&ctx.db, acting.workspace_id(), id).await?;
    if !ticket_imports::ActiveModel::transition(
        &ctx.db,
        id,
        &[ImportStatus::Draft],
        ImportStatus::Cancelled,
    )
    .await?
    {
        return Err(TicketFailure::field("mapping", NOT_DRAFT));
    }
    drop_source(ctx, &row).await?;
    Ok(())
}

/// Lepas key berkas sumber lalu hapus berkasnya (best-effort).
async fn drop_source(ctx: &AppContext, row: &ticket_imports::Model) -> Result<(), DbErr> {
    if let Some(key) = &row.source_key {
        ticket_imports::Entity::update_many()
            .col_expr(
                ticket_imports::Column::SourceKey,
                Expr::value(Option::<String>::None),
            )
            .filter(ticket_imports::Column::Id.eq(row.id))
            .exec(&ctx.db)
            .await?;
        crate::services::tickets::attachments::remove_file_quietly(&ctx.storage, key).await;
    }
    Ok(())
}

/// Hitungan satu langkah worker.
#[derive(Debug, Default)]
struct Step {
    imported: i32,
    warned: i32,
    skipped: Option<SkippedRow>,
}

/// Simpan kemajuan baris `index` (0-based) di transaksi `txn`. `false` = baris impor bukan lagi
/// milik job ini (status/`processed` berubah).
async fn advance<C: ConnectionTrait>(
    db: &C,
    id: Uuid,
    index: usize,
    step: &Step,
) -> Result<bool, DbErr> {
    let index = i32::try_from(index).unwrap_or(i32::MAX);
    let skipped: Vec<&SkippedRow> = step.skipped.iter().collect();
    let res = ticket_imports::Entity::update_many()
        .col_expr(ticket_imports::Column::Processed, Expr::value(index + 1))
        .col_expr(
            ticket_imports::Column::Imported,
            Expr::col(ticket_imports::Column::Imported).add(step.imported),
        )
        .col_expr(
            ticket_imports::Column::Warned,
            Expr::col(ticket_imports::Column::Warned).add(step.warned),
        )
        .col_expr(
            ticket_imports::Column::Skipped,
            Expr::col(ticket_imports::Column::Skipped).add(i32::from(step.skipped.is_some())),
        )
        .col_expr(
            ticket_imports::Column::SkippedRows,
            Expr::cust_with_values(
                "skipped_rows || $1::jsonb",
                [serde_json::to_value(skipped).unwrap_or_default()],
            ),
        )
        .col_expr(
            ticket_imports::Column::UpdatedAt,
            Expr::value(chrono::DateTime::<chrono::FixedOffset>::from(Utc::now())),
        )
        .filter(ticket_imports::Column::Id.eq(id))
        .filter(
            ticket_imports::Column::Status
                .eq(sea_orm::ActiveEnum::to_value(&ImportStatus::Running)),
        )
        .filter(ticket_imports::Column::Processed.eq(index))
        .exec(db)
        .await?;
    Ok(res.rows_affected == 1)
}

async fn fail(ctx: &AppContext, row: &ticket_imports::Model, code: &str) -> Result<(), DbErr> {
    ticket_imports::Entity::update_many()
        .col_expr(ticket_imports::Column::Error, Expr::value(code))
        .filter(ticket_imports::Column::Id.eq(row.id))
        .exec(&ctx.db)
        .await?;
    ticket_imports::ActiveModel::transition(
        &ctx.db,
        row.id,
        &[ImportStatus::Queued, ImportStatus::Running],
        ImportStatus::Failed,
    )
    .await?;
    drop_source(ctx, row).await
}

/// Worker: jalankan impor `id` (idempoten; dilanjutkan dari `processed`).
///
/// # Errors
///
/// Error database (job diulang); kegagalan per baris dicatat sebagai baris dilewati.
#[allow(clippy::too_many_lines)]
pub async fn run(ctx: &AppContext, id: Uuid) -> Result<(), DbErr> {
    let db = &ctx.db;
    let Some(row) = ticket_imports::Entity::find_by_id(id).one(db).await? else {
        return Ok(());
    };
    match row.status() {
        ImportStatus::Queued => {
            if !ticket_imports::ActiveModel::transition(
                db,
                id,
                &[ImportStatus::Queued],
                ImportStatus::Running,
            )
            .await?
            {
                return Ok(());
            }
        }
        // Dilanjutkan sapuan (job sebelumnya mati di tengah jalan).
        ImportStatus::Running => {}
        _ => return Ok(()),
    }
    let Some(workspace) = workspaces::Entity::find_by_id(row.workspace_id)
        .one(db)
        .await?
    else {
        return Ok(());
    };
    let wid = workspace.workspace_id();
    let user = match row.created_by {
        Some(uid) => users::Entity::find_by_id(uid).one(db).await?,
        None => None,
    };
    let Some(user) = user else {
        return fail(ctx, &row, ERROR_ACCESS).await;
    };
    let Some((_, role)) = workspace_members::Model::find_with_role(db, wid, user.id).await? else {
        return fail(ctx, &row, ERROR_ACCESS).await;
    };
    let acting = Acting {
        workspace: &workspace,
        user: &user,
        role: &role,
        request_id: None,
    };
    let Some(project_id) = row.project_id else {
        return fail(ctx, &row, ERROR_ACCESS).await;
    };
    let project = match target_project(db, &acting, project_id).await {
        Ok(p) => p,
        Err(TicketFailure::Db(e)) => return Err(e),
        Err(_) => return fail(ctx, &row, ERROR_ACCESS).await,
    };
    let file = match read_source(ctx, &row).await {
        Ok(f) => f,
        Err(TicketFailure::Db(DbErr::Custom(e))) => {
            tracing::error!(import_id = %id, error = %e, "import source unreadable");
            return fail(ctx, &row, ERROR_SOURCE).await;
        }
        Err(_) => return fail(ctx, &row, ERROR_SOURCE).await,
    };
    let mapping = row.mapping();
    let pctx = plan::Context::load(db, wid, &project, &alias_values(&file, &mapping)).await?;
    let mut seen = plan::Seen::default();
    let start = usize::try_from(row.processed).unwrap_or(0);
    for (i, values) in file.rows.iter().enumerate() {
        let plan = plan::plan_row(&pctx, &mut seen, &mapping, row_number(i), values);
        if i < start {
            continue;
        }
        let skip = |code: &str, arg: String| Step {
            skipped: Some(SkippedRow {
                row: plan.row,
                reason: code.to_string(),
                arg,
                values: values.clone(),
            }),
            ..Step::default()
        };
        let txn = db.begin().await?;
        let step = if plan.outcome == plan::Outcome::Skipped {
            let n = plan.notes.first();
            skip(
                n.map_or(plan::SKIP_FAILED, |n| n.code),
                n.map(|n| n.arg.clone()).unwrap_or_default(),
            )
        } else {
            match import_row(&txn, &acting, project.id, &plan).await {
                Ok(true) => Step {
                    imported: 1,
                    warned: i32::from(plan.outcome == plan::Outcome::Warning),
                    skipped: None,
                },
                Ok(false) => {
                    txn.rollback().await?;
                    let txn = db.begin().await?;
                    let step = skip(
                        plan::SKIP_ALIAS_TAKEN,
                        plan.alias.clone().unwrap_or_default(),
                    );
                    if advance(&txn, id, i, &step).await? {
                        txn.commit().await?;
                        continue;
                    }
                    return Ok(());
                }
                Err(TicketFailure::Db(e)) => return Err(e),
                Err(TicketFailure::NotFound) => {
                    txn.rollback().await?;
                    return fail(ctx, &row, ERROR_ACCESS).await;
                }
                Err(TicketFailure::Invalid(errors)) => {
                    txn.rollback().await?;
                    let code = errors
                        .errors
                        .values()
                        .flat_map(|es| es.iter().map(|e| e.code.to_string()))
                        .next()
                        .unwrap_or_default();
                    let txn = db.begin().await?;
                    let step = skip(plan::SKIP_FAILED, code);
                    if advance(&txn, id, i, &step).await? {
                        txn.commit().await?;
                        continue;
                    }
                    return Ok(());
                }
            }
        };
        if !advance(&txn, id, i, &step).await? {
            // Job lain sudah memproses baris ini.
            txn.rollback().await?;
            return Ok(());
        }
        txn.commit().await?;
    }
    finish(ctx, &acting, id).await
}

/// Buat satu tiket dari rencana di `txn`. `Ok(false)` = alias bentrok (pemanggil rollback).
async fn import_row(
    txn: &sea_orm::DatabaseTransaction,
    acting: &Acting<'_>,
    project_id: Uuid,
    plan: &plan::RowPlan,
) -> Result<bool, TicketFailure> {
    let wid = acting.workspace_id();
    let ticket = create::run_in(txn, acting, project_id, &plan.form).await?;
    if let Some(alias) = &plan.alias {
        if !ticket_codes::ActiveModel::claim(txn, wid, ticket.id, alias).await? {
            return Ok(false);
        }
    }
    if let Some(at) = plan.created_at {
        tickets::Entity::update_many()
            .col_expr(
                tickets::Column::CreatedAt,
                Expr::value(chrono::DateTime::<chrono::FixedOffset>::from(at)),
            )
            .filter(tickets::Column::Id.eq(ticket.id))
            .exec(txn)
            .await?;
    }
    notifications::Entity::delete_many()
        .filter(notifications::Column::WorkspaceId.eq(wid.get()))
        .filter(notifications::Column::TicketId.eq(ticket.id))
        .exec(txn)
        .await?;
    Ok(true)
}

/// `running → done` + audit di satu transaksi; berkas sumber dihapus setelah commit.
async fn finish(ctx: &AppContext, acting: &Acting<'_>, id: Uuid) -> Result<(), DbErr> {
    let txn = ctx.db.begin().await?;
    let Some(row) = ticket_imports::Entity::find_by_id(id).one(&txn).await? else {
        return Ok(());
    };
    if !ticket_imports::ActiveModel::transition(
        &txn,
        id,
        &[ImportStatus::Running],
        ImportStatus::Done,
    )
    .await?
    {
        return Ok(());
    }
    if row.skipped > 0 {
        let expires: chrono::DateTime<chrono::FixedOffset> =
            (Utc::now() + Duration::days(REPORT_RETENTION_DAYS)).into();
        ticket_imports::Entity::update_many()
            .col_expr(
                ticket_imports::Column::ReportExpiresAt,
                Expr::value(expires),
            )
            .filter(ticket_imports::Column::Id.eq(id))
            .exec(&txn)
            .await?;
    }
    audit::record(
        &txn,
        AuditEntry {
            workspace_id: acting.workspace_id(),
            actor_id: Some(acting.user.id),
            action: AuditAction::ImportCompleted,
            subject: (AuditSubject::TicketImport, id),
            changes: serde_json::json!({
                "filename": row.filename,
                "project_id": row.project_id,
                "imported": row.imported,
                "warned": row.warned,
                "skipped": row.skipped,
            }),
            request_id: None,
        },
    )
    .await?;
    txn.commit().await?;
    drop_source(ctx, &row).await
}

/// Laporan baris dilewati (CSV: Baris, Alasan, lalu kolom asli). `reason` = terjemahan alasan.
#[must_use]
pub fn report_csv(row: &ticket_imports::Model, reason: impl Fn(&SkippedRow) -> String) -> String {
    let mut out: Vec<Vec<String>> = Vec::new();
    let mut head = vec!["Baris".to_string(), "Alasan".to_string()];
    head.extend(row.headers());
    out.push(head);
    for s in row.skipped_rows() {
        let mut line = vec![s.row.to_string(), reason(&s)];
        line.extend(s.values.iter().cloned());
        out.push(line);
    }
    csv_file::write(&out)
}
