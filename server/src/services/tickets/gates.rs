//! Evaluasi gate (spec: "Transisi ditolak server kalau gate belum terpenuhi").
//!
//! Gate dinilai dari data di server, bukan klaim pengguna:
//! - `artifact_exists`: artifact output stage (nama `output`, atau artifact apa pun milik stage
//!   bila stage tanpa nama output) punya minimal satu versi;
//! - `acceptance_criteria`: deskripsi tiket atau artifact output memuat checklist (`- [ ] …`);
//! - `subtasks_done`: semua subtask aktif sudah berkategori `done`;
//! - `tests_passed` / `screenshot_exists`: evidence bertanda tangan runner (M5) sejak tiket masuk
//!   stage ini (`services::jobs::evidence::gate_status`); hasil test = laporan terbaru;
//! - gate git (PR, CI) dari provider git (M6). Sumber itu belum ada, jadi gate ini selalu belum
//!   terpenuhi dan transisinya ditolak; Admin bisa memaksa transisi.

use agentgate_core::workflow::{Gate, Stage};
use sea_orm::{ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter};
use serde::Serialize;

use crate::models::{
    artifact_versions, ticket_artifacts,
    tickets::{self, TicketStatus},
    workspaces::WorkspaceId,
};

/// Hasil satu gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct GateCheck {
    pub gate: Gate,
    pub ok: bool,
}

/// Semua gate terpenuhi.
#[must_use]
pub fn all_ok(checks: &[GateCheck]) -> bool {
    checks.iter().all(|c| c.ok)
}

/// Nilai setiap gate `stage` untuk `ticket`.
///
/// # Errors
///
/// Error database.
pub async fn check<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    ticket: &tickets::Model,
    stage: &Stage,
) -> Result<Vec<GateCheck>, DbErr> {
    if stage.gates.is_empty() {
        return Ok(Vec::new());
    }
    let output = output_body(db, wid, ticket, stage).await?;
    let evidence = if stage
        .gates
        .iter()
        .any(|g| matches!(g, Gate::TestsPassed | Gate::ScreenshotExists))
    {
        crate::services::jobs::evidence::gate_status(db, wid, ticket).await?
    } else {
        (false, false)
    };
    let mut out = Vec::with_capacity(stage.gates.len());
    for gate in &stage.gates {
        let ok = match gate {
            Gate::ArtifactExists => output.is_some(),
            Gate::AcceptanceCriteria => {
                has_checklist(&ticket.description) || output.as_deref().is_some_and(has_checklist)
            }
            Gate::SubtasksDone => subtasks_done(db, wid, ticket).await?,
            Gate::TestsPassed => evidence.0,
            Gate::ScreenshotExists => evidence.1,
            Gate::PrOpen | Gate::PrsMerged | Gate::CiGreen => false,
        };
        out.push(GateCheck { gate: *gate, ok });
    }
    Ok(out)
}

/// Isi versi terbaru artifact output stage, bila ada.
async fn output_body<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    ticket: &tickets::Model,
    stage: &Stage,
) -> Result<Option<String>, DbErr> {
    let artifacts = ticket_artifacts::Model::of_ticket(db, wid, ticket.id).await?;
    let candidates = artifacts.iter().filter(|a| {
        if stage.output.is_empty() {
            a.stage_key == stage.key
        } else {
            a.name.eq_ignore_ascii_case(&stage.output)
        }
    });
    for a in candidates {
        if let Some(v) = artifact_versions::Model::latest(db, wid, a.id).await? {
            return Ok(Some(v.body));
        }
    }
    Ok(None)
}

async fn subtasks_done<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    ticket: &tickets::Model,
) -> Result<bool, DbErr> {
    let open = tickets::Entity::find()
        .filter(tickets::Column::WorkspaceId.eq(wid.get()))
        .filter(tickets::Column::ParentId.eq(ticket.id))
        .filter(tickets::active())
        .filter(tickets::Column::Status.ne(sea_orm::ActiveEnum::to_value(&TicketStatus::Done)))
        .one(db)
        .await?;
    Ok(open.is_none())
}

/// Ada baris checklist Markdown berisi teks (`- [ ] …` atau `- [x] …`).
#[must_use]
pub fn has_checklist(text: &str) -> bool {
    text.lines().any(|l| {
        let l = l.trim_start();
        let rest = l
            .strip_prefix("- [ ]")
            .or_else(|| l.strip_prefix("- [x]"))
            .or_else(|| l.strip_prefix("- [X]"))
            .or_else(|| l.strip_prefix("* [ ]"))
            .or_else(|| l.strip_prefix("* [x]"));
        rest.is_some_and(|r| !r.trim().is_empty())
    })
}

#[cfg(test)]
mod tests {
    use super::has_checklist;

    #[test]
    fn checklist_needs_text() {
        assert!(has_checklist("## AC\n- [ ] QR tampil"));
        assert!(has_checklist("  - [x] selesai"));
        assert!(!has_checklist("- [ ] "));
        assert!(!has_checklist("- QR tampil"));
    }
}
