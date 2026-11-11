//! Undang lewat email (Pengaturan → Anggota): undang, kirim ulang, dan batalkan.
//!
//! - [`run`]: email dinormalisasi; anggota yang sudah ada ditolak (`invite-already-member`);
//!   role harus boleh diberikan pelaku (`roles::can_assign`). Undangan tertunda lama untuk email
//!   yang sama dibatalkan dan yang baru dibuat di satu transaksi bersama audit `member.invited`.
//! - [`resend`]: token baru untuk undangan tertunda (yang lama dibatalkan), audit
//!   `member.invited` dengan `resent: true`.
//! - [`revoke`]: batalkan undangan tertunda, audit `member.invitation_revoked`.
//!
//! Undangan yang role-nya tidak boleh diberikan pelaku (mis. undangan Owner bagi Admin) tidak
//! bisa disentuh pelaku itu: kirim ulang, batalkan, maupun undang ulang email yang sama (yang
//! akan membatalkannya) ditolak `member-role-not-assignable`.
//!
//! Email dikirim **setelah** commit lewat job (rules §5, §8). Kelas efek samping: best-effort.
//! Bila antrean gagal, kegagalannya dicatat dan request tetap berhasil; pengundang bisa
//! "kirim ulang" dari halaman Anggota. Token mentah hanya ada di args job email, tidak di log
//! maupun audit.

use loco_rs::{app::AppContext, validation::ModelValidationErrors};
use sea_orm::{DatabaseTransaction, DbErr, SqlErr, TransactionTrait};
use uuid::Uuid;
use validator::{ValidateEmail, ValidationError, ValidationErrors};

use super::Acting;
use crate::{
    common::settings::Settings,
    mailers::invitation::{InvitationMail, InvitationMailer},
    models::{
        invitations::{self, NewInvitation},
        job_titles,
        roles::{self, MEMBER_ROLE_NOT_ASSIGNABLE},
        users::{self, EMAIL_INVALID},
        workspace_members,
    },
    services::audit::{self, AuditAction, AuditSubject},
};

/// Email yang diundang sudah anggota workspace ini (field `email`).
pub const INVITE_ALREADY_MEMBER: &str = "invite-already-member";
/// Kode error milik service ini; masing-masing punya pesan di Fluent (dijaga tes i18n).
pub const ERROR_CODES: &[&str] = &[INVITE_ALREADY_MEMBER];

/// Isian form undang.
#[derive(Debug, Clone)]
pub struct InviteParams {
    pub email: String,
    pub role_id: Uuid,
    pub job_title_id: Option<Uuid>,
}

#[derive(Debug, thiserror::Error)]
pub enum InviteError {
    /// Field form tidak valid (`email`, `role_id`); tidak ada yang dibuat atau dikirim.
    #[error("invalid invitation: {0}")]
    Invalid(ModelValidationErrors),
    /// Role, jabatan, atau undangan tertunda tidak ada di workspace ini.
    #[error("not found in workspace")]
    NotFound,
    /// Kirim ulang undangan dengan role yang tidak boleh diberikan pelaku.
    #[error("role not assignable by actor")]
    NotAssignable,
    #[error(transparent)]
    Db(#[from] DbErr),
    /// Settings tidak terbaca.
    #[error(transparent)]
    App(#[from] loco_rs::Error),
}

impl From<InviteError> for loco_rs::Error {
    fn from(err: InviteError) -> Self {
        match err {
            InviteError::Invalid(errors) => Self::Validation(errors),
            InviteError::NotFound => Self::NotFound,
            InviteError::NotAssignable => Self::BadRequest(err.to_string()),
            InviteError::Db(e) => Self::DB(e),
            InviteError::App(e) => e,
        }
    }
}

/// Undang `params.email` ke workspace pelaku.
///
/// # Errors
///
/// [`InviteError::Invalid`] (email tidak valid, sudah anggota, role tidak boleh diberikan),
/// [`InviteError::NotFound`] (role/jabatan workspace lain), atau error database.
pub async fn run(
    ctx: &AppContext,
    acting: &Acting<'_>,
    params: InviteParams,
) -> Result<invitations::Model, InviteError> {
    let wid = acting.workspace_id();
    let email = users::normalize_email(&params.email);
    let role = roles::Model::find_in(&ctx.db, wid, params.role_id)
        .await?
        .ok_or(InviteError::NotFound)?;
    if let Some(id) = params.job_title_id {
        job_titles::Model::find_in(&ctx.db, wid, id)
            .await?
            .ok_or(InviteError::NotFound)?;
    }

    let mut errors = ValidationErrors::new();
    if !email.validate_email() {
        errors.add("email", ValidationError::new(EMAIL_INVALID));
    } else if is_member(ctx, acting, &email).await? {
        errors.add("email", ValidationError::new(INVITE_ALREADY_MEMBER));
    }
    if !roles::can_assign(acting.role, &role) {
        errors.add("role_id", ValidationError::new(MEMBER_ROLE_NOT_ASSIGNABLE));
    }
    if !errors.is_empty() {
        return Err(InviteError::Invalid(errors.into()));
    }

    let new = NewInvitation {
        email,
        role_id: role.id,
        job_title_id: params.job_title_id,
        invited_by: acting.user.id,
        ttl: ttl(ctx)?,
    };
    // Undangan bersamaan untuk email yang sama: yang kalah melanggar partial unique index
    // setelah yang menang commit; percobaan kedua membatalkan undangan itu lalu membuat baru.
    let mut attempt = 0;
    let (invitation, raw) = loop {
        attempt += 1;
        let txn = ctx.db.begin().await?;
        match issue_in(&txn, acting, new.clone(), &role, false).await {
            Ok(issued) => {
                txn.commit().await?;
                break issued;
            }
            Err(InviteError::Db(e))
                if attempt < 2
                    && matches!(e.sql_err(), Some(SqlErr::UniqueConstraintViolation(_))) =>
            {
                txn.rollback().await?;
            }
            Err(InviteError::NotAssignable) => {
                let mut errors = ValidationErrors::new();
                errors.add("email", ValidationError::new(MEMBER_ROLE_NOT_ASSIGNABLE));
                return Err(InviteError::Invalid(errors.into()));
            }
            Err(e) => return Err(e),
        }
    };
    send(ctx, acting, &invitation, &role, &raw).await;
    Ok(invitation)
}

/// Kirim ulang undangan tertunda `id` dengan token baru; token lama tidak berlaku lagi.
///
/// # Errors
///
/// [`InviteError::NotFound`] (bukan undangan tertunda di workspace ini),
/// [`InviteError::NotAssignable`], atau error database.
pub async fn resend(
    ctx: &AppContext,
    acting: &Acting<'_>,
    id: Uuid,
) -> Result<invitations::Model, InviteError> {
    let wid = acting.workspace_id();
    let txn = ctx.db.begin().await?;
    let current = invitations::Model::lock_pending_in(&txn, wid, id)
        .await?
        .ok_or(InviteError::NotFound)?;
    let role = ensure_can_touch(&txn, acting, &current).await?;
    let new = NewInvitation {
        email: current.email,
        role_id: current.role_id,
        job_title_id: current.job_title_id,
        invited_by: acting.user.id,
        ttl: ttl(ctx)?,
    };
    let (invitation, raw) = issue_in(&txn, acting, new, &role, true).await?;
    txn.commit().await?;
    send(ctx, acting, &invitation, &role, &raw).await;
    Ok(invitation)
}

/// Batalkan undangan tertunda `id`: link-nya tidak berlaku lagi.
///
/// # Errors
///
/// [`InviteError::NotFound`] (bukan undangan tertunda di workspace ini),
/// [`InviteError::NotAssignable`], atau error database.
pub async fn revoke(ctx: &AppContext, acting: &Acting<'_>, id: Uuid) -> Result<(), InviteError> {
    let txn = ctx.db.begin().await?;
    let current = invitations::Model::lock_pending_in(&txn, acting.workspace_id(), id)
        .await?
        .ok_or(InviteError::NotFound)?;
    ensure_can_touch(&txn, acting, &current).await?;
    let revoked = current.revoke(&txn).await?;
    audit::record(
        &txn,
        acting.actor().audit(
            AuditAction::MemberInvitationRevoked,
            (AuditSubject::Invitation, revoked.id),
            serde_json::json!({ "email": revoked.email }),
        ),
    )
    .await?;
    txn.commit().await?;
    Ok(())
}

/// Pelaku boleh memberikan role undangan `invitation` (`roles::can_assign`).
async fn ensure_can_touch(
    txn: &DatabaseTransaction,
    acting: &Acting<'_>,
    invitation: &invitations::Model,
) -> Result<roles::Model, InviteError> {
    let role = roles::Model::find_in(txn, acting.workspace_id(), invitation.role_id)
        .await?
        .ok_or(InviteError::NotFound)?;
    if roles::can_assign(acting.role, &role) {
        Ok(role)
    } else {
        Err(InviteError::NotAssignable)
    }
}

/// Batalkan undangan tertunda untuk email itu, buat yang baru, dan tulis audit-nya di `txn`.
/// Undangan tertunda yang role-nya tidak boleh diberikan pelaku tidak dibatalkan diam-diam:
/// [`InviteError::NotAssignable`].
async fn issue_in(
    txn: &DatabaseTransaction,
    acting: &Acting<'_>,
    new: NewInvitation,
    role: &roles::Model,
    resent: bool,
) -> Result<(invitations::Model, String), InviteError> {
    let wid = acting.workspace_id();
    for pending in invitations::Model::lock_pending_for(txn, wid, &new.email).await? {
        ensure_can_touch(txn, acting, &pending).await?;
    }
    invitations::Model::revoke_pending_for(txn, wid, &new.email).await?;
    let (invitation, raw) = invitations::ActiveModel::issue(txn, wid, new).await?;
    let mut changes = serde_json::json!({ "email": invitation.email, "role": role.name });
    if resent {
        changes["resent"] = serde_json::Value::Bool(true);
    }
    audit::record(
        txn,
        acting.actor().audit(
            AuditAction::MemberInvited,
            (AuditSubject::Invitation, invitation.id),
            changes,
        ),
    )
    .await?;
    Ok((invitation, raw))
}

/// `email` sudah anggota workspace pelaku.
async fn is_member(ctx: &AppContext, acting: &Acting<'_>, email: &str) -> Result<bool, DbErr> {
    let Some(user) = users::Model::find_by_email(&ctx.db, email).await? else {
        return Ok(false);
    };
    Ok(
        workspace_members::Model::find_with_role(&ctx.db, acting.workspace_id(), user.id)
            .await?
            .is_some(),
    )
}

/// Umur undangan (`settings.auth.invitation_days`).
fn ttl(ctx: &AppContext) -> Result<chrono::Duration, InviteError> {
    let days = Settings::from_ctx(ctx)?.auth.invitation_days;
    Ok(chrono::Duration::days(days))
}

/// Antrekan email undangan (setelah commit). Best-effort: kegagalan dicatat tanpa token, dan
/// undangan bisa dikirim ulang.
async fn send(
    ctx: &AppContext,
    acting: &Acting<'_>,
    invitation: &invitations::Model,
    role: &roles::Model,
    raw: &str,
) {
    let mail = InvitationMail {
        to: invitation.email.clone(),
        workspace_name: acting.workspace.name.clone(),
        inviter_name: acting.user.name.clone(),
        role_name: role.name.clone(),
    };
    if let Err(e) = InvitationMailer::send_invitation(ctx, &mail, raw).await {
        tracing::error!(
            invitation_id = %invitation.id,
            workspace_id = %invitation.workspace_id,
            error = %e,
            "invitation email could not be enqueued; can be resent from the members page"
        );
    }
}
