//! Artifact tiket dan versinya (spec "Artifact & versi"): dokumen per stage (PRD, desain, plan…).
//! Setiap simpan = versi baru (tidak menimpa). Versi yang disimpan saat stage-nya menunggu review
//! langsung `pending`; selain itu `draft`. Approve/reject stage memutuskan versi terbaru artifact
//! stage itu (lihat [`super::stages`]).

use sea_orm::DatabaseConnection;
use uuid::Uuid;

use super::{TicketFailure, Working, stages::Flow};
use crate::models::{
    artifact_versions::{self, VersionStatus},
    permission::Permission,
    ticket_artifacts::{self, BODY_MAX_LEN, BODY_TOO_LONG, NAME_INVALID, NAME_TAKEN, UNCHANGED},
    ticket_events::EventKind,
    tickets::StageState,
};

fn status_for(ticket: &crate::models::tickets::Model, stage_key: &str) -> VersionStatus {
    if ticket.stage_key == stage_key && ticket.stage_state().ok() == Some(StageState::Review) {
        VersionStatus::Pending
    } else {
        VersionStatus::Draft
    }
}

fn check_body(body: &str) -> Result<(), TicketFailure> {
    if body.chars().count() > BODY_MAX_LEN {
        return Err(TicketFailure::field("body", BODY_TOO_LONG));
    }
    Ok(())
}

/// Buat artifact di stage sekarang. `name` kosong = nama output stage. Isi awal = template output
/// stage (bila nama sama dengan output stage) atau kosong; versi 1 langsung dibuat.
///
/// # Errors
///
/// `Invalid` (`artifact-name-invalid`, `artifact-name-taken`), `NotFound`, atau error database.
pub async fn create(
    db: &DatabaseConnection,
    acting: &crate::services::workspaces::Acting<'_>,
    ticket_id: Uuid,
    name: &str,
) -> Result<ticket_artifacts::Model, TicketFailure> {
    let work = Working::begin(db, acting, ticket_id, Permission::EditProject, false).await?;
    let flow = Flow::load(&work.txn, work.wid, &work.ticket).await?;
    let rule = flow.rule(&work.ticket.stage_key).cloned();
    let raw = if name.trim().is_empty() {
        rule.as_ref().map(|r| r.output.clone()).unwrap_or_default()
    } else {
        name.to_string()
    };
    let name = ticket_artifacts::parse_name(&raw)
        .ok_or_else(|| TicketFailure::field("name", NAME_INVALID))?;
    if ticket_artifacts::Model::find_named(&work.txn, work.wid, ticket_id, &name)
        .await?
        .is_some()
    {
        return Err(TicketFailure::field("name", NAME_TAKEN));
    }
    let body = rule
        .as_ref()
        .filter(|r| r.output.eq_ignore_ascii_case(&name))
        .map(|r| r.output_template.clone())
        .unwrap_or_default();
    let artifact = ticket_artifacts::ActiveModel::create(
        &work.txn,
        work.wid,
        ticket_id,
        &work.ticket.stage_key,
        &name,
        acting.user.id,
    )
    .await?;
    artifact_versions::ActiveModel::append(
        &work.txn,
        work.wid,
        artifact.id,
        &body,
        status_for(&work.ticket, &artifact.stage_key),
        acting.user.id,
    )
    .await?;
    work.event(
        acting.user.id,
        EventKind::ArtifactSaved,
        serde_json::json!({ "filename": name, "version": 1 }),
    )
    .await?;
    work.txn.commit().await?;
    Ok(artifact)
}

/// Simpan versi baru artifact. Isi yang sama dengan versi terakhir ditolak (`artifact-unchanged`).
///
/// # Errors
///
/// `Invalid`, `NotFound`, atau error database.
pub async fn save(
    db: &DatabaseConnection,
    acting: &crate::services::workspaces::Acting<'_>,
    ticket_id: Uuid,
    artifact_id: Uuid,
    body: &str,
) -> Result<artifact_versions::Model, TicketFailure> {
    check_body(body)?;
    let work = Working::begin(db, acting, ticket_id, Permission::EditProject, false).await?;
    let artifact = ticket_artifacts::Model::find_of(&work.txn, work.wid, ticket_id, artifact_id)
        .await?
        .ok_or(TicketFailure::NotFound)?;
    let body = body.replace("\r\n", "\n");
    if artifact_versions::Model::latest(&work.txn, work.wid, artifact.id)
        .await?
        .is_some_and(|v| v.body == body)
    {
        return Err(TicketFailure::field("body", UNCHANGED));
    }
    let version = artifact_versions::ActiveModel::append(
        &work.txn,
        work.wid,
        artifact.id,
        &body,
        status_for(&work.ticket, &artifact.stage_key),
        acting.user.id,
    )
    .await?;
    work.event(
        acting.user.id,
        EventKind::ArtifactSaved,
        serde_json::json!({ "filename": artifact.name, "version": version.number }),
    )
    .await?;
    work.txn.commit().await?;
    Ok(version)
}

/// Satu baris diff baris-per-baris.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DiffLine {
    /// `same`, `add`, `del`.
    pub kind: &'static str,
    pub text: String,
}

/// Diff baris sederhana (LCS) antara dua isi. Untuk dokumen besar (> 2.000 baris salah satunya)
/// hanya menampilkan semua baris lama sebagai dihapus dan baru sebagai ditambah.
#[must_use]
pub fn diff(old: &str, new: &str) -> Vec<DiffLine> {
    let a: Vec<&str> = old.lines().collect();
    let b: Vec<&str> = new.lines().collect();
    if a.len() > 2_000 || b.len() > 2_000 {
        return a
            .iter()
            .map(|l| DiffLine {
                kind: "del",
                text: (*l).to_string(),
            })
            .chain(b.iter().map(|l| DiffLine {
                kind: "add",
                text: (*l).to_string(),
            }))
            .collect();
    }
    let (n, m) = (a.len(), b.len());
    let mut lcs = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut out = Vec::new();
    while i < n && j < m {
        if a[i] == b[j] {
            out.push(DiffLine {
                kind: "same",
                text: a[i].to_string(),
            });
            i += 1;
            j += 1;
        } else if lcs[i + 1][j] >= lcs[i][j + 1] {
            out.push(DiffLine {
                kind: "del",
                text: a[i].to_string(),
            });
            i += 1;
        } else {
            out.push(DiffLine {
                kind: "add",
                text: b[j].to_string(),
            });
            j += 1;
        }
    }
    out.extend(a[i..].iter().map(|l| DiffLine {
        kind: "del",
        text: (*l).to_string(),
    }));
    out.extend(b[j..].iter().map(|l| DiffLine {
        kind: "add",
        text: (*l).to_string(),
    }));
    out
}

#[cfg(test)]
mod tests {
    use super::diff;

    #[test]
    fn diff_marks_added_and_removed_lines() {
        let d = diff("a\nb\nc", "a\nx\nc");
        let kinds: Vec<&str> = d.iter().map(|l| l.kind).collect();
        assert_eq!(kinds, ["same", "del", "add", "same"]);
        assert_eq!(d[2].text, "x");
    }
}
