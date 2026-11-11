//! Pengaturan → Anggota: tabel anggota (ganti role dan jabatan, keluarkan), undangan tertunda
//! (kirim ulang, batalkan), dan form undang. Juga halaman konfirmasi keluarkan anggota dan
//! keluar dari workspace (konfirmasi lewat halaman biasa, tanpa JS).
use axum::http::StatusCode;
use loco_rs::{prelude::*, validation::ModelValidationErrors};
use serde::Serialize;

use super::{
    flash::Flash,
    form::{FieldErrors, field_errors},
    layout::{AppShell, app_page},
};
use crate::models::{invitations, job_titles, roles, users, workspace_members};

pub const FLASH_INVITATION_SENT: &str = "flash-invitation-sent";
pub const FLASH_INVITATION_RESENT: &str = "flash-invitation-resent";
pub const FLASH_INVITATION_REVOKED: &str = "flash-invitation-revoked";
pub const FLASH_ROLE_CHANGED: &str = "flash-member-role-changed";
pub const FLASH_JOB_TITLE_CHANGED: &str = "flash-member-job-title-changed";
pub const FLASH_REMOVED: &str = "flash-member-removed";
/// Kunci Fluent yang dikirim sebagai variabel ke template (dijaga tes i18n).
pub const MESSAGE_KEYS: &[&str] = &[
    FLASH_INVITATION_SENT,
    FLASH_INVITATION_RESENT,
    FLASH_INVITATION_REVOKED,
    FLASH_ROLE_CHANGED,
    FLASH_JOB_TITLE_CHANGED,
    FLASH_REMOVED,
];

/// Field form undang, urutan sama dengan template.
pub const INVITE_FIELDS: &[&str] = &["email", "role_id"];

/// Format tanggal di tabel (bergabung, kedaluwarsa).
const DATE_FORMAT: &str = "%Y-%m-%d";

/// Satu pilihan `<select>` (role atau jabatan).
#[derive(Debug, Clone, Serialize)]
pub struct OptionView {
    pub id: String,
    pub name: String,
}

impl OptionView {
    #[must_use]
    pub fn new(id: impl ToString, name: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
        }
    }

    #[must_use]
    pub fn role(role: &roles::Model) -> Self {
        Self {
            id: role.id.to_string(),
            name: role.name.clone(),
        }
    }

    #[must_use]
    pub fn job_title(title: &job_titles::Model) -> Self {
        Self {
            id: title.id.to_string(),
            name: title.name.clone(),
        }
    }
}

/// Satu anggota. `manageable` = pelaku boleh mengganti role/jabatan dan mengeluarkannya
/// (aturan `roles::can_assign`, dihitung controller); bila tidak, baris hanya teks.
#[derive(Debug, Serialize)]
pub struct MemberRow {
    pub id: String,
    pub name: String,
    pub email: String,
    /// Inisial avatar (lihat `views::layout::UserView`).
    pub initials: String,
    pub role_id: String,
    pub role_name: String,
    /// Id jabatan, `""` bila tanpa jabatan (nilai opsi "tanpa jabatan").
    pub job_title_id: String,
    pub job_title_name: String,
    pub joined: String,
    pub manageable: bool,
    pub role_action: String,
    pub job_title_action: String,
    pub remove_href: String,
}

impl MemberRow {
    /// `base` = path halaman anggota (`/w/{slug}/settings/members`).
    #[must_use]
    pub fn new(
        member: &workspace_members::Model,
        user: &users::Model,
        role: &roles::Model,
        job_title: Option<&job_titles::Model>,
        manageable: bool,
        base: &str,
    ) -> Self {
        Self {
            id: member.id.to_string(),
            name: user.name.clone(),
            email: user.email.clone(),
            initials: super::layout::UserView::new(&user.name, &user.email).initials,
            role_id: role.id.to_string(),
            role_name: role.name.clone(),
            job_title_id: job_title.map(|t| t.id.to_string()).unwrap_or_default(),
            job_title_name: job_title.map(|t| t.name.clone()).unwrap_or_default(),
            joined: member.joined_at.format(DATE_FORMAT).to_string(),
            manageable,
            role_action: format!("{base}/{}/role", member.id),
            job_title_action: format!("{base}/{}/job-title", member.id),
            remove_href: format!("{base}/{}/remove", member.id),
        }
    }
}

/// Satu undangan tertunda. `manageable` = pelaku boleh mengirim ulang atau membatalkannya
/// (role undangan boleh ia berikan, dihitung controller).
#[derive(Debug, Serialize)]
pub struct InvitationRow {
    pub manageable: bool,
    pub email: String,
    pub role_name: String,
    pub expires: String,
    pub expired: bool,
    pub resend_action: String,
    pub revoke_action: String,
}

impl InvitationRow {
    /// `invite_action` = path POST undang (`…/members/invitations`).
    #[must_use]
    pub fn new(
        invitation: &invitations::Model,
        role_name: &str,
        manageable: bool,
        invite_action: &str,
    ) -> Self {
        Self {
            manageable,
            email: invitation.email.clone(),
            role_name: role_name.to_string(),
            expires: invitation.expires_at.format(DATE_FORMAT).to_string(),
            expired: invitation.expires_at.to_utc() <= chrono::Utc::now(),
            resend_action: format!("{invite_action}/{}/resend", invitation.id),
            revoke_action: format!("{invite_action}/{}/revoke", invitation.id),
        }
    }
}

/// Isian form undang dengan error per field.
#[derive(Debug, Serialize)]
pub struct InviteFormView {
    pub email: String,
    pub role_id: String,
    pub job_title_id: String,
    pub errors: FieldErrors,
}

impl InviteFormView {
    #[must_use]
    pub fn new(
        email: &str,
        role_id: &str,
        job_title_id: &str,
        errors: Option<&ModelValidationErrors>,
    ) -> Self {
        Self {
            email: email.to_string(),
            role_id: role_id.to_string(),
            job_title_id: job_title_id.to_string(),
            errors: field_errors(INVITE_FIELDS, errors),
        }
    }
}

/// Data `workspaces/members/index.html`. `roles` = role yang boleh diberikan pelaku (pilihan
/// form undang dan ganti role).
#[derive(Debug, Serialize)]
pub struct MembersPage {
    pub members: Vec<MemberRow>,
    pub invitations: Vec<InvitationRow>,
    pub roles: Vec<OptionView>,
    pub job_titles: Vec<OptionView>,
    pub invite: InviteFormView,
    pub invite_action: String,
    pub flash: Option<Flash>,
}

pub fn index(
    v: &impl ViewRenderer,
    shell: &AppShell,
    page: &MembersPage,
    status: StatusCode,
) -> Result<Response> {
    format::render()
        .status(status)
        .view(v, "workspaces/members/index.html", app_page(shell, page))
}

/// Konfirmasi keluarkan anggota: nama/email dan tombol POST `action`, kembali ke `back`.
pub fn confirm_remove(
    v: &impl ViewRenderer,
    shell: &AppShell,
    member: &MemberRow,
    back: &str,
) -> Result<Response> {
    format::render().view(
        v,
        "workspaces/members/remove.html",
        app_page(shell, data!({ "member": member, "back": back })),
    )
}

/// Konfirmasi keluar dari workspace. `error`: kunci Fluent (Owner terakhir) atau kosong.
pub fn confirm_leave(
    v: &impl ViewRenderer,
    shell: &AppShell,
    workspace_name: &str,
    action: &str,
    error: &str,
    status: StatusCode,
) -> Result<Response> {
    format::render().status(status).view(
        v,
        "workspaces/leave.html",
        app_page(
            shell,
            data!({ "workspace_name": workspace_name, "action": action, "error": error }),
        ),
    )
}
