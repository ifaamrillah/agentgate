//! Dokumen knowledge dan versinya: buat, simpan (cek versi dasar), pulihkan versi, tandai
//! basi/masih benar, arsipkan, pemilik knowledge project, sumber dokumen.

use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait,
    IntoActiveModel, QueryFilter, QuerySelect, Set, TransactionTrait,
};
use uuid::Uuid;

use super::{
    CONFLICT, Failure, OWNER_INVALID, RESERVED_SLUGS, TYPE_INVALID, UNCHANGED, VERSION_INVALID,
    can_manage_workspace, clean_body, clean_title, now, slugify, user_can,
};
use crate::{
    models::{
        knowledge_catalog, knowledge_docs, knowledge_sources, knowledge_types,
        knowledge_versions::{self, VersionSource},
        permission::Permission,
        projects, repos,
        workspaces::WorkspaceId,
    },
    services::{tickets::access_in, workspaces::Acting},
};

/// Isi dokumen baru.
#[derive(Debug, Clone, Default)]
pub struct NewDoc {
    /// `None` = dokumen workspace.
    pub project_id: Option<Uuid>,
    pub title: String,
    /// Kunci jenis (bawaan atau custom). Kosong untuk dokumen workspace tanpa jenis.
    pub type_key: String,
    pub body: String,
    /// Slug yang diminta (kosong = dari judul).
    pub slug: String,
}

/// Satu sumber dokumen (repo + path/prefix) dari agent.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SourceRef {
    #[serde(default)]
    pub repo: String,
    pub path: String,
}

/// Kelompok untuk jenis `type_key` di project (bawaan, atau jenis custom aktif).
///
/// # Errors
///
/// Error database.
pub async fn group_of<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    type_key: &str,
) -> Result<Option<String>, sea_orm::DbErr> {
    if let Some(t) = knowledge_catalog::builtin(type_key) {
        return Ok(Some(t.group.to_string()));
    }
    Ok(knowledge_types::Model::active_in(db, wid)
        .await?
        .into_iter()
        .find(|t| t.key == type_key)
        .map(|t| t.group_key))
}

/// Pastikan pelaku boleh mengubah dokumen di `project_id` (`None` = dokumen workspace).
///
/// # Errors
///
/// `NotFound` bila tidak boleh, atau error database.
pub async fn ensure_can_edit<C: ConnectionTrait>(
    db: &C,
    acting: &Acting<'_>,
    project_id: Option<Uuid>,
) -> Result<(), Failure> {
    match project_id {
        Some(p) => access_in(db, acting, p, Permission::EditProject)
            .await
            .map(|_| ()),
        None if can_manage_workspace(acting) => Ok(()),
        None => Err(Failure::NotFound),
    }
}

/// Slug unik di project/workspace dari `wanted` (atau judul).
async fn unique_slug<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project_id: Option<Uuid>,
    wanted: &str,
) -> Result<String, Failure> {
    let base = slugify(wanted);
    let base = if RESERVED_SLUGS.contains(&base.as_str()) {
        format!("{base}-doc")
    } else {
        base
    };
    let mut slug = base.clone();
    let mut n = 2;
    while knowledge_docs::Model::by_slug(db, wid, project_id, &slug)
        .await?
        .is_some()
    {
        slug = format!("{base}-{n}");
        n += 1;
    }
    Ok(slug)
}

/// Sisipkan dokumen + versi 1 di `db` (transaksi pemanggil). Tanpa cek izin.
pub(crate) async fn insert<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    author: Option<Uuid>,
    new: &NewDoc,
    source: VersionSource,
    proposal_id: Option<Uuid>,
    persona: &str,
) -> Result<knowledge_docs::Model, Failure> {
    let title = clean_title(&new.title)?;
    let body = clean_body(&new.body)?;
    let group_key = match (new.project_id, new.type_key.as_str()) {
        (None, "") => String::new(),
        (_, key) => group_of(db, wid, key)
            .await?
            .ok_or_else(|| Failure::field("type_key", TYPE_INVALID))?,
    };
    let wanted = if new.slug.trim().is_empty() {
        &title
    } else {
        &new.slug
    };
    let slug = unique_slug(db, wid, new.project_id, wanted).await?;
    let doc = knowledge_docs::ActiveModel {
        workspace_id: Set(wid.get()),
        project_id: Set(new.project_id),
        slug: Set(slug),
        title: Set(title.clone()),
        group_key: Set(group_key),
        type_key: Set(new.type_key.clone()),
        body: Set(body.clone()),
        version: Set(1),
        stale_reason: Set(String::new()),
        applies_to: Set(Vec::new()),
        personas: Set(Vec::new()),
        created_by: Set(author),
        updated_by: Set(author),
        ..Default::default()
    }
    .insert(db)
    .await?;
    knowledge_versions::ActiveModel {
        workspace_id: Set(wid.get()),
        doc_id: Set(doc.id),
        number: Set(1),
        title: Set(title),
        body: Set(body),
        message: Set(String::new()),
        source: Set(sea_orm::ActiveEnum::to_value(&source)),
        author_id: Set(author),
        persona: Set(persona.to_string()),
        proposal_id: Set(proposal_id),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(doc)
}

/// Buat dokumen (versi 1). Project: `EditProject`; workspace: kelola knowledge workspace.
///
/// # Errors
///
/// `Invalid` (judul, isi, jenis), `NotFound`, atau error database.
pub async fn create(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    new: &NewDoc,
) -> Result<knowledge_docs::Model, Failure> {
    ensure_can_edit(db, acting, new.project_id).await?;
    let txn = db.begin().await?;
    let doc = insert(
        &txn,
        acting.workspace_id(),
        Some(acting.user.id),
        new,
        VersionSource::Create,
        None,
        "",
    )
    .await?;
    txn.commit().await?;
    Ok(doc)
}

/// Dokumen terkunci (`FOR UPDATE`).
pub(crate) async fn lock<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    id: Uuid,
) -> Result<knowledge_docs::Model, Failure> {
    knowledge_docs::Entity::find_by_id(id)
        .filter(knowledge_docs::Column::WorkspaceId.eq(wid.get()))
        .filter(knowledge_docs::Column::ArchivedAt.is_null())
        .lock_exclusive()
        .one(db)
        .await?
        .ok_or(Failure::NotFound)
}

/// Satu perubahan isi dokumen menjadi versi baru.
pub(crate) struct Change<'a> {
    pub title: &'a str,
    pub body: &'a str,
    pub message: &'a str,
    pub source: VersionSource,
    pub author: Option<Uuid>,
    pub persona: &'a str,
    pub proposal_id: Option<Uuid>,
}

/// Terapkan `change` ke dokumen terkunci: versi + 1, status basi dibersihkan. Isi dan judul yang
/// sama ditolak (`knowledge-unchanged`).
pub(crate) async fn apply<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    doc: knowledge_docs::Model,
    change: &Change<'_>,
) -> Result<knowledge_docs::Model, Failure> {
    let title = clean_title(change.title)?;
    let body = clean_body(change.body)?;
    if title == doc.title && body == doc.body {
        return Err(Failure::field("body", UNCHANGED));
    }
    let number = doc.version + 1;
    let mut m = doc.into_active_model();
    m.title = Set(title.clone());
    m.body = Set(body.clone());
    m.version = Set(number);
    m.stale_reason = Set(String::new());
    m.stale_at = Set(None);
    m.updated_by = Set(change.author);
    let doc = m.update(db).await?;
    knowledge_versions::ActiveModel {
        workspace_id: Set(wid.get()),
        doc_id: Set(doc.id),
        number: Set(number),
        title: Set(title),
        body: Set(body),
        message: Set(change.message.trim().chars().take(300).collect()),
        source: Set(sea_orm::ActiveEnum::to_value(&change.source)),
        author_id: Set(change.author),
        persona: Set(change.persona.to_string()),
        proposal_id: Set(change.proposal_id),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(doc)
}

/// Simpan isi oleh manusia. `base_version` = versi yang sedang diedit; berbeda dengan versi
/// sekarang → `knowledge-conflict` (orang/AI lain menyimpan lebih dulu).
///
/// # Errors
///
/// `Invalid` (konflik, sama, judul/isi), `NotFound`, error database.
pub async fn save(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
    title: &str,
    body: &str,
    base_version: i32,
    message: &str,
) -> Result<knowledge_docs::Model, Failure> {
    let wid = acting.workspace_id();
    let txn = db.begin().await?;
    let doc = lock(&txn, wid, id).await?;
    ensure_can_edit(&txn, acting, doc.project_id).await?;
    if doc.version != base_version {
        return Err(Failure::field("body", CONFLICT));
    }
    let doc = apply(
        &txn,
        wid,
        doc,
        &Change {
            title,
            body,
            message,
            source: VersionSource::Edit,
            author: Some(acting.user.id),
            persona: "",
            proposal_id: None,
        },
    )
    .await?;
    txn.commit().await?;
    Ok(doc)
}

/// Pulihkan versi `number` sebagai versi baru.
///
/// # Errors
///
/// `knowledge-version-invalid`, `knowledge-unchanged`, `NotFound`, error database.
pub async fn restore(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
    number: i32,
) -> Result<knowledge_docs::Model, Failure> {
    let wid = acting.workspace_id();
    let txn = db.begin().await?;
    let doc = lock(&txn, wid, id).await?;
    ensure_can_edit(&txn, acting, doc.project_id).await?;
    let old = knowledge_versions::Model::number(&txn, wid, doc.id, number)
        .await?
        .ok_or_else(|| Failure::field("version", VERSION_INVALID))?;
    let message = format!("Pulihkan v{number}");
    let doc = apply(
        &txn,
        wid,
        doc,
        &Change {
            title: &old.title,
            body: &old.body,
            message: &message,
            source: VersionSource::Restore,
            author: Some(acting.user.id),
            persona: "",
            proposal_id: None,
        },
    )
    .await?;
    txn.commit().await?;
    Ok(doc)
}

/// Tandai dokumen basi (sistem: pemicu merge/audit). Dokumen yang sudah basi tidak diubah.
///
/// # Errors
///
/// Error database.
pub async fn mark_stale<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    ids: &[Uuid],
    reason: &str,
) -> Result<u64, sea_orm::DbErr> {
    if ids.is_empty() {
        return Ok(0);
    }
    let reason: String = reason.chars().take(300).collect();
    let res = knowledge_docs::Entity::update_many()
        .col_expr(
            knowledge_docs::Column::StaleReason,
            sea_orm::sea_query::Expr::value(reason),
        )
        .col_expr(
            knowledge_docs::Column::StaleAt,
            sea_orm::sea_query::Expr::value(now()),
        )
        .filter(knowledge_docs::Column::WorkspaceId.eq(wid.get()))
        .filter(knowledge_docs::Column::Id.is_in(ids.iter().copied()))
        .filter(knowledge_docs::Column::StaleReason.eq(""))
        .exec(db)
        .await?;
    Ok(res.rows_affected)
}

/// "Masih benar": hapus tanda basi tanpa membuat versi.
///
/// # Errors
///
/// `NotFound`, error database.
pub async fn mark_fresh(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
) -> Result<knowledge_docs::Model, Failure> {
    let txn = db.begin().await?;
    let doc = lock(&txn, acting.workspace_id(), id).await?;
    ensure_can_edit(&txn, acting, doc.project_id).await?;
    let mut m = doc.into_active_model();
    m.stale_reason = Set(String::new());
    m.stale_at = Set(None);
    let doc = m.update(&txn).await?;
    txn.commit().await?;
    Ok(doc)
}

/// Arsipkan dokumen (hilang dari daftar, konteks AI, dan pencarian; versi tetap disimpan).
///
/// # Errors
///
/// `NotFound`, error database.
pub async fn archive(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
) -> Result<knowledge_docs::Model, Failure> {
    let txn = db.begin().await?;
    let doc = lock(&txn, acting.workspace_id(), id).await?;
    ensure_can_edit(&txn, acting, doc.project_id).await?;
    let mut m = doc.into_active_model();
    m.archived_at = Set(Some(now()));
    m.updated_by = Set(Some(acting.user.id));
    let doc = m.update(&txn).await?;
    txn.commit().await?;
    Ok(doc)
}

/// Ubah metadata dokumen workspace: project tempat berlaku, persona pembaca, pemilik.
///
/// # Errors
///
/// `NotFound` (bukan dokumen workspace / tidak boleh), error database.
pub async fn set_workspace_meta(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
    applies_to: Vec<Uuid>,
    personas: Vec<String>,
    owner: Option<Uuid>,
) -> Result<knowledge_docs::Model, Failure> {
    let wid = acting.workspace_id();
    let txn = db.begin().await?;
    let doc = lock(&txn, wid, id).await?;
    if doc.project_id.is_some() || !can_manage_workspace(acting) {
        return Err(Failure::NotFound);
    }
    let mut projects_ok = Vec::new();
    for p in applies_to {
        if projects::Model::find_in(&txn, wid, p).await?.is_some() && !projects_ok.contains(&p) {
            projects_ok.push(p);
        }
    }
    if let Some(u) = owner
        && crate::models::workspace_members::Model::find_with_role(&txn, wid, u)
            .await?
            .is_none()
    {
        return Err(Failure::field("owner", OWNER_INVALID));
    }
    let mut personas: Vec<String> = personas
        .into_iter()
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect();
    personas.sort();
    personas.dedup();
    let mut m = doc.into_active_model();
    m.applies_to = Set(projects_ok);
    m.personas = Set(personas);
    m.owner_id = Set(owner);
    m.updated_by = Set(Some(acting.user.id));
    let doc = m.update(&txn).await?;
    txn.commit().await?;
    Ok(doc)
}

/// Ganti pemilik knowledge project (`AdministerProject`). Pemilik baru harus bisa mengubah project.
///
/// # Errors
///
/// `knowledge-owner-invalid`, `NotFound`, error database.
pub async fn set_owner(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    project_id: Uuid,
    owner: Option<Uuid>,
) -> Result<projects::Model, Failure> {
    let wid = acting.workspace_id();
    access_in(db, acting, project_id, Permission::AdministerProject).await?;
    if let Some(u) = owner
        && !user_can(db, wid, u, project_id, Permission::EditProject).await?
    {
        return Err(Failure::field("owner", OWNER_INVALID));
    }
    let project = projects::Model::find_in(db, wid, project_id)
        .await?
        .ok_or(Failure::NotFound)?;
    let mut m = project.into_active_model();
    m.knowledge_owner_id = Set(owner);
    Ok(m.update(db).await?)
}

/// Ganti sumber dokumen dengan `sources` (repo dicocokkan dengan nama repo project; yang tidak
/// dikenal dilewati).
///
/// # Errors
///
/// Error database.
pub async fn set_sources<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    doc: &knowledge_docs::Model,
    sources: &[SourceRef],
) -> Result<(), sea_orm::DbErr> {
    let Some(project_id) = doc.project_id else {
        return Ok(());
    };
    let project_repos = repos::Model::of_project(db, wid, project_id).await?;
    knowledge_sources::Entity::delete_many()
        .filter(knowledge_sources::Column::WorkspaceId.eq(wid.get()))
        .filter(knowledge_sources::Column::DocId.eq(doc.id))
        .exec(db)
        .await?;
    let mut seen = std::collections::HashSet::new();
    for s in sources.iter().take(50) {
        let repo = project_repos
            .iter()
            .find(|r| r.name.eq_ignore_ascii_case(s.repo.trim()))
            .or_else(|| {
                (project_repos.len() == 1 && s.repo.trim().is_empty()).then(|| &project_repos[0])
            });
        let Some(repo) = repo else { continue };
        let path = s
            .path
            .trim()
            .trim_start_matches("./")
            .trim_start_matches('/');
        let path: String = path.chars().take(500).collect();
        if !seen.insert((repo.id, path.clone())) {
            continue;
        }
        knowledge_sources::ActiveModel {
            workspace_id: Set(wid.get()),
            doc_id: Set(doc.id),
            repo_id: Set(repo.id),
            path: Set(path),
            ..Default::default()
        }
        .insert(db)
        .await?;
    }
    Ok(())
}

/// Sumber dokumen (repo + path).
///
/// # Errors
///
/// Error database.
pub async fn sources_of<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    doc_id: Uuid,
) -> Result<Vec<knowledge_sources::Model>, sea_orm::DbErr> {
    knowledge_sources::Entity::find()
        .filter(knowledge_sources::Column::WorkspaceId.eq(wid.get()))
        .filter(knowledge_sources::Column::DocId.eq(doc_id))
        .all(db)
        .await
}

/// Dokumen yang boleh dibaca di project `project_id`: dokumen project + dokumen workspace yang
/// berlaku untuk project itu. `persona` (kunci) membatasi ke jenis yang boleh dibaca persona dan
/// dokumen workspace yang menyebut persona itu.
///
/// # Errors
///
/// Error database.
pub async fn readable<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project_id: Uuid,
    persona: Option<&crate::models::personas::Model>,
) -> Result<Vec<knowledge_docs::Model>, sea_orm::DbErr> {
    let mut out = knowledge_docs::Model::of_project(db, wid, project_id).await?;
    out.extend(
        knowledge_docs::Model::of_workspace(db, wid)
            .await?
            .into_iter()
            .filter(|d| d.applies_to.is_empty() || d.applies_to.contains(&project_id)),
    );
    if let Some(p) = persona {
        out.retain(|d| persona_reads(p, d));
    }
    Ok(out)
}

/// Persona boleh membaca dokumen: jenis termasuk `knowledge_types` persona (kosong = semua);
/// dokumen workspace juga harus menyebut persona (kosong = semua persona).
#[must_use]
pub fn persona_reads(
    persona: &crate::models::personas::Model,
    doc: &knowledge_docs::Model,
) -> bool {
    if doc.is_workspace_doc() {
        return doc.personas.is_empty() || doc.personas.contains(&persona.key);
    }
    persona.knowledge_types.is_empty() || persona.knowledge_types.contains(&doc.type_key)
}
