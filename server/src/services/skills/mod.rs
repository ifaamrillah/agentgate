//! Skill pack workspace (`ManageTicketSettings`): buat sendiri (draf + editor berkas), impor dari
//! repo GitHub publik, kirim ke review (pemindaian statis server), approve atau tolak, hapus.
//! Semua perubahan mengunci workspace, membaca ulang role pelaku, dan mencatat audit
//! `skill_pack.*` di transaksi yang sama. Pengunduhan repo terjadi sebelum transaksi.

pub mod fetch;
pub mod scan;

use sea_orm::{
    ActiveEnum, ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection,
    DatabaseTransaction, EntityTrait, IntoActiveModel, QueryFilter, TransactionTrait,
};
use uuid::Uuid;
use validator::ValidationErrors;

use crate::{
    common::{settings::SkillSettings, text},
    models::{
        permission::Permission,
        persona_skills, personas, skill_files,
        skill_packs::{
            self, CONFIRM_MISMATCH, DESCRIPTION_MAX_LEN, MAX_FILE_BYTES, MAX_FILES, NAME_TAKEN,
            NOT_EDITABLE, NOT_PENDING, NOTE_REQUIRED, PATH_INVALID, SKILL_MD_REQUIRED, SKILL_TOOLS,
            SkillRisk, SkillSource, SkillStatus, TOO_MANY_FILES, TOOL_UNKNOWN,
        },
    },
    services::{
        audit::{self, AuditAction, AuditSubject},
        tickets::TicketFailure,
        workspaces::{Acting, lock_as},
    },
};

pub type SkillFailure = TicketFailure;

async fn begin(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
) -> Result<DatabaseTransaction, SkillFailure> {
    let txn = db.begin().await?;
    lock_as(&txn, acting, |r| r.has(Permission::ManageTicketSettings))
        .await?
        .ok_or(TicketFailure::NotFound)?;
    Ok(txn)
}

async fn locked(
    txn: &DatabaseTransaction,
    acting: &Acting<'_>,
    id: Uuid,
) -> Result<skill_packs::Model, SkillFailure> {
    skill_packs::Model::lock_in(txn, acting.workspace_id(), id)
        .await?
        .ok_or(TicketFailure::NotFound)
}

fn editable(pack: &skill_packs::Model) -> Result<(), SkillFailure> {
    if pack.editable() && pack.source() == SkillSource::Custom {
        Ok(())
    } else {
        Err(TicketFailure::field("pack", NOT_EDITABLE))
    }
}

/// Nama + deskripsi skill pack yang sudah dinormalisasi.
fn info(name: &str, description: &str) -> Result<(String, String), SkillFailure> {
    let mut errors = ValidationErrors::new();
    let name = text::normalize_name(name);
    if let Err(e) = text::validate_name("skill", &name) {
        errors.add("name", e);
    }
    let description = description.replace("\r\n", "\n").trim().to_string();
    if description.chars().count() > DESCRIPTION_MAX_LEN {
        errors.add(
            "description",
            validator::ValidationError::new(skill_packs::DESCRIPTION_TOO_LONG),
        );
    }
    if errors.is_empty() {
        Ok((name, description))
    } else {
        Err(TicketFailure::errors(errors))
    }
}

fn tools(requested: &[String]) -> Result<Vec<String>, SkillFailure> {
    if requested.iter().any(|t| !SKILL_TOOLS.contains(&t.as_str())) {
        return Err(TicketFailure::field("tools", TOOL_UNKNOWN));
    }
    Ok(SKILL_TOOLS
        .iter()
        .filter(|t| requested.iter().any(|r| r == *t))
        .map(|t| (*t).to_string())
        .collect())
}

async fn name_free(
    txn: &DatabaseTransaction,
    acting: &Acting<'_>,
    name: &str,
    except: Option<Uuid>,
) -> Result<(), SkillFailure> {
    let taken = skill_packs::Model::all_in(txn, acting.workspace_id())
        .await?
        .iter()
        .any(|p| Some(p.id) != except && p.name.to_lowercase() == name.to_lowercase());
    if taken {
        Err(TicketFailure::field("name", NAME_TAKEN))
    } else {
        Ok(())
    }
}

async fn fresh_key(
    txn: &DatabaseTransaction,
    acting: &Acting<'_>,
    name: &str,
) -> Result<String, SkillFailure> {
    let taken: Vec<String> = skill_packs::Model::all_in(txn, acting.workspace_id())
        .await?
        .into_iter()
        .map(|p| p.key)
        .collect();
    let key = personas::fresh_key(name, &taken);
    Ok(if key == "persona" {
        "skill".to_string()
    } else {
        key
    })
}

fn starter(name: &str, description: &str) -> String {
    format!(
        "# {name}\n\n{description}\n\n## Kapan dipakai\n- …\n\n## Langkah\n1. …\n\n## Aturan\n- …\n"
    )
}

async fn record(
    txn: &DatabaseTransaction,
    acting: &Acting<'_>,
    action: AuditAction,
    pack: &skill_packs::Model,
    changes: serde_json::Value,
) -> Result<(), SkillFailure> {
    let mut changes = changes;
    if let Some(obj) = changes.as_object_mut() {
        obj.insert("name".into(), pack.name.clone().into());
    }
    audit::record(
        txn,
        acting
            .actor()
            .audit(action, (AuditSubject::SkillPack, pack.id), changes),
    )
    .await?;
    Ok(())
}

/// Buat skill pack sendiri (draf) dengan `SKILL.md` awal.
///
/// # Errors
///
/// `Invalid` (nama, deskripsi, nama dipakai), `NotFound`, error database.
pub async fn create_custom(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    name: &str,
    description: &str,
) -> Result<skill_packs::Model, SkillFailure> {
    let (name, description) = info(name, description)?;
    let txn = begin(db, acting).await?;
    name_free(&txn, acting, &name, None).await?;
    let key = fresh_key(&txn, acting, &name).await?;
    let pack = skill_packs::ActiveModel {
        workspace_id: Set(acting.workspace_id().get()),
        key: Set(key),
        name: Set(name.clone()),
        description: Set(description.clone()),
        source: Set(SkillSource::Custom.to_value()),
        status: Set(SkillStatus::Draft.to_value()),
        created_by: Set(Some(acting.user.id)),
        ..Default::default()
    }
    .insert(&txn)
    .await?;
    skill_files::ActiveModel {
        workspace_id: Set(acting.workspace_id().get()),
        pack_id: Set(pack.id),
        path: Set("SKILL.md".to_string()),
        body: Set(starter(&name, &description)),
        ..Default::default()
    }
    .insert(&txn)
    .await?;
    record(
        &txn,
        acting,
        AuditAction::SkillPackCreated,
        &pack,
        serde_json::json!({ "source": "custom" }),
    )
    .await?;
    txn.commit().await?;
    Ok(pack)
}

/// Ubah nama, deskripsi, dan tools yang diminta (draf/ditolak, buatan sendiri).
///
/// # Errors
///
/// `Invalid`, `skill-not-editable`, `NotFound`, error database.
pub async fn update_info(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
    name: &str,
    description: &str,
    requested: &[String],
) -> Result<skill_packs::Model, SkillFailure> {
    let (name, description) = info(name, description)?;
    let requested = tools(requested)?;
    let txn = begin(db, acting).await?;
    let pack = locked(&txn, acting, id).await?;
    editable(&pack)?;
    name_free(&txn, acting, &name, Some(pack.id)).await?;
    let mut changes = serde_json::Map::new();
    for (k, c) in [
        ("name", audit::change(&pack.name, &name)),
        (
            "description",
            audit::change(&pack.description, &description),
        ),
        ("tools", audit::change(&pack.requested_tools, &requested)),
    ] {
        if let Some(c) = c {
            changes.insert(k.to_string(), c);
        }
    }
    if changes.is_empty() {
        txn.rollback().await?;
        return Ok(pack);
    }
    let mut row = pack.into_active_model();
    row.name = Set(name);
    row.description = Set(description);
    row.requested_tools = Set(requested);
    let updated = row.update(&txn).await?;
    record(
        &txn,
        acting,
        AuditAction::SkillPackUpdated,
        &updated,
        serde_json::Value::Object(changes),
    )
    .await?;
    txn.commit().await?;
    Ok(updated)
}

/// Simpan (buat atau ganti) satu berkas skill pack. Tidak diaudit per simpan: audit di kirim ke
/// review (isi akhir yang dinilai).
///
/// # Errors
///
/// `skill-path-invalid`, `skill-file-too-large`, `skill-too-many-files`, `skill-not-editable`,
/// `NotFound`, error database.
pub async fn save_file(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
    path: &str,
    body: &str,
) -> Result<(), SkillFailure> {
    let path = path.trim().trim_start_matches('/');
    if !skill_files::valid_path(path) {
        return Err(TicketFailure::field("path", PATH_INVALID));
    }
    let body = body.replace("\r\n", "\n");
    if body.len() > MAX_FILE_BYTES {
        return Err(TicketFailure::field("body", skill_packs::FILE_TOO_LARGE));
    }
    let txn = begin(db, acting).await?;
    let pack = locked(&txn, acting, id).await?;
    editable(&pack)?;
    let files = skill_files::Model::of_pack(&txn, pack.id).await?;
    if let Some(f) = files.iter().find(|f| f.path == path) {
        let mut row = f.clone().into_active_model();
        row.body = Set(body);
        row.update(&txn).await?;
    } else {
        if files.len() >= MAX_FILES {
            return Err(TicketFailure::field("path", TOO_MANY_FILES));
        }
        skill_files::ActiveModel {
            workspace_id: Set(acting.workspace_id().get()),
            pack_id: Set(pack.id),
            path: Set(path.to_string()),
            body: Set(body),
            ..Default::default()
        }
        .insert(&txn)
        .await?;
    }
    txn.commit().await?;
    Ok(())
}

/// Hapus satu berkas (`SKILL.md` tidak bisa dihapus).
///
/// # Errors
///
/// `skill-skill-md-required`, `skill-not-editable`, `NotFound`, error database.
pub async fn delete_file(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
    path: &str,
) -> Result<(), SkillFailure> {
    if path == "SKILL.md" {
        return Err(TicketFailure::field("path", SKILL_MD_REQUIRED));
    }
    let txn = begin(db, acting).await?;
    let pack = locked(&txn, acting, id).await?;
    editable(&pack)?;
    skill_files::Entity::delete_many()
        .filter(skill_files::Column::PackId.eq(pack.id))
        .filter(skill_files::Column::Path.eq(path))
        .exec(&txn)
        .await?;
    txn.commit().await?;
    Ok(())
}

fn report_json(report: &scan::Report) -> serde_json::Value {
    serde_json::to_value(&report.checks).unwrap_or_else(|_| serde_json::json!([]))
}

async fn submit_in(
    txn: &DatabaseTransaction,
    acting: &Acting<'_>,
    pack: skill_packs::Model,
    note: &str,
) -> Result<skill_packs::Model, SkillFailure> {
    let files: Vec<(String, String)> = skill_files::Model::of_pack(txn, pack.id)
        .await?
        .into_iter()
        .map(|f| (f.path, f.body))
        .collect();
    if !files.iter().any(|(p, _)| p.ends_with("SKILL.md")) {
        return Err(TicketFailure::field("pack", SKILL_MD_REQUIRED));
    }
    let report = scan::scan(&files, &pack.requested_tools);
    let mut row = pack.into_active_model();
    row.status = Set(SkillStatus::Pending.to_value());
    row.risk = Set(Some(report.risk.to_value()));
    row.report = Set(report_json(&report));
    row.review_note = Set(note.trim().to_string());
    row.submitted_at = Set(Some(chrono::Utc::now().into()));
    row.reviewed_by = Set(None);
    row.reviewed_at = Set(None);
    let updated = row.update(txn).await?;
    record(
        txn,
        acting,
        AuditAction::SkillPackSubmitted,
        &updated,
        serde_json::json!({ "risk": report.risk.to_value(), "files": files.len() }),
    )
    .await?;
    Ok(updated)
}

/// Kirim draf (atau skill pack yang ditolak) ke review: pemindaian statis → `pending`.
///
/// # Errors
///
/// `skill-not-editable`, `skill-skill-md-required`, `NotFound`, error database.
pub async fn submit(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
    note: &str,
) -> Result<skill_packs::Model, SkillFailure> {
    let txn = begin(db, acting).await?;
    let pack = locked(&txn, acting, id).await?;
    editable(&pack)?;
    let updated = submit_in(&txn, acting, pack, note).await?;
    txn.commit().await?;
    Ok(updated)
}

/// Impor skill pack dari repo GitHub publik lalu langsung kirim ke review.
///
/// # Errors
///
/// `skill-repo-url-invalid`/`skill-path-invalid` (`url`/`path`), `skill-fetch-failed` dan batas
/// ukuran (`url`), `NotFound`, error database.
pub async fn import(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    settings: &SkillSettings,
    url: &str,
    git_ref: &str,
    path: &str,
) -> Result<skill_packs::Model, SkillFailure> {
    let repo = fetch::parse(url, git_ref, path).map_err(|code| {
        TicketFailure::field(if code == PATH_INVALID { "path" } else { "url" }, code)
    })?;
    // Izin dicek sebelum mengunduh (permintaan tanpa izin tidak memicu unduhan).
    begin(db, acting).await?.rollback().await?;
    let fetched = fetch::fetch(settings, &repo)
        .await
        .map_err(|code| TicketFailure::field("url", code))?;
    let txn = begin(db, acting).await?;
    let base_name = if repo.path.is_empty() {
        repo.repo.clone()
    } else {
        format!(
            "{} {}",
            repo.repo,
            repo.path.rsplit('/').next().unwrap_or_default()
        )
    };
    let mut name = base_name.clone();
    let mut n = 2;
    while name_free(&txn, acting, &name, None).await.is_err() {
        name = format!("{base_name} {n}");
        n += 1;
    }
    let key = fresh_key(&txn, acting, &name).await?;
    let requested = scan::frontmatter_tools(&fetched.files);
    let pack = skill_packs::ActiveModel {
        workspace_id: Set(acting.workspace_id().get()),
        key: Set(key),
        name: Set(name),
        description: Set(String::new()),
        source: Set(SkillSource::Repo.to_value()),
        repo_url: Set(Some(repo.url())),
        repo_ref: Set(Some(repo.git_ref.clone())),
        repo_path: Set(Some(repo.path.clone()).filter(|p| !p.is_empty())),
        commit_sha: Set(fetched.commit_sha.clone()),
        version: Set(fetched
            .commit_sha
            .as_deref()
            .map_or_else(|| repo.git_ref.clone(), |s| s[..7].to_string())),
        requested_tools: Set(requested),
        status: Set(SkillStatus::Draft.to_value()),
        created_by: Set(Some(acting.user.id)),
        ..Default::default()
    }
    .insert(&txn)
    .await?;
    for (path, body) in &fetched.files {
        skill_files::ActiveModel {
            workspace_id: Set(acting.workspace_id().get()),
            pack_id: Set(pack.id),
            path: Set(path.clone()),
            body: Set(body.clone()),
            ..Default::default()
        }
        .insert(&txn)
        .await?;
    }
    record(
        &txn,
        acting,
        AuditAction::SkillPackCreated,
        &pack,
        serde_json::json!({
            "source": "repo",
            "repo": repo.url(),
            "ref": repo.git_ref,
            "commit": fetched.commit_sha,
        }),
    )
    .await?;
    let pack = submit_in(&txn, acting, pack, "").await?;
    txn.commit().await?;
    Ok(pack)
}

/// Approve skill pack `pending`. Risiko "Berbahaya" wajib mengetik kunci skill pack.
///
/// # Errors
///
/// `skill-not-pending`, `skill-confirm-mismatch`, `NotFound`, error database.
pub async fn approve(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
    confirm: &str,
) -> Result<skill_packs::Model, SkillFailure> {
    let txn = begin(db, acting).await?;
    let pack = locked(&txn, acting, id).await?;
    if pack.status() != SkillStatus::Pending {
        return Err(TicketFailure::field("decision", NOT_PENDING));
    }
    if pack.risk() == Some(SkillRisk::Danger) && confirm.trim() != pack.key {
        return Err(TicketFailure::field("confirm", CONFIRM_MISMATCH));
    }
    let mut row = pack.into_active_model();
    row.status = Set(SkillStatus::Approved.to_value());
    row.reviewed_by = Set(Some(acting.user.id));
    row.reviewed_at = Set(Some(chrono::Utc::now().into()));
    let updated = row.update(&txn).await?;
    record(
        &txn,
        acting,
        AuditAction::SkillPackApproved,
        &updated,
        serde_json::json!({ "risk": updated.risk }),
    )
    .await?;
    txn.commit().await?;
    Ok(updated)
}

/// Tolak skill pack `pending` dengan alasan.
///
/// # Errors
///
/// `skill-review-note-required`, `skill-not-pending`, `NotFound`, error database.
pub async fn reject(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
    note: &str,
) -> Result<skill_packs::Model, SkillFailure> {
    let note = note.trim();
    if note.is_empty() || note.chars().count() > 1000 {
        return Err(TicketFailure::field("note", NOTE_REQUIRED));
    }
    let txn = begin(db, acting).await?;
    let pack = locked(&txn, acting, id).await?;
    if pack.status() != SkillStatus::Pending {
        return Err(TicketFailure::field("decision", NOT_PENDING));
    }
    let mut row = pack.into_active_model();
    row.status = Set(SkillStatus::Rejected.to_value());
    row.review_note = Set(note.to_string());
    row.reviewed_by = Set(Some(acting.user.id));
    row.reviewed_at = Set(Some(chrono::Utc::now().into()));
    let updated = row.update(&txn).await?;
    record(
        &txn,
        acting,
        AuditAction::SkillPackRejected,
        &updated,
        serde_json::json!({ "note": note }),
    )
    .await?;
    txn.commit().await?;
    Ok(updated)
}

/// Hapus skill pack (dicopot dari semua persona).
///
/// # Errors
///
/// `NotFound`, error database.
pub async fn delete(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
) -> Result<(), SkillFailure> {
    let txn = begin(db, acting).await?;
    let pack = locked(&txn, acting, id).await?;
    let used = persona_skills::Entity::delete_many()
        .filter(persona_skills::Column::PackId.eq(pack.id))
        .exec(&txn)
        .await?
        .rows_affected;
    record(
        &txn,
        acting,
        AuditAction::SkillPackDeleted,
        &pack,
        serde_json::json!({ "personas": used }),
    )
    .await?;
    skill_packs::Entity::delete_by_id(pack.id)
        .exec(&txn)
        .await?;
    txn.commit().await?;
    Ok(())
}
