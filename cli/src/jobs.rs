//! `agentgate jobs` dan `agentgate job claim|release|cancel`.

use std::io::Write;

use agentgate_core::runner::protocol::JobSummary;
use serde_json::json;
use uuid::Uuid;

use crate::{
    api::Api,
    config::{Paths, RunnerIdentity},
    error::{Error, Result},
    ui::clip,
};

/// `GET /api/cli/jobs`.
///
/// # Errors
///
/// Lihat [`crate::api::ApiError`].
pub async fn list(api: &Api) -> Result<Vec<JobSummary>> {
    Ok(api.get("/api/cli/jobs").await?)
}

/// Tabel antrean job.
#[must_use]
pub fn table(jobs: &[JobSummary]) -> String {
    if jobs.is_empty() {
        return "Tidak ada job di antrean.\n".into();
    }
    let rows: Vec<[String; 7]> = jobs
        .iter()
        .map(|j| {
            let flag = if j.mine {
                "milik Anda"
            } else if j.can_claim {
                "bisa diambil"
            } else {
                ""
            };
            [
                j.id.to_string()[..8].to_string(),
                j.ticket.clone(),
                clip(&j.title, 40),
                j.stage_name.clone(),
                j.persona.clone(),
                format!("{} {}", j.status.as_str(), j.mode.as_str()),
                flag.to_string(),
            ]
        })
        .collect();
    let header = ["ID", "TIKET", "JUDUL", "STAGE", "PERSONA", "STATUS", ""];
    let mut widths = header.map(|h| h.chars().count());
    for r in &rows {
        for (w, c) in widths.iter_mut().zip(r) {
            *w = (*w).max(c.chars().count());
        }
    }
    let line = |cells: &[String]| {
        cells
            .iter()
            .zip(widths)
            .map(|(c, w)| format!("{c:<w$}"))
            .collect::<Vec<_>>()
            .join("  ")
            .trim_end()
            .to_string()
    };
    let mut out = line(&header.map(str::to_string));
    out.push('\n');
    for r in &rows {
        out.push_str(&line(r));
        out.push('\n');
    }
    out
}

/// Id job dari argumen: UUID lengkap, atau awalan unik dari antrean.
///
/// # Errors
///
/// Tidak ditemukan, ambigu, server gagal.
pub async fn resolve(api: &Api, arg: &str) -> Result<Uuid> {
    if let Ok(id) = Uuid::try_parse(arg.trim()) {
        return Ok(id);
    }
    let prefix = arg.trim().to_ascii_lowercase();
    if prefix.is_empty() {
        return Err(Error::JobNotFound(arg.into()));
    }
    let jobs = list(api).await?;
    let matches: Vec<Uuid> = jobs
        .iter()
        .map(|j| j.id)
        .filter(|id| id.to_string().starts_with(&prefix))
        .collect();
    match matches.as_slice() {
        [id] => Ok(*id),
        [] => Err(Error::JobNotFound(arg.into())),
        _ => Err(Error::JobAmbiguous(arg.into())),
    }
}

/// Aksi job dari CLI.
#[derive(Debug, Clone, Copy)]
pub enum Action {
    Claim,
    Release,
    Cancel,
}

/// `POST /api/cli/jobs/{id}/claim|release|cancel`.
///
/// # Errors
///
/// Lihat [`crate::api::ApiError`].
pub async fn act(
    paths: &Paths,
    api: &Api,
    arg: &str,
    action: Action,
    out: &mut impl Write,
) -> Result<()> {
    let id = resolve(api, arg).await?;
    let (path, done) = match action {
        Action::Claim => ("claim", "diambil"),
        Action::Release => ("release", "dilepas ke antrean"),
        Action::Cancel => ("cancel", "dibatalkan"),
    };
    let body = match action {
        // Klaim dari perangkat yang punya runner langsung dikunci ke runner itu.
        Action::Claim => {
            let runner = RunnerIdentity::load(paths)?
                .filter(|r| r.server == api.base())
                .and_then(|r| r.runner_id);
            json!({ "runner_id": runner })
        }
        Action::Release => json!({ "note": "" }),
        Action::Cancel => json!({}),
    };
    let res: serde_json::Value = api
        .post(&format!("/api/cli/jobs/{id}/{path}"), &body)
        .await?;
    let status = res.get("status").and_then(|s| s.as_str()).unwrap_or("");
    let _ = writeln!(
        out,
        "Job {} {done} (status {status}).",
        &id.to_string()[..8]
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use agentgate_core::runner::protocol::{JobKind, JobMode, JobStatus};

    use super::*;

    #[test]
    fn table_lists_jobs_with_short_ids() {
        let job = JobSummary {
            id: Uuid::try_parse("0193a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b").unwrap(),
            kind: JobKind::Stage,
            mode: JobMode::Auto,
            status: JobStatus::Queued,
            ticket: "KSR-26".into(),
            title: "Validasi stok".into(),
            stage: "dev".into(),
            stage_name: "Development".into(),
            persona: "dev".into(),
            priority: 2,
            mine: false,
            can_claim: true,
            created_at: String::new(),
        };
        let t = table(&[job]);
        let lines: Vec<&str> = t.lines().collect();
        assert!(lines[0].starts_with("ID"));
        assert!(lines[1].starts_with("0193a1b2  KSR-26  Validasi stok"));
        assert!(lines[1].ends_with("queued auto  bisa diambil"));
        assert_eq!(table(&[]), "Tidak ada job di antrean.\n");
    }
}
