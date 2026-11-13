//! Sinkron knowledge ↔ repo lewat CLI (`agentgate knowledge pull|status|sync`) dan backup aturan
//! lama dari `agentgate init`.
//!
//! - **Ekspor**: semua dokumen project (aktif) beserta versinya; CLI menulis
//!   `.agentgate/knowledge/<slug>.md` dengan front matter.
//! - **Impor**: file lokal yang berubah. Versi dasar = versi server → versi baru langsung
//!   (sumber `repo`); versi dasar berbeda atau dokumen baru → usulan `repo` (ditinjau pemilik).
//! - **Backup**: arsip aturan lama disimpan di storage (`backups/…`) + baris `rule_backups`;
//!   isi teksnya menjadi usulan `init` untuk dokumen "Konvensi & Coding Style".
use std::path::Path;

use axum::body::Bytes;
use loco_rs::storage::Storage;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, DatabaseConnection, TransactionTrait};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    BODY_TOO_LONG, CONFLICT, Failure, UNCHANGED,
    docs::{self, Change},
    proposals::{self, NewProposal},
};
use crate::{
    models::{
        knowledge_docs, knowledge_proposals::ProposalTrigger, knowledge_versions::VersionSource,
        permission::Permission, projects, rule_backups,
    },
    services::{tickets::access_in, workspaces::Acting},
};

/// Batas arsip backup (aturan lama biasanya beberapa KB).
pub const BACKUP_MAX_BYTES: usize = 5 * 1024 * 1024;
/// Batas isi teks aturan lama yang dimasukkan ke usulan Konvensi.
const BACKUP_TEXT_MAX: usize = 150_000;

/// Satu dokumen hasil ekspor.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExportDoc {
    pub slug: String,
    pub title: String,
    #[serde(rename = "type")]
    pub type_key: String,
    pub group: String,
    pub version: i32,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Export {
    pub project: String,
    pub docs: Vec<ExportDoc>,
}

/// Satu file lokal yang dikirim CLI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportDoc {
    pub slug: String,
    #[serde(default)]
    pub title: String,
    #[serde(default, rename = "type")]
    pub type_key: String,
    /// Versi server saat file terakhir ditarik (0 = file baru di repo).
    #[serde(default)]
    pub base_version: i32,
    pub body: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImportRequest {
    pub project: String,
    pub docs: Vec<ImportDoc>,
}

/// Hasil per file: `saved` (versi baru), `proposed` (usulan menunggu), `behind` (file lokal =
/// versi lama server), `unchanged`, `error`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImportResult {
    pub slug: String,
    pub result: String,
    #[serde(default)]
    pub version: i32,
    #[serde(default)]
    pub error: String,
}

async fn project_for(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    key: &str,
    permission: Permission,
) -> Result<projects::Model, Failure> {
    let project = projects::Model::find_by_key(db, acting.workspace_id(), key.trim())
        .await?
        .ok_or(Failure::NotFound)?;
    access_in(db, acting, project.id, permission).await?;
    Ok(project)
}

/// Ekspor dokumen project.
///
/// # Errors
///
/// `NotFound` (project tidak ada / tanpa akses), error database.
pub async fn export(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    project_key: &str,
) -> Result<Export, Failure> {
    let project = project_for(db, acting, project_key, Permission::ViewProject).await?;
    let docs = knowledge_docs::Model::of_project(db, acting.workspace_id(), project.id).await?;
    Ok(Export {
        project: project.key,
        docs: docs
            .into_iter()
            .map(|d| ExportDoc {
                slug: d.slug,
                title: d.title,
                type_key: d.type_key,
                group: d.group_key,
                version: d.version,
                body: d.body,
            })
            .collect(),
    })
}

fn code_of(e: &Failure) -> String {
    match e {
        Failure::Invalid(errors) => errors
            .errors
            .values()
            .flat_map(|es| es.iter().map(|e| e.code.to_string()))
            .next()
            .unwrap_or_else(|| "invalid".into()),
        Failure::NotFound => "not_found".into(),
        Failure::Db(_) => "db".into(),
    }
}

/// Impor perubahan lokal (lihat dokumentasi modul).
///
/// # Errors
///
/// `NotFound` (project / izin `EditProject`), error database. Kegagalan per file dilaporkan di
/// hasil, bukan sebagai error.
pub async fn import(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    req: &ImportRequest,
) -> Result<Vec<ImportResult>, Failure> {
    let project = project_for(db, acting, &req.project, Permission::EditProject).await?;
    let wid = acting.workspace_id();
    let mut out = Vec::new();
    for f in &req.docs {
        let existing = knowledge_docs::Model::by_slug(db, wid, Some(project.id), f.slug.trim())
            .await?
            .filter(|d| d.archived_at.is_none());
        let result = match existing {
            Some(doc) if doc.version == f.base_version => {
                let title = if f.title.trim().is_empty() {
                    doc.title.clone()
                } else {
                    f.title.clone()
                };
                let txn = db.begin().await?;
                let locked = docs::lock(&txn, wid, doc.id).await?;
                if locked.version == f.base_version {
                    match docs::apply(
                        &txn,
                        wid,
                        locked,
                        &Change {
                            title: &title,
                            body: &f.body,
                            message: "Dari repo",
                            source: VersionSource::Repo,
                            author: Some(acting.user.id),
                            persona: "",
                            proposal_id: None,
                        },
                    )
                    .await
                    {
                        Ok(d) => {
                            txn.commit().await?;
                            Ok(("saved", d.version))
                        }
                        Err(e) => Err(e),
                    }
                } else {
                    Err(Failure::field("body", CONFLICT))
                }
            }
            Some(doc)
                if f.base_version > 0
                    && crate::models::knowledge_versions::Model::number(
                        db,
                        wid,
                        doc.id,
                        f.base_version,
                    )
                    .await?
                    .is_some_and(|v| v.body.trim() == f.body.trim()) =>
            {
                // File lokal = versi lama yang belum ditarik: bukan perubahan.
                Ok(("behind", doc.version))
            }
            _ => {
                // Versi dasar berbeda atau dokumen baru → usulan untuk ditinjau.
                let mut new = NewProposal::new(project.id, ProposalTrigger::Repo);
                new.slug = f.slug.trim().to_string();
                new.title = f.title.clone();
                new.type_key = if f.type_key.trim().is_empty() {
                    "overview".into()
                } else {
                    f.type_key.trim().to_string()
                };
                new.body = f.body.clone();
                new.reason = if f.base_version == 0 {
                    "Dokumen baru dari repo".into()
                } else {
                    format!(
                        "Diedit di repo dari v{}; dokumen server sudah berubah sejak itu",
                        f.base_version
                    )
                };
                new.trigger_ref = "agentgate knowledge sync".into();
                new.persona = String::new();
                new.created_by = Some(acting.user.id);
                proposals::propose(db, wid, new)
                    .await
                    .map(|_| ("proposed", f.base_version))
            }
        };
        out.push(match result {
            Ok((r, version)) => ImportResult {
                slug: f.slug.clone(),
                result: r.into(),
                version,
                error: String::new(),
            },
            Err(e) => {
                let code = code_of(&e);
                if let Failure::Db(e) = e {
                    return Err(Failure::Db(e));
                }
                ImportResult {
                    slug: f.slug.clone(),
                    result: if code == UNCHANGED {
                        "unchanged".into()
                    } else {
                        "error".into()
                    },
                    version: f.base_version,
                    error: if code == UNCHANGED {
                        String::new()
                    } else {
                        code
                    },
                }
            }
        });
    }
    Ok(out)
}

/// Isi teks satu file aturan lama (untuk usulan Konvensi).
#[derive(Debug, Clone, Deserialize)]
pub struct BackupText {
    pub path: String,
    #[serde(default)]
    pub content: String,
}

/// Unggahan backup `agentgate init`.
#[derive(Debug, Clone, Deserialize)]
pub struct BackupRequest {
    pub project: String,
    #[serde(default)]
    pub repo: String,
    /// Nama arsip lokal (`<waktu>.tar.gz`).
    pub archive: String,
    /// Arsip tar.gz, base64.
    pub data: String,
    /// Manifest (`created_at`, `files` dengan sha256/size).
    #[serde(default)]
    pub manifest: serde_json::Value,
    #[serde(default)]
    pub texts: Vec<BackupText>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BackupOut {
    pub id: Uuid,
    pub proposal: Option<Uuid>,
}

/// Simpan backup aturan lama dan buat usulan `init` "Konvensi & Coding Style".
///
/// # Errors
///
/// `NotFound` (project / `EditProject`), `Invalid` (arsip kosong/rusak/terlalu besar), error
/// storage (sebagai `Db` custom) atau database.
pub async fn backup(
    db: &DatabaseConnection,
    storage: &Storage,
    acting: &Acting<'_>,
    req: &BackupRequest,
) -> Result<BackupOut, Failure> {
    use base64::Engine;
    let project = project_for(db, acting, &req.project, Permission::EditProject).await?;
    let wid = acting.workspace_id();
    let data = base64::engine::general_purpose::STANDARD
        .decode(req.data.trim())
        .map_err(|_| Failure::field("data", "backup-invalid"))?;
    if data.is_empty() {
        return Err(Failure::field("data", "backup-invalid"));
    }
    if data.len() > BACKUP_MAX_BYTES {
        return Err(Failure::field("data", BODY_TOO_LONG));
    }
    let archive: String = req
        .archive
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        .take(80)
        .collect();
    let archive = if archive.is_empty() {
        "backup.tar.gz".to_string()
    } else {
        archive
    };
    let key = format!("backups/{}/{}/{archive}", project.id, Uuid::now_v7());
    storage
        .upload(Path::new(&key), &Bytes::from(data.clone()))
        .await
        .map_err(|e| Failure::Db(sea_orm::DbErr::Custom(format!("storage: {e}"))))?;
    let files = req.manifest["files"].as_array().map_or(0, Vec::len);
    let row = rule_backups::ActiveModel {
        workspace_id: Set(wid.get()),
        project_id: Set(project.id),
        repo: Set(req.repo.chars().take(120).collect()),
        archive: Set(archive.clone()),
        storage_key: Set(key),
        size: Set(i64::try_from(data.len()).unwrap_or(i64::MAX)),
        files: Set(i32::try_from(files).unwrap_or(i32::MAX)),
        manifest: Set(req.manifest.clone()),
        created_by: Set(Some(acting.user.id)),
        ..Default::default()
    }
    .insert(db)
    .await?;
    let texts: Vec<&BackupText> = req
        .texts
        .iter()
        .filter(|t| !t.content.trim().is_empty())
        .collect();
    let proposal = if texts.is_empty() {
        None
    } else {
        let mut body = String::from(
            "# Konvensi & Coding Style\n\nDiserap dari aturan AI lama repo saat `agentgate init`. \
             Rapikan seperlunya sebelum menyetujui.\n",
        );
        for t in texts {
            body.push_str(&format!("\n## Dari `{}`\n\n{}\n", t.path, t.content.trim()));
            if body.len() > BACKUP_TEXT_MAX {
                body.truncate(BACKUP_TEXT_MAX);
                body.push_str("\n\n_(dipotong)_\n");
                break;
            }
        }
        let mut new = NewProposal::new(project.id, ProposalTrigger::Init);
        new.slug = "konvensi-coding-style".into();
        new.title = "Konvensi & Coding Style".into();
        new.type_key = "konvensi".into();
        new.body = body;
        new.reason = format!("Aturan lama dari `agentgate init` ({archive})");
        new.trigger_ref = req.repo.clone();
        new.created_by = Some(acting.user.id);
        match proposals::propose(db, wid, new).await {
            Ok(p) => Some(p.id),
            Err(Failure::Db(e)) => return Err(Failure::Db(e)),
            Err(_) => None,
        }
    };
    Ok(BackupOut {
        id: row.id,
        proposal,
    })
}

/// Backup project (untuk halaman Runner & CLI dan unduh).
///
/// # Errors
///
/// `NotFound`, error database.
pub async fn backups(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    project_key: &str,
) -> Result<Vec<rule_backups::Model>, Failure> {
    let project = project_for(db, acting, project_key, Permission::ViewProject).await?;
    Ok(rule_backups::Model::of_project(db, acting.workspace_id(), project.id, 50).await?)
}
