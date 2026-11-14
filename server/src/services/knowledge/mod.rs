//! Knowledge base (M8): dokumen living document project dan workspace, versi, usulan dari AI yang
//! harus disetujui pemilik knowledge, run scan/update (job knowledge tanpa tiket di runner),
//! pemicu otomatis, pencarian, sinkron `.agentgate/knowledge/` lewat CLI, dan backup aturan lama.
//!
//! Izin:
//! - membaca: `ViewProject`;
//! - membuat/mengedit dokumen project, meminta scan/update, mengajukan usulan (AI atas nama user):
//!   `EditProject`;
//! - approve/tolak usulan: pemilik knowledge project, atau `AdministerProject` (juga bila belum
//!   ada pemilik);
//! - dokumen workspace, jenis custom, pengaturan update otomatis: `ManageKnowledge`
//!   (Owner/Admin bawaan; role custom bisa diberi izin ini).
//!
//! AI tidak pernah menimpa dokumen langsung: semua hasilnya masuk sebagai usulan.

pub mod catalog;
pub mod context;
pub mod docs;
pub mod proposals;
pub mod runs;
pub mod search;
pub mod settings;
pub mod sync;
pub mod triggers;

use sea_orm::{ConnectionTrait, DbErr};
use uuid::Uuid;

pub use super::tickets::TicketFailure as Failure;
use super::{tickets::access_in, workspaces::Acting};
use crate::models::{permission::Permission, projects};

/// Dokumen berubah sejak versi yang sedang diedit.
pub const CONFLICT: &str = "knowledge-conflict";
/// Isi sama dengan versi sekarang (tidak ada yang disimpan/diusulkan).
pub const UNCHANGED: &str = "knowledge-unchanged";
/// Usulan sudah diputuskan.
pub const NOT_PENDING: &str = "knowledge-not-pending";
/// Hanya pemilik knowledge (atau Admin project) yang boleh memutuskan usulan.
pub const NOT_OWNER: &str = "knowledge-not-owner";
/// Judul kosong atau terlalu panjang.
pub const TITLE_INVALID: &str = "knowledge-title-invalid";
/// Isi terlalu panjang.
pub const BODY_TOO_LONG: &str = "knowledge-body-too-long";
/// Jenis/kelompok dokumen tidak dikenal.
pub const TYPE_INVALID: &str = "knowledge-type-invalid";
/// Alasan penolakan wajib diisi.
pub const NOTE_REQUIRED: &str = "knowledge-note-required";
/// Pemilik knowledge harus anggota yang bisa membuka project.
pub const OWNER_INVALID: &str = "knowledge-owner-invalid";
/// Versi yang diminta tidak ada.
pub const VERSION_INVALID: &str = "knowledge-version-invalid";
/// Batal keputusan tidak bisa: dokumen sudah berubah lagi atau ada usulan lain.
pub const UNDO_BLOCKED: &str = "knowledge-undo-blocked";
/// Sudah ada run terbuka untuk project ini.
pub const RUN_OPEN: &str = "knowledge-run-open";
pub const ERROR_CODES: &[&str] = &[
    CONFLICT,
    UNCHANGED,
    NOT_PENDING,
    NOT_OWNER,
    TITLE_INVALID,
    BODY_TOO_LONG,
    TYPE_INVALID,
    NOTE_REQUIRED,
    OWNER_INVALID,
    VERSION_INVALID,
    UNDO_BLOCKED,
    RUN_OPEN,
];

/// Pelaku boleh menyetujui/menolak usulan knowledge project: pemilik knowledge (yang masih bisa
/// mengubah project) atau `AdministerProject`.
///
/// # Errors
///
/// Error database.
pub async fn can_approve<C: ConnectionTrait>(
    db: &C,
    acting: &Acting<'_>,
    project: &projects::Model,
) -> Result<bool, DbErr> {
    let check = |p| access_in(db, acting, project.id, p);
    if project.knowledge_owner_id == Some(acting.user.id) {
        return match check(Permission::EditProject).await {
            Ok(_) => Ok(true),
            Err(Failure::Db(e)) => Err(e),
            Err(_) => Ok(false),
        };
    }
    match check(Permission::AdministerProject).await {
        Ok(_) => Ok(true),
        Err(Failure::Db(e)) => Err(e),
        Err(_) => Ok(false),
    }
}

/// User `user_id` (anggota workspace) memegang `permission` di project.
///
/// # Errors
///
/// Error database.
pub async fn user_can<C: ConnectionTrait>(
    db: &C,
    wid: crate::models::workspaces::WorkspaceId,
    user_id: Uuid,
    project_id: Uuid,
    permission: Permission,
) -> Result<bool, DbErr> {
    let Some((_, role)) =
        crate::models::workspace_members::Model::find_with_role(db, wid, user_id).await?
    else {
        return Ok(false);
    };
    let who = crate::models::project_access::Accessor {
        workspace_id: wid,
        user_id,
        role: &role,
    };
    Ok(
        crate::models::project_access::effective_role(db, &who, project_id)
            .await?
            .is_some_and(|a| a.has(permission)),
    )
}

/// Pelaku boleh mengelola knowledge workspace (dokumen workspace, jenis custom, pengaturan).
#[must_use]
pub fn can_manage_workspace(acting: &Acting<'_>) -> bool {
    acting.role.has(Permission::ManageKnowledge)
}

/// Slug dari judul: huruf kecil ASCII, angka, tanda hubung; maksimal 60 karakter.
#[must_use]
pub fn slugify(title: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in title.chars().flat_map(char::to_lowercase) {
        let c = match c {
            'à'..='å' => 'a',
            'è'..='ë' => 'e',
            'ì'..='ï' => 'i',
            'ò'..='ö' => 'o',
            'ù'..='ü' => 'u',
            _ => c,
        };
        if c.is_ascii_alphanumeric() {
            out.push(c);
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
        if out.len() >= 60 {
            break;
        }
    }
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() {
        "dokumen".into()
    } else {
        out
    }
}

/// Segmen URL yang dipakai rute knowledge, tidak boleh menjadi slug dokumen.
pub const RESERVED_SLUGS: &[&str] = &["g", "proposals", "new", "scan", "owner", "search", "runs"];

fn now() -> chrono::DateTime<chrono::FixedOffset> {
    chrono::Utc::now().into()
}

/// Judul valid (1–160 karakter setelah trim).
fn clean_title(title: &str) -> Result<String, Failure> {
    let t = title.trim();
    if t.is_empty() || t.chars().count() > crate::models::knowledge_docs::TITLE_MAX {
        return Err(Failure::field("title", TITLE_INVALID));
    }
    Ok(t.to_string())
}

/// Isi valid (≤ `BODY_MAX` karakter); baris akhir dinormalkan satu newline.
fn clean_body(body: &str) -> Result<String, Failure> {
    if body.chars().count() > crate::models::knowledge_docs::BODY_MAX {
        return Err(Failure::field("body", BODY_TOO_LONG));
    }
    let b = body.replace("\r\n", "\n");
    let b = b.trim_end();
    Ok(if b.is_empty() {
        String::new()
    } else {
        format!("{b}\n")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_ascii() {
        assert_eq!(slugify("Arsitektur Sistem"), "arsitektur-sistem");
        assert_eq!(slugify("Modul: payment & struk"), "modul-payment-struk");
        assert_eq!(slugify("  ---  "), "dokumen");
        assert_eq!(slugify("Café"), "cafe");
    }

    #[test]
    fn body_normalized() {
        assert_eq!(clean_body("a\r\nb\n\n\n").ok().as_deref(), Some("a\nb\n"));
        assert_eq!(clean_body("  \n").ok().as_deref(), Some(""));
    }
}
