//! PR tiket: dibuka atas permintaan runner (branch sudah di-push), diperbarui dari provider
//! (webhook), ditautkan ke tiket lewat kode di nama branch/judul, dan di-merge otomatis saat
//! stage Code Review di-approve (mode merge `auto_on_review`).

use agentgate_core::{
    git::{CiState, MergeMode, PrState, summarize, ticket_code_in},
    runner::protocol::{LogStream, PullRequestInfo, PullRequestOpen},
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, DbErr, EntityTrait,
    IntoActiveModel, QueryFilter,
};
use uuid::Uuid;

use super::{NOT_CONNECTED, invalid, provider::PullInfo, provider_code, provider_for};
use crate::{
    common::settings::Settings,
    models::{
        git_connections, jobs,
        notifications::NotificationKind,
        projects,
        pull_requests::{self, Check},
        repos, tickets,
        workspaces::WorkspaceId,
    },
    services::{
        audit::{self, AuditAction, AuditEntry, AuditSubject},
        jobs::{log, notify_users},
        tickets::TicketFailure,
    },
};

pub const TITLE_MAX: usize = 200;
pub const BODY_MAX: usize = 20_000;

fn now() -> chrono::DateTime<chrono::FixedOffset> {
    chrono::Utc::now().into()
}

/// Tiket yang dirujuk branch/judul PR (kode sekarang atau alias), di project repo.
async fn ticket_for(
    db: &DatabaseConnection,
    wid: WorkspaceId,
    repo: &repos::Model,
    info: &PullInfo,
) -> Result<Option<Uuid>, DbErr> {
    for text in [&info.head_branch, &info.title] {
        if let Some(code) = ticket_code_in(text)
            && let Some((t, _)) = tickets::Model::find_by_code(db, wid, &code).await?
            && t.project_id == repo.project_id
        {
            return Ok(Some(t.id));
        }
    }
    Ok(None)
}

/// Simpan keadaan PR dari provider. PR baru ditautkan ke `ticket` (bila diberikan) atau ke
/// tiket dari kode di branch/judul. Head sha berubah → check lama dibuang (CI baru).
///
/// # Errors
///
/// Error database.
pub async fn upsert(
    db: &DatabaseConnection,
    wid: WorkspaceId,
    repo: &repos::Model,
    info: &PullInfo,
    ticket: Option<Uuid>,
    job: Option<Uuid>,
) -> Result<(pull_requests::Model, bool), DbErr> {
    let existing = pull_requests::Model::by_number(db, repo.id, info.number).await?;
    let ticket = match (ticket, existing.as_ref().and_then(|p| p.ticket_id)) {
        (Some(t), _) | (None, Some(t)) => Some(t),
        (None, None) => ticket_for(db, wid, repo, info).await?,
    };
    let title: String = info.title.chars().take(TITLE_MAX).collect();
    let merged_at = (info.state == PrState::Merged).then(now);
    let closed_at = matches!(info.state, PrState::Closed | PrState::Merged).then(now);
    match existing {
        Some(pr) => {
            let sha_changed = !info.head_sha.is_empty() && pr.head_sha != info.head_sha;
            let mut m = pr.clone().into_active_model();
            m.title = Set(title);
            m.url = Set(info.url.clone());
            m.head_branch = Set(info.head_branch.clone());
            m.base_branch = Set(info.base_branch.clone());
            m.state = Set(info.state.as_str().into());
            m.ticket_id = Set(ticket);
            if info.additions != 0 || info.deletions != 0 {
                m.additions = Set(info.additions);
                m.deletions = Set(info.deletions);
            }
            if !info.author.is_empty() {
                m.author = Set(info.author.clone());
            }
            if sha_changed {
                m.head_sha = Set(info.head_sha.clone());
                m.checks = Set(serde_json::json!([]));
                m.ci_state = Set(CiState::None.as_str().into());
            }
            if pr.merged_at.is_none() {
                m.merged_at = Set(merged_at);
            }
            if pr.closed_at.is_none() || info.state == PrState::Open {
                m.closed_at = Set(closed_at);
            }
            if job.is_some() {
                m.job_id = Set(job);
            }
            Ok((m.update(db).await?, false))
        }
        None => {
            let m = pull_requests::ActiveModel {
                workspace_id: Set(wid.get()),
                project_id: Set(repo.project_id),
                repo_id: Set(repo.id),
                ticket_id: Set(ticket),
                number: Set(info.number),
                title: Set(title),
                url: Set(info.url.clone()),
                head_branch: Set(info.head_branch.clone()),
                base_branch: Set(info.base_branch.clone()),
                head_sha: Set(info.head_sha.clone()),
                state: Set(info.state.as_str().into()),
                ci_state: Set(CiState::None.as_str().into()),
                checks: Set(serde_json::json!([])),
                additions: Set(info.additions),
                deletions: Set(info.deletions),
                author: Set(info.author.clone()),
                job_id: Set(job),
                merged_at: Set(merged_at),
                closed_at: Set(closed_at),
                ..Default::default()
            };
            Ok((m.insert(db).await?, true))
        }
    }
}

/// Catat satu check CI untuk head `sha` di repo; ringkasan CI PR dihitung ulang. Mengembalikan
/// PR yang berubah.
///
/// # Errors
///
/// Error database.
pub async fn apply_check(
    db: &DatabaseConnection,
    repo: &repos::Model,
    sha: &str,
    check: Check,
) -> Result<Vec<pull_requests::Model>, DbErr> {
    if sha.is_empty() {
        return Ok(Vec::new());
    }
    let prs = pull_requests::Entity::find()
        .filter(pull_requests::Column::RepoId.eq(repo.id))
        .filter(pull_requests::Column::HeadSha.eq(sha))
        .all(db)
        .await?;
    let mut out = Vec::new();
    for pr in prs {
        let mut checks = pr.checks();
        match checks.iter_mut().find(|c| c.name == check.name) {
            Some(c) => *c = check.clone(),
            None => checks.push(check.clone()),
        }
        checks.truncate(100);
        let states: Vec<CiState> = checks.iter().map(|c| c.state).collect();
        let mut m = pr.into_active_model();
        m.ci_state = Set(summarize(&states).as_str().into());
        m.checks = Set(serde_json::to_value(&checks).unwrap_or_default());
        out.push(m.update(db).await?);
    }
    Ok(out)
}

/// Runner meminta PR untuk branch tiket yang sudah di-push: PR terbuka untuk branch itu
/// dipakai, selain itu dibuat. PR ditautkan ke tiket job.
///
/// # Errors
///
/// `NotFound` (repo bukan milik project job), `git-not-connected`, kode provider, error database.
pub async fn open_for_job(
    db: &DatabaseConnection,
    settings: &Settings,
    wid: WorkspaceId,
    job: &jobs::Model,
    req: &PullRequestOpen,
) -> Result<PullRequestInfo, TicketFailure> {
    let repo = repos::Model::of_project(db, wid, job.project_id)
        .await?
        .into_iter()
        .find(|r| r.name.eq_ignore_ascii_case(req.repo.trim()))
        .ok_or(TicketFailure::NotFound)?;
    let conn = connection_of(db, wid, &repo)
        .await?
        .ok_or_else(|| invalid("repo", NOT_CONNECTED))?;
    let provider = provider_for(&conn, settings)?;
    let branch = req.branch.trim();
    let base = if req.base.trim().is_empty() {
        repo.default_branch.clone()
    } else {
        req.base.trim().to_string()
    };
    let existing = provider
        .find_open_pull(&repo.full_name, branch)
        .await
        .map_err(|e| invalid("repo", provider_code(&e)))?;
    let (info, created) = match existing {
        Some(p) => (p, false),
        None => (
            provider
                .create_pull(
                    &repo.full_name,
                    &super::provider::NewPull {
                        head: branch.to_string(),
                        base,
                        title: req.title.trim().chars().take(TITLE_MAX).collect(),
                        body: req.body.chars().take(BODY_MAX).collect(),
                    },
                )
                .await
                .map_err(|e| invalid("repo", provider_code(&e)))?,
            true,
        ),
    };
    let (pr, _) = upsert(db, wid, &repo, &info, Some(job.ticket_id), Some(job.id)).await?;
    log(
        db,
        wid,
        job.id,
        LogStream::Ok,
        &format!(
            "PR #{} {} di {}: {}",
            pr.number,
            if created { "dibuka" } else { "sudah ada" },
            repo.name,
            pr.url
        ),
    )
    .await?;
    if created {
        notify_ticket(
            db,
            wid,
            job.claimed_by,
            job.ticket_id,
            NotificationKind::PrOpened,
        )
        .await?;
    }
    Ok(PullRequestInfo {
        number: pr.number,
        state: pr.state(),
        url: pr.url,
        created,
    })
}

/// Koneksi provider repo (bila tertaut dan koneksinya masih ada).
///
/// # Errors
///
/// Error database.
pub async fn connection_of(
    db: &DatabaseConnection,
    wid: WorkspaceId,
    repo: &repos::Model,
) -> Result<Option<git_connections::Model>, DbErr> {
    match repo.connection_id {
        Some(id) if !repo.full_name.is_empty() => {
            git_connections::Model::find_in(db, wid, id).await
        }
        _ => Ok(None),
    }
}

/// Beri tahu assignee + watcher tiket.
pub(crate) async fn notify_ticket(
    db: &DatabaseConnection,
    wid: WorkspaceId,
    actor: Option<Uuid>,
    ticket_id: Uuid,
    kind: NotificationKind,
) -> Result<(), DbErr> {
    let Some(ticket) = tickets::Model::find_in(db, wid, ticket_id).await? else {
        return Ok(());
    };
    let watchers = crate::models::ticket_watchers::Entity::find()
        .filter(crate::models::ticket_watchers::Column::TicketId.eq(ticket.id))
        .all(db)
        .await?
        .into_iter()
        .map(|w| w.user_id);
    let mut users: Vec<Uuid> = ticket.assignee_id.into_iter().chain(watchers).collect();
    users.sort_unstable();
    users.dedup();
    notify_users(db, wid, actor, users, kind, ticket.id).await
}

/// Mode merge otomatis: setelah stage ber-persona Code Reviewer di-approve, PR terbuka tiket
/// yang CI-nya hijau di-merge lewat provider. Gagal → notifikasi `merge_failed`, tidak
/// membatalkan approve. Mengembalikan jumlah PR yang di-merge.
///
/// # Errors
///
/// Error database.
pub async fn auto_merge(
    db: &DatabaseConnection,
    settings: &Settings,
    wid: WorkspaceId,
    actor: Uuid,
    ticket: &tickets::Model,
    request_id: Option<String>,
) -> Result<usize, DbErr> {
    let Some(project) = projects::Model::find_in(db, wid, ticket.project_id).await? else {
        return Ok(0);
    };
    if project.git_settings().merge_mode != MergeMode::AutoOnReview {
        return Ok(0);
    }
    let mut merged = 0;
    for pr in pull_requests::Model::of_ticket(db, wid, ticket.id).await? {
        if !matches!(pr.state(), PrState::Open) {
            continue;
        }
        let Some(repo) = repos::Entity::find_by_id(pr.repo_id).one(db).await? else {
            continue;
        };
        let result = if pr.ci() == CiState::Success {
            match connection_of(db, wid, &repo).await? {
                Some(conn) => match provider_for(&conn, settings) {
                    Ok(p) => p
                        .merge_pull(&repo.full_name, pr.number)
                        .await
                        .map_err(|e| provider_code(&e).to_string()),
                    Err(_) => Err(super::TOKEN_INVALID.to_string()),
                },
                None => Err(NOT_CONNECTED.to_string()),
            }
        } else {
            Err("ci-not-green".to_string())
        };
        match result {
            Ok(()) => {
                merged += 1;
                let mut m = pr.clone().into_active_model();
                m.state = Set(PrState::Merged.as_str().into());
                m.merged_at = Set(Some(now()));
                m.closed_at = Set(Some(now()));
                let pr = m.update(db).await?;
                audit::record(
                    db,
                    AuditEntry {
                        workspace_id: wid,
                        actor_id: Some(actor),
                        action: AuditAction::PullRequestMerged,
                        subject: (AuditSubject::PullRequest, pr.id),
                        changes: serde_json::json!({
                            "repo": repo.full_name, "number": pr.number, "ticket": ticket.code,
                        }),
                        request_id: request_id.clone(),
                    },
                )
                .await?;
            }
            Err(code) => {
                tracing::warn!(pr = pr.number, code, "auto merge failed");
                notify_ticket(db, wid, None, ticket.id, NotificationKind::MergeFailed).await?;
            }
        }
    }
    Ok(merged)
}

/// Persona Code Reviewer bawaan: approve stage-nya memicu merge otomatis.
pub const REVIEWER_PERSONA: &str = "reviewer";

/// Dipanggil setelah approve berhasil. `before` = tiket sebelum approve (stage yang
/// di-approve). Merge otomatis hanya untuk stage ber-persona Code Reviewer.
///
/// # Errors
///
/// Error database.
pub async fn after_approve(
    db: &DatabaseConnection,
    settings: &Settings,
    wid: WorkspaceId,
    actor: Uuid,
    before: &tickets::Model,
    request_id: Option<String>,
) -> Result<usize, DbErr> {
    let flow = crate::services::tickets::stages::Flow::load(db, wid, before).await?;
    let reviewer = flow
        .rule(&before.stage_key)
        .and_then(|s| s.persona.as_deref())
        .is_some_and(|p| p == REVIEWER_PERSONA);
    if !reviewer {
        return Ok(0);
    }
    auto_merge(db, settings, wid, actor, before, request_id).await
}

/// Branch PR sinkronisasi aturan (sama dengan CLI `agentgate rules sync`).
pub const SYNC_BRANCH: &str = "agentgate/sync-rules";
pub const SYNC_TITLE: &str = "Update aturan Agent Gate";

/// `agentgate rules sync` sudah mem-push [`SYNC_BRANCH`]: buka PR "Update aturan Agent Gate"
/// ke branch default repo, atau kembalikan PR yang sudah terbuka. Butuh `EditProject`.
///
/// # Errors
///
/// `NotFound` (project/repo/izin), `git-not-connected`, kode provider, error database.
pub async fn open_rules_pr(
    db: &DatabaseConnection,
    settings: &Settings,
    acting: &crate::services::workspaces::Acting<'_>,
    project_key: &str,
    repo_name: &str,
) -> Result<PullRequestInfo, TicketFailure> {
    let wid = acting.workspace_id();
    let project = projects::Model::accessible_for(db, &acting.accessor())
        .await?
        .into_iter()
        .find(|p| p.key.eq_ignore_ascii_case(project_key.trim()))
        .ok_or(TicketFailure::NotFound)?;
    crate::services::tickets::access_in(
        db,
        acting,
        project.id,
        crate::models::permission::Permission::EditProject,
    )
    .await?;
    let repo = repos::Model::of_project(db, wid, project.id)
        .await?
        .into_iter()
        .find(|r| r.name.eq_ignore_ascii_case(repo_name.trim()))
        .ok_or(TicketFailure::NotFound)?;
    let conn = connection_of(db, wid, &repo)
        .await?
        .ok_or_else(|| invalid("repo", NOT_CONNECTED))?;
    let provider = provider_for(&conn, settings)?;
    if let Some(p) = provider
        .find_open_pull(&repo.full_name, SYNC_BRANCH)
        .await
        .map_err(|e| invalid("repo", provider_code(&e)))?
    {
        let (pr, _) = upsert(db, wid, &repo, &p, None, None).await?;
        return Ok(PullRequestInfo {
            number: pr.number,
            state: pr.state(),
            url: pr.url,
            created: false,
        });
    }
    let body = format!(
        "File pembuka atau hooks Agent Gate berubah di server. PR ini memperbarui file aturan di \
         repo `{}` agar sama dengan versi server.\n\nJob tetap memakai aturan terbaru dari \
         server walau PR ini belum di-merge.\n\nDibuat oleh `agentgate rules sync` ({}).",
        repo.name, acting.user.email
    );
    let info = provider
        .create_pull(
            &repo.full_name,
            &super::provider::NewPull {
                head: SYNC_BRANCH.into(),
                base: repo.default_branch.clone(),
                title: SYNC_TITLE.into(),
                body,
            },
        )
        .await
        .map_err(|e| invalid("repo", provider_code(&e)))?;
    let (pr, _) = upsert(db, wid, &repo, &info, None, None).await?;
    Ok(PullRequestInfo {
        number: pr.number,
        state: pr.state(),
        url: pr.url,
        created: true,
    })
}
