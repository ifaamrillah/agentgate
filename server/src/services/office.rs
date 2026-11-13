//! AI Office (M10): keadaan kantor dari data nyata dan kustom karakter.
//!
//! [`state`] murni membaca (GET): job terbuka, tiket menunggu review/keputusan, izin aksi yang
//! menunggu, runner online, dan persona aktif, hanya di project yang boleh dibuka pelaku.
//! Aturan penempatan:
//! - job `auto` claimed/running → persona "Bekerja" di ruangan project-nya; `paused`/`quota` →
//!   "Dijeda"; job `interactive` running → Ruang rapat bersama pemilik runner;
//! - izin aksi menunggu pada job itu → "Butuh approval" dengan teks izin;
//! - tiket `review` (job terakhir stage itu sukses) → "Butuh approval"; `decision` → "Butuh
//!   keputusan", persona = persona job terakhir stage itu;
//! - persona tanpa karakter lain → Pantry ("Menganggur");
//! - manusia = pemilik runner online yang memegang job terbuka, duduk di ruangan tempat job-nya
//!   paling banyak (Ruang rapat bila sedang sesi interaktif).
use std::collections::{HashMap, HashSet};

use agentgate_core::runner::protocol::{JobMode, JobStatus, PermissionStatus, StepState};
use sea_orm::{
    ActiveEnum, ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter,
    QueryOrder, Set, TransactionTrait,
};
use uuid::Uuid;

use crate::models::{
    job_permissions, jobs,
    office::{self, Char, CharStatus, Office, OfficeRunner, Who},
    office_looks::{self, Look, LookSubject},
    permission::Permission,
    personas, project_access, projects, runners,
    tickets::{self, StageState},
};
use crate::services::{
    audit::{self, AuditAction, AuditSubject},
    tickets::TicketFailure,
    workspaces::{Acting, lock_as},
};

/// Maks. tiket menunggu review/keputusan per project yang ditampilkan.
const WAITING_PER_PROJECT: usize = 12;

fn steps_of(job: &jobs::Model) -> (String, usize, usize) {
    let steps = job.steps();
    let done = steps.iter().filter(|s| s.state == StepState::Ok).count();
    let now = steps
        .iter()
        .find(|s| s.state == StepState::Now)
        .map(|s| s.name.clone())
        .unwrap_or_default();
    let step = if now.is_empty() { done } else { done + 1 };
    (now, step.min(steps.len()), steps.len())
}

/// Kumpulan sementara per pemilik runner: job per ruangan, kode tiket, ada job interaktif, runner.
type HumanAcc = (HashMap<String, usize>, Vec<String>, bool, Uuid);

/// Keadaan kantor untuk pelaku.
///
/// # Errors
///
/// Error database.
#[allow(clippy::too_many_lines)]
pub async fn state(db: &DatabaseConnection, acting: &Acting<'_>) -> Result<Office, DbErr> {
    let wid = acting.workspace_id();
    let projects = projects::Model::accessible_for(db, &acting.accessor()).await?;
    let project_ids: Vec<Uuid> = projects.iter().map(|p| p.id).collect();
    let key_of: HashMap<Uuid, &projects::Model> = projects.iter().map(|p| (p.id, p)).collect();
    let mut chars: Vec<Char> = Vec::new();

    let open = if project_ids.is_empty() {
        Vec::new()
    } else {
        jobs::Entity::find()
            .filter(jobs::Column::WorkspaceId.eq(wid.get()))
            .filter(jobs::Column::ProjectId.is_in(project_ids.clone()))
            .filter(jobs::Column::Status.is_in([
                JobStatus::Claimed.as_str(),
                JobStatus::Running.as_str(),
                JobStatus::Paused.as_str(),
                JobStatus::Quota.as_str(),
            ]))
            .order_by_asc(jobs::Column::CreatedAt)
            .all(db)
            .await?
    };
    let waiting = if project_ids.is_empty() {
        Vec::new()
    } else {
        tickets::Entity::find()
            .filter(tickets::Column::WorkspaceId.eq(wid.get()))
            .filter(tickets::Column::ProjectId.is_in(project_ids.clone()))
            .filter(tickets::Column::DeletedAt.is_null())
            .filter(tickets::Column::StageState.is_in([
                StageState::Review.to_value(),
                StageState::Decision.to_value(),
            ]))
            .order_by_asc(tickets::Column::UpdatedAt)
            .all(db)
            .await?
    };
    let mut ticket_ids: HashSet<Uuid> = open.iter().filter_map(|j| j.ticket_id).collect();
    ticket_ids.extend(waiting.iter().map(|t| t.id));
    let ticket_map: HashMap<Uuid, tickets::Model> = if ticket_ids.is_empty() {
        HashMap::new()
    } else {
        tickets::Entity::find()
            .filter(tickets::Column::WorkspaceId.eq(wid.get()))
            .filter(tickets::Column::Id.is_in(ticket_ids))
            .all(db)
            .await?
            .into_iter()
            .map(|t| (t.id, t))
            .collect()
    };
    let all_runners = runners::Entity::find()
        .filter(runners::Column::WorkspaceId.eq(wid.get()))
        .filter(runners::Column::RevokedAt.is_null())
        .order_by_desc(runners::Column::LastSeenAt)
        .all(db)
        .await?;
    let runner_map: HashMap<Uuid, &runners::Model> =
        all_runners.iter().map(|r| (r.id, r)).collect();
    let pending: Vec<job_permissions::Model> = if open.is_empty() {
        Vec::new()
    } else {
        job_permissions::Entity::find()
            .filter(job_permissions::Column::WorkspaceId.eq(wid.get()))
            .filter(job_permissions::Column::Status.eq(PermissionStatus::Pending.as_str()))
            .filter(job_permissions::Column::JobId.is_in(open.iter().map(|j| j.id)))
            .order_by_asc(job_permissions::Column::CreatedAt)
            .all(db)
            .await?
    };

    // Job terbuka.
    for job in &open {
        let Some(project) = key_of.get(&job.project_id) else {
            continue;
        };
        let ticket = job.ticket_id.and_then(|t| ticket_map.get(&t));
        if ticket.is_some_and(tickets::Model::is_trashed) {
            continue;
        }
        let interactive = job.mode() == JobMode::Interactive;
        let perm = pending.iter().find(|p| p.job_id == job.id);
        let status = if perm.is_some() {
            CharStatus::Review
        } else {
            match job.status() {
                JobStatus::Paused | JobStatus::Quota => CharStatus::Paused,
                _ if interactive => CharStatus::Meeting,
                _ => CharStatus::Working,
            }
        };
        let (now, step, steps) = steps_of(job);
        let runner = job.runner_id.and_then(|r| runner_map.get(&r));
        chars.push(Char {
            id: format!("j-{}", job.id.simple()),
            who: Who::Persona(job.persona.clone()),
            status,
            room: if interactive && status != CharStatus::Paused {
                office::MEET.into()
            } else {
                project.key.clone()
            },
            job_id: Some(job.id),
            ticket_id: job.ticket_id,
            ticket_code: ticket.map_or_else(|| "KB".into(), |t| t.code.clone()),
            ticket_title: ticket.map_or_else(|| job.prompt_note.clone(), |t| t.title.clone()),
            stage_key: job.stage_key.clone(),
            project_key: project.key.clone(),
            runner_id: job.runner_id,
            runner_name: runner.map(|r| r.name.clone()).unwrap_or_default(),
            doing: perm.map_or(now, |p| p.command.clone()),
            step,
            steps,
            since: job.started_at.or(job.claimed_at).map(|t| t.to_utc()),
            jobs: Vec::new(),
            permission_id: perm.map(|p| p.id),
        });
    }

    // Tiket menunggu review/keputusan (tanpa job terbuka di tiket itu).
    let busy: HashSet<Uuid> = open.iter().filter_map(|j| j.ticket_id).collect();
    let mut per_project: HashMap<Uuid, usize> = HashMap::new();
    for t in &waiting {
        if busy.contains(&t.id) || t.is_trashed() {
            continue;
        }
        let Some(project) = key_of.get(&t.project_id) else {
            continue;
        };
        let n = per_project.entry(t.project_id).or_default();
        if *n >= WAITING_PER_PROJECT {
            continue;
        }
        let decision = t.stage_state().ok() == Some(StageState::Decision);
        let last = jobs::Entity::find()
            .filter(jobs::Column::WorkspaceId.eq(wid.get()))
            .filter(jobs::Column::TicketId.eq(t.id))
            .filter(jobs::Column::StageKey.eq(t.stage_key.clone()))
            .order_by_desc(jobs::Column::CreatedAt)
            .one(db)
            .await?;
        let Some(last) = last else {
            continue;
        };
        if !decision && last.status() != JobStatus::Succeeded {
            continue;
        }
        *n += 1;
        let (_, _, steps) = steps_of(&last);
        chars.push(Char {
            id: format!("t-{}", t.id.simple()),
            who: Who::Persona(last.persona.clone()),
            status: if decision {
                CharStatus::Decision
            } else {
                CharStatus::Review
            },
            room: project.key.clone(),
            job_id: Some(last.id),
            ticket_id: Some(t.id),
            ticket_code: t.code.clone(),
            ticket_title: t.title.clone(),
            stage_key: t.stage_key.clone(),
            project_key: project.key.clone(),
            runner_id: last.runner_id,
            runner_name: last
                .runner_id
                .and_then(|r| runner_map.get(&r))
                .map(|r| r.name.clone())
                .unwrap_or_default(),
            doing: last.summary.lines().next().unwrap_or_default().to_string(),
            step: steps,
            steps,
            since: last.finished_at.map(|t| t.to_utc()),
            jobs: Vec::new(),
            permission_id: None,
        });
    }

    // Manusia: pemilik runner online yang memegang job terbuka.
    let mut humans: HashMap<Uuid, HumanAcc> = HashMap::new();
    for job in &open {
        let Some(runner) = job.runner_id.and_then(|r| runner_map.get(&r)) else {
            continue;
        };
        if !runner.is_online() || !key_of.contains_key(&job.project_id) {
            continue;
        }
        let entry = humans
            .entry(runner.user_id)
            .or_insert_with(|| (HashMap::new(), Vec::new(), false, runner.id));
        let room = chars
            .iter()
            .find(|c| c.job_id == Some(job.id))
            .map_or_else(String::new, |c| c.room.clone());
        if room == office::MEET {
            entry.2 = true;
        } else if !room.is_empty() {
            *entry.0.entry(room).or_default() += 1;
        }
        if let Some(c) = chars.iter().find(|c| c.job_id == Some(job.id)) {
            entry.1.push(c.ticket_code.clone());
        }
    }
    for (user, (rooms, codes, meeting, runner_id)) in humans {
        let room = if meeting {
            office::MEET.to_string()
        } else {
            let mut list: Vec<(String, usize)> = rooms.into_iter().collect();
            list.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            list.into_iter()
                .next()
                .map_or_else(|| office::PANTRY.to_string(), |(k, _)| k)
        };
        let runner = runner_map.get(&runner_id);
        chars.push(Char {
            id: format!("u-{}", user.simple()),
            who: Who::Human(user),
            status: if meeting {
                CharStatus::Meeting
            } else {
                CharStatus::Online
            },
            project_key: if room == office::MEET {
                String::new()
            } else {
                room.clone()
            },
            room,
            job_id: None,
            ticket_id: None,
            ticket_code: String::new(),
            ticket_title: String::new(),
            stage_key: String::new(),
            runner_id: Some(runner_id),
            runner_name: runner.map(|r| r.name.clone()).unwrap_or_default(),
            doing: String::new(),
            step: 0,
            steps: 0,
            since: None,
            jobs: codes,
            permission_id: None,
        });
    }

    // Persona tanpa pekerjaan → Pantry.
    let active = personas::Model::active_in(db, wid).await?;
    let used: HashSet<String> = chars
        .iter()
        .filter_map(|c| c.persona_key().map(str::to_string))
        .collect();
    for p in &active {
        if used.contains(&p.key) {
            continue;
        }
        chars.push(Char {
            id: format!("p-{}", p.key),
            who: Who::Persona(p.key.clone()),
            status: CharStatus::Idle,
            room: office::PANTRY.into(),
            job_id: None,
            ticket_id: None,
            ticket_code: String::new(),
            ticket_title: String::new(),
            stage_key: String::new(),
            project_key: String::new(),
            runner_id: None,
            runner_name: String::new(),
            doing: String::new(),
            step: 0,
            steps: 0,
            since: None,
            jobs: Vec::new(),
            permission_id: None,
        });
    }

    // Denah: project paling ramai dulu.
    let busy_of = |key: &str| chars.iter().filter(|c| c.room == key).count();
    let (rooms, hidden) = office::plan_rooms(
        projects
            .iter()
            .map(|p| {
                (
                    p.key.clone(),
                    p.name.clone(),
                    p.color.clone().unwrap_or_default(),
                    busy_of(&p.key),
                )
            })
            .collect(),
    );
    // Karakter di project yang tidak muat di denah tetap ada di data (panel "Perlu tindakan"),
    // kanvas hanya menggambar ruangan yang ada di denah.
    let runners_view = all_runners
        .iter()
        .map(|r| {
            let jobs: Vec<String> = open
                .iter()
                .filter(|j| j.runner_id == Some(r.id) && key_of.contains_key(&j.project_id))
                .filter_map(|j| chars.iter().find(|c| c.job_id == Some(j.id)))
                .map(|c| c.ticket_code.clone())
                .collect();
            OfficeRunner {
                id: r.id,
                name: r.name.clone(),
                client: r.client.clone(),
                owner: r.user_id,
                online: r.is_online(),
                last_seen: r.last_seen_at.map(|t| t.to_utc()),
                busy: !jobs.is_empty(),
                jobs,
            }
        })
        .collect();
    let looks = office_looks::Model::of_workspace(db, wid)
        .await?
        .into_iter()
        .map(|m| {
            let look = m.look();
            (m.subject, m.subject_key, look)
        })
        .collect();
    Ok(Office {
        rooms,
        chars,
        runners: runners_view,
        looks,
        hidden_projects: hidden,
    })
}

/// Pelaku boleh mengubah tampilan karakter ini? Persona: `ManageTicketSettings` (sama dengan
/// kelola persona). Manusia: dirinya sendiri atau Admin workspace.
#[must_use]
pub fn can_customize(acting: &Acting<'_>, who: &Who) -> bool {
    match who {
        Who::Persona(_) => acting.role.has(Permission::ManageTicketSettings),
        Who::Human(u) => *u == acting.user.id || project_access::is_admin_everywhere(acting.role),
    }
}

/// Simpan tampilan karakter (upsert per workspace + subjek) dan catat audit.
///
/// # Errors
///
/// `NotFound` bila pelaku tidak berhak atau persona tidak ada; `Field("look", ..)` bila isian
/// tidak valid.
pub async fn update_look(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    who: &Who,
    input: Look,
) -> Result<Look, TicketFailure> {
    let look = input
        .normalized()
        .map_err(|code| TicketFailure::field("look", code))?;
    let txn = db.begin().await?;
    lock_as(&txn, acting, |_| true)
        .await?
        .ok_or(TicketFailure::NotFound)?;
    if !can_customize(acting, who) {
        return Err(TicketFailure::NotFound);
    }
    let wid = acting.workspace_id();
    let (subject, key) = match who {
        Who::Persona(k) => {
            personas::Model::find_active(&txn, wid, k.as_str())
                .await?
                .ok_or(TicketFailure::NotFound)?;
            (LookSubject::Persona, k.clone())
        }
        Who::Human(u) => (LookSubject::User, u.to_string()),
    };
    let existing = office_looks::Entity::find()
        .filter(office_looks::Column::WorkspaceId.eq(wid.get()))
        .filter(office_looks::Column::Subject.eq(subject.to_value()))
        .filter(office_looks::Column::SubjectKey.eq(key.clone()))
        .one(&txn)
        .await?;
    let before = existing
        .as_ref()
        .map(office_looks::Model::look)
        .unwrap_or_default();
    if before == look {
        txn.rollback().await?;
        return Ok(look);
    }
    let value = serde_json::to_value(&look).unwrap_or_default();
    if let Some(row) = existing {
        let mut m: office_looks::ActiveModel = row.into();
        m.look = Set(value.clone());
        m.updated_by = Set(Some(acting.user.id));
        m.update(&txn).await?;
    } else {
        office_looks::ActiveModel {
            workspace_id: Set(wid.get()),
            subject: Set(subject.to_value()),
            subject_key: Set(key.clone()),
            look: Set(value.clone()),
            updated_by: Set(Some(acting.user.id)),
            ..Default::default()
        }
        .insert(&txn)
        .await?;
    }
    audit::record(
        &txn,
        acting.actor().audit(
            AuditAction::OfficeLookUpdated,
            (AuditSubject::Workspace, wid.get()),
            serde_json::json!({
                "subject": subject.to_value(),
                "key": key,
                "from": serde_json::to_value(before).unwrap_or_default(),
                "to": value,
            }),
        ),
    )
    .await?;
    txn.commit().await?;
    Ok(look)
}

/// Karakter `id` dari keadaan kantor pelaku (`None` = tidak ada / tidak boleh dilihat).
///
/// # Errors
///
/// Error database.
pub async fn find_char(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: &str,
) -> Result<Option<(Office, Char)>, DbErr> {
    let office = state(db, acting).await?;
    let found = office.char(id).cloned();
    Ok(found.map(|c| (office, c)))
}
