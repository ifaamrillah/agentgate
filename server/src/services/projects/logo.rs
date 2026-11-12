//! Logo project (rules §8 storage): file disimpan lewat `ctx.storage` dengan key dari id
//! (`projects/<project>/logo-<uuid>`), database menyimpan key, tipe, dan ukuran. Tipe dibaca
//! dari isi berkas (PNG, JPEG, WebP), bukan dari nama atau header. SVG ditolak: bisa memuat
//! skrip dan butuh sanitasi yang belum ada.
//!
//! Urutan tulis: upload dulu, lalu baris DB di transaksi bersama audit; bila transaksi gagal,
//! file baru dihapus. File lama tidak pernah ditimpa: dihapus setelah commit.

use std::path::Path;

use axum::body::Bytes;
use loco_rs::storage::Storage;
use sea_orm::DatabaseConnection;
use uuid::Uuid;
use validator::{ValidationError, ValidationErrors};

use super::{Administering, ProjectFailure};
use crate::{
    models::projects::{self, Logo},
    services::{
        audit::{self, AuditAction, AuditSubject},
        workspaces::Acting,
    },
};

/// Ukuran maksimal logo (1 MB).
pub const MAX_BYTES: usize = 1024 * 1024;
pub const LOGO_TYPE_INVALID: &str = "project-logo-type-invalid";
pub const LOGO_TOO_LARGE: &str = "project-logo-too-large";
pub const LOGO_EMPTY: &str = "project-logo-empty";
/// Kode error field `logo`; masing-masing punya pesan di Fluent (dijaga tes i18n).
pub const ERROR_CODES: &[&str] = &[LOGO_TYPE_INVALID, LOGO_TOO_LARGE, LOGO_EMPTY];

/// Content type dari isi berkas; `None` = bukan PNG/JPEG/WebP.
#[must_use]
pub fn sniff(data: &[u8]) -> Option<&'static str> {
    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if data.len() >= 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

fn invalid(code: &'static str) -> ProjectFailure {
    let mut errors = ValidationErrors::new();
    let mut e = ValidationError::new(code);
    if code == LOGO_TOO_LARGE {
        e.add_param("max".into(), &(MAX_BYTES / 1024));
    }
    errors.add("logo", e);
    ProjectFailure::Invalid(errors.into())
}

/// Validasi berkas logo: tidak kosong, maksimal [`MAX_BYTES`], tipe dari isi.
///
/// # Errors
///
/// [`ProjectFailure::Invalid`] pada field `logo`.
pub fn validate(data: &[u8]) -> Result<&'static str, ProjectFailure> {
    if data.is_empty() {
        return Err(invalid(LOGO_EMPTY));
    }
    if data.len() > MAX_BYTES {
        return Err(invalid(LOGO_TOO_LARGE));
    }
    sniff(data).ok_or_else(|| invalid(LOGO_TYPE_INVALID))
}

async fn remove_quietly(storage: &Storage, key: &str) {
    if let Err(e) = storage.delete(Path::new(key)).await {
        tracing::warn!(error = %e, "gagal menghapus file logo lama");
    }
}

/// Simpan logo baru (menggantikan yang lama). Admin project saja.
///
/// # Errors
///
/// [`ProjectFailure::Invalid`] (berkas tidak valid), [`ProjectFailure::NotFound`] (project atau
/// izin), error storage, atau error database.
pub async fn replace(
    db: &DatabaseConnection,
    storage: &Storage,
    acting: &Acting<'_>,
    project_id: Uuid,
    data: Bytes,
) -> Result<projects::Model, loco_rs::Error> {
    let content_type = validate(&data)?;
    let admin = Administering::begin(db, acting, project_id).await?;
    let key = format!("projects/{project_id}/logo-{}", Uuid::now_v7());
    let size = i32::try_from(data.len()).unwrap_or(i32::MAX);
    storage
        .upload(Path::new(&key), &data)
        .await
        .map_err(|e| loco_rs::Error::string(&e.to_string()))?;
    let result = store(
        admin,
        acting,
        Some(Logo {
            key: key.clone(),
            content_type: content_type.to_string(),
            size,
        }),
    )
    .await;
    match result {
        Ok((project, old)) => {
            if let Some(old) = old {
                remove_quietly(storage, &old.key).await;
            }
            Ok(project)
        }
        Err(e) => {
            remove_quietly(storage, &key).await;
            Err(e.into())
        }
    }
}

/// Hapus logo (project kembali memakai inisial dan warna). Tanpa logo: tidak menulis apa pun.
///
/// # Errors
///
/// [`ProjectFailure::NotFound`] (project atau izin), atau error database.
pub async fn remove(
    db: &DatabaseConnection,
    storage: &Storage,
    acting: &Acting<'_>,
    project_id: Uuid,
) -> Result<projects::Model, ProjectFailure> {
    let admin = Administering::begin(db, acting, project_id).await?;
    if admin.project.logo().is_none() {
        let project = admin.project.clone();
        admin.txn.rollback().await?;
        return Ok(project);
    }
    let (project, old) = store(admin, acting, None).await?;
    if let Some(old) = old {
        remove_quietly(storage, &old.key).await;
    }
    Ok(project)
}

/// Tulis kolom logo + audit di transaksi `admin`; kembalikan logo lama untuk dihapus.
async fn store(
    admin: Administering,
    acting: &Acting<'_>,
    logo: Option<Logo>,
) -> Result<(projects::Model, Option<Logo>), ProjectFailure> {
    let old = admin.project.logo();
    let project = admin.project.set_logo(&admin.txn, logo).await?;
    audit::record(
        &admin.txn,
        acting.actor().audit(
            AuditAction::ProjectUpdated,
            (AuditSubject::Project, project.id),
            serde_json::json!({
                "logo": {
                    "from": old.as_ref().map(|l| &l.content_type),
                    "to": project.logo_content_type,
                }
            }),
        ),
    )
    .await?;
    admin.txn.commit().await?;
    Ok((project, old))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniffs_png_jpeg_webp_only() {
        assert_eq!(sniff(b"\x89PNG\r\n\x1a\nrest"), Some("image/png"));
        assert_eq!(sniff(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("image/jpeg"));
        assert_eq!(sniff(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
        assert_eq!(sniff(b"<svg xmlns='http://www.w3.org/2000/svg'/>"), None);
        assert_eq!(sniff(b"GIF89a"), None);
    }

    #[test]
    fn rejects_empty_and_large() {
        assert!(matches!(validate(b""), Err(ProjectFailure::Invalid(_))));
        let big = vec![0x89; MAX_BYTES + 1];
        assert!(matches!(validate(&big), Err(ProjectFailure::Invalid(_))));
    }
}
