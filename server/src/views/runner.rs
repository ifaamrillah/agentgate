//! Tampilan runner & job (M5): status job, baris antrean, runner, permintaan izin, log, langkah,
//! dan evidence. Teks lewat kunci Fluent (`ui-runner.ftl`).
use agentgate_core::runner::protocol::{
    EvidenceKind, JobKind, JobMode, JobStatus, LogStream, PermissionStatus, Step, StepState,
};
use chrono::{DateTime, FixedOffset, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    models::{
        job_evidence, job_logs, job_permissions, jobs,
        personas::{PersonaLabel, PersonaLabels},
        runners, users,
    },
    views::tickets::PersonView,
};

/// Pesan flash halaman runner/job.
pub const FLASH_CLAIMED: &str = "job-flash-claimed";
pub const FLASH_RELEASED: &str = "job-flash-released";
pub const FLASH_CANCELLED: &str = "job-flash-cancelled";
pub const FLASH_QUOTA_WAIT: &str = "job-flash-quota-wait";
pub const FLASH_PERMISSION: &str = "job-flash-permission";
pub const FLASH_PREFS: &str = "runner-flash-prefs";
pub const FLASH_REVOKED: &str = "runner-flash-revoked";

/// Kunci teks yang dipakai dari Rust (kontrak i18n).
pub const MESSAGE_KEYS: &[&str] = &[
    FLASH_CLAIMED,
    FLASH_RELEASED,
    FLASH_CANCELLED,
    FLASH_QUOTA_WAIT,
    FLASH_PERMISSION,
    FLASH_PREFS,
    FLASH_REVOKED,
];

/// Kunci Fluent + varian badge status job.
#[must_use]
pub fn status_badge(status: JobStatus) -> (&'static str, &'static str) {
    match status {
        JobStatus::Queued => ("job-status-queued", "info"),
        JobStatus::Claimed => ("job-status-claimed", "secondary"),
        JobStatus::Running => ("job-status-running", "ai"),
        JobStatus::Paused => ("job-status-paused", "gate"),
        JobStatus::Quota => ("job-status-quota", "destructive"),
        JobStatus::Succeeded => ("job-status-succeeded", "success"),
        JobStatus::Failed => ("job-status-failed", "destructive"),
        JobStatus::Cancelled => ("job-status-cancelled", "neutral"),
    }
}

/// Tanda persona (warna + ikon) untuk baris job.
#[derive(Debug, Clone, Serialize)]
pub struct PersonaMark {
    pub key: String,
    pub name: String,
    pub icon: String,
    pub color: String,
}

impl PersonaMark {
    #[must_use]
    pub fn of(key: &str, labels: &PersonaLabels) -> Self {
        labels.get(key).map_or_else(
            || Self {
                key: key.to_string(),
                name: key.to_string(),
                icon: "robot".into(),
                color: String::new(),
            },
            |l: &PersonaLabel| Self {
                key: l.key.clone(),
                name: l.name.clone(),
                icon: l.icon.clone(),
                color: l.color.clone(),
            },
        )
    }
}

/// Umur singkat ("baru saja", "5 mnt", "3 jam", "2 hari") sebagai kunci Fluent + angka.
#[derive(Debug, Clone, Serialize)]
pub struct Age {
    pub key: &'static str,
    pub n: i64,
}

#[must_use]
pub fn age(at: &DateTime<FixedOffset>, now: DateTime<Utc>) -> Age {
    let secs = (now - at.with_timezone(&Utc)).num_seconds().max(0);
    match secs {
        0..60 => Age {
            key: "age-now",
            n: 0,
        },
        60..3_600 => Age {
            key: "age-minutes",
            n: secs / 60,
        },
        3_600..86_400 => Age {
            key: "age-hours",
            n: secs / 3_600,
        },
        _ => Age {
            key: "age-days",
            n: secs / 86_400,
        },
    }
}

/// Jam `HH.MM:SS` WIB (stempel baris log).
#[must_use]
pub fn clock(at: &DateTime<FixedOffset>) -> String {
    let wib = FixedOffset::east_opt(7 * 3600).unwrap_or_else(|| at.offset().to_owned());
    at.with_timezone(&wib).format("%H.%M:%S").to_string()
}

/// Job di daftar (antrean, kartu stage tiket, halaman log).
#[derive(Debug, Clone, Serialize)]
pub struct JobView {
    pub id: Uuid,
    pub href: String,
    pub kind: &'static str,
    pub mode_key: &'static str,
    pub status: &'static str,
    pub status_key: &'static str,
    pub status_variant: &'static str,
    pub open: bool,
    pub ticket: String,
    pub title: String,
    pub ticket_href: String,
    pub stage: String,
    pub stage_name: String,
    /// Nama job untuk judul dan baris: nama persona untuk job komentar, nama stage selain itu.
    pub label: String,
    pub persona: PersonaMark,
    pub priority: u8,
    pub priority_key: &'static str,
    pub age: Age,
    pub holder: Option<PersonView>,
    pub runner: Option<String>,
    pub mine: bool,
    pub can_claim: bool,
    pub can_release: bool,
    pub can_cancel: bool,
    pub can_quota: bool,
    pub note: String,
    pub summary: String,
    pub branch: String,
    pub client: String,
    pub model: String,
    pub tokens: Option<i64>,
    pub started: Option<String>,
    pub finished: Option<String>,
    pub quota_reset: Option<String>,
    pub actions: JobActions,
}

/// URL aksi POST job.
#[derive(Debug, Clone, Serialize)]
pub struct JobActions {
    pub claim: String,
    pub release: String,
    pub cancel: String,
    pub quota: String,
}

impl JobActions {
    #[must_use]
    pub fn of(slug: &str, id: Uuid) -> Self {
        let base = format!("/w/{slug}/jobs/{id}");
        Self {
            claim: format!("{base}/claim"),
            release: format!("{base}/release"),
            cancel: format!("{base}/cancel"),
            quota: format!("{base}/quota"),
        }
    }
}

/// Bahan [`JobView`] di luar model job.
pub struct JobContext<'a> {
    pub slug: &'a str,
    pub ticket_code: &'a str,
    pub ticket_title: &'a str,
    pub stage_name: String,
    pub labels: &'a PersonaLabels,
    pub priority: u8,
    pub holder: Option<&'a users::Model>,
    pub runner: Option<&'a runners::Model>,
    pub viewer: Uuid,
    pub can_claim: bool,
    pub is_admin: bool,
    pub now: DateTime<Utc>,
}

impl JobView {
    #[must_use]
    pub fn of(job: &jobs::Model, c: &JobContext<'_>) -> Self {
        let status = job.status();
        let (status_key, status_variant) = status_badge(status);
        let mine = job.claimed_by == Some(c.viewer);
        let holder_or_admin = mine || c.is_admin;
        Self {
            id: job.id,
            href: format!("/w/{}/jobs/{}", c.slug, job.id),
            kind: job.kind().as_str(),
            mode_key: match (job.kind(), job.mode()) {
                (JobKind::Comment, _) => "job-mode-comment",
                (_, JobMode::Auto) => "job-mode-auto",
                (_, JobMode::Interactive) => "job-mode-interactive",
            },
            status: status.as_str(),
            status_key,
            status_variant,
            open: status.is_open(),
            ticket: c.ticket_code.to_string(),
            title: c.ticket_title.to_string(),
            ticket_href: format!("/w/{}/t/{}", c.slug, c.ticket_code),
            stage: job.stage_key.clone(),
            stage_name: c.stage_name.clone(),
            label: if job.kind() == JobKind::Comment {
                PersonaMark::of(&job.persona, c.labels).name
            } else {
                c.stage_name.clone()
            },
            persona: PersonaMark::of(&job.persona, c.labels),
            priority: c.priority,
            priority_key: match c.priority {
                1 => "ticket-priority-urgent",
                2 => "ticket-priority-high",
                4 => "ticket-priority-low",
                _ => "ticket-priority-medium",
            },
            age: age(&job.created_at, c.now),
            holder: c.holder.map(PersonView::of),
            runner: c.runner.map(|r| r.name.clone()),
            mine,
            can_claim: c.can_claim,
            can_release: holder_or_admin && status.is_open() && status != JobStatus::Queued,
            can_cancel: status.is_open()
                && (holder_or_admin || (job.claimed_by.is_none() && c.can_claim)),
            can_quota: status == JobStatus::Quota && holder_or_admin,
            note: job.prompt_note.clone(),
            summary: job.summary.clone(),
            branch: job.branch.clone(),
            client: job.client.clone(),
            model: job.model.clone(),
            tokens: job.tokens,
            started: job.started_at.as_ref().map(super::time::wib_datetime),
            finished: job.finished_at.as_ref().map(super::time::wib_datetime),
            quota_reset: job.quota_reset_at.as_ref().map(super::time::wib_datetime),
            actions: JobActions::of(c.slug, job.id),
        }
    }
}

/// Runner di daftar.
#[derive(Debug, Clone, Serialize)]
pub struct RunnerView {
    pub id: Uuid,
    pub name: String,
    pub os: String,
    pub client: String,
    pub version: String,
    /// `online` | `busy` | `offline`.
    pub state: &'static str,
    pub state_key: &'static str,
    pub job: Option<RunnerJob>,
    pub last_seen: Option<String>,
    pub registered: String,
    pub owner: PersonView,
    pub mine: bool,
    pub revoke_href: Option<String>,
}

/// Job yang sedang dikerjakan runner.
#[derive(Debug, Clone, Serialize)]
pub struct RunnerJob {
    pub ticket: String,
    pub href: String,
}

impl RunnerView {
    #[must_use]
    pub fn of(
        r: &runners::Model,
        owner: &users::Model,
        job: Option<RunnerJob>,
        slug: &str,
        viewer: Uuid,
        can_revoke: bool,
    ) -> Self {
        let online = r.is_online();
        let (state, state_key) = match (online, job.is_some()) {
            (true, true) => ("busy", "runner-state-busy"),
            (true, false) => ("online", "runner-state-online"),
            _ => ("offline", "runner-state-offline"),
        };
        Self {
            id: r.id,
            name: r.name.clone(),
            os: r.os.clone(),
            client: r.client.clone(),
            version: r.version.clone(),
            state,
            state_key,
            job,
            last_seen: r.last_seen_at.as_ref().map(super::time::wib_datetime),
            registered: super::time::wib_datetime(&r.created_at),
            owner: PersonView::of(owner),
            mine: r.user_id == viewer,
            revoke_href: can_revoke.then(|| format!("/w/{slug}/runner/runners/{}/revoke", r.id)),
        }
    }
}

/// Permintaan izin aksi berbahaya.
#[derive(Debug, Clone, Serialize)]
pub struct PermissionView {
    pub id: Uuid,
    pub action: String,
    pub command: String,
    pub reason: String,
    pub status: &'static str,
    pub status_key: &'static str,
    pub ticket: String,
    pub ticket_title: String,
    pub ticket_href: String,
    pub job_href: String,
    pub persona: PersonaMark,
    pub runner: Option<String>,
    pub age: Age,
    pub decided_by: Option<String>,
    pub decide_href: String,
}

/// Bahan [`PermissionView`] di luar model izin dan job.
pub struct PermissionContext<'a> {
    pub slug: &'a str,
    pub labels: &'a PersonaLabels,
    pub runner: Option<&'a runners::Model>,
    pub decided_by: Option<&'a users::Model>,
    pub now: DateTime<Utc>,
}

impl PermissionView {
    #[must_use]
    pub fn of(
        p: &job_permissions::Model,
        job: &jobs::Model,
        ticket: (&str, &str),
        c: &PermissionContext<'_>,
    ) -> Self {
        let PermissionContext {
            slug,
            labels,
            runner,
            decided_by,
            now,
        } = *c;
        let status = p.status();
        Self {
            id: p.id,
            action: p.action.clone(),
            command: p.command.clone(),
            reason: p.reason.clone(),
            status: status.as_str(),
            status_key: match status {
                PermissionStatus::Pending => "permission-status-pending",
                PermissionStatus::Allowed => "permission-status-allowed",
                PermissionStatus::AllowedTicket => "permission-status-allowed-ticket",
                PermissionStatus::Denied => "permission-status-denied",
            },
            ticket: ticket.0.to_string(),
            ticket_title: ticket.1.to_string(),
            ticket_href: format!("/w/{slug}/t/{}", ticket.0),
            job_href: format!("/w/{slug}/jobs/{}", job.id),
            persona: PersonaMark::of(&job.persona, labels),
            runner: runner.map(|r| r.name.clone()),
            age: age(&p.created_at, now),
            decided_by: decided_by.map(|u| {
                if u.name.is_empty() {
                    u.email.clone()
                } else {
                    u.name.clone()
                }
            }),
            decide_href: format!("/w/{slug}/runner/permissions/{}", p.id),
        }
    }
}

/// Satu baris log.
#[derive(Debug, Clone, Serialize)]
pub struct LogLineView {
    pub stream: &'static str,
    pub at: String,
    pub text: String,
}

impl LogLineView {
    #[must_use]
    pub fn of(l: &job_logs::Model) -> Self {
        Self {
            stream: LogStream::parse(&l.stream).map_or("info", LogStream::as_str),
            at: clock(&l.created_at),
            text: l.body.clone(),
        }
    }
}

/// Satu langkah job.
#[derive(Debug, Clone, Serialize)]
pub struct StepView {
    pub name: String,
    pub state: &'static str,
    pub seconds: Option<String>,
}

/// Langkah job + jumlah selesai.
#[derive(Debug, Clone, Serialize)]
pub struct StepsView {
    pub items: Vec<StepView>,
    pub done: usize,
    pub percent: usize,
}

impl StepsView {
    #[must_use]
    pub fn of(steps: &[Step]) -> Self {
        let items: Vec<StepView> = steps
            .iter()
            .map(|s| StepView {
                name: s.name.clone(),
                state: s.state.as_str(),
                seconds: s.seconds.map(|n| {
                    if n >= 60 {
                        format!("{}m {:02}s", n / 60, n % 60)
                    } else {
                        format!("{n}s")
                    }
                }),
            })
            .collect();
        let done = steps.iter().filter(|s| s.state == StepState::Ok).count();
        let percent = if steps.is_empty() {
            0
        } else {
            done * 100 / steps.len()
        };
        Self {
            items,
            done,
            percent,
        }
    }
}

/// Evidence (berkas bertanda tangan runner).
#[derive(Debug, Clone, Serialize)]
pub struct EvidenceView {
    pub id: Uuid,
    pub kind: &'static str,
    pub icon: &'static str,
    pub name: String,
    pub passed: Option<bool>,
    pub total: Option<i32>,
    pub failed: Option<i32>,
    pub note: String,
    pub size: String,
    pub at: String,
    pub href: String,
    pub job_href: String,
    pub image: bool,
}

impl EvidenceView {
    #[must_use]
    pub fn of(e: &job_evidence::Model, slug: &str) -> Self {
        let kind = e.kind().unwrap_or(EvidenceKind::Log);
        Self {
            id: e.id,
            kind: kind.as_str(),
            icon: match kind {
                EvidenceKind::TestReport => "test-pipe",
                EvidenceKind::Screenshot => "photo",
                EvidenceKind::Log => "file-text",
            },
            name: e.name.clone(),
            passed: e.passed,
            total: e.total,
            failed: e.failed,
            note: e.note.clone(),
            size: human_size(e.size_bytes),
            at: super::time::wib_datetime(&e.created_at),
            href: format!("/w/{slug}/evidence/{}", e.id),
            job_href: format!("/w/{slug}/jobs/{}", e.job_id),
            image: kind == EvidenceKind::Screenshot,
        }
    }
}

fn human_size(bytes: i32) -> String {
    let b = f64::from(bytes.max(0));
    if b >= 1_048_576.0 {
        format!("{:.1} MB", b / 1_048_576.0)
    } else if b >= 1024.0 {
        format!("{:.0} KB", b / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn ages_are_bucketed() {
        let now = Utc.with_ymd_and_hms(2026, 11, 13, 10, 0, 0).unwrap();
        let at = |secs: i64| (now - chrono::Duration::seconds(secs)).fixed_offset();
        assert_eq!(age(&at(5), now).key, "age-now");
        let a = age(&at(300), now);
        assert_eq!((a.key, a.n), ("age-minutes", 5));
        let a = age(&at(7_200), now);
        assert_eq!((a.key, a.n), ("age-hours", 2));
        let a = age(&at(200_000), now);
        assert_eq!((a.key, a.n), ("age-days", 2));
    }

    #[test]
    fn steps_count_done() {
        let steps = vec![
            Step {
                name: "a".into(),
                state: StepState::Ok,
                seconds: Some(90),
            },
            Step {
                name: "b".into(),
                state: StepState::Now,
                seconds: None,
            },
        ];
        let v = StepsView::of(&steps);
        assert_eq!((v.done, v.percent), (1, 50));
        assert_eq!(v.items[0].seconds.as_deref(), Some("1m 30s"));
    }

    #[test]
    fn sizes_are_human() {
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(2048), "2 KB");
        assert_eq!(human_size(3 * 1_048_576), "3.0 MB");
    }
}
