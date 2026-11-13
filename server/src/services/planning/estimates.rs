//! Estimasi tiket dan kalibrasinya.
//!
//! - **Usulan AI** masuk lewat MCP (`ag_planning estimate`, `ag_draft_tickets` → tiket): satu baris
//!   `ticket_estimates` sumber `ai`. Bila tiket belum punya story point, poin AI langsung dipakai
//!   (bisa diubah manusia).
//! - **Ubahan manusia** (panel estimasi tiket): baris sumber `human` + story point tiket.
//! - **Aktual** per stage dihitung dari riwayat `stage_changed` (jam kerja 09.00–17.00 WIB,
//!   Senin–Jumat); waktu AI = durasi job di stage itu, sisanya waktu manusia (kerja + menunggu).
//! - **Kalibrasi** per stage = median `aktual / estimasi` dari tiket Selesai di project yang punya
//!   estimasi AI; dipakai untuk perkiraan selesai (roadmap, kalender, rilis).
use std::collections::{BTreeMap, HashMap};

use chrono::{DateTime, Duration, NaiveDate, Timelike, Utc};
use sea_orm::{
    ActiveEnum, ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait,
    DatabaseConnection, DbErr, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder,
    TransactionTrait,
};
use uuid::Uuid;

use super::{ESTIMATE_INVALID, Failure, HOURS_PER_DAY, scope};
use crate::{
    models::{
        jobs,
        permission::Permission,
        ticket_estimates::{self, BASIS_MAX_LEN, EstimateSource, HOURS_MAX, StageHours},
        ticket_events::{self, EventKind},
        tickets::{self, STORY_POINTS_MAX},
        workspaces::WorkspaceId,
    },
    services::{tickets::access_in, workspaces::Acting},
};

/// Isian estimasi (AI atau manusia).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EstimateInput {
    pub points: Option<i32>,
    pub stages: Vec<StageHours>,
    pub basis: String,
}

/// Validasi poin dan jam.
///
/// # Errors
///
/// `planning-estimate-invalid`.
pub fn validate(input: &EstimateInput) -> Result<EstimateInput, Failure> {
    if input
        .points
        .is_some_and(|p| !(0..=STORY_POINTS_MAX).contains(&p))
    {
        return Err(Failure::field("points", ESTIMATE_INVALID));
    }
    let mut stages: Vec<StageHours> = Vec::new();
    for s in &input.stages {
        let key = s.stage.trim();
        let ok = |h: f64| h.is_finite() && (0.0..=HOURS_MAX).contains(&h);
        if key.is_empty() || key.len() > 60 || !ok(s.ai_hours) || !ok(s.human_hours) {
            return Err(Failure::field("stages", ESTIMATE_INVALID));
        }
        if stages.iter().any(|x| x.stage == key) {
            continue;
        }
        stages.push(StageHours {
            stage: key.to_string(),
            ai_hours: (s.ai_hours * 10.0).round() / 10.0,
            human_hours: (s.human_hours * 10.0).round() / 10.0,
        });
    }
    if stages.len() > 30 {
        return Err(Failure::field("stages", ESTIMATE_INVALID));
    }
    let basis: String = input.basis.trim().chars().take(BASIS_MAX_LEN).collect();
    Ok(EstimateInput {
        points: input.points,
        stages,
        basis,
    })
}

/// Simpan satu estimasi untuk tiket (transaksi pemanggil). Sumber AI mengisi story point bila
/// masih kosong; sumber manusia selalu menetapkan story point.
///
/// # Errors
///
/// Validasi, error database.
pub async fn record_in<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    ticket: &tickets::Model,
    source: EstimateSource,
    input: &EstimateInput,
    author: Option<Uuid>,
    persona: &str,
) -> Result<ticket_estimates::Model, Failure> {
    let valid = validate(input)?;
    let row = ticket_estimates::ActiveModel {
        workspace_id: Set(wid.get()),
        ticket_id: Set(ticket.id),
        source: Set(source.to_value()),
        points: Set(valid.points),
        stages: Set(serde_json::to_value(&valid.stages).unwrap_or_default()),
        basis: Set(valid.basis.clone()),
        author_id: Set(author),
        persona: Set(persona.chars().take(60).collect()),
        ..Default::default()
    }
    .insert(db)
    .await?;
    let set_points = match source {
        EstimateSource::Ai => ticket.story_points.is_none() && valid.points.is_some(),
        EstimateSource::Human => ticket.story_points != valid.points,
    };
    if set_points {
        let mut m = ticket.clone().into_active_model();
        m.story_points = Set(valid.points);
        let updated = m.update(db).await?;
        scope::points_changed(db, wid, &updated, valid.points).await?;
        ticket_events::ActiveModel::record(
            db,
            wid,
            ticket.id,
            author,
            EventKind::Updated,
            serde_json::json!({
                "story_points": { "from": ticket.story_points, "to": valid.points }
            }),
        )
        .await?;
    }
    Ok(row)
}

/// Manusia mengubah estimasi dari panel tiket (`EditProject`).
///
/// # Errors
///
/// Validasi, `NotFound`, error database.
pub async fn save_human(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    ticket_id: Uuid,
    input: &EstimateInput,
) -> Result<ticket_estimates::Model, Failure> {
    let wid = acting.workspace_id();
    let txn = db.begin().await?;
    let ticket = tickets::Model::lock_in(&txn, wid, ticket_id)
        .await?
        .filter(|t| !t.is_trashed())
        .ok_or(Failure::NotFound)?;
    access_in(&txn, acting, ticket.project_id, Permission::EditProject).await?;
    let row = record_in(
        &txn,
        wid,
        &ticket,
        EstimateSource::Human,
        input,
        Some(acting.user.id),
        "",
    )
    .await?;
    txn.commit().await?;
    Ok(row)
}

/// Estimasi terbaru per tiket: usulan AI terakhir dan estimasi yang berlaku (baris terakhir).
#[derive(Debug, Clone, Default)]
pub struct Latest {
    pub ai: Option<ticket_estimates::Model>,
    pub current: Option<ticket_estimates::Model>,
}

impl Latest {
    /// Jam (AI, manusia) estimasi yang berlaku.
    #[must_use]
    pub fn hours(&self) -> Option<(f64, f64)> {
        self.current.as_ref().map(ticket_estimates::Model::totals)
    }
}

/// Estimasi terbaru untuk tiket-tiket ini.
///
/// # Errors
///
/// Error database.
pub async fn latest<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    ids: &[Uuid],
) -> Result<HashMap<Uuid, Latest>, DbErr> {
    let mut out: HashMap<Uuid, Latest> = HashMap::new();
    for row in ticket_estimates::Model::of_tickets(db, wid, ids).await? {
        let e = out.entry(row.ticket_id).or_default();
        if row.source() == EstimateSource::Ai {
            e.ai = Some(row.clone());
        }
        e.current = Some(row);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// Aktual

/// Jam kerja (09.00–17.00 WIB, Senin–Jumat) di antara dua waktu.
#[must_use]
pub fn work_hours(from: DateTime<Utc>, to: DateTime<Utc>) -> f64 {
    if to <= from {
        return 0.0;
    }
    let wib = Duration::hours(7);
    let (a, b) = (from + wib, to + wib);
    let mut day = a.date_naive();
    let mut total = 0.0;
    while day <= b.date_naive() {
        if super::is_workday(day) {
            let open = day.and_hms_opt(9, 0, 0).unwrap_or_default().and_utc();
            let close = day.and_hms_opt(17, 0, 0).unwrap_or_default().and_utc();
            let s = a.max(open);
            let e = b.min(close);
            if e > s {
                #[allow(clippy::cast_precision_loss)]
                let secs = (e - s).num_seconds() as f64;
                total += secs / 3600.0;
            }
        }
        day = day.succ_opt().unwrap_or(day);
        if day == NaiveDate::MAX {
            break;
        }
    }
    total
}

/// Kunjungan stage satu tiket: (kunci stage, masuk, keluar).
pub type Visit = (String, DateTime<Utc>, DateTime<Utc>);

/// Kunjungan stage dari riwayat (`stage_changed` dan perubahan lain berisi `stage.to_key`).
/// Stage terakhir berakhir di `now` (atau `done_at` bila tiket selesai).
#[must_use]
pub fn visits(
    ticket: &tickets::Model,
    events: &[ticket_events::Model],
    now: DateTime<Utc>,
) -> Vec<Visit> {
    let mut out = Vec::new();
    let mut current: Option<(String, DateTime<Utc>)> = None;
    for e in events {
        let stage = &e.changes["stage"];
        let Some(to) = stage["to_key"].as_str() else {
            continue;
        };
        let at = e.created_at.with_timezone(&Utc);
        let (from_key, entered) = current.take().unwrap_or_else(|| {
            (
                stage["from_key"].as_str().unwrap_or_default().to_string(),
                ticket.created_at.with_timezone(&Utc),
            )
        });
        if !from_key.is_empty() {
            out.push((from_key, entered, at));
        }
        current = Some((to.to_string(), at));
    }
    let end = ticket.done_at.map_or(now, |d| d.with_timezone(&Utc));
    let (key, entered) = current.unwrap_or_else(|| {
        (
            ticket.stage_key.clone(),
            ticket.created_at.with_timezone(&Utc),
        )
    });
    if ticket.done_at.is_none() || entered < end {
        out.push((key, entered, end.max(entered)));
    }
    out
}

/// Jam aktual per stage: (AI, manusia).
pub type StageActual = BTreeMap<String, (f64, f64)>;

/// Jam aktual per stage tiket dari kunjungan dan job.
#[must_use]
pub fn actual_hours(visits: &[Visit], jobs: &[jobs::Model]) -> StageActual {
    let mut out: StageActual = BTreeMap::new();
    for (key, from, to) in visits {
        let total = work_hours(*from, *to);
        let e = out.entry(key.clone()).or_default();
        e.1 += total;
    }
    for j in jobs {
        let (Some(s), Some(f)) = (j.started_at, j.finished_at) else {
            continue;
        };
        #[allow(clippy::cast_precision_loss)]
        let ai = (f - s).num_seconds().max(0) as f64 / 3600.0;
        let e = out.entry(j.stage_key.clone()).or_default();
        e.0 += ai;
    }
    for v in out.values_mut() {
        v.1 = (v.1 - v.0).max(0.0);
    }
    out
}

/// Riwayat stage + job untuk sekumpulan tiket.
///
/// # Errors
///
/// Error database.
pub async fn actuals<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    ts: &[tickets::Model],
) -> Result<HashMap<Uuid, StageActual>, DbErr> {
    let ids: Vec<Uuid> = ts.iter().map(|t| t.id).collect();
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let events = ticket_events::Entity::find()
        .filter(ticket_events::Column::WorkspaceId.eq(wid.get()))
        .filter(ticket_events::Column::TicketId.is_in(ids.clone()))
        .order_by_asc(ticket_events::Column::CreatedAt)
        .order_by_asc(ticket_events::Column::Id)
        .all(db)
        .await?;
    let all_jobs = jobs::Entity::find()
        .filter(jobs::Column::WorkspaceId.eq(wid.get()))
        .filter(jobs::Column::TicketId.is_in(ids))
        .all(db)
        .await?;
    let now = Utc::now();
    Ok(ts
        .iter()
        .map(|t| {
            let ev: Vec<ticket_events::Model> = events
                .iter()
                .filter(|e| e.ticket_id == t.id)
                .cloned()
                .collect();
            let js: Vec<jobs::Model> = all_jobs
                .iter()
                .filter(|j| j.ticket_id == Some(t.id) && j.kind == "stage")
                .cloned()
                .collect();
            (t.id, actual_hours(&visits(t, &ev, now), &js))
        })
        .collect())
}

// ---------------------------------------------------------------------------------------------
// Kalibrasi

/// Faktor koreksi satu stage.
#[derive(Debug, Clone, PartialEq)]
pub struct StageFactor {
    pub stage: String,
    /// Median estimasi (AI, manusia) jam.
    pub estimate: (f64, f64),
    /// Median aktual (AI, manusia) jam.
    pub actual: (f64, f64),
    /// Median aktual / estimasi.
    pub factor: f64,
    pub samples: usize,
}

/// Satu titik estimasi vs aktual (scatter).
#[derive(Debug, Clone, PartialEq)]
pub struct CalibPoint {
    pub ticket: Uuid,
    pub code: String,
    pub estimate: f64,
    pub actual: f64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Calibration {
    pub stages: Vec<StageFactor>,
    pub points: Vec<CalibPoint>,
    /// Rata-rata `min(e,a)/max(e,a)` (0–1); `None` tanpa data.
    pub accuracy: Option<f64>,
    /// Median aktual/estimasi seluruh tiket (>1 = AI terlalu optimis).
    pub overall: Option<f64>,
}

impl Calibration {
    /// Faktor stage (1 bila belum ada data).
    #[must_use]
    pub fn factor(&self, stage: &str) -> f64 {
        self.stages
            .iter()
            .find(|s| s.stage == stage)
            .map_or_else(|| self.overall.unwrap_or(1.0), |s| s.factor)
    }
}

pub fn median(mut v: Vec<f64>) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(f64::total_cmp);
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        f64::midpoint(v[n / 2 - 1], v[n / 2])
    }
}

/// Hitung kalibrasi dari tiket selesai: estimasi AI terakhir vs aktual.
#[must_use]
pub fn calibrate(
    done: &[(Uuid, String, ticket_estimates::Model, StageActual)],
) -> Calibration {
    let mut per_stage: BTreeMap<String, Vec<(f64, f64, f64, f64)>> = BTreeMap::new();
    let mut points = Vec::new();
    let mut ratios = Vec::new();
    let mut accuracy = Vec::new();
    for (id, code, est, actual) in done {
        let stages = est.stages();
        if stages.is_empty() {
            continue;
        }
        let (mut e_total, mut a_total) = (0.0, 0.0);
        for s in &stages {
            let (aa, ah) = actual.get(&s.stage).copied().unwrap_or_default();
            e_total += s.ai_hours + s.human_hours;
            a_total += aa + ah;
            per_stage
                .entry(s.stage.clone())
                .or_default()
                .push((s.ai_hours, s.human_hours, aa, ah));
        }
        if e_total > 0.0 {
            ratios.push(a_total / e_total);
            accuracy.push(e_total.min(a_total) / e_total.max(a_total).max(f64::EPSILON));
            points.push(CalibPoint {
                ticket: *id,
                code: code.clone(),
                estimate: e_total,
                actual: a_total,
            });
        }
    }
    let stages = per_stage
        .into_iter()
        .map(|(stage, rows)| {
            let est = rows.iter().map(|r| r.0 + r.1).collect::<Vec<_>>();
            let act = rows.iter().map(|r| r.2 + r.3).collect::<Vec<_>>();
            let factors: Vec<f64> = rows
                .iter()
                .filter(|r| r.0 + r.1 > 0.0)
                .map(|r| (r.2 + r.3) / (r.0 + r.1))
                .collect();
            let _ = (est, act);
            StageFactor {
                stage,
                estimate: (
                    median(rows.iter().map(|r| r.0).collect()),
                    median(rows.iter().map(|r| r.1).collect()),
                ),
                actual: (
                    median(rows.iter().map(|r| r.2).collect()),
                    median(rows.iter().map(|r| r.3).collect()),
                ),
                factor: if factors.is_empty() {
                    1.0
                } else {
                    median(factors)
                },
                samples: rows.len(),
            }
        })
        .collect();
    #[allow(clippy::cast_precision_loss)]
    let acc = (!accuracy.is_empty()).then(|| accuracy.iter().sum::<f64>() / accuracy.len() as f64);
    Calibration {
        stages,
        points,
        accuracy: acc,
        overall: (!ratios.is_empty()).then(|| median(ratios)),
    }
}

/// Kalibrasi project: tiket Selesai dengan estimasi AI.
///
/// # Errors
///
/// Error database.
pub async fn calibration<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project_id: Uuid,
) -> Result<Calibration, DbErr> {
    let done = tickets::Entity::find()
        .filter(tickets::Column::WorkspaceId.eq(wid.get()))
        .filter(tickets::Column::ProjectId.eq(project_id))
        .filter(tickets::Column::DeletedAt.is_null())
        .filter(tickets::Column::Status.eq("done"))
        .all(db)
        .await?;
    let ids: Vec<Uuid> = done.iter().map(|t| t.id).collect();
    let est = latest(db, wid, &ids).await?;
    let with: Vec<tickets::Model> = done
        .into_iter()
        .filter(|t| est.get(&t.id).is_some_and(|e| e.ai.is_some()))
        .collect();
    let act = actuals(db, wid, &with).await?;
    let rows: Vec<_> = with
        .iter()
        .filter_map(|t| {
            Some((
                t.id,
                t.code.clone(),
                est.get(&t.id)?.ai.clone()?,
                act.get(&t.id).cloned().unwrap_or_default(),
            ))
        })
        .collect();
    Ok(calibrate(&rows))
}

/// Sisa jam terkalibrasi tiket: stage estimasi mulai dari stage sekarang (atau semua bila stage
/// sekarang tidak ada di estimasi). `None` = tanpa estimasi.
#[must_use]
pub fn remaining_hours(
    ticket: &tickets::Model,
    estimate: Option<&ticket_estimates::Model>,
    calib: &Calibration,
) -> Option<f64> {
    if ticket.status == "done" {
        return Some(0.0);
    }
    let stages = estimate?.stages();
    if stages.is_empty() {
        return None;
    }
    let from = stages
        .iter()
        .position(|s| s.stage == ticket.stage_key)
        .unwrap_or(0);
    Some(
        stages[from..]
            .iter()
            .map(|s| (s.ai_hours + s.human_hours) * calib.factor(&s.stage))
            .sum(),
    )
}

/// Perkiraan tanggal selesai dari sisa jam, mulai `from` (hari kerja).
#[must_use]
pub fn finish_date(from: NaiveDate, hours: f64) -> NaiveDate {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let days = ((hours / HOURS_PER_DAY).ceil().max(1.0) as u32).saturating_sub(1);
    super::add_workdays(from, days)
}

/// Jam dari waktu (tampilan).
#[must_use]
pub fn hour_of(at: DateTime<Utc>) -> u32 {
    (at + Duration::hours(7)).hour()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    #[test]
    fn work_hours_count_office_hours_on_weekdays() {
        // Jumat 13 Nov 2026 10.00 WIB → Senin 16 Nov 11.00 WIB = 7 + 2 jam.
        let h = work_hours(t("2026-11-13T03:00:00Z"), t("2026-11-16T04:00:00Z"));
        assert!((h - 9.0).abs() < 1e-9, "{h}");
        assert!(work_hours(t("2026-11-14T03:00:00Z"), t("2026-11-15T09:00:00Z")).abs() < 1e-9);
    }

    #[test]
    fn validation_bounds_points_and_hours() {
        let ok = validate(&EstimateInput {
            points: Some(5),
            stages: vec![StageHours {
                stage: "dev".into(),
                ai_hours: 2.25,
                human_hours: 1.0,
            }],
            basis: "  mirip KSR-2 ".into(),
        })
        .unwrap();
        assert_eq!(ok.stages[0].ai_hours, 2.3);
        assert_eq!(ok.basis, "mirip KSR-2");
        assert!(
            validate(&EstimateInput {
                points: Some(-1),
                ..Default::default()
            })
            .is_err()
        );
        assert!(
            validate(&EstimateInput {
                stages: vec![StageHours {
                    stage: "dev".into(),
                    ai_hours: f64::NAN,
                    human_hours: 0.0
                }],
                ..Default::default()
            })
            .is_err()
        );
    }

    #[test]
    fn finish_date_uses_working_days() {
        let fri = NaiveDate::from_ymd_opt(2026, 11, 13).unwrap();
        assert_eq!(finish_date(fri, 4.0), fri);
        assert_eq!(finish_date(fri, 9.0), NaiveDate::from_ymd_opt(2026, 11, 16).unwrap());
    }

    #[test]
    fn median_handles_even_and_odd() {
        assert!((median(vec![3.0, 1.0, 2.0]) - 2.0).abs() < 1e-9);
        assert!((median(vec![1.0, 2.0]) - 1.5).abs() < 1e-9);
    }
}
