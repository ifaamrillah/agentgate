//! Pembersihan harian (job `workers::cleanup`, dijadwalkan scheduler 03:00 UTC; juga task
//! `bin/loco task cleanup`). Sembilan langkah, masing-masing berdiri sendiri: langkah atau item
//! yang gagal dicatat di log dan dihitung `failed`, lalu pembersihan berlanjut (rules §8).
//! Idempoten: menjalankannya lagi hanya menemukan yang belum dibersihkan.
//!
//! 1. Sesi kedaluwarsa di store sesi Loco.
//! 2. `login_throttles` yang tidak terkunci dan jendelanya sudah lewat.
//! 3. `user_tokens` yang kedaluwarsa atau dipakai lebih dari [`TOKEN_RETENTION_DAYS`] hari lalu.
//! 4. Undangan yang selesai lebih dari [`INVITATION_RETENTION_DAYS`] hari lalu (riwayatnya
//!    tetap di audit log).
//! 5. Workspace yang terjadwal hapus lebih dari masa tenggang: satu transaksi per workspace;
//!    seluruh datanya, termasuk audit log-nya, ikut terhapus (FK `ON DELETE CASCADE`).
//! 6. Tiket yang di sampah lebih dari [`tickets::trash::RETENTION_DAYS`] hari, beserta file
//!    lampirannya (`services::tickets::trash::purge_expired`).
//! 7. Ekspor data `ready` yang lewat 7 hari → `expired`, berkas ZIP-nya dihapus (M11).
//! 8. Laporan baris dilewati impor CSV yang lewat 30 hari dikosongkan (M11).
//! 9. Draf impor CSV yang ditinggal lebih dari 7 hari dibatalkan, berkas sumbernya dihapus.
//!
//! [`CleanupMode::DryRun`] hanya menghitung (task tanpa `confirm:yes`).

use chrono::{Duration, Utc};
use loco_rs::{app::AppContext, auth::session_store};
use sea_orm::{ConnectionTrait, DbBackend, DbErr, Statement, TransactionTrait};

use crate::{
    common::settings::Settings,
    models::{invitations, login_throttles, user_tokens, workspaces},
    services::tickets,
};

/// Token email yang sudah tidak berlaku disimpan seminggu untuk penelusuran (mis. user
/// melapor link reset tidak jalan), lalu dihapus.
pub const TOKEN_RETENTION_DAYS: i64 = 7;
/// Undangan yang sudah selesai disimpan sebulan (daftar undangan di Pengaturan → Anggota).
pub const INVITATION_RETENTION_DAYS: i64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanupMode {
    /// Hanya menghitung yang akan dibersihkan.
    DryRun,
    Apply,
}

/// Langkah pembersihan, urut sesuai jalannya.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanupStep {
    Sessions,
    LoginThrottles,
    UserTokens,
    Invitations,
    Workspaces,
    Tickets,
    Exports,
    ImportReports,
    ImportDrafts,
}

impl CleanupStep {
    /// Nama langkah untuk log dan output task.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Sessions => "sessions",
            Self::LoginThrottles => "login_throttles",
            Self::UserTokens => "user_tokens",
            Self::Invitations => "invitations",
            Self::Workspaces => "workspaces",
            Self::Tickets => "tickets",
            Self::Exports => "exports",
            Self::ImportReports => "import_reports",
            Self::ImportDrafts => "import_drafts",
        }
    }
}

/// Hasil satu langkah: baris yang dibersihkan (atau akan, saat dry run) dan item yang gagal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepOutcome {
    pub step: CleanupStep,
    pub count: u64,
    pub failed: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CleanupReport {
    pub steps: Vec<StepOutcome>,
}

impl CleanupReport {
    /// Jumlah yang dibersihkan langkah `step`.
    #[must_use]
    pub fn count(&self, step: CleanupStep) -> u64 {
        self.steps
            .iter()
            .filter(|s| s.step == step)
            .map(|s| s.count)
            .sum()
    }

    /// Total langkah atau item yang gagal.
    #[must_use]
    pub fn failed(&self) -> u64 {
        self.steps.iter().map(|s| s.failed).sum()
    }

    fn push(&mut self, step: CleanupStep, result: Result<u64, loco_rs::Error>) {
        let outcome = match result {
            Ok(count) => StepOutcome {
                step,
                count,
                failed: 0,
            },
            Err(err) => {
                tracing::error!(step = step.name(), error = %err, "cleanup step failed");
                StepOutcome {
                    step,
                    count: 0,
                    failed: 1,
                }
            }
        };
        self.steps.push(outcome);
    }
}

/// Jalankan semua langkah. Tidak pernah gagal karena satu langkah: kegagalan ada di
/// [`CleanupReport::failed`] dan di log.
pub async fn run(ctx: &AppContext, mode: CleanupMode) -> CleanupReport {
    let now = Utc::now();
    let apply = mode == CleanupMode::Apply;
    let mut report = CleanupReport::default();

    report.push(CleanupStep::Sessions, sessions(ctx, apply).await);

    let throttles = async {
        let window = Duration::minutes(Settings::from_ctx(ctx)?.auth.login_window_minutes);
        Ok::<_, loco_rs::Error>(if apply {
            login_throttles::delete_stale(&ctx.db, window, now).await?
        } else {
            login_throttles::count_stale(&ctx.db, window, now).await?
        })
    };
    report.push(CleanupStep::LoginThrottles, throttles.await);

    let cutoff = now - Duration::days(TOKEN_RETENTION_DAYS);
    let tokens = if apply {
        user_tokens::delete_finished_before(&ctx.db, cutoff).await
    } else {
        user_tokens::count_finished_before(&ctx.db, cutoff).await
    };
    report.push(CleanupStep::UserTokens, tokens.map_err(Into::into));

    let cutoff = now - Duration::days(INVITATION_RETENTION_DAYS);
    let finished = if apply {
        invitations::Model::delete_finished_before(&ctx.db, cutoff).await
    } else {
        invitations::Model::count_finished_before(&ctx.db, cutoff).await
    };
    report.push(CleanupStep::Invitations, finished.map_err(Into::into));

    report.steps.push(purge_workspaces(ctx, now, apply).await);

    let trashed = tickets::trash::purge_expired(&ctx.db, &ctx.storage, now, apply).await;
    report.push(CleanupStep::Tickets, trashed.map_err(Into::into));

    let exports = crate::services::exports::expire(ctx, now, apply).await;
    report.push(CleanupStep::Exports, exports.map_err(Into::into));
    let reports =
        crate::models::ticket_imports::ActiveModel::purge_reports(&ctx.db, now, apply).await;
    report.push(CleanupStep::ImportReports, reports.map_err(Into::into));
    let drafts = crate::services::imports::expire_drafts(ctx, now, apply).await;
    report.push(CleanupStep::ImportDrafts, drafts.map_err(Into::into));
    report
}

/// Sesi kedaluwarsa. Store Loco menghapus per batch, jadi dipanggil sampai habis. Dry run
/// menghitung langsung di tabel store database Loco (`expires_at` = detik unix).
async fn sessions(ctx: &AppContext, apply: bool) -> Result<u64, loco_rs::Error> {
    if apply {
        let store = session_store::of(ctx).await?;
        let mut total = 0;
        loop {
            let purged = store.purge_expired().await?;
            total += purged;
            if purged == 0 {
                return Ok(total);
            }
        }
    }
    // Tabel dibuat store saat sesi pertama dipakai; nama tabel di query di-resolve saat parse,
    // jadi keberadaannya dicek dengan query terpisah lebih dulu.
    let exists = ctx
        .db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT to_regclass('loco_sessions') IS NOT NULL AS present",
        ))
        .await?
        .map(|row| row.try_get::<bool>("", "present"))
        .transpose()?
        .unwrap_or(false);
    if !exists {
        return Ok(0);
    }
    let count = ctx
        .db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT count(*) AS n FROM loco_sessions WHERE expires_at < $1",
            [Utc::now().timestamp().into()],
        ))
        .await?
        .map(|row| row.try_get::<i64>("", "n"))
        .transpose()?
        .unwrap_or(0);
    Ok(u64::try_from(count).unwrap_or_default())
}

/// Workspace yang masa tenggangnya lewat, satu transaksi per workspace: yang gagal dicatat
/// dan dilewati, sisanya tetap dihapus.
async fn purge_workspaces(
    ctx: &AppContext,
    now: chrono::DateTime<Utc>,
    apply: bool,
) -> StepOutcome {
    let step = CleanupStep::Workspaces;
    let due = match workspaces::Model::due_for_purge(&ctx.db, now).await {
        Ok(due) => due,
        Err(err) => {
            tracing::error!(step = step.name(), error = %err, "cleanup step failed");
            return StepOutcome {
                step,
                count: 0,
                failed: 1,
            };
        }
    };
    if !apply {
        return StepOutcome {
            step,
            count: due.len() as u64,
            failed: 0,
        };
    }
    let mut outcome = StepOutcome {
        step,
        count: 0,
        failed: 0,
    };
    for id in due {
        match purge_one(ctx, id, now).await {
            Ok(true) => {
                // Hanya id: nama/slug workspace yang sudah dihapus tidak ditulis ke log.
                tracing::info!(workspace_id = %id, "workspace purged after grace period");
                outcome.count += 1;
            }
            // Dibatalkan atau sudah dihapus proses lain sejak daftar dibaca.
            Ok(false) => {}
            Err(err) => {
                tracing::error!(workspace_id = %id, error = %err, "workspace purge failed");
                outcome.failed += 1;
            }
        }
    }
    outcome
}

async fn purge_one(
    ctx: &AppContext,
    id: uuid::Uuid,
    now: chrono::DateTime<Utc>,
) -> Result<bool, DbErr> {
    let txn = ctx.db.begin().await?;
    let purged = workspaces::Model::purge_if_due(&txn, id, now).await?;
    txn.commit().await?;
    Ok(purged)
}
