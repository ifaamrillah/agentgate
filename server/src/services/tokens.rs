//! Penghemat token (M10): pengaturan fitur opsional per project, pencatat penghematan yang
//! terukur server, dan agregasi pemakaian token dari laporan client (per minggu, tiket, job).
//!
//! Token ≈ byte / 4 (perkiraan kasar yang sama dengan estimasi scan knowledge M8). Penghematan
//! dicatat sebagai selisih antara yang harus dibaca AI tanpa fitur itu (mis. file utuh) dan yang
//! benar-benar dikirim (mis. potongan fungsi). RTK dan prefix stabil terjadi di laptop user, jadi
//! tidak terukur di sini.

use std::collections::HashMap;

use chrono::{DateTime, Duration, FixedOffset, NaiveDate};
use sea_orm::{
    ActiveEnum, ActiveModelTrait, ConnectionTrait, DbBackend, DbErr, FromQueryResult,
    IntoActiveModel, Iterable, Set, Statement, Value,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::{
    projects,
    token_savings::{self, SavingFeature},
    workspaces::WorkspaceId,
};

/// Perkiraan token dari jumlah byte teks.
#[must_use]
pub fn tokens_of_bytes(bytes: i64) -> i64 {
    bytes.max(0) / 4
}

/// Toggle fitur opsional per project (`projects.token_saver`). Fitur wajib tidak bisa dimatikan
/// dan tidak disimpan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenSaver {
    /// Gaya ringkas persona (chat dan log); artifact tetap lengkap.
    #[serde(default = "yes")]
    pub concise: bool,
    /// Model per persona (model murah untuk tugas ringan).
    #[serde(default = "yes")]
    pub persona_models: bool,
}

fn yes() -> bool {
    true
}

impl Default for TokenSaver {
    fn default() -> Self {
        Self {
            concise: true,
            persona_models: true,
        }
    }
}

impl TokenSaver {
    #[must_use]
    pub fn of(project: &projects::Model) -> Self {
        serde_json::from_value(project.token_saver.clone()).unwrap_or_default()
    }

    /// Simpan toggle project.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn save<C: ConnectionTrait>(
        self,
        db: &C,
        project: &projects::Model,
    ) -> Result<projects::Model, DbErr> {
        let mut m = project.clone().into_active_model();
        m.token_saver = Set(serde_json::to_value(self).unwrap_or_default());
        m.update(db).await
    }
}

#[derive(Debug, FromQueryResult)]
struct Sum {
    total: Option<i64>,
}

#[derive(Debug, FromQueryResult)]
struct JobRef {
    id: Uuid,
}

/// Catat satu penghematan (`saved_bytes` = byte yang tidak perlu dibaca AI). Job = job terbuka
/// terbaru milik `user` di project itu (dan tiket itu, bila ada): sesi MCP tidak membawa id job.
/// Nilai ≤ 0 diabaikan.
///
/// # Errors
///
/// Error database.
#[allow(clippy::too_many_arguments)]
pub async fn record<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project_id: Uuid,
    ticket_id: Option<Uuid>,
    user_id: Uuid,
    feature: SavingFeature,
    tool: &str,
    saved_bytes: i64,
) -> Result<(), DbErr> {
    let tokens = tokens_of_bytes(saved_bytes);
    if tokens <= 0 {
        return Ok(());
    }
    let job = JobRef::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT id FROM jobs WHERE workspace_id = $1 AND project_id = $2 AND claimed_by = $3 \
         AND status IN ('claimed', 'running') AND ($4::uuid IS NULL OR ticket_id = $4) \
         ORDER BY started_at DESC NULLS LAST, created_at DESC LIMIT 1",
        [
            wid.into(),
            project_id.into(),
            user_id.into(),
            Value::Uuid(ticket_id),
        ],
    ))
    .one(db)
    .await?;
    token_savings::ActiveModel {
        workspace_id: Set(wid.get()),
        project_id: Set(project_id),
        job_id: Set(job.map(|j| j.id)),
        ticket_id: Set(ticket_id),
        feature: Set(feature.to_value()),
        tool: Set(tool.chars().take(40).collect()),
        tokens: Set(tokens),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(())
}

/// Total byte file (berbeda) tempat simbol-simbol ini berada.
///
/// # Errors
///
/// Error database.
pub async fn file_bytes_of_symbols<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    symbol_ids: &[Uuid],
) -> Result<i64, DbErr> {
    if symbol_ids.is_empty() {
        return Ok(0);
    }
    let row = Sum::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT COALESCE(SUM(f.bytes), 0)::bigint AS total FROM graph_files f \
         WHERE f.workspace_id = $1 AND f.id IN (SELECT DISTINCT s.file_id FROM graph_symbols s \
         WHERE s.workspace_id = $1 AND s.id = ANY($2))",
        [wid.into(), Value::from(symbol_ids.to_vec())],
    ))
    .one(db)
    .await?;
    Ok(row.and_then(|r| r.total).unwrap_or(0))
}

/// Pemakaian token dalam satu rentang (job yang selesai).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, FromQueryResult)]
pub struct Usage {
    pub input: i64,
    pub output: i64,
    pub cache_read: i64,
    pub cache_write: i64,
    /// Total = jumlah keempatnya, atau `jobs.tokens` bila client hanya melapor total.
    pub total: i64,
    pub cost_micros: i64,
    /// Job yang melaporkan pemakaian.
    pub jobs: i64,
    /// Client berbeda yang melapor.
    pub clients: i64,
}

const USAGE_COLUMNS: &str = "COALESCE(SUM(j.tokens_input), 0)::bigint AS input, \
     COALESCE(SUM(j.tokens_output), 0)::bigint AS output, \
     COALESCE(SUM(j.tokens_cache_read), 0)::bigint AS cache_read, \
     COALESCE(SUM(j.tokens_cache_write), 0)::bigint AS cache_write, \
     COALESCE(SUM(j.tokens), 0)::bigint AS total, \
     COALESCE(SUM(j.cost_usd_micros), 0)::bigint AS cost_micros, \
     COUNT(*)::bigint AS jobs, COUNT(DISTINCT NULLIF(j.client, ''))::bigint AS clients";

/// Pemakaian project sejak `since`.
///
/// # Errors
///
/// Error database.
pub async fn usage_since<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project_id: Uuid,
    since: DateTime<FixedOffset>,
) -> Result<Usage, DbErr> {
    Ok(Usage::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        format!(
            "SELECT {USAGE_COLUMNS} FROM jobs j WHERE j.workspace_id = $1 AND j.project_id = $2 \
             AND j.tokens IS NOT NULL AND j.finished_at >= $3"
        ),
        [wid.into(), project_id.into(), since.into()],
    ))
    .one(db)
    .await?
    .unwrap_or_default())
}

#[derive(Debug, FromQueryResult)]
struct FeatureSum {
    feature: String,
    total: i64,
}

/// Token yang dihemat per fitur sejak `since`.
///
/// # Errors
///
/// Error database.
pub async fn saved_since<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project_id: Uuid,
    since: DateTime<FixedOffset>,
) -> Result<HashMap<SavingFeature, i64>, DbErr> {
    let rows = FeatureSum::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT feature, COALESCE(SUM(tokens), 0)::bigint AS total FROM token_savings \
         WHERE workspace_id = $1 AND project_id = $2 AND created_at >= $3 GROUP BY feature",
        [wid.into(), project_id.into(), since.into()],
    ))
    .all(db)
    .await?;
    let mut out: HashMap<SavingFeature, i64> = SavingFeature::iter().map(|f| (f, 0)).collect();
    for r in rows {
        if let Ok(f) = SavingFeature::try_from_value(&r.feature) {
            out.insert(f, r.total);
        }
    }
    Ok(out)
}

/// Satu minggu (mulai Senin) di grafik tren.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Week {
    pub start: NaiveDate,
    pub used: i64,
    pub saved: i64,
}

#[derive(Debug, FromQueryResult)]
struct WeekRow {
    week: NaiveDate,
    total: i64,
}

/// `n` minggu terakhir sampai minggu `today` (UTC), terpakai vs dihemat.
///
/// # Errors
///
/// Error database.
pub async fn weeks<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project_id: Uuid,
    today: NaiveDate,
    n: u32,
) -> Result<Vec<Week>, DbErr> {
    use chrono::Datelike;
    let this_monday =
        today - Duration::days(i64::from(today.weekday().num_days_from_monday()));
    let first = this_monday - Duration::weeks(i64::from(n.saturating_sub(1)));
    let since = first
        .and_hms_opt(0, 0, 0)
        .map(|d| d.and_utc().fixed_offset())
        .unwrap_or_default();
    let fetch = |sql: &str| {
        WeekRow::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            [wid.into(), project_id.into(), since.into()],
        ))
        .all(db)
    };
    let used = fetch(
        "SELECT (date_trunc('week', finished_at AT TIME ZONE 'UTC'))::date AS week, \
         COALESCE(SUM(tokens), 0)::bigint AS total FROM jobs WHERE workspace_id = $1 \
         AND project_id = $2 AND tokens IS NOT NULL AND finished_at >= $3 GROUP BY 1",
    )
    .await?;
    let saved = fetch(
        "SELECT (date_trunc('week', created_at AT TIME ZONE 'UTC'))::date AS week, \
         COALESCE(SUM(tokens), 0)::bigint AS total FROM token_savings WHERE workspace_id = $1 \
         AND project_id = $2 AND created_at >= $3 GROUP BY 1",
    )
    .await?;
    let get = |rows: &[WeekRow], d: NaiveDate| {
        rows.iter()
            .find(|r| r.week == d)
            .map_or(0, |r| r.total)
    };
    Ok((0..n)
        .map(|i| {
            let start = first + Duration::weeks(i64::from(i));
            Week {
                start,
                used: get(&used, start),
                saved: get(&saved, start),
            }
        })
        .collect())
}

/// Pemakaian satu tiket.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, FromQueryResult)]
pub struct TicketUsage {
    pub code: String,
    pub title: String,
    pub input: i64,
    pub output: i64,
    pub cache: i64,
    pub total: i64,
    pub cost_micros: i64,
    pub jobs: i64,
}

/// Tiket dengan pemakaian terbesar sejak `since`.
///
/// # Errors
///
/// Error database.
pub async fn per_ticket<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project_id: Uuid,
    since: DateTime<FixedOffset>,
    limit: u64,
) -> Result<Vec<TicketUsage>, DbErr> {
    TicketUsage::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT t.code, t.title, \
         COALESCE(SUM(j.tokens_input), 0)::bigint AS input, \
         COALESCE(SUM(j.tokens_output), 0)::bigint AS output, \
         COALESCE(SUM(COALESCE(j.tokens_cache_read, 0) + COALESCE(j.tokens_cache_write, 0)), 0)::bigint AS cache, \
         COALESCE(SUM(j.tokens), 0)::bigint AS total, \
         COALESCE(SUM(j.cost_usd_micros), 0)::bigint AS cost_micros, COUNT(*)::bigint AS jobs \
         FROM jobs j JOIN tickets t ON t.id = j.ticket_id AND t.workspace_id = j.workspace_id \
         WHERE j.workspace_id = $1 AND j.project_id = $2 AND j.tokens IS NOT NULL \
         AND j.finished_at >= $3 AND t.deleted_at IS NULL \
         GROUP BY t.id, t.code, t.title ORDER BY total DESC, t.code LIMIT $4",
        [
            wid.into(),
            project_id.into(),
            since.into(),
            i64::try_from(limit).unwrap_or(i64::MAX).into(),
        ],
    ))
    .all(db)
    .await
}

/// Satu job di tabel pemakaian.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, FromQueryResult)]
pub struct JobUsage {
    pub id: Uuid,
    pub code: Option<String>,
    pub persona: String,
    pub client: String,
    pub model: String,
    pub status: String,
    pub input: Option<i64>,
    pub output: Option<i64>,
    pub cache: Option<i64>,
    pub total: Option<i64>,
    pub cost_micros: Option<i64>,
    pub finished_at: Option<DateTime<FixedOffset>>,
    pub saved: i64,
}

/// Job terbaru project yang punya data pemakaian atau sedang berjalan (maks. `limit`).
///
/// # Errors
///
/// Error database.
pub async fn per_job<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project_id: Uuid,
    limit: u64,
) -> Result<Vec<JobUsage>, DbErr> {
    JobUsage::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT j.id, t.code, j.persona, j.client, j.model, j.status, \
         j.tokens_input AS input, j.tokens_output AS output, \
         CASE WHEN j.tokens_cache_read IS NULL AND j.tokens_cache_write IS NULL THEN NULL \
              ELSE COALESCE(j.tokens_cache_read, 0) + COALESCE(j.tokens_cache_write, 0) END AS cache, \
         j.tokens AS total, j.cost_usd_micros AS cost_micros, j.finished_at, \
         COALESCE((SELECT SUM(s.tokens) FROM token_savings s WHERE s.job_id = j.id), 0)::bigint AS saved \
         FROM jobs j LEFT JOIN tickets t ON t.id = j.ticket_id AND t.workspace_id = j.workspace_id \
         WHERE j.workspace_id = $1 AND j.project_id = $2 \
         AND (j.tokens IS NOT NULL OR j.status IN ('claimed', 'running')) \
         ORDER BY COALESCE(j.finished_at, j.started_at, j.created_at) DESC LIMIT $3",
        [
            wid.into(),
            project_id.into(),
            i64::try_from(limit).unwrap_or(i64::MAX).into(),
        ],
    ))
    .all(db)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_to_tokens_never_negative() {
        assert_eq!(tokens_of_bytes(4_000), 1_000);
        assert_eq!(tokens_of_bytes(-10), 0);
    }

    #[test]
    fn saver_defaults_to_everything_on() {
        assert_eq!(
            serde_json::from_value::<TokenSaver>(serde_json::json!({})).unwrap(),
            TokenSaver::default()
        );
        let off: TokenSaver = serde_json::from_value(serde_json::json!({ "concise": false })).unwrap();
        assert!(!off.concise && off.persona_models);
    }
}
