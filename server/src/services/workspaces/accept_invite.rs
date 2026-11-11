//! Menerima undangan dari link email (`/invitations/{token}`, route publik).
//!
//! - [`find`]: data halaman undangan tanpa mengonsumsi token (pemindai link email membuka link
//!   tanpa diklik user, rules §7).
//! - [`run`]: user yang login menerima undangan. Email user harus sama dengan email undangan
//!   (tanpa membedakan huruf besar, `invite-email-mismatch`): token bisa diteruskan ke orang
//!   lain, jadi token saja tidak cukup untuk bergabung dengan akun lain. Token di-claim
//!   bersyarat (rules §5); user yang sudah anggota tetap menyelesaikan undangan tanpa mengubah
//!   role-nya. Audit `member.joined`.
//! - [`register`]: daftar lewat undangan. Email diambil dari undangan, bukan dari form, dan
//!   langsung terverifikasi: token sampai lewat inbox itu. Pembuatan akun memakai
//!   `registration::create_account` yang sama dengan pendaftaran publik, tetapi tidak
//!   bergantung pada `registration_open` (undangan adalah jalan masuk yang disengaja). Email yang
//!   sudah punya akun diarahkan ke login; tidak ada enumerasi akun karena pemegang token sudah
//!   tahu email-nya.

use loco_rs::{Result, app::AppContext, validation::ModelValidationErrors};
use sea_orm::{ConnectionTrait, DbErr, TransactionTrait};
use secrecy::SecretString;

use crate::{
    common::settings::Settings,
    models::{invitations, roles, users, workspace_members, workspaces},
    services::{
        audit::{self, AuditAction, AuditEntry, AuditSubject},
        auth::registration::{self, Account, Registration},
    },
};

/// User yang login bukan pemilik email undangan.
pub const INVITE_EMAIL_MISMATCH: &str = "invite-email-mismatch";
/// Kode error milik service ini; masing-masing punya pesan di Fluent (dijaga tes i18n).
pub const ERROR_CODES: &[&str] = &[INVITE_EMAIL_MISMATCH];

/// Undangan yang masih berlaku beserta yang ditampilkan halaman undangan.
#[derive(Debug, Clone)]
pub struct PendingInvitation {
    pub invitation: invitations::Model,
    pub workspace: workspaces::Model,
    pub role_name: String,
    /// `None` bila pengundang sudah dihapus.
    pub inviter_name: Option<String>,
}

impl PendingInvitation {
    /// Undangan ini untuk `user` (email sama setelah normalisasi).
    #[must_use]
    pub fn is_for(&self, user: &users::Model) -> bool {
        users::normalize_email(&user.email) == self.invitation.email
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AcceptError {
    /// Token tidak ada, kedaluwarsa, dibatalkan, atau sudah dipakai (sengaja tidak dibedakan).
    #[error("invitation is not valid")]
    Invalid,
    /// Email user berbeda dengan email undangan.
    #[error("invitation is for another email")]
    EmailMismatch,
    #[error(transparent)]
    Db(#[from] DbErr),
}

/// Undangan bertoken `raw` bila masih berlaku, tanpa mengonsumsinya. Workspace yang dijadwalkan
/// hapus dianggap tidak berlaku: anggota baru tidak akan bisa membukanya.
///
/// # Errors
///
/// Error database.
pub async fn find<C: ConnectionTrait>(
    db: &C,
    raw: &str,
) -> Result<Option<PendingInvitation>, DbErr> {
    let Some(invitation) = invitations::Model::find_usable(db, raw).await? else {
        return Ok(None);
    };
    let Some(workspace) = workspaces::Model::find_by_id(db, invitation.workspace_id)
        .await?
        .filter(|w| w.deletion_scheduled_at.is_none())
    else {
        return Ok(None);
    };
    let Some(role) =
        roles::Model::find_in(db, workspace.workspace_id(), invitation.role_id).await?
    else {
        return Ok(None);
    };
    let inviter_name = match invitation.invited_by {
        Some(id) => users::Model::find_by_id(db, id).await?.map(|u| u.name),
        None => None,
    };
    Ok(Some(PendingInvitation {
        invitation,
        workspace,
        role_name: role.name,
        inviter_name,
    }))
}

/// `user` menerima undangan bertoken `raw` dan bergabung dengan role dan jabatan undangan.
/// Mengembalikan workspace-nya (untuk redirect ke `/w/{slug}`).
///
/// # Errors
///
/// [`AcceptError::Invalid`], [`AcceptError::EmailMismatch`], atau error database.
pub async fn run(
    ctx: &AppContext,
    raw: &str,
    user: &users::Model,
    request_id: Option<String>,
) -> std::result::Result<workspaces::Model, AcceptError> {
    let pending = find(&ctx.db, raw).await?.ok_or(AcceptError::Invalid)?;
    if !pending.is_for(user) {
        return Err(AcceptError::EmailMismatch);
    }
    let wid = pending.workspace.workspace_id();
    let txn = ctx.db.begin().await?;
    // Claim bersyarat: request lain yang menerima token yang sama lebih dulu membuat ini None.
    let invitation = invitations::Model::claim(&txn, raw)
        .await?
        .ok_or(AcceptError::Invalid)?;
    let joined = workspace_members::ActiveModel::add_unless_member(
        &txn,
        wid,
        user.id,
        invitation.role_id,
        invitation.job_title_id,
    )
    .await?;
    // Sudah anggota (bergabung lewat jalan lain): undangan selesai tanpa mengubah role.
    if let Some(member) = joined {
        audit::record(
            &txn,
            AuditEntry {
                workspace_id: wid,
                actor_id: Some(user.id),
                action: AuditAction::MemberJoined,
                subject: (AuditSubject::Member, member.id),
                changes: serde_json::json!({
                    "invitation_id": invitation.id,
                    "role": pending.role_name,
                }),
                request_id,
            },
        )
        .await?;
    }
    txn.commit().await?;
    Ok(pending.workspace)
}

/// Isian form daftar lewat undangan. Email tidak ada di sini: selalu email undangan.
#[derive(Debug)]
pub struct InviteSignup {
    pub name: String,
    pub password: SecretString,
}

/// User baru dan workspace yang baru ia masuki ([`InviteRegistration::Joined`]).
#[derive(Debug)]
pub struct Joined {
    pub user: users::Model,
    pub workspace: workspaces::Model,
}

/// Hasil [`register`].
#[derive(Debug)]
pub enum InviteRegistration {
    /// Akun dibuat (terverifikasi) dan sudah bergabung: pemanggil memulai sesi login.
    Joined(Box<Joined>),
    /// Field tidak valid; tidak ada yang dibuat.
    Invalid(ModelValidationErrors),
    /// Email undangan sudah punya akun: masuk dulu, lalu terima undangan.
    AccountExists,
    /// Undangan tidak berlaku.
    InvalidInvite,
}

/// Daftar dengan email undangan `raw`, tandai email terverifikasi, lalu terima undangannya.
/// Bila undangan dibatalkan di antara pembuatan akun dan claim, akun tetap ada (email-nya
/// sudah terbukti) tetapi tidak bergabung: hasilnya `InvalidInvite`.
///
/// # Errors
///
/// Error database atau hashing (503).
pub async fn register(
    ctx: &AppContext,
    settings: &Settings,
    raw: &str,
    form: InviteSignup,
    request_id: Option<String>,
) -> Result<InviteRegistration> {
    let Some(pending) = find(&ctx.db, raw).await? else {
        return Ok(InviteRegistration::InvalidInvite);
    };
    if users::Model::find_by_email(&ctx.db, &pending.invitation.email)
        .await?
        .is_some()
    {
        return Ok(InviteRegistration::AccountExists);
    }
    let registration = Registration {
        name: form.name,
        email: pending.invitation.email,
        password: form.password,
    };
    if let Err(errors) = registration::validate(&registration) {
        return Ok(InviteRegistration::Invalid(errors));
    }
    let user = match registration::create_account(ctx, settings, &registration).await? {
        Account::Created(user) => user.mark_email_verified(&ctx.db).await?,
        // Akun dibuat request lain di antara pengecekan dan insert.
        Account::Existing(_) => return Ok(InviteRegistration::AccountExists),
    };
    match run(ctx, raw, &user, request_id).await {
        Ok(workspace) => Ok(InviteRegistration::Joined(Box::new(Joined {
            user,
            workspace,
        }))),
        Err(AcceptError::Invalid | AcceptError::EmailMismatch) => {
            Ok(InviteRegistration::InvalidInvite)
        }
        Err(AcceptError::Db(e)) => Err(e.into()),
    }
}
