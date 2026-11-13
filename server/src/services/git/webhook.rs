//! Webhook GitHub `POST /webhooks/github/{connection}`.
//!
//! Tanda tangan `X-Hub-Signature-256` (HMAC-SHA256 secret koneksi atas body mentah) diperiksa
//! constant-time **sebelum** body di-parse. Event yang dipakai: `ping`, `pull_request`,
//! `check_run`, `status`; lainnya diterima lalu diabaikan. Setelah PR/CI berubah, tiket yang
//! stage-nya kini memenuhi semua gate diselesaikan otomatis ([`advance_ready`]).

use agentgate_core::{
    git::{CiState, PrState},
    workflow::{Exit, Gate},
};
use hmac::{Hmac, Mac};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, DbErr, EntityTrait,
    IntoActiveModel, QueryFilter,
};
use sha2::Sha256;
use uuid::Uuid;

use super::{key, provider::github_pull_from_json, pulls};
use crate::{
    common::{crypto, settings::Settings},
    models::{
        git_connections, jobs,
        notifications::NotificationKind,
        pull_requests::{self, Check},
        repos, tickets,
        tickets::StageState,
        users, workspace_members, workspaces,
    },
    services::{
        tickets::{gates, stages},
        workspaces::Acting,
    },
};

/// Batas body webhook.
pub const BODY_MAX: usize = 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum WebhookError {
    #[error("unknown connection")]
    UnknownConnection,
    #[error("bad signature")]
    BadSignature,
    #[error("bad payload")]
    BadPayload,
    #[error(transparent)]
    Db(#[from] DbErr),
}

/// `X-Hub-Signature-256: sha256=<hex>` cocok dengan HMAC-SHA256 `secret` atas `body`.
#[must_use]
pub fn verify(secret: &str, body: &[u8], header: Option<&str>) -> bool {
    let Some(hex_sig) = header.and_then(|h| h.trim().strip_prefix("sha256=")) else {
        return false;
    };
    let Ok(sig) = hex::decode(hex_sig) else {
        return false;
    };
    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(body);
    crypto::ct_eq(&mac.finalize().into_bytes(), &sig)
}

/// Tanda tangan untuk tes/fake provider.
#[must_use]
pub fn sign(secret: &str, body: &[u8]) -> String {
    // HMAC menerima kunci sepanjang apa pun; cabang `Err` tidak terjadi.
    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(secret.as_bytes()) else {
        return String::new();
    };
    mac.update(body);
    format!("sha256={}", hex::encode(mac.finalize().into_bytes()))
}

/// Hasil penanganan: event dipakai atau diabaikan.
#[derive(Debug, PartialEq, Eq)]
pub enum Handled {
    /// Dipakai; `reindex` = repo yang code graph-nya perlu di-index ulang (PR di-merge ke
    /// branch utama, M7), di-enqueue controller.
    Applied {
        reindex: Vec<repos::Model>,
    },
    Ignored,
}

fn now() -> chrono::DateTime<chrono::FixedOffset> {
    chrono::Utc::now().into()
}

fn conclusion(status: &str, conclusion: Option<&str>) -> CiState {
    if status != "completed" {
        return CiState::Pending;
    }
    match conclusion.unwrap_or_default() {
        "success" | "neutral" | "skipped" => CiState::Success,
        _ => CiState::Failure,
    }
}

fn commit_status(state: &str) -> CiState {
    match state {
        "success" => CiState::Success,
        "pending" => CiState::Pending,
        _ => CiState::Failure,
    }
}

/// Tangani satu delivery.
///
/// # Errors
///
/// Koneksi tidak dikenal, tanda tangan salah, payload rusak, error database.
pub async fn handle(
    db: &DatabaseConnection,
    settings: &Settings,
    connection_id: Uuid,
    event: &str,
    signature: Option<&str>,
    body: &[u8],
) -> Result<Handled, WebhookError> {
    let conn = git_connections::Entity::find_by_id(connection_id)
        .one(db)
        .await?
        .ok_or(WebhookError::UnknownConnection)?;
    let secret = key(settings)
        .ok()
        .and_then(|k| conn.webhook_secret(&k).ok())
        .ok_or(WebhookError::BadSignature)?;
    if !verify(&secret, body, signature) {
        return Err(WebhookError::BadSignature);
    }
    let payload: serde_json::Value =
        serde_json::from_slice(body).map_err(|_| WebhookError::BadPayload)?;
    let mut m = conn.clone().into_active_model();
    m.last_event_at = Set(Some(now()));
    m.update(db).await?;
    let full_name = payload["repository"]["full_name"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let repos = repos::Entity::find()
        .filter(repos::Column::ConnectionId.eq(conn.id))
        .all(db)
        .await?
        .into_iter()
        .filter(|r| !full_name.is_empty() && r.full_name.eq_ignore_ascii_case(&full_name))
        .collect::<Vec<_>>();
    for r in &repos {
        let mut m = r.clone().into_active_model();
        m.last_event_at = Set(Some(now()));
        m.update(db).await?;
    }
    let wid = workspaces::Model::find_by_id(db, conn.workspace_id)
        .await?
        .ok_or(WebhookError::UnknownConnection)?
        .workspace_id();
    let mut touched: Vec<Uuid> = Vec::new();
    let mut reindex: Vec<repos::Model> = Vec::new();
    match event {
        "ping" => {
            return Ok(Handled::Applied {
                reindex: Vec::new(),
            });
        }
        "pull_request" => {
            let info = github_pull_from_json(&payload["pull_request"])
                .map_err(|_| WebhookError::BadPayload)?;
            for repo in &repos {
                let before = pull_requests::Model::by_number(db, repo.id, info.number).await?;
                let (pr, _) = pulls::upsert(db, wid, repo, &info, None, None).await?;
                let was_merged = before.is_some_and(|b| b.state() == PrState::Merged);
                if pr.state() == PrState::Merged
                    && !was_merged
                    && info.base_branch == repo.default_branch
                    && crate::models::graph_repos::Model::of_repo(db, wid, repo.id)
                        .await?
                        .is_some_and(|g| g.indexed_at.is_some())
                {
                    crate::services::graph::ingest::queue(db, wid, repo, None).await?;
                    reindex.push(repo.clone());
                }
                if let Some(t) = pr.ticket_id {
                    if pr.state() == PrState::Merged && !was_merged {
                        pulls::notify_ticket(db, wid, None, t, NotificationKind::PrMerged).await?;
                    }
                    touched.push(t);
                }
            }
        }
        "check_run" => {
            let run = &payload["check_run"];
            let check = Check {
                name: run["name"]
                    .as_str()
                    .unwrap_or("check")
                    .chars()
                    .take(100)
                    .collect(),
                state: conclusion(
                    run["status"].as_str().unwrap_or_default(),
                    run["conclusion"].as_str(),
                ),
                url: run["html_url"].as_str().unwrap_or_default().to_string(),
            };
            let sha = run["head_sha"].as_str().unwrap_or_default();
            touched.extend(apply(db, wid, &repos, sha, check).await?);
        }
        "status" => {
            let check = Check {
                name: payload["context"]
                    .as_str()
                    .unwrap_or("status")
                    .chars()
                    .take(100)
                    .collect(),
                state: commit_status(payload["state"].as_str().unwrap_or_default()),
                url: payload["target_url"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
            };
            let sha = payload["sha"].as_str().unwrap_or_default();
            touched.extend(apply(db, wid, &repos, sha, check).await?);
        }
        _ => return Ok(Handled::Ignored),
    }
    touched.sort_unstable();
    touched.dedup();
    for t in touched {
        if let Err(e) = advance_ready(db, wid, t).await {
            tracing::warn!(error = %e, ticket = %t, "auto advance after webhook failed");
        }
    }
    Ok(Handled::Applied { reindex })
}

async fn apply(
    db: &DatabaseConnection,
    wid: workspaces::WorkspaceId,
    repos: &[repos::Model],
    sha: &str,
    check: Check,
) -> Result<Vec<Uuid>, DbErr> {
    let mut touched = Vec::new();
    for repo in repos {
        let before: Vec<(Uuid, CiState)> = pull_requests::Entity::find()
            .filter(pull_requests::Column::RepoId.eq(repo.id))
            .filter(pull_requests::Column::HeadSha.eq(sha))
            .all(db)
            .await?
            .iter()
            .map(|p| (p.id, p.ci()))
            .collect();
        for pr in pulls::apply_check(db, repo, sha, check.clone()).await? {
            let Some(t) = pr.ticket_id else { continue };
            let was = before.iter().find(|(id, _)| *id == pr.id).map(|(_, c)| *c);
            if pr.ci() == CiState::Failure && was != Some(CiState::Failure) {
                pulls::notify_ticket(db, wid, None, t, NotificationKind::CiFailed).await?;
            }
            touched.push(t);
        }
    }
    Ok(touched)
}

/// Gate git yang bisa berubah karena webhook.
fn git_gate(g: &Gate) -> bool {
    matches!(g, Gate::PrOpen | Gate::PrsMerged | Gate::CiGreen)
}

/// Tiket yang stage-nya aktif, keluar `auto`/`approval`, punya gate git, tanpa job terbuka, dan
/// kini memenuhi semua gate: diselesaikan atas nama executor job terakhir stage itu (yang
/// sebelumnya gagal menyelesaikan karena gate). Tanpa job sebelumnya → tidak ada yang dilakukan.
///
/// # Errors
///
/// Error database.
pub async fn advance_ready(
    db: &DatabaseConnection,
    wid: workspaces::WorkspaceId,
    ticket_id: Uuid,
) -> Result<bool, DbErr> {
    let Some(ticket) = tickets::Model::find_in(db, wid, ticket_id).await? else {
        return Ok(false);
    };
    if ticket.deleted_at.is_some() || ticket.stage_state().ok() != Some(StageState::Active) {
        return Ok(false);
    }
    let flow = stages::Flow::load(db, wid, &ticket).await?;
    let Some(rule) = flow.rule(&ticket.stage_key).cloned() else {
        return Ok(false);
    };
    if rule.exit == Exit::Manual || !rule.gates.iter().any(git_gate) {
        return Ok(false);
    }
    let checks = gates::check(db, wid, &ticket, &rule).await?;
    if !gates::all_ok(&checks) {
        return Ok(false);
    }
    let stage_jobs = jobs::Entity::find()
        .filter(jobs::Column::TicketId.eq(ticket.id))
        .filter(jobs::Column::StageKey.eq(ticket.stage_key.clone()))
        .all(db)
        .await?;
    if stage_jobs
        .iter()
        .any(|j| jobs::OPEN.contains(&j.status.as_str()))
    {
        return Ok(false);
    }
    let Some(user_id) = stage_jobs
        .iter()
        .filter(|j| j.status == "succeeded" && j.started_at >= Some(ticket.stage_entered_at))
        .max_by_key(|j| j.finished_at)
        .and_then(|j| j.claimed_by)
    else {
        return Ok(false);
    };
    let Some(user) = users::Model::find_by_id(db, user_id).await? else {
        return Ok(false);
    };
    let Some(workspace) = workspaces::Model::find_by_id(db, wid.get()).await? else {
        return Ok(false);
    };
    let Some((_, role)) = workspace_members::Model::find_with_role(db, wid, user.id).await? else {
        return Ok(false);
    };
    let acting = Acting {
        workspace: &workspace,
        user: &user,
        role: &role,
        request_id: None,
    };
    Ok(stages::complete(db, &acting, ticket.id).await.is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_round_trip_and_tamper() {
        let body = br#"{"zen":"ok"}"#;
        let sig = sign("rahasia", body);
        assert!(verify("rahasia", body, Some(&sig)));
        assert!(!verify("lain", body, Some(&sig)));
        assert!(!verify("rahasia", br#"{"zen":"ok!"}"#, Some(&sig)));
        assert!(!verify("rahasia", body, None));
        assert!(!verify("rahasia", body, Some("sha1=abcd")));
        assert!(!verify("rahasia", body, Some("sha256=zz")));
    }

    #[test]
    fn check_states() {
        assert_eq!(conclusion("in_progress", None), CiState::Pending);
        assert_eq!(conclusion("completed", Some("success")), CiState::Success);
        assert_eq!(conclusion("completed", Some("skipped")), CiState::Success);
        assert_eq!(conclusion("completed", Some("timed_out")), CiState::Failure);
        assert_eq!(commit_status("error"), CiState::Failure);
    }
}
