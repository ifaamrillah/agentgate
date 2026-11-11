//! Undang dari Pengaturan → Anggota (`Require<ManageMembers>`): kirim undangan, kirim ulang
//! (token baru), dan batalkan. Form undang tidak valid → 422 halaman anggota dengan isian dan
//! error-nya; role/jabatan/undangan workspace lain atau id tidak valid → 404.
use axum::http::StatusCode;
use loco_rs::prelude::*;
use serde::Deserialize;

use super::members::{
    acting, empty_invite, form_id, optional_form_id, redirect_with, render_index,
};
use crate::{
    controllers::extractors::{IdOr404, RequestId, Require, ValidForm, require::ManageMembers},
    models::{invitations, roles::MEMBER_ROLE_NOT_ASSIGNABLE},
    services::workspaces::invite::{self, InviteError, InviteParams},
    views::{self, flash::Flash, members::InviteFormView},
};

#[derive(Debug, Deserialize)]
pub struct InviteForm {
    #[serde(default)]
    email: String,
    #[serde(default)]
    role_id: String,
    #[serde(default)]
    job_title_id: String,
}

/// `Require` sebelum `ValidForm`: tanpa izin → 404 sebelum body dibaca (rules §6).
#[debug_handler]
pub(super) async fn create(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageMembers>,
    session: Session,
    RequestId(request_id): RequestId,
    ValidForm(form): ValidForm<InviteForm>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let params = InviteParams {
        email: form.email.clone(),
        role_id: form_id(&form.role_id)?,
        job_title_id: optional_form_id(&form.job_title_id)?,
    };
    match invite::run(&ctx, &acting(tenant, request_id), params).await {
        Ok(_) => redirect_with(session, tenant, views::members::FLASH_INVITATION_SENT).await,
        Err(InviteError::Invalid(errors)) => {
            let invite = InviteFormView::new(
                form.email.trim(),
                form.role_id.trim(),
                form.job_title_id.trim(),
                Some(&errors),
            );
            render_index(
                &ctx,
                &v,
                tenant,
                invite,
                None,
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => Err(e.into()),
    }
}

#[debug_handler]
pub(super) async fn resend(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageMembers>,
    path: IdOr404<invitations::Entity>,
    session: Session,
    RequestId(request_id): RequestId,
) -> Result<Response> {
    let tenant = &access.tenant;
    match invite::resend(&ctx, &acting(tenant, request_id), path.id).await {
        Ok(_) => redirect_with(session, tenant, views::members::FLASH_INVITATION_RESENT).await,
        Err(InviteError::NotAssignable) => {
            let invite = empty_invite(&ctx, tenant).await?;
            render_index(
                &ctx,
                &v,
                tenant,
                invite,
                Some(Flash::error(MEMBER_ROLE_NOT_ASSIGNABLE)),
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => Err(e.into()),
    }
}

#[debug_handler]
pub(super) async fn revoke(
    State(ctx): State<AppContext>,
    access: Require<ManageMembers>,
    path: IdOr404<invitations::Entity>,
    session: Session,
    RequestId(request_id): RequestId,
) -> Result<Response> {
    let tenant = &access.tenant;
    invite::revoke(&ctx, &acting(tenant, request_id), path.id).await?;
    redirect_with(session, tenant, views::members::FLASH_INVITATION_REVOKED).await
}
