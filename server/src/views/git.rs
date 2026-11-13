//! Tampilan git & PR (M6): badge PR di kartu/header tiket, baris PR, status CI, koneksi provider.
//! Teks lewat kunci Fluent (`ui-git.ftl`).
use std::collections::HashMap;

use agentgate_core::git::{CiState, PrState};
use chrono::{DateTime, FixedOffset};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    models::{git_connections, pull_requests, repos},
    views::runner::{Age, age},
};

/// Pesan flash halaman git.
pub const FLASH_CONNECTED: &str = "git-flash-connected";
pub const FLASH_DISCONNECTED: &str = "git-flash-disconnected";
pub const FLASH_RELINKED: &str = "git-flash-relinked";
pub const FLASH_SAVED: &str = "git-flash-saved";

/// Kunci teks yang dipakai dari Rust (kontrak i18n).
pub const MESSAGE_KEYS: &[&str] = &[
    FLASH_CONNECTED,
    FLASH_DISCONNECTED,
    FLASH_RELINKED,
    FLASH_SAVED,
    "pr-state-open",
    "pr-state-draft",
    "pr-state-closed",
    "pr-state-merged",
    "ci-state-none",
    "ci-state-pending",
    "ci-state-success",
    "ci-state-failure",
];

/// Kunci Fluent, varian badge, ikon untuk status PR.
#[must_use]
pub fn pr_badge(state: PrState) -> (&'static str, &'static str, &'static str) {
    match state {
        PrState::Open => ("pr-state-open", "success", "git-pull-request"),
        PrState::Draft => ("pr-state-draft", "neutral", "git-pull-request"),
        PrState::Closed => ("pr-state-closed", "destructive", "git-pull-request"),
        PrState::Merged => ("pr-state-merged", "info", "git-merge"),
    }
}

/// Kunci Fluent, varian badge, ikon untuk status CI.
#[must_use]
pub fn ci_badge(state: CiState) -> (&'static str, &'static str, &'static str) {
    match state {
        CiState::None => ("ci-state-none", "neutral", "circle-dashed"),
        CiState::Pending => ("ci-state-pending", "gate", "loader-2"),
        CiState::Success => ("ci-state-success", "success", "circle-check"),
        CiState::Failure => ("ci-state-failure", "destructive", "circle-x"),
    }
}

/// Status dalam bentuk siap render.
#[derive(Debug, Clone, Serialize)]
pub struct StateView {
    pub value: &'static str,
    pub key: &'static str,
    pub variant: &'static str,
    pub icon: &'static str,
}

impl StateView {
    #[must_use]
    pub fn pr(state: PrState) -> Self {
        let (key, variant, icon) = pr_badge(state);
        Self {
            value: state.as_str(),
            key,
            variant,
            icon,
        }
    }

    #[must_use]
    pub fn ci(state: CiState) -> Self {
        let (key, variant, icon) = ci_badge(state);
        Self {
            value: state.as_str(),
            key,
            variant,
            icon,
        }
    }
}

/// Badge PR ringkas untuk satu tiket: PR utama (terbuka dulu, lalu merged, lalu terbaru) dan
/// jumlah PR lain.
#[derive(Debug, Clone, Serialize)]
pub struct PrBadge {
    pub number: i64,
    pub url: String,
    /// Jumlah PR tiket (tidak termasuk yang ditutup tanpa merge bila ada yang lain).
    pub count: usize,
    pub state: StateView,
    pub ci: StateView,
}

fn rank(pr: &pull_requests::Model) -> u8 {
    match pr.state() {
        PrState::Open | PrState::Draft => 0,
        PrState::Merged => 1,
        PrState::Closed => 2,
    }
}

impl PrBadge {
    /// Badge dari PR satu tiket; `None` bila kosong.
    #[must_use]
    pub fn of(prs: &[&pull_requests::Model]) -> Option<Self> {
        let main = prs
            .iter()
            .min_by(|a, b| rank(a).cmp(&rank(b)).then(b.updated_at.cmp(&a.updated_at)))?;
        let counted = prs.iter().filter(|p| p.counts()).count();
        Some(Self {
            number: main.number,
            url: main.url.clone(),
            count: counted.max(1),
            state: StateView::pr(main.state()),
            ci: StateView::ci(main.ci()),
        })
    }

    /// Badge per tiket; PR tanpa tiket diabaikan.
    #[must_use]
    pub fn by_ticket(prs: &[pull_requests::Model]) -> HashMap<Uuid, Self> {
        let mut grouped: HashMap<Uuid, Vec<&pull_requests::Model>> = HashMap::new();
        for pr in prs {
            if let Some(t) = pr.ticket_id {
                grouped.entry(t).or_default().push(pr);
            }
        }
        grouped
            .into_iter()
            .filter_map(|(t, list)| Self::of(&list).map(|b| (t, b)))
            .collect()
    }
}

/// Satu check CI.
#[derive(Debug, Clone, Serialize)]
pub struct CheckView {
    pub name: String,
    pub url: String,
    pub state: StateView,
}

/// Tiket yang ditautkan ke PR.
#[derive(Debug, Clone, Serialize)]
pub struct TicketRef {
    pub code: String,
    pub title: String,
    pub href: String,
}

/// Satu PR untuk daftar PR project / tab PR tiket.
#[derive(Debug, Clone, Serialize)]
pub struct PrView {
    pub id: Uuid,
    pub number: i64,
    pub title: String,
    pub url: String,
    pub repo: String,
    pub head_branch: String,
    pub base_branch: String,
    pub head_sha: String,
    pub additions: i32,
    pub deletions: i32,
    pub author: String,
    pub state: StateView,
    pub ci: StateView,
    pub checks: Vec<CheckView>,
    pub ticket: Option<TicketRef>,
    pub job_href: Option<String>,
    pub updated: Age,
    pub merged_at: Option<DateTime<FixedOffset>>,
}

impl PrView {
    #[must_use]
    pub fn of(
        slug: &str,
        pr: &pull_requests::Model,
        repo: Option<&repos::Model>,
        ticket: Option<TicketRef>,
    ) -> Self {
        Self {
            id: pr.id,
            number: pr.number,
            title: pr.title.clone(),
            url: pr.url.clone(),
            repo: repo
                .map(|r| {
                    if r.full_name.is_empty() {
                        r.name.clone()
                    } else {
                        r.full_name.clone()
                    }
                })
                .unwrap_or_default(),
            head_branch: pr.head_branch.clone(),
            base_branch: pr.base_branch.clone(),
            head_sha: pr.head_sha.chars().take(7).collect(),
            additions: pr.additions,
            deletions: pr.deletions,
            author: pr.author.clone(),
            state: StateView::pr(pr.state()),
            ci: StateView::ci(pr.ci()),
            checks: pr
                .checks()
                .into_iter()
                .map(|c| CheckView {
                    name: c.name,
                    url: c.url,
                    state: StateView::ci(c.state),
                })
                .collect(),
            ticket,
            job_href: pr.job_id.map(|j| format!("/w/{slug}/jobs/{j}")),
            updated: age(&pr.updated_at, chrono::Utc::now()),
            merged_at: pr.merged_at,
        }
    }
}

/// Koneksi provider git di halaman pengaturan workspace.
#[derive(Debug, Clone, Serialize)]
pub struct ConnectionView {
    pub id: Uuid,
    pub provider: String,
    pub provider_key: &'static str,
    pub host: String,
    pub web_base: String,
    pub account: String,
    pub webhook_url: String,
    /// Secret HMAC webhook untuk pemasangan manual (halaman hanya untuk `ManageIntegrations`).
    pub webhook_secret: String,
    pub repos: usize,
    pub last_event: Option<Age>,
    /// Aksi form (ganti token, putuskan).
    pub disconnect_action: String,
}

impl ConnectionView {
    #[must_use]
    pub fn of(
        c: &git_connections::Model,
        webhook_url: String,
        webhook_secret: String,
        repos: usize,
        base: &str,
    ) -> Self {
        let kind = c.kind();
        Self {
            id: c.id,
            provider: c.provider.clone(),
            provider_key: kind.label(),
            host: c.host.clone(),
            web_base: c.web_base.clone(),
            account: c.account.clone(),
            webhook_url,
            webhook_secret,
            repos,
            last_event: c.last_event_at.map(|at| age(&at, chrono::Utc::now())),
            disconnect_action: format!("{base}/{}/disconnect", c.id),
        }
    }
}

/// Repo project beserta status tautannya ke provider.
#[derive(Debug, Clone, Serialize)]
pub struct RepoLinkView {
    pub id: Uuid,
    pub name: String,
    pub url: String,
    pub full_name: String,
    pub project_key: String,
    pub project_name: String,
    pub project_color: String,
    pub provider: &'static str,
    /// `active` (webhook terpasang), `error` (gagal dipasang), `none` (tanpa koneksi provider).
    pub webhook: &'static str,
    pub webhook_error: String,
    pub last_event: Option<Age>,
    pub relink_action: String,
}

impl RepoLinkView {
    #[must_use]
    pub fn of(
        r: &repos::Model,
        project: (&str, &str, &str),
        conn: Option<&git_connections::Model>,
        base: &str,
    ) -> Self {
        let webhook = match (conn, r.webhook_id) {
            (None, _) => "none",
            (Some(_), Some(_)) => "active",
            (Some(_), None) => "error",
        };
        Self {
            id: r.id,
            name: r.name.clone(),
            url: r.url.clone(),
            full_name: if r.full_name.is_empty() {
                r.name.clone()
            } else {
                r.full_name.clone()
            },
            project_key: project.0.to_string(),
            project_name: project.1.to_string(),
            project_color: project.2.to_string(),
            provider: conn.map_or("", |c| c.kind().label()),
            webhook,
            webhook_error: r.webhook_error.clone(),
            last_event: r.last_event_at.map(|at| age(&at, chrono::Utc::now())),
            relink_action: format!("{base}/repos/{}/relink", r.id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pr(ticket: Option<Uuid>, number: i64, state: &str, ci: &str) -> pull_requests::Model {
        let now: DateTime<FixedOffset> = chrono::Utc::now().into();
        pull_requests::Model {
            id: Uuid::new_v4(),
            workspace_id: Uuid::nil(),
            project_id: Uuid::nil(),
            repo_id: Uuid::nil(),
            ticket_id: ticket,
            number,
            title: "t".into(),
            url: format!("https://github.com/a/b/pull/{number}"),
            head_branch: "ag/KSR-1-x".into(),
            base_branch: "main".into(),
            head_sha: "abcdef0123".into(),
            state: state.into(),
            ci_state: ci.into(),
            checks: serde_json::json!([]),
            additions: 1,
            deletions: 2,
            author: "bot".into(),
            job_id: None,
            merged_at: None,
            closed_at: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn badge_prefers_open_pull_and_counts_live_ones() {
        let t = Uuid::new_v4();
        let list = [
            pr(Some(t), 140, "merged", "success"),
            pr(Some(t), 141, "open", "failure"),
            pr(Some(t), 139, "closed", "none"),
            pr(None, 99, "open", "none"),
        ];
        let map = PrBadge::by_ticket(&list);
        assert_eq!(map.len(), 1);
        let b = &map[&t];
        assert_eq!((b.number, b.count), (141, 2));
        assert_eq!((b.state.variant, b.ci.variant), ("success", "destructive"));
    }

    #[test]
    fn state_views() {
        assert_eq!(StateView::pr(PrState::Merged).icon, "git-merge");
        assert_eq!(StateView::ci(CiState::Pending).key, "ci-state-pending");
    }
}
