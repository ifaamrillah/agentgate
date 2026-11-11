//! Halaman undangan publik (prefix `/invitations` ada di `public_routes`). Token ada di path,
//! jadi request log menyamarkan segmen itu (`middleware::request_log`) dan setiap halaman
//! membawa `Referrer-Policy: no-referrer`.
//!
//! - `GET /invitations/{token}`: nama workspace, pengundang, dan role. Tidak mengonsumsi token.
//!   Sudah login → tombol terima (atau pesan bila email berbeda); belum login → Daftar dan Masuk
//!   (`/login?return_to=` kembali ke sini).
//! - `POST /invitations/{token}/accept`: user yang login menerima undangan → `/w/{slug}`.
//! - `GET|POST /invitations/{token}/register`: daftar dengan email undangan (terkunci), lalu
//!   bergabung dan login (`sign_in::complete`) → `/w/{slug}`.
//!
//! Token tidak berlaku karena alasan apa pun → halaman `invitations/invalid.html`, 404, sama
//! untuk semua alasan.
use axum::http::StatusCode;
use loco_rs::{auth::session::encode, prelude::*, validation::ModelValidationErrors};
use secrecy::SecretString;
use serde::Deserialize;

use crate::{
    common::settings::Settings,
    controllers::extractors::{CurrentUser, MaybeCurrentUser, RequestId, ValidForm},
    models::users,
    services::{
        auth::sign_in::{self, SignInStep},
        workspaces::accept_invite::{
            self, AcceptError, INVITE_EMAIL_MISMATCH, InviteRegistration, InviteSignup,
            PendingInvitation,
        },
    },
    views::{
        self,
        invitations::{InvitationView, RegisterPage, ShowPage},
    },
};

#[derive(Debug, Deserialize)]
pub struct RegisterForm {
    #[serde(default)]
    name: String,
    password: SecretString,
}

fn invitation_path(token: &str) -> String {
    format!("/invitations/{token}")
}

fn workspace_path(slug: &str) -> String {
    format!("/w/{slug}")
}

fn invitation_view(pending: &PendingInvitation, token: &str) -> InvitationView {
    let path = invitation_path(token);
    InvitationView {
        workspace_name: pending.workspace.name.clone(),
        inviter_name: pending.inviter_name.clone().unwrap_or_default(),
        role_name: pending.role_name.clone(),
        email: pending.invitation.email.clone(),
        accept_action: format!("{path}/accept"),
        register_href: format!("{path}/register"),
        login_href: format!("/login?return_to={}", encode(&path)),
    }
}

fn show_page(
    pending: &PendingInvitation,
    token: &str,
    user: Option<&users::Model>,
    error: &'static str,
) -> ShowPage {
    ShowPage {
        invitation: invitation_view(pending, token),
        signed_in: user.is_some(),
        email_matches: user.is_some_and(|u| pending.is_for(u)),
        error,
    }
}

#[debug_handler]
async fn show(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    MaybeCurrentUser(user): MaybeCurrentUser,
    Path(token): Path<String>,
) -> Result<Response> {
    let Some(pending) = accept_invite::find(&ctx.db, &token).await? else {
        return views::invitations::invalid(&v);
    };
    let error = match &user {
        Some(u) if !pending.is_for(u) => INVITE_EMAIL_MISMATCH,
        _ => "",
    };
    let page = show_page(&pending, &token, user.as_ref(), error);
    views::invitations::show(&v, &page, StatusCode::OK)
}

#[debug_handler]
async fn accept(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    CurrentUser(user): CurrentUser,
    RequestId(request_id): RequestId,
    Path(token): Path<String>,
) -> Result<Response> {
    match accept_invite::run(&ctx, &token, &user, request_id).await {
        Ok(workspace) => format::redirect(&workspace_path(&workspace.slug)),
        Err(AcceptError::Invalid) => views::invitations::invalid(&v),
        Err(AcceptError::EmailMismatch) => {
            let Some(pending) = accept_invite::find(&ctx.db, &token).await? else {
                return views::invitations::invalid(&v);
            };
            let page = show_page(&pending, &token, Some(&user), INVITE_EMAIL_MISMATCH);
            views::invitations::show(&v, &page, StatusCode::UNPROCESSABLE_ENTITY)
        }
        Err(AcceptError::Db(e)) => Err(e.into()),
    }
}

/// Form daftar (atau tautan Masuk bila email undangan sudah punya akun). User yang sudah login
/// diarahkan ke halaman undangan.
#[debug_handler]
async fn register_form(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    MaybeCurrentUser(user): MaybeCurrentUser,
    Path(token): Path<String>,
) -> Result<Response> {
    if user.is_some() {
        return format::redirect(&invitation_path(&token));
    }
    render_register(&ctx, &v, &token, "", None, StatusCode::OK).await
}

#[debug_handler]
async fn register(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    MaybeCurrentUser(user): MaybeCurrentUser,
    RequestId(request_id): RequestId,
    session: Session,
    Path(token): Path<String>,
    ValidForm(form): ValidForm<RegisterForm>,
) -> Result<Response> {
    if user.is_some() {
        return format::redirect(&invitation_path(&token));
    }
    let settings = Settings::from_ctx(&ctx)?;
    let signup = InviteSignup {
        name: form.name.clone(),
        password: form.password,
    };
    match accept_invite::register(&ctx, &settings, &token, signup, request_id).await? {
        InviteRegistration::Joined(joined) => {
            let target = workspace_path(&joined.workspace.slug);
            match sign_in::complete(&ctx, session, &joined.user, Some(&target))? {
                SignInStep::Done(session) => {
                    Ok((session, format::redirect(&target)?).into_response())
                }
                SignInStep::NeedsSecondFactor(session) => {
                    Ok((session, format::redirect(sign_in::SECOND_FACTOR_PATH)?).into_response())
                }
            }
        }
        InviteRegistration::Invalid(errors) => {
            render_register(
                &ctx,
                &v,
                &token,
                &form.name,
                Some(&errors),
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        InviteRegistration::AccountExists => {
            render_register(
                &ctx,
                &v,
                &token,
                &form.name,
                None,
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        InviteRegistration::InvalidInvite => views::invitations::invalid(&v),
    }
}

/// Halaman daftar untuk undangan `token` yang masih berlaku, atau halaman tidak berlaku.
async fn render_register(
    ctx: &AppContext,
    v: &TeraView,
    token: &str,
    name: &str,
    errors: Option<&ModelValidationErrors>,
    status: StatusCode,
) -> Result<Response> {
    let Some(pending) = accept_invite::find(&ctx.db, token).await? else {
        return views::invitations::invalid(v);
    };
    let account_exists = users::Model::find_by_email(&ctx.db, &pending.invitation.email)
        .await?
        .is_some();
    let page = RegisterPage::new(
        invitation_view(&pending, token),
        account_exists,
        name,
        errors,
    );
    views::invitations::register(v, &page, status)
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/invitations")
        .add("/{token}", get(show))
        .add("/{token}/accept", post(accept))
        .add("/{token}/register", get(register_form).post(register))
}
