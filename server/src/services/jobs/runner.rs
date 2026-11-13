//! Sisi runner (API `/api/runner/…`, token bearer device flow/OAuth): daftar runner, heartbeat,
//! event WebSocket, mulai job (→ `JobSpec`), log, langkah, izin aksi berbahaya, evidence
//! bertanda tangan, selesai, dan laporan drift.
//!
//! Runner hanya boleh menyentuh job yang dikunci untuk pemiliknya (`claimed_by`) dan dijalankan
//! di runner itu (`runner_id`); selain itu `NotFound`.

use std::collections::HashSet;

use agentgate_core::{
    persona::{self, Level},
    runner::{
        branch_name,
        protocol::{
            DangerRule, DriftReport, FinishReport, JobKind, JobMode, JobSpec, JobStatus,
            JobSummary, LogBatch, LogStream, Outcome, PermissionAsk, PermissionInfo,
            PermissionStatus, RegisterRunner, RuleFile, ServerMsg, StepsUpdate,
        },
        rules, signing,
    },
};
use chrono::Duration;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr,
    EntityTrait, IntoActiveModel, PaginatorTrait, QueryFilter, QueryOrder, TransactionTrait,
    sea_query::Expr,
};
use uuid::Uuid;

use super::{CLOSED, NOT_HOLDER, QueueRow, claim, find, invalid, log, notify_users, now, queue};
use crate::{
    models::{
        job_permissions, jobs,
        notifications::NotificationKind,
        personas, projects, repos, rule_checks,
        runner_prefs::{self, QuotaPolicy},
        runners, tickets,
        tickets::StageState,
        workspaces::WorkspaceId,
    },
    services::{
        audit::{self, AuditAction, AuditEntry, AuditSubject},
        mcp::{Session, prompts},
        tickets::{TicketFailure, stages},
    },
};

pub const LOG_LINE_MAX: usize = 4_000;
pub const LOG_BATCH_MAX: usize = 500;
pub const STEPS_MAX: usize = 40;
pub const NAME_MAX: usize = 80;
pub const SUMMARY_MAX: usize = 20_000;
pub const PERMISSION_TEXT_MAX: usize = 1_000;
pub const NAME_INVALID: &str = "runner-name-invalid";
pub const KEY_INVALID: &str = "runner-key-invalid";
pub const BATCH_TOO_LARGE: &str = "runner-batch-too-large";
pub const ERROR_CODES: &[&str] = &[NAME_INVALID, KEY_INVALID, BATCH_TOO_LARGE];

fn clip(s: &str, max: usize) -> String {
    s.trim().chars().take(max).collect()
}

/// Daftarkan (atau perbarui) runner perangkat milik pemegang token.
///
/// # Errors
///
/// `runner-name-invalid`, `runner-key-invalid`, error database.
pub async fn register(
    db: &DatabaseConnection,
    session: &Session,
    grant_id: Option<Uuid>,
    req: &RegisterRunner,
) -> Result<runners::Model, TicketFailure> {
    let name = clip(&req.name, NAME_MAX);
    if name.is_empty() {
        return Err(TicketFailure::field("name", NAME_INVALID));
    }
    if !signing::public_key_valid(&req.public_key) {
        return Err(TicketFailure::field("public_key", KEY_INVALID));
    }
    let client = persona::client(req.client.trim()).map_or("claude", |c| c.key);
    let wid = session.workspace.workspace_id();
    let txn = db.begin().await?;
    let existing = match req.runner_id {
        Some(id) => {
            runners::Entity::find_by_id(id)
                .filter(runners::Column::WorkspaceId.eq(wid.get()))
                .filter(runners::Column::UserId.eq(session.user.id))
                .filter(runners::Column::RevokedAt.is_null())
                .one(&txn)
                .await?
        }
        None => None,
    };
    let fresh = existing.is_none();
    let mut m = existing.map_or_else(
        || runners::ActiveModel {
            workspace_id: Set(wid.get()),
            user_id: Set(session.user.id),
            ..Default::default()
        },
        IntoActiveModel::into_active_model,
    );
    m.grant_id = Set(grant_id);
    m.name = Set(name.clone());
    m.os = Set(clip(&req.os, NAME_MAX));
    m.client = Set(client.to_string());
    m.version = Set(clip(&req.version, 40));
    m.public_key = Set(req.public_key.trim().to_string());
    m.last_seen_at = Set(Some(now()));
    let runner = m.save(&txn).await?;
    let runner = runners::Entity::find_by_id(
        runner
            .id
            .clone()
            .take()
            .ok_or_else(|| DbErr::RecordNotFound("runner".into()))?,
    )
    .one(&txn)
    .await?
    .ok_or_else(|| DbErr::RecordNotFound("runner".into()))?;
    if fresh {
        audit::record(
            &txn,
            AuditEntry {
                workspace_id: wid,
                actor_id: Some(session.user.id),
                action: AuditAction::RunnerRegistered,
                subject: (AuditSubject::Runner, runner.id),
                changes: serde_json::json!({ "name": name, "os": runner.os, "client": client }),
                request_id: None,
            },
        )
        .await?;
    }
    txn.commit().await?;
    Ok(runner)
}

/// Runner aktif milik pemegang token.
///
/// # Errors
///
/// `NotFound`, error database.
pub async fn own(
    db: &DatabaseConnection,
    session: &Session,
    runner_id: Uuid,
) -> Result<runners::Model, TicketFailure> {
    runners::Entity::find_by_id(runner_id)
        .filter(runners::Column::WorkspaceId.eq(session.workspace.id))
        .filter(runners::Column::UserId.eq(session.user.id))
        .filter(runners::Column::RevokedAt.is_null())
        .one(db)
        .await?
        .ok_or(TicketFailure::NotFound)
}

/// Runner masih hidup.
///
/// # Errors
///
/// Error database.
pub async fn heartbeat(
    db: &DatabaseConnection,
    runner: &runners::Model,
    connected: bool,
) -> Result<(), DbErr> {
    let mut q = runners::Entity::update_many()
        .col_expr(runners::Column::LastSeenAt, Expr::value(now()))
        .filter(runners::Column::Id.eq(runner.id));
    if connected {
        q = q.col_expr(runners::Column::ConnectedAt, Expr::value(now()));
    }
    q.exec(db).await?;
    Ok(())
}

/// Ringkasan job untuk protokol.
fn summary(row: &QueueRow, user: Uuid, stage_name: String) -> JobSummary {
    JobSummary {
        id: row.job.id,
        kind: row.job.kind(),
        mode: row.job.mode(),
        status: row.job.status(),
        ticket: row.ticket.code.clone(),
        title: row.ticket.title.clone(),
        stage: row.job.stage_key.clone(),
        stage_name,
        persona: row.job.persona.clone(),
        priority: i16::from(row.priority),
        mine: row.job.claimed_by == Some(user),
        can_claim: row.can_claim,
        created_at: row.job.created_at.to_rfc3339(),
    }
}

async fn stage_name<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    ticket: &tickets::Model,
    key: &str,
) -> String {
    match stages::Flow::load(db, wid, ticket).await {
        Ok(flow) => flow
            .rule(key)
            .or_else(|| flow.current.stage(key))
            .map_or_else(|| key.to_string(), |s| s.name.clone()),
        Err(_) => key.to_string(),
    }
}

/// Antrean job untuk CLI (`agentgate jobs`).
///
/// # Errors
///
/// Error database.
pub async fn list(db: &DatabaseConnection, session: &Session) -> Result<Vec<JobSummary>, DbErr> {
    let acting = session.acting();
    let wid = acting.workspace_id();
    let mut out = Vec::new();
    for row in queue(db, &acting).await? {
        let name = stage_name(db, wid, &row.ticket, &row.job.stage_key).await;
        out.push(summary(&row, session.user.id, name));
    }
    Ok(out)
}

/// Event yang sudah dikirim ke satu koneksi WebSocket (tidak dikirim ulang).
#[derive(Debug, Default)]
pub struct EventState {
    ready: HashSet<Uuid>,
    cancelled: HashSet<Uuid>,
    permissions: HashSet<Uuid>,
}

/// Event baru untuk `runner`: job siap dijalankan (termasuk auto-ambil), job dibatalkan, izin
/// dijawab.
///
/// # Errors
///
/// Error database.
pub async fn events(
    db: &DatabaseConnection,
    session: &Session,
    runner: &runners::Model,
    state: &mut EventState,
) -> Result<Vec<ServerMsg>, DbErr> {
    let acting = session.acting();
    let wid = acting.workspace_id();
    let mut out = Vec::new();
    auto_claim(db, session, runner).await?;
    let ready = jobs::Entity::find()
        .filter(jobs::Column::WorkspaceId.eq(wid.get()))
        .filter(jobs::Column::ClaimedBy.eq(session.user.id))
        .filter(jobs::Column::Status.eq(JobStatus::Claimed.as_str()))
        .filter(
            sea_orm::Condition::any()
                .add(jobs::Column::RunnerId.is_null())
                .add(jobs::Column::RunnerId.eq(runner.id)),
        )
        .order_by_asc(jobs::Column::ClaimedAt)
        .all(db)
        .await?;
    for job in ready {
        if !state.ready.insert(job.id) {
            continue;
        }
        let Some(ticket) = tickets::Model::find_in(db, wid, job.ticket_id).await? else {
            continue;
        };
        let name = stage_name(db, wid, &ticket, &job.stage_key).await;
        let priority = ticket.priority().map_or(3, tickets::TicketPriority::rank);
        let project = projects::Model::find_in(db, wid, job.project_id).await?;
        let Some(project) = project else { continue };
        let row = QueueRow {
            job,
            ticket,
            project,
            priority,
            can_claim: false,
        };
        out.push(ServerMsg::JobReady {
            job: summary(&row, session.user.id, name),
        });
    }
    let recent = now() - Duration::minutes(10);
    let cancelled = jobs::Entity::find()
        .filter(jobs::Column::RunnerId.eq(runner.id))
        .filter(jobs::Column::Status.eq(JobStatus::Cancelled.as_str()))
        .filter(jobs::Column::FinishedAt.gt(recent))
        .all(db)
        .await?;
    for job in cancelled {
        if state.cancelled.insert(job.id) {
            out.push(ServerMsg::JobCancelled { job_id: job.id });
        }
    }
    let on_runner: Vec<Uuid> = jobs::Entity::find()
        .filter(jobs::Column::RunnerId.eq(runner.id))
        .filter(jobs::Column::Status.is_in(jobs::OPEN.iter().copied()))
        .all(db)
        .await?
        .into_iter()
        .map(|j| j.id)
        .collect();
    if !on_runner.is_empty() {
        let decided = job_permissions::Entity::find()
            .filter(job_permissions::Column::JobId.is_in(on_runner))
            .filter(job_permissions::Column::Status.ne(PermissionStatus::Pending.as_str()))
            .filter(job_permissions::Column::DecidedAt.gt(recent))
            .all(db)
            .await?;
        for p in decided {
            if state.permissions.insert(p.id) {
                out.push(ServerMsg::PermissionDecided {
                    job_id: p.job_id,
                    permission_id: p.id,
                    status: p.status(),
                });
            }
        }
    }
    Ok(out)
}

/// Auto-ambil (pengaturan runner): selama runner idle (di bawah batas paralel), job antrean
/// yang boleh diambil pemilik runner langsung dikunci ke runner ini.
async fn auto_claim(
    db: &DatabaseConnection,
    session: &Session,
    runner: &runners::Model,
) -> Result<(), DbErr> {
    let acting = session.acting();
    let wid = acting.workspace_id();
    let prefs = runner_prefs::Model::of(db, wid, session.user.id).await?;
    if !prefs.auto_claim {
        return Ok(());
    }
    let busy = jobs::Entity::find()
        .filter(jobs::Column::RunnerId.eq(runner.id))
        .filter(jobs::Column::Status.is_in(["claimed", "running", "paused"]))
        .all(db)
        .await?
        .len();
    let limit = prefs
        .max_parallel
        .map_or(1, |m| usize::try_from(m).unwrap_or(1));
    if busy >= limit {
        return Ok(());
    }
    if let Some(row) = queue(db, &acting)
        .await?
        .into_iter()
        .find(|r| r.can_claim && r.job.status() == JobStatus::Queued)
    {
        match claim(db, &acting, row.job.id, Some(runner.id)).await {
            Ok(_) | Err(TicketFailure::Invalid(_) | TicketFailure::NotFound) => {}
            Err(TicketFailure::Db(e)) => return Err(e),
        }
    }
    Ok(())
}

/// Job yang dipegang runner ini (dikunci untuk pemiliknya dan berjalan di runner ini).
///
/// # Errors
///
/// `NotFound`, error database.
pub async fn held(
    db: &DatabaseConnection,
    session: &Session,
    runner: &runners::Model,
    job_id: Uuid,
) -> Result<jobs::Model, TicketFailure> {
    find(db, session.workspace.workspace_id(), job_id)
        .await?
        .filter(|j| j.claimed_by == Some(session.user.id) && j.runner_id == Some(runner.id))
        .ok_or(TicketFailure::NotFound)
}

fn job_prompt(
    spec_stage: &str,
    ticket: &tickets::Model,
    persona_name: &str,
    job: &jobs::Model,
) -> String {
    match job.kind() {
        JobKind::Comment => format!(
            "Anda adalah Agent {persona_name} di Agent Gate. Seseorang menyebut Anda di komentar tiket {code} ({title}).\n\n\
             Komentar:\n{note}\n\n\
             1. Panggil ag_context dengan ticket {code} untuk memuat konteks.\n\
             2. Kerjakan permintaan di komentar di worktree ini.\n\
             3. Tulis jawaban ringkas sebagai pesan terakhir Anda; runner membalasnya sebagai komentar.\n\
             Jangan memindah stage tiket.",
            code = ticket.code,
            title = ticket.title,
            note = job.prompt_note,
        ),
        JobKind::Stage => format!(
            "Kerjakan stage \"{spec_stage}\" untuk tiket {code} ({title}) sebagai persona {persona_name}. \
             Job ini berjalan otomatis di runner tanpa diawasi manusia.\n\n\
             1. Panggil ag_context dengan ticket {code} (include: [\"skills\"]) dan ikuti instruksi persona, prompt stage, dan skill-nya.\n\
             2. Baca artifact input stage lewat ag_artifact bila ada.\n\
             3. Kerjakan di worktree ini. Simpan screenshot bukti di .agentgate/evidence/.\n\
             4. Simpan hasil ke artifact output stage lewat ag_artifact action=save.\n\
             5. Bila butuh keputusan manusia, pakai ag_ask lalu berhenti.\n\
             6. Jangan memanggil ag_transition: setelah Anda selesai, runner menjalankan tes, mengirim evidence, dan mengajukan stage untuk direview.\n\n\
             Stage key: {key}.",
            code = ticket.code,
            title = ticket.title,
            key = job.stage_key,
        ),
    }
}

/// "Job paralel maksimal" pemilik runner: job baru tidak dimulai bila runner ini sudah
/// menjalankan (`running`/`paused`) sebanyak batasnya. Melanjutkan job yang sudah berjalan
/// selalu boleh.
async fn runner_has_room(
    db: &DatabaseConnection,
    session: &Session,
    runner: &runners::Model,
    job: &jobs::Model,
) -> Result<(), TicketFailure> {
    if job.status().is_active() && job.runner_id == Some(runner.id) {
        return Ok(());
    }
    let prefs =
        runner_prefs::Model::of(db, session.workspace.workspace_id(), session.user.id).await?;
    let Some(max) = prefs.max_parallel else {
        return Ok(());
    };
    let active = jobs::Entity::find()
        .filter(jobs::Column::RunnerId.eq(runner.id))
        .filter(
            jobs::Column::Status.is_in([JobStatus::Running.as_str(), JobStatus::Paused.as_str()]),
        )
        .filter(jobs::Column::Id.ne(job.id))
        .count(db)
        .await?;
    if active >= u64::try_from(max.max(1)).unwrap_or(1) {
        return Err(invalid(super::RUNNER_BUSY));
    }
    Ok(())
}

/// Mulai (atau lanjutkan) job di runner: kunci ke runner ini, status `running`, dan semua yang
/// dibutuhkan runner.
///
/// # Errors
///
/// `job-not-holder`, `job-closed`, `NotFound`, error database.
pub async fn start(
    db: &DatabaseConnection,
    session: &Session,
    runner: &runners::Model,
    job_id: Uuid,
    base_url: &str,
) -> Result<JobSpec, TicketFailure> {
    let wid = session.workspace.workspace_id();
    let job = find(db, wid, job_id)
        .await?
        .ok_or(TicketFailure::NotFound)?;
    if job.claimed_by != Some(session.user.id) {
        return Err(invalid(NOT_HOLDER));
    }
    if !matches!(
        job.status(),
        JobStatus::Claimed | JobStatus::Running | JobStatus::Paused
    ) {
        return Err(invalid(CLOSED));
    }
    runner_has_room(db, session, runner, &job).await?;
    let ticket = tickets::Model::find_in(db, wid, job.ticket_id)
        .await?
        .ok_or(TicketFailure::NotFound)?;
    let project = projects::Model::find_in(db, wid, job.project_id)
        .await?
        .ok_or(TicketFailure::NotFound)?;
    let persona = personas::Model::resolve(db, wid, &job.persona).await?;
    let client = runner.client.clone();
    let choice = persona.as_ref().and_then(|p| p.model_for(&client));
    let flow = stages::Flow::load(db, wid, &ticket).await?;
    let stage_name = flow
        .rule(&job.stage_key)
        .or_else(|| flow.current.stage(&job.stage_key))
        .map_or_else(|| job.stage_key.clone(), |s| s.name.clone());
    let branch = branch_name(&ticket.code, &ticket.title);
    let txn = db.begin().await?;
    let updated = jobs::Entity::update_many()
        .col_expr(jobs::Column::RunnerId, Expr::value(runner.id))
        .col_expr(
            jobs::Column::Status,
            Expr::value(JobStatus::Running.as_str()),
        )
        .col_expr(jobs::Column::Client, Expr::value(client.clone()))
        .col_expr(
            jobs::Column::Model,
            Expr::value(choice.as_ref().map(|c| c.model.clone()).unwrap_or_default()),
        )
        .col_expr(jobs::Column::Branch, Expr::value(branch.clone()))
        .col_expr(
            jobs::Column::StartedAt,
            Expr::value(job.started_at.unwrap_or_else(now)),
        )
        .col_expr(jobs::Column::UpdatedAt, Expr::value(now()))
        .filter(jobs::Column::Id.eq(job.id))
        .filter(jobs::Column::ClaimedBy.eq(session.user.id))
        .filter(
            sea_orm::Condition::any()
                .add(jobs::Column::RunnerId.is_null())
                .add(jobs::Column::RunnerId.eq(runner.id)),
        )
        .exec(&txn)
        .await?;
    if updated.rows_affected != 1 {
        return Err(invalid(NOT_HOLDER));
    }
    log(
        &txn,
        wid,
        job.id,
        LogStream::Info,
        &format!(
            "Job {} · stage {stage_name} · runner {} ({})",
            ticket.code, runner.name, session.user.name
        ),
    )
    .await?;
    txn.commit().await?;
    let allowed_actions = job_permissions::Entity::find()
        .filter(job_permissions::Column::TicketId.eq(ticket.id))
        .filter(job_permissions::Column::Status.eq(PermissionStatus::AllowedTicket.as_str()))
        .all(db)
        .await?
        .into_iter()
        .map(|p| p.action)
        .collect();
    let danger = persona.as_ref().map_or_else(
        || {
            persona::DANGER_ACTIONS
                .iter()
                .map(|(a, l)| (*a, *l))
                .collect::<Vec<_>>()
        },
        personas::Model::danger_levels,
    );
    let persona_name = persona
        .as_ref()
        .map_or_else(|| job.persona.clone(), |p| p.name.clone());
    Ok(JobSpec {
        id: job.id,
        kind: job.kind(),
        mode: job.mode(),
        ticket: ticket.code.clone(),
        title: ticket.title.clone(),
        url: format!("{base_url}/w/{}/t/{}", session.workspace.slug, ticket.code),
        project: project.key.clone(),
        repos: repos::Model::of_project(db, wid, project.id)
            .await?
            .into_iter()
            .map(|r| r.name)
            .collect(),
        stage: job.stage_key.clone(),
        stage_name: stage_name.clone(),
        persona: persona
            .as_ref()
            .map_or_else(|| job.persona.clone(), |p| p.key.clone()),
        persona_name: persona_name.clone(),
        model: choice.as_ref().map(|c| c.model.clone()).unwrap_or_default(),
        effort: choice.map(|c| c.effort).unwrap_or_default(),
        client,
        command: format!("/{}", prompts::stage_prompt_name(&job.stage_key)),
        prompt: job_prompt(&stage_name, &ticket, &persona_name, &job),
        branch,
        mcp_url: format!("{base_url}/mcp"),
        rules: rule_files(base_url, &project.key),
        danger: danger
            .into_iter()
            .map(|(action, level)| DangerRule {
                action: action.to_string(),
                level,
            })
            .chain(persona::ALWAYS_BLOCKED.iter().map(|a| DangerRule {
                action: (*a).to_string(),
                level: Level::Block,
            }))
            .collect(),
        allowed_actions,
    })
}

/// File aturan Agent Gate versi server untuk project `key`.
#[must_use]
pub fn rule_files(base_url: &str, key: &str) -> Vec<RuleFile> {
    rules::agent_gate_files(base_url, key)
}

/// Tambah log dari runner.
///
/// # Errors
///
/// `runner-batch-too-large`, error database.
pub async fn append_logs(
    db: &DatabaseConnection,
    session: &Session,
    job: &jobs::Model,
    batch: &LogBatch,
) -> Result<(), TicketFailure> {
    if batch.lines.len() > LOG_BATCH_MAX {
        return Err(TicketFailure::field("lines", BATCH_TOO_LARGE));
    }
    let wid = session.workspace.workspace_id();
    let txn = db.begin().await?;
    for line in &batch.lines {
        log(&txn, wid, job.id, line.stream, &line.text).await?;
    }
    txn.commit().await?;
    Ok(())
}

/// Ganti daftar langkah job.
///
/// # Errors
///
/// `runner-batch-too-large`, error database.
pub async fn set_steps(
    db: &DatabaseConnection,
    job: &jobs::Model,
    update: &StepsUpdate,
) -> Result<(), TicketFailure> {
    if update.steps.len() > STEPS_MAX {
        return Err(TicketFailure::field("steps", BATCH_TOO_LARGE));
    }
    let steps: Vec<_> = update
        .steps
        .iter()
        .map(|s| agentgate_core::runner::protocol::Step {
            name: clip(&s.name, 200),
            state: s.state,
            seconds: s.seconds,
        })
        .collect();
    let mut m = job.clone().into_active_model();
    m.steps = Set(serde_json::to_value(steps).unwrap_or_default());
    m.update(db).await?;
    Ok(())
}

/// Agent ingin menjalankan aksi berbahaya bertingkat `ask`: job dijeda, pemegang lock mendapat
/// notifikasi. Aksi yang sudah diizinkan untuk tiket ini langsung `allowed_ticket`.
///
/// # Errors
///
/// Error database.
pub async fn ask_permission(
    db: &DatabaseConnection,
    wid: WorkspaceId,
    job: &jobs::Model,
    ask: &PermissionAsk,
) -> Result<PermissionInfo, TicketFailure> {
    let action = clip(&ask.action, 60);
    if persona::ALWAYS_BLOCKED.contains(&action.as_str()) {
        return Err(TicketFailure::field("action", super::ACTION_BLOCKED));
    }
    let standing = job_permissions::Entity::find()
        .filter(job_permissions::Column::TicketId.eq(job.ticket_id))
        .filter(job_permissions::Column::Action.eq(action.clone()))
        .filter(job_permissions::Column::Status.eq(PermissionStatus::AllowedTicket.as_str()))
        .one(db)
        .await?;
    if let Some(p) = standing {
        return Ok(PermissionInfo {
            id: p.id,
            status: PermissionStatus::AllowedTicket,
        });
    }
    let txn = db.begin().await?;
    let perm = job_permissions::ActiveModel {
        workspace_id: Set(wid.get()),
        job_id: Set(job.id),
        ticket_id: Set(job.ticket_id),
        action: Set(action),
        command: Set(clip(&ask.command, PERMISSION_TEXT_MAX)),
        reason: Set(clip(&ask.reason, PERMISSION_TEXT_MAX)),
        status: Set(PermissionStatus::Pending.as_str().into()),
        ..Default::default()
    }
    .insert(&txn)
    .await?;
    let mut m = job.clone().into_active_model();
    m.status = Set(JobStatus::Paused.as_str().into());
    m.update(&txn).await?;
    log(
        &txn,
        wid,
        job.id,
        LogStream::Warn,
        &format!("⏸ izin {}", perm.command),
    )
    .await?;
    notify_users(
        &txn,
        wid,
        None,
        job.claimed_by,
        NotificationKind::JobPermission,
        job.ticket_id,
    )
    .await?;
    txn.commit().await?;
    Ok(PermissionInfo {
        id: perm.id,
        status: PermissionStatus::Pending,
    })
}

/// Status permintaan izin job ini.
///
/// # Errors
///
/// `NotFound`, error database.
pub async fn permission(
    db: &DatabaseConnection,
    job: &jobs::Model,
    id: Uuid,
) -> Result<PermissionInfo, TicketFailure> {
    let p = job_permissions::Entity::find_by_id(id)
        .filter(job_permissions::Column::JobId.eq(job.id))
        .one(db)
        .await?
        .ok_or(TicketFailure::NotFound)?;
    Ok(PermissionInfo {
        id: p.id,
        status: p.status(),
    })
}

/// Runner melaporkan hasil job. Sukses stage → stage diselesaikan sebagai executor (minta
/// review / maju bila gate terpenuhi); gagal gate dicatat di log. Sukses `@agent` → jawaban
/// dibalas sebagai komentar.
///
/// # Errors
///
/// `job-closed`, error database.
pub async fn finish(
    db: &DatabaseConnection,
    session: &Session,
    job: &jobs::Model,
    report: &FinishReport,
) -> Result<jobs::Model, TicketFailure> {
    let wid = session.workspace.workspace_id();
    if !job.status().is_open() {
        return Err(invalid(CLOSED));
    }
    let outcome = report.outcome.unwrap_or(Outcome::Failed);
    let summary = clip(&report.summary, SUMMARY_MAX);
    let mut m = job.clone().into_active_model();
    m.summary = Set(summary.clone());
    m.tokens = Set(report.tokens.and_then(|t| i64::try_from(t).ok()));
    match outcome {
        Outcome::Succeeded => {
            m.status = Set(JobStatus::Succeeded.as_str().into());
            m.finished_at = Set(Some(now()));
        }
        Outcome::Failed => {
            m.status = Set(JobStatus::Failed.as_str().into());
            m.finished_at = Set(Some(now()));
        }
        Outcome::Cancelled => {
            m.status = Set(JobStatus::Cancelled.as_str().into());
            m.finished_at = Set(Some(now()));
        }
        Outcome::Quota => {
            m.status = Set(JobStatus::Quota.as_str().into());
            m.quota_reset_at = Set(report
                .quota_reset_at
                .as_deref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()));
        }
    }
    let job = m.update(db).await?;
    let acting = session.acting();
    match outcome {
        Outcome::Succeeded => {
            log(db, wid, job.id, LogStream::Ok, "✓ Job selesai").await?;
            match job.kind() {
                JobKind::Stage => complete_stage(db, session, &job).await?,
                JobKind::Comment => {
                    if !summary.is_empty() {
                        let body = format!("**Agent {}**\n\n{summary}", job.persona);
                        if let Err(e) = crate::services::tickets::comments::post(
                            db,
                            &acting,
                            job.ticket_id,
                            &body,
                        )
                        .await
                        {
                            tracing::warn!(error = %e, "agent reply comment failed");
                        }
                    }
                }
            }
        }
        Outcome::Failed => {
            log(db, wid, job.id, LogStream::Fail, "✗ Job gagal").await?;
            notify_users(
                db,
                wid,
                None,
                job.claimed_by,
                NotificationKind::JobFailed,
                job.ticket_id,
            )
            .await?;
        }
        Outcome::Quota => {
            log(db, wid, job.id, LogStream::Warn, "Kuota AI habis").await?;
            notify_users(
                db,
                wid,
                None,
                job.claimed_by,
                NotificationKind::JobQuota,
                job.ticket_id,
            )
            .await?;
            let prefs = runner_prefs::Model::of(db, wid, session.user.id).await?;
            if prefs.quota_policy == QuotaPolicy::Release {
                return super::release(db, &acting, job.id, "Kuota AI habis").await;
            }
        }
        Outcome::Cancelled => {
            log(db, wid, job.id, LogStream::Fail, "Job dihentikan runner").await?;
        }
    }
    Ok(job)
}

/// Selesaikan stage tiket setelah job stage sukses, bila tiket masih di stage itu dan aktif.
async fn complete_stage(
    db: &DatabaseConnection,
    session: &Session,
    job: &jobs::Model,
) -> Result<(), TicketFailure> {
    let wid = session.workspace.workspace_id();
    let Some(ticket) = tickets::Model::find_in(db, wid, job.ticket_id).await? else {
        return Ok(());
    };
    if ticket.stage_key != job.stage_key || ticket.stage_state()? != StageState::Active {
        return Ok(());
    }
    let acting = session.acting();
    match stages::complete(db, &acting, ticket.id).await {
        Ok(t) => {
            let text = if t.stage_state()? == StageState::Review {
                "Stage diajukan untuk direview".to_string()
            } else {
                format!("Tiket pindah ke stage {}", t.stage_key)
            };
            log(db, wid, job.id, LogStream::Ok, &text).await?;
        }
        Err(TicketFailure::Invalid(errors)) => {
            let codes: Vec<String> = errors
                .errors
                .iter()
                .flat_map(|(f, es)| es.iter().map(move |e| format!("{f}: {}", e.code)))
                .collect();
            log(
                db,
                wid,
                job.id,
                LogStream::Warn,
                &format!("Stage belum bisa diselesaikan ({})", codes.join(", ")),
            )
            .await?;
        }
        Err(TicketFailure::NotFound) => {}
        Err(e @ TicketFailure::Db(_)) => return Err(e),
    }
    Ok(())
}

/// Simpan hasil cek drift aturan satu repo.
///
/// # Errors
///
/// Error database.
pub async fn report_drift(
    db: &DatabaseConnection,
    session: &Session,
    report: &DriftReport,
) -> Result<(), DbErr> {
    let wid = session.workspace.workspace_id();
    let repo = clip(&report.repo, 200);
    let existing = rule_checks::Entity::find()
        .filter(rule_checks::Column::WorkspaceId.eq(wid.get()))
        .filter(rule_checks::Column::Repo.eq(repo.clone()))
        .one(db)
        .await?;
    let mut m = existing.map_or_else(
        || rule_checks::ActiveModel {
            workspace_id: Set(wid.get()),
            repo: Set(repo),
            ..Default::default()
        },
        IntoActiveModel::into_active_model,
    );
    m.checksum = Set(clip(&report.checksum, 80));
    m.changed = Set(report
        .changed
        .iter()
        .take(20)
        .map(|c| clip(c, 200))
        .collect());
    m.checked_by = Set(Some(session.user.id));
    m.updated_at = Set(now());
    m.save(db).await?;
    Ok(())
}

/// Mode job untuk tampilan.
#[must_use]
pub fn is_interactive(job: &jobs::Model) -> bool {
    job.mode() == JobMode::Interactive
}

/// Terminal stage interaktif gagal dibuka: executor diberi tahu untuk menjalankan perintah
/// `/ag-…` sendiri.
///
/// # Errors
///
/// Error database.
pub async fn terminal_failed(
    db: &DatabaseConnection,
    session: &Session,
    job: &jobs::Model,
) -> Result<(), TicketFailure> {
    let wid = session.workspace.workspace_id();
    log(
        db,
        wid,
        job.id,
        LogStream::Warn,
        &format!(
            "Terminal tidak bisa dibuka. Jalankan /{} di client Anda.",
            prompts::stage_prompt_name(&job.stage_key)
        ),
    )
    .await?;
    notify_users(
        db,
        wid,
        None,
        job.claimed_by,
        NotificationKind::JobTerminal,
        job.ticket_id,
    )
    .await?;
    Ok(())
}
