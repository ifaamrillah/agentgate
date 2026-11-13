//! Tool planning (M9): `ag_planning`.
//!
//! - `estimate`: simpan usulan estimasi AI tiket (story point + jam AI/manusia per stage + dasar).
//!   Bila tiket belum punya story point, poin AI langsung dipakai.
//! - `sprint`: sprint aktif/rencana project beserta tiket, kapasitas, dan velocity.
//! - `release_notes`: tanpa `body` membaca rilis (tiket + notes sekarang); dengan `body`
//!   menyimpan versi baru release notes (sumber AI).
//!
//! Membaca butuh `ViewProject`, menulis `EditProject`.
use sea_orm::DatabaseConnection;
use serde_json::{Value, json};

use super::{Args, Session, ToolError, find_ticket, required};
use crate::{
    models::{
        release_note_versions::VersionSource,
        sprints::{self, SprintState},
        ticket_estimates::EstimateSource,
        ticket_types,
    },
    services::planning::{
        self, Failure,
        estimates::{self, EstimateInput},
        releases, sprints as sprint_service,
    },
};

fn failure(e: Failure) -> ToolError {
    match e {
        Failure::NotFound => ToolError::new("Tidak ditemukan atau Anda tidak punya izin mengubah."),
        Failure::Invalid(errors) => {
            let text = serde_json::to_string(&errors).unwrap_or_default();
            let msg = if text.contains(planning::ESTIMATE_INVALID) {
                "Estimasi tidak valid: `points` 0–1000, jam per stage 0–1000, `stage` = kunci stage workflow."
            } else if text.contains(planning::UNCHANGED) {
                "Isi release notes sama dengan versi sekarang; tidak ada yang disimpan."
            } else if text.contains(planning::NOTES_TOO_LONG) {
                "`body` terlalu panjang (maks. 100.000 karakter)."
            } else {
                "Permintaan tidak valid."
            };
            ToolError::new(msg)
        }
        Failure::Db(e) => e.into(),
    }
}

/// Jalankan `ag_planning`.
///
/// # Errors
///
/// [`ToolError`] berisi alasan yang bisa dibaca model.
pub async fn call(db: &DatabaseConnection, s: &Session, a: &Args) -> Result<Value, ToolError> {
    match a.action.as_str() {
        "estimate" => estimate(db, s, a).await,
        "sprint" => sprint(db, s, a).await,
        "release_notes" => release_notes(db, s, a).await,
        _ => Err(ToolError::new(
            "`action` harus estimate, sprint, atau release_notes.",
        )),
    }
}

async fn estimate(db: &DatabaseConnection, s: &Session, a: &Args) -> Result<Value, ToolError> {
    let (ticket, _) = find_ticket(db, s, required(&a.ticket, "ticket")?).await?;
    if a.stages.is_empty() && a.points.is_none() {
        return Err(ToolError::new("Isi `points` dan/atau `stages`."));
    }
    let input = EstimateInput {
        points: a.points,
        stages: a.stages.clone(),
        basis: a.basis.clone(),
    };
    let row = estimates::save(
        db,
        &s.acting(),
        ticket.id,
        EstimateSource::Ai,
        &input,
        a.persona.trim(),
    )
    .await
    .map_err(failure)?;
    let (ai, human) = row.totals();
    Ok(json!({
        "saved": true,
        "ticket": ticket.code,
        "points": row.points,
        "ai_hours": ai,
        "human_hours": human,
        "note": "Estimasi tersimpan sebagai usulan AI. Manusia bisa mengubahnya dari panel estimasi tiket.",
    }))
}

async fn sprint(db: &DatabaseConnection, s: &Session, a: &Args) -> Result<Value, ToolError> {
    let project = super::graph::project_of(db, s, a).await?;
    let wid = s.workspace.workspace_id();
    let done = sprint_service::velocity(db, wid, project.id, 6).await?;
    let avg = sprint_service::average_velocity(&done);
    let mut out = Vec::new();
    for sp in sprints::Model::of_project(db, wid, project.id)
        .await?
        .into_iter()
        .filter(|x| x.state() != SprintState::Completed)
    {
        let tickets: Vec<Value> = sprint_service::tickets_of(db, wid, sp.id)
            .await?
            .iter()
            .map(|t| json!({ "code": t.code, "title": t.title, "status": t.status, "stage": t.stage_key, "points": t.story_points }))
            .collect();
        out.push(json!({
            "name": sp.name,
            "state": sp.state,
            "goal": sp.goal,
            "starts_on": sp.starts_on.to_string(),
            "ends_on": sp.ends_on.to_string(),
            "capacity": sprint_service::capacity(&sp, avg),
            "tickets": tickets,
        }));
    }
    Ok(json!({
        "project": project.key,
        "velocity_average": avg,
        "velocity": done.iter().map(|x| json!({ "name": x.name, "committed": x.committed_points, "completed": x.completed_points })).collect::<Vec<_>>(),
        "sprints": out,
    }))
}

async fn release_notes(db: &DatabaseConnection, s: &Session, a: &Args) -> Result<Value, ToolError> {
    let project = super::graph::project_of(db, s, a).await?;
    let wid = s.workspace.workspace_id();
    let name = required(&a.release, "release")?;
    let release = releases::by_name(db, wid, project.id, name)
        .await?
        .ok_or_else(|| {
            ToolError::new(format!(
                "Rilis \"{name}\" tidak ditemukan di {}.",
                project.key
            ))
        })?;
    if a.body.trim().is_empty() {
        let types = ticket_types::Model::all_in(db, wid).await?;
        let tickets: Vec<Value> = releases::tickets_of(db, wid, release.id)
            .await?
            .iter()
            .map(|t| {
                json!({
                    "code": t.code,
                    "title": t.title,
                    "type": types.iter().find(|ty| ty.id == t.type_id).map(|ty| ty.key.clone()),
                    "status": t.status,
                    "description": t.description.chars().take(400).collect::<String>(),
                })
            })
            .collect();
        return Ok(json!({
            "release": release.name,
            "description": release.description,
            "status": release.status,
            "target_on": release.target_on.map(|d| d.to_string()),
            "notes": release.notes,
            "notes_version": release.notes_version,
            "tickets": tickets,
            "hint": "Kirim ulang dengan `body` (Markdown) untuk menyimpan versi baru release notes.",
        }));
    }
    let saved = releases::save_notes(
        db,
        &s.acting(),
        release.id,
        &a.body,
        VersionSource::Ai,
        if a.persona.trim().is_empty() {
            releases::NOTES_PERSONA
        } else {
            a.persona.trim()
        },
    )
    .await
    .map_err(failure)?;
    Ok(json!({
        "saved": true,
        "release": saved.name,
        "version": saved.notes_version,
        "note": "Release notes tersimpan sebagai versi baru. Manusia meninjau dan menerbitkannya.",
    }))
}
