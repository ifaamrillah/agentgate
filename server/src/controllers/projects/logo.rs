//! Logo project: `GET …/logo` (gambar, semua yang punya akses project; tanpa akses → 404),
//! `POST …/settings/logo` (multipart, field `file`) dan `POST …/settings/logo/delete` (Admin
//! project). Berkas tidak valid → 422 halaman Umum dengan pesan di pengunggah.
use std::path::Path;

use axum::{
    body::Bytes,
    extract::Multipart,
    http::{StatusCode, header},
};
use loco_rs::prelude::*;

use super::{acting, paths, projects::render_settings};
use crate::{
    controllers::{
        extractors::{ProjectAccess, ProjectAdmin, RequestId},
        flash,
    },
    models::permission::Permission,
    services::projects::{ProjectFailure, logo as service},
    views::{
        self,
        flash::Flash,
        projects::{FLASH_LOGO_REMOVED, FLASH_LOGO_SAVED, LOGO_FIELDS, ProjectFormView},
    },
};

#[debug_handler]
pub(super) async fn show(State(ctx): State<AppContext>, access: ProjectAccess) -> Result<Response> {
    access.require(Permission::ViewProject)?;
    let logo = access.project.logo().ok_or(Error::NotFound)?;
    let data: Vec<u8> = ctx
        .storage
        .download(Path::new(&logo.key))
        .await
        .map_err(|_| Error::NotFound)?;
    Ok((
        [
            (header::CONTENT_TYPE, logo.content_type),
            // URL memuat versi (`?v=`), jadi aman di-cache lama; privat karena butuh akses.
            (
                header::CACHE_CONTROL,
                "private, max-age=31536000, immutable".to_string(),
            ),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_string()),
        ],
        data,
    )
        .into_response())
}

/// Isi field `file` dari multipart; field lain diabaikan. Body rusak → 400.
async fn file_field(mut multipart: Multipart) -> Result<Bytes> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| Error::BadRequest("multipart".into()))?
    {
        if field.name() == Some("file") {
            return field
                .bytes()
                .await
                .map_err(|_| Error::BadRequest("multipart".into()));
        }
    }
    Ok(Bytes::new())
}

/// `ProjectAdmin` sebelum `Multipart`: tanpa izin → 404 sebelum body dibaca (rules §6).
#[debug_handler]
pub(super) async fn upload(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    ProjectAdmin(access): ProjectAdmin,
    session: Session,
    RequestId(request_id): RequestId,
    multipart: Multipart,
) -> Result<Response> {
    let data = file_field(multipart).await?;
    match service::replace(
        &ctx.db,
        &ctx.storage,
        &acting(&access, request_id),
        access.project.id,
        data,
    )
    .await
    {
        Ok(_) => {
            let session = flash::set(session, &Flash::success(FLASH_LOGO_SAVED))?;
            Ok((session, format::redirect(&paths(&access).settings)?).into_response())
        }
        Err(Error::Validation(errors)) => {
            let code = views::form::field_errors(LOGO_FIELDS, Some(&errors))
                .get("logo")
                .map(|e| e.code.clone())
                .unwrap_or_default();
            render_settings(
                &ctx,
                &v,
                &access,
                &ProjectFormView::of(&access.project),
                &code,
                None,
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => Err(e),
    }
}

#[debug_handler]
pub(super) async fn delete(
    State(ctx): State<AppContext>,
    ProjectAdmin(access): ProjectAdmin,
    session: Session,
    RequestId(request_id): RequestId,
) -> Result<Response> {
    service::remove(
        &ctx.db,
        &ctx.storage,
        &acting(&access, request_id),
        access.project.id,
    )
    .await
    .map_err(|e: ProjectFailure| -> Error { e.into() })?;
    let session = flash::set(session, &Flash::success(FLASH_LOGO_REMOVED))?;
    Ok((session, format::redirect(&paths(&access).settings)?).into_response())
}
