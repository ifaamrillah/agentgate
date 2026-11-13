//! Knowledge sebagai konteks agent: daftar ringkas dokumen yang boleh dibaca persona (untuk
//! `ag_context` dan prompt job stage). Isi dokumen diambil saat dibutuhkan lewat
//! `ag_knowledge read`, supaya konteks awal tetap kecil.

use sea_orm::{ConnectionTrait, DbErr};
use uuid::Uuid;

use super::docs;
use crate::models::{personas, workspaces::WorkspaceId};

/// Ringkasan satu dokumen untuk agent.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Brief {
    pub slug: String,
    pub title: String,
    #[serde(rename = "type")]
    pub type_key: String,
    pub group: String,
    pub version: i32,
    /// Dokumen workspace (standar lintas project).
    pub workspace: bool,
    /// Alasan basi (kosong = terbaru).
    pub stale: String,
}

/// Batas dokumen yang disebut di konteks.
pub const MAX_BRIEFS: usize = 60;

/// Dokumen yang boleh dibaca `persona` (kunci; kosong = semua) di project.
///
/// # Errors
///
/// Error database.
pub async fn briefs<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project_id: Uuid,
    persona: &str,
) -> Result<Vec<Brief>, DbErr> {
    let persona = if persona.is_empty() {
        None
    } else {
        personas::Model::resolve(db, wid, persona).await?
    };
    Ok(docs::readable(db, wid, project_id, persona.as_ref())
        .await?
        .into_iter()
        .take(MAX_BRIEFS)
        .map(|d| Brief {
            workspace: d.is_workspace_doc(),
            slug: d.slug,
            title: d.title,
            type_key: d.type_key,
            group: d.group_key,
            version: d.version,
            stale: d.stale_reason,
        })
        .collect())
}

/// Bagian prompt job stage: dokumen knowledge yang relevan dibaca sebelum bekerja.
#[must_use]
pub fn prompt_section(briefs: &[Brief], project: &str) -> String {
    if briefs.is_empty() {
        return String::new();
    }
    let list: Vec<String> = briefs
        .iter()
        .map(|b| {
            format!(
                "- `{}` {}{}",
                b.slug,
                b.title,
                if b.workspace {
                    " (standar workspace)"
                } else {
                    ""
                }
            )
        })
        .collect();
    format!(
        "\n\nKnowledge project {project} (baca yang relevan dengan `ag_knowledge` action=read sebelum \
         mengubah kode; dokumen workspace adalah standar yang wajib dipatuhi):\n{}",
        list.join("\n")
    )
}
