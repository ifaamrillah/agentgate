//! Rilis (Fix version) dan release notes.
//!
//! - Rilis: Rencana → Belum dirilis (otomatis saat ada tiket yang mulai dikerjakan, atau manual)
//!   → Dirilis → Diarsipkan. Rilis yang dirilis/diarsipkan tidak bisa dipilih di tiket.
//! - "Rilis sekarang": tiket yang belum selesai dipindah ke rilis lain atau dilepas.
//! - Release notes Markdown berversi: setiap simpan (manual atau AI lewat
//!   `ag_planning release_notes`) menambah versi; versi lama bisa dipulihkan.
//! - "Buat dengan AI": job knowledge tanpa tiket untuk persona `writer`
//!   (`stage_key = release_notes`); agent menyimpan hasilnya lewat MCP.
use agentgate_core::runner::protocol::{JobKind, JobMode, JobStatus};
use chrono::NaiveDate;
use sea_orm::{
    ActiveEnum, ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait,
    DatabaseConnection, DbErr, EntityTrait, IntoActiveModel, QueryFilter, SqlErr, TransactionTrait,
};
use uuid::Uuid;

use super::{
    DATES_INVALID, Failure, NAME_INVALID, NAME_TAKEN, NOTES_TOO_LONG, RELEASE_STATE,
    TARGET_INVALID, TEXT_TOO_LONG, UNCHANGED,
};
use crate::{
    models::{
        jobs,
        permission::Permission,
        release_note_versions::{self, VersionSource},
        releases::{
            self, DESCRIPTION_MAX_LEN, NAME_MAX_LEN, NOTES_MAX_LEN, NotesSource, ReleaseStatus,
        },
        ticket_events::{self, EventKind},
        tickets,
        workspaces::WorkspaceId,
    },
    services::{tickets::access_in, workspaces::Acting},
};

/// Persona penulis release notes.
pub const NOTES_PERSONA: &str = "writer";
/// `stage_key` job release notes.
pub const NOTES_STAGE: &str = "release_notes";

/// Isian rilis mentah dari form.
#[derive(Debug, Clone, Default)]
pub struct ReleaseInput {
    pub name: String,
    pub description: String,
    /// `YYYY-MM-DD` atau kosong.
    pub starts_on: String,
    pub target_on: String,
}

struct Valid {
    name: String,
    description: String,
    starts_on: Option<NaiveDate>,
    target_on: Option<NaiveDate>,
}

fn validate(input: &ReleaseInput) -> Result<Valid, Failure> {
    let name = input.name.trim().to_string();
    if name.is_empty() || name.chars().count() > NAME_MAX_LEN {
        return Err(Failure::field("name", NAME_INVALID));
    }
    let description = input.description.trim().to_string();
    if description.chars().count() > DESCRIPTION_MAX_LEN {
        return Err(Failure::field("description", TEXT_TOO_LONG));
    }
    let date = |raw: &str, field: &'static str| -> Result<Option<NaiveDate>, Failure> {
        match raw.trim() {
            "" => Ok(None),
            v => NaiveDate::parse_from_str(v, "%Y-%m-%d")
                .map(Some)
                .map_err(|_| Failure::field(field, DATES_INVALID)),
        }
    };
    let starts_on = date(&input.starts_on, "starts_on")?;
    let target_on = date(&input.target_on, "target_on")?;
    if let (Some(s), Some(t)) = (starts_on, target_on)
        && t < s
    {
        return Err(Failure::field("target_on", DATES_INVALID));
    }
    Ok(Valid {
        name,
        description,
        starts_on,
        target_on,
    })
}

fn taken(e: DbErr) -> Failure {
    if matches!(e.sql_err(), Some(SqlErr::UniqueConstraintViolation(_))) {
        Failure::field("name", NAME_TAKEN)
    } else {
        Failure::Db(e)
    }
}

/// Buat rilis (keadaan Rencana).
///
/// # Errors
///
/// Validasi, `NotFound`, error database.
pub async fn create(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    project_id: Uuid,
    input: &ReleaseInput,
) -> Result<releases::Model, Failure> {
    let v = validate(input)?;
    access_in(db, acting, project_id, Permission::EditProject).await?;
    releases::ActiveModel {
        workspace_id: Set(acting.workspace_id().get()),
        project_id: Set(project_id),
        name: Set(v.name),
        description: Set(v.description),
        status: Set(ReleaseStatus::Planned.to_value()),
        starts_on: Set(v.starts_on),
        target_on: Set(v.target_on),
        notes: Set(String::new()),
        notes_version: Set(0),
        notes_source: Set(NotesSource::None.to_value()),
        notes_published: Set(false),
        created_by: Set(Some(acting.user.id)),
        ..Default::default()
    }
    .insert(db)
    .await
    .map_err(taken)
}

/// Rilis yang boleh dibuka (`ViewProject`).
///
/// # Errors
///
/// `NotFound`, error database.
pub async fn find_visible<C: ConnectionTrait>(
    db: &C,
    acting: &Acting<'_>,
    id: Uuid,
) -> Result<releases::Model, Failure> {
    let r = releases::Model::find_in(db, acting.workspace_id(), id)
        .await?
        .ok_or(Failure::NotFound)?;
    access_in(db, acting, r.project_id, Permission::ViewProject).await?;
    Ok(r)
}

async fn editable<C: ConnectionTrait>(
    db: &C,
    acting: &Acting<'_>,
    id: Uuid,
) -> Result<releases::Model, Failure> {
    let r = releases::Model::find_in(db, acting.workspace_id(), id)
        .await?
        .ok_or(Failure::NotFound)?;
    access_in(db, acting, r.project_id, Permission::EditProject).await?;
    Ok(r)
}

/// Ubah nama, deskripsi, tanggal.
///
/// # Errors
///
/// Validasi, `NotFound`, error database.
pub async fn update(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
    input: &ReleaseInput,
) -> Result<releases::Model, Failure> {
    let v = validate(input)?;
    let r = editable(db, acting, id).await?;
    let mut m = r.into_active_model();
    m.name = Set(v.name);
    m.description = Set(v.description);
    m.starts_on = Set(v.starts_on);
    m.target_on = Set(v.target_on);
    m.update(db).await.map_err(taken)
}

/// Hapus rilis (tiketnya kehilangan Fix version).
///
/// # Errors
///
/// `NotFound`, error database.
pub async fn delete(db: &DatabaseConnection, acting: &Acting<'_>, id: Uuid) -> Result<(), Failure> {
    let r = editable(db, acting, id).await?;
    releases::Entity::delete_by_id(r.id).exec(db).await?;
    Ok(())
}

/// Tiket aktif (bukan sampah) di rilis.
///
/// # Errors
///
/// Error database.
pub async fn tickets_of<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    release_id: Uuid,
) -> Result<Vec<tickets::Model>, DbErr> {
    use sea_orm::QueryOrder;
    tickets::Entity::find()
        .filter(tickets::Column::WorkspaceId.eq(wid.get()))
        .filter(tickets::Column::ReleaseId.eq(release_id))
        .filter(tickets::Column::DeletedAt.is_null())
        .order_by_asc(tickets::Column::BacklogRank)
        .all(db)
        .await
}

/// Keadaan efektif: Rencana yang tiketnya sudah ada yang dikerjakan tampil Belum dirilis.
#[must_use]
pub fn effective_status(r: &releases::Model, tickets: &[tickets::Model]) -> ReleaseStatus {
    match r.status() {
        ReleaseStatus::Planned
            if tickets
                .iter()
                .any(|t| t.status != "backlog" && t.status != "todo") =>
        {
            ReleaseStatus::Unreleased
        }
        s => s,
    }
}

/// Rilis sekarang: tiket belum selesai dipindah ke `move_to` (rilis lain project yang sama,
/// belum dirilis) atau dilepas (`None`).
///
/// # Errors
///
/// `planning-release-state`, `planning-target-invalid`, `NotFound`, error database.
pub async fn release_now(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
    move_to: Option<Uuid>,
) -> Result<releases::Model, Failure> {
    let wid = acting.workspace_id();
    let txn = db.begin().await?;
    let r = editable(&txn, acting, id).await?;
    if matches!(
        r.status(),
        ReleaseStatus::Released | ReleaseStatus::Archived
    ) {
        return Err(Failure::field("release", RELEASE_STATE));
    }
    let target = match move_to {
        Some(t) => Some(
            releases::Model::find_in(&txn, wid, t)
                .await?
                .filter(|x| {
                    x.project_id == r.project_id
                        && x.id != r.id
                        && !matches!(
                            x.status(),
                            ReleaseStatus::Released | ReleaseStatus::Archived
                        )
                })
                .ok_or_else(|| Failure::field("target", TARGET_INVALID))?,
        ),
        None => None,
    };
    for t in tickets_of(&txn, wid, r.id).await? {
        if t.status == "done" {
            continue;
        }
        let mut m = t.clone().into_active_model();
        m.release_id = Set(target.as_ref().map(|x| x.id));
        m.update(&txn).await?;
        ticket_events::ActiveModel::record(
            &txn,
            wid,
            t.id,
            Some(acting.user.id),
            EventKind::Updated,
            serde_json::json!({
                "release": { "from": r.name, "to": target.as_ref().map(|x| x.name.clone()) }
            }),
        )
        .await?;
    }
    let mut m = r.into_active_model();
    m.status = Set(ReleaseStatus::Released.to_value());
    m.released_at = Set(Some(chrono::Utc::now().into()));
    let r = m.update(&txn).await?;
    txn.commit().await?;
    Ok(r)
}

/// Ubah keadaan manual: `archived`, atau kembali `unreleased` (batal rilis / buka arsip).
///
/// # Errors
///
/// `planning-release-state`, `NotFound`, error database.
pub async fn set_status(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
    status: ReleaseStatus,
) -> Result<releases::Model, Failure> {
    let r = editable(db, acting, id).await?;
    if status == ReleaseStatus::Released {
        return Err(Failure::field("release", RELEASE_STATE));
    }
    let mut m = r.into_active_model();
    m.status = Set(status.to_value());
    if status != ReleaseStatus::Archived {
        m.released_at = Set(None);
    }
    Ok(m.update(db).await?)
}

/// Simpan versi baru release notes (transaksi pemanggil, izin sudah dicek).
async fn save_notes_in<C: ConnectionTrait>(
    db: &C,
    acting: &Acting<'_>,
    r: releases::Model,
    body: &str,
    source: VersionSource,
    persona: &str,
) -> Result<releases::Model, Failure> {
    let body = body.trim().to_string();
    if body.chars().count() > NOTES_MAX_LEN {
        return Err(Failure::field("notes", NOTES_TOO_LONG));
    }
    if body == r.notes.trim() {
        return Err(Failure::field("notes", UNCHANGED));
    }
    let number = r.notes_version + 1;
    release_note_versions::ActiveModel {
        workspace_id: Set(acting.workspace_id().get()),
        release_id: Set(r.id),
        number: Set(number),
        body: Set(body.clone()),
        source: Set(source.to_value()),
        author_id: Set(Some(acting.user.id)),
        persona: Set(persona.chars().take(60).collect()),
        ..Default::default()
    }
    .insert(db)
    .await?;
    let mut m = r.into_active_model();
    m.notes = Set(body);
    m.notes_version = Set(number);
    m.notes_source = Set(match source {
        VersionSource::Ai => NotesSource::Ai,
        VersionSource::Manual => NotesSource::Manual,
    }
    .to_value());
    m.notes_updated_at = Set(Some(chrono::Utc::now().into()));
    Ok(m.update(db).await?)
}

/// Simpan release notes (manual dari editor, atau AI lewat MCP).
///
/// # Errors
///
/// `planning-notes-too-long`, `planning-unchanged`, `NotFound`, error database.
pub async fn save_notes(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
    body: &str,
    source: VersionSource,
    persona: &str,
) -> Result<releases::Model, Failure> {
    let txn = db.begin().await?;
    let r = editable(&txn, acting, id).await?;
    let r = save_notes_in(&txn, acting, r, body, source, persona).await?;
    txn.commit().await?;
    Ok(r)
}

/// Pulihkan versi lama sebagai versi baru.
///
/// # Errors
///
/// `NotFound`, `planning-unchanged`, error database.
pub async fn restore_notes(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
    number: i32,
) -> Result<releases::Model, Failure> {
    let txn = db.begin().await?;
    let r = editable(&txn, acting, id).await?;
    let v = release_note_versions::Entity::find()
        .filter(release_note_versions::Column::WorkspaceId.eq(acting.workspace_id().get()))
        .filter(release_note_versions::Column::ReleaseId.eq(r.id))
        .filter(release_note_versions::Column::Number.eq(number))
        .one(&txn)
        .await?
        .ok_or(Failure::NotFound)?;
    let r = save_notes_in(&txn, acting, r, &v.body, v.source(), &v.persona).await?;
    txn.commit().await?;
    Ok(r)
}

/// Tandai release notes diterbitkan/ditarik (tampil di halaman rilis untuk pembaca).
///
/// # Errors
///
/// `NotFound`, error database.
pub async fn publish_notes(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
    published: bool,
) -> Result<releases::Model, Failure> {
    let r = editable(db, acting, id).await?;
    let mut m = r.into_active_model();
    m.notes_published = Set(published);
    Ok(m.update(db).await?)
}

/// Prompt job release notes AI.
#[must_use]
pub fn notes_prompt(project_key: &str, release: &releases::Model) -> String {
    format!(
        "Anda adalah persona {NOTES_PERSONA} di Agent Gate. Tulis release notes untuk rilis \"{name}\" \
         di project {project_key}.\n\n\
         1. Panggil ag_planning dengan action=release_notes, project={project_key}, release=\"{name}\" \
            tanpa `body` untuk membaca daftar tiket rilis (judul, tipe, deskripsi singkat) dan isi \
            release notes sekarang.\n\
         2. Tulis release notes Markdown dalam bahasa Indonesia untuk pengguna akhir: ringkasan, \
            lalu bagian Fitur baru, Perbaikan, dan Perubahan lain. Sebut kode tiket di akhir butir.\n\
         3. Simpan dengan ag_planning action=release_notes, release=\"{name}\", body=<Markdown>.\n\
         Jangan mengubah tiket.",
        name = release.name,
    )
}

/// Minta AI menulis release notes: job knowledge tanpa tiket untuk persona `writer`.
/// Satu job terbuka per rilis.
///
/// # Errors
///
/// `NotFound`, `planning-release-state` (job masih berjalan), error database.
pub async fn request_ai_notes(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
    project_key: &str,
) -> Result<jobs::Model, Failure> {
    let wid = acting.workspace_id();
    let r = editable(db, acting, id).await?;
    let prompt = notes_prompt(project_key, &r);
    let open = jobs::Entity::find()
        .filter(jobs::Column::WorkspaceId.eq(wid.get()))
        .filter(jobs::Column::ProjectId.eq(r.project_id))
        .filter(jobs::Column::StageKey.eq(NOTES_STAGE))
        .filter(jobs::Column::PromptNote.eq(prompt.clone()))
        .filter(
            jobs::Column::Status.is_in([JobStatus::Queued.as_str(), JobStatus::Running.as_str()]),
        )
        .one(db)
        .await?;
    if open.is_some() {
        return Err(Failure::field("release", RELEASE_STATE));
    }
    Ok(jobs::ActiveModel {
        workspace_id: Set(wid.get()),
        project_id: Set(r.project_id),
        ticket_id: Set(None),
        kind: Set(JobKind::Knowledge.as_str().into()),
        mode: Set(JobMode::Auto.as_str().into()),
        stage_key: Set(NOTES_STAGE.into()),
        persona: Set(NOTES_PERSONA.into()),
        status: Set(JobStatus::Queued.as_str().into()),
        prompt_note: Set(prompt),
        requested_by: Set(Some(acting.user.id)),
        steps: Set(serde_json::json!([])),
        ..Default::default()
    }
    .insert(db)
    .await?)
}

/// Job release notes yang masih terbuka untuk rilis ini.
///
/// # Errors
///
/// Error database.
pub async fn open_notes_job<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project_key: &str,
    r: &releases::Model,
) -> Result<Option<jobs::Model>, DbErr> {
    jobs::Entity::find()
        .filter(jobs::Column::WorkspaceId.eq(wid.get()))
        .filter(jobs::Column::ProjectId.eq(r.project_id))
        .filter(jobs::Column::StageKey.eq(NOTES_STAGE))
        .filter(jobs::Column::PromptNote.eq(notes_prompt(project_key, r)))
        .filter(
            jobs::Column::Status.is_in([JobStatus::Queued.as_str(), JobStatus::Running.as_str()]),
        )
        .one(db)
        .await
}

/// Cari rilis project berdasarkan nama (tanpa beda huruf besar/kecil).
///
/// # Errors
///
/// Error database.
pub async fn by_name<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project_id: Uuid,
    name: &str,
) -> Result<Option<releases::Model>, DbErr> {
    let n = name.trim().to_lowercase();
    Ok(releases::Model::of_project(db, wid, project_id)
        .await?
        .into_iter()
        .find(|r| r.name.to_lowercase() == n || r.id.to_string() == n))
}
