//! Pengaturan knowledge workspace (update otomatis) dan jenis dokumen custom.

use sea_orm::{
    ActiveEnum, ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel,
    QueryFilter, Set, sea_query::OnConflict,
};
use uuid::Uuid;

use super::{Failure, TITLE_INVALID, TYPE_INVALID, can_manage_workspace, clean_body, now, slugify};
use crate::{
    models::{
        knowledge_catalog, knowledge_runs::Depth, knowledge_settings, knowledge_types, projects,
    },
    services::workspaces::Acting,
};

/// Isian pengaturan update otomatis.
#[derive(Debug, Clone)]
pub struct SettingsForm {
    pub on_done: bool,
    pub on_merge: bool,
    pub audit: bool,
    /// 0 = Minggu … 6 = Sabtu.
    pub audit_day: i16,
    pub default_depth: Depth,
    pub repo_sync: bool,
}

/// Simpan pengaturan update otomatis.
///
/// # Errors
///
/// `NotFound` (tidak boleh), `Invalid` (hari), error database.
pub async fn save(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    form: &SettingsForm,
) -> Result<knowledge_settings::Model, Failure> {
    if !can_manage_workspace(acting) {
        return Err(Failure::NotFound);
    }
    if !(0..=6).contains(&form.audit_day) {
        return Err(Failure::field("audit_day", "knowledge-audit-day-invalid"));
    }
    let wid = acting.workspace_id();
    let m = knowledge_settings::ActiveModel {
        workspace_id: Set(wid.get()),
        on_done: Set(form.on_done),
        on_merge: Set(form.on_merge),
        audit: Set(form.audit),
        audit_day: Set(form.audit_day),
        default_depth: Set(form.default_depth.to_value()),
        repo_sync: Set(form.repo_sync),
        updated_by: Set(Some(acting.user.id)),
        updated_at: Set(now()),
        ..Default::default()
    };
    knowledge_settings::Entity::insert(m)
        .on_conflict(
            OnConflict::column(knowledge_settings::Column::WorkspaceId)
                .update_columns([
                    knowledge_settings::Column::OnDone,
                    knowledge_settings::Column::OnMerge,
                    knowledge_settings::Column::Audit,
                    knowledge_settings::Column::AuditDay,
                    knowledge_settings::Column::DefaultDepth,
                    knowledge_settings::Column::RepoSync,
                    knowledge_settings::Column::UpdatedBy,
                    knowledge_settings::Column::UpdatedAt,
                ])
                .to_owned(),
        )
        .exec(db)
        .await?;
    Ok(knowledge_settings::Model::of(db, wid).await?)
}

/// Isian jenis dokumen custom.
#[derive(Debug, Clone, Default)]
pub struct TypeForm {
    pub name: String,
    pub group_key: String,
    pub template: String,
    pub instructions: String,
    /// Kosong = semua project.
    pub projects: Vec<Uuid>,
    pub ai_scan: bool,
    pub on_done: bool,
}

async fn clean_type(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    form: &TypeForm,
) -> Result<(String, Vec<Uuid>), Failure> {
    let name = form.name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(Failure::field("name", TITLE_INVALID));
    }
    if knowledge_catalog::group(&form.group_key).is_none() {
        return Err(Failure::field("group_key", TYPE_INVALID));
    }
    let wid = acting.workspace_id();
    let mut ps = Vec::new();
    for p in &form.projects {
        if projects::Model::find_in(db, wid, *p).await?.is_some() && !ps.contains(p) {
            ps.push(*p);
        }
    }
    Ok((name.to_string(), ps))
}

/// Buat jenis custom. Kunci = `x-<slug nama>` (tidak bentrok dengan jenis bawaan).
///
/// # Errors
///
/// `NotFound` (tidak boleh), `Invalid`, error database.
pub async fn create_type(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    form: &TypeForm,
) -> Result<knowledge_types::Model, Failure> {
    if !can_manage_workspace(acting) {
        return Err(Failure::NotFound);
    }
    let (name, ps) = clean_type(db, acting, form).await?;
    let wid = acting.workspace_id();
    let base = format!("x-{}", slugify(&name));
    let existing: Vec<String> = knowledge_types::Entity::find()
        .filter(knowledge_types::Column::WorkspaceId.eq(wid.get()))
        .all(db)
        .await?
        .into_iter()
        .map(|t| t.key)
        .collect();
    let mut key = base.clone();
    let mut n = 2;
    while existing.contains(&key) {
        key = format!("{base}-{n}");
        n += 1;
    }
    Ok(knowledge_types::ActiveModel {
        workspace_id: Set(wid.get()),
        key: Set(key),
        name: Set(name),
        group_key: Set(form.group_key.clone()),
        template: Set(clean_body(&form.template)?),
        instructions: Set(clean_body(&form.instructions)?),
        projects: Set(ps),
        ai_scan: Set(form.ai_scan),
        on_done: Set(form.on_done),
        created_by: Set(Some(acting.user.id)),
        ..Default::default()
    }
    .insert(db)
    .await?)
}

/// Ubah jenis custom (kunci tetap).
///
/// # Errors
///
/// `NotFound`, `Invalid`, error database.
pub async fn update_type(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
    form: &TypeForm,
) -> Result<knowledge_types::Model, Failure> {
    if !can_manage_workspace(acting) {
        return Err(Failure::NotFound);
    }
    let t = knowledge_types::Model::find_in(db, acting.workspace_id(), id)
        .await?
        .ok_or(Failure::NotFound)?;
    let (name, ps) = clean_type(db, acting, form).await?;
    let mut m = t.into_active_model();
    m.name = Set(name);
    m.group_key = Set(form.group_key.clone());
    m.template = Set(clean_body(&form.template)?);
    m.instructions = Set(clean_body(&form.instructions)?);
    m.projects = Set(ps);
    m.ai_scan = Set(form.ai_scan);
    m.on_done = Set(form.on_done);
    Ok(m.update(db).await?)
}

/// Arsipkan jenis custom (dokumen yang sudah ada tetap; jenis tidak bisa dipilih lagi).
///
/// # Errors
///
/// `NotFound`, error database.
pub async fn archive_type(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
) -> Result<knowledge_types::Model, Failure> {
    if !can_manage_workspace(acting) {
        return Err(Failure::NotFound);
    }
    let t = knowledge_types::Model::find_in(db, acting.workspace_id(), id)
        .await?
        .ok_or(Failure::NotFound)?;
    let mut m = t.into_active_model();
    m.archived_at = Set(Some(now()));
    Ok(m.update(db).await?)
}
