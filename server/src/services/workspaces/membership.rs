//! Keanggotaan workspace (Pengaturan → Anggota, dan "Keluar dari workspace"): ganti role, ganti
//! jabatan, keluarkan anggota, dan keluar sendiri, masing-masing dengan audit `member.*` di
//! transaksi yang sama.
//!
//! Dua aturan dijaga di sini:
//! - **Owner terakhir** ([`ensure_not_last_owner`]): workspace selalu punya minimal satu Owner.
//!   Satu-satunya fungsi untuk aturan ini, dipanggil ganti role, keluarkan, dan keluar. Setiap
//!   operasi lebih dulu mengunci baris workspace (`workspaces::Model::lock_for_membership`),
//!   sehingga dua Owner yang turun bersamaan berjalan berurutan dan yang kedua melihat dirinya
//!   Owner terakhir. Tanpa kunci, keduanya menghitung dua Owner dan workspace kehilangan Owner.
//! - **Penugasan role** (`roles::can_assign`): pelaku hanya memberi atau mencabut role yang boleh
//!   ia berikan, jadi non-Owner tidak bisa menyentuh Owner.
//! - **Izin pelaku dibaca ulang setelah kunci**, bukan dari awal request: pelaku yang role-nya
//!   tidak lagi memegang `ManageMembers` (diturunkan di tengah jalan) mendapat `NotFound`, sama
//!   dengan guard `Require<ManageMembers>`. Pengecualian: [`leave`], yang terbuka untuk semua
//!   anggota.

use loco_rs::app::AppContext;
use sea_orm::{DatabaseTransaction, DbErr, ModelTrait, TransactionTrait};
use uuid::Uuid;

use super::Acting;
use crate::{
    models::{
        job_titles,
        permission::Permission,
        roles::{self, MEMBER_ROLE_NOT_ASSIGNABLE},
        workspace_members,
        workspaces::{self, WorkspaceId},
    },
    services::audit::{self, AuditAction, AuditSubject},
};

/// Perubahan yang membuat workspace tanpa Owner.
pub const WORKSPACE_LAST_OWNER: &str = "workspace-last-owner";
/// Kode error milik service ini; masing-masing punya pesan di Fluent (dijaga tes i18n).
pub const ERROR_CODES: &[&str] = &[WORKSPACE_LAST_OWNER];

#[derive(Debug, thiserror::Error)]
pub enum MembershipError {
    /// Member, role, atau jabatan tidak ada di workspace ini (atau pelaku sudah bukan anggota).
    #[error("not found in workspace")]
    NotFound,
    /// Pelaku tidak boleh memberi atau mencabut role itu (`member-role-not-assignable`).
    #[error("role not assignable by actor")]
    NotAssignable,
    /// Perubahan akan menghapus Owner terakhir (`workspace-last-owner`).
    #[error("workspace would lose its last owner")]
    LastOwner,
    #[error(transparent)]
    Db(#[from] DbErr),
}

impl MembershipError {
    /// Kode Fluent untuk penolakan yang ditampilkan ke user.
    #[must_use]
    pub fn code(&self) -> Option<&'static str> {
        match self {
            Self::NotAssignable => Some(MEMBER_ROLE_NOT_ASSIGNABLE),
            Self::LastOwner => Some(WORKSPACE_LAST_OWNER),
            Self::NotFound | Self::Db(_) => None,
        }
    }
}

impl From<MembershipError> for loco_rs::Error {
    fn from(err: MembershipError) -> Self {
        match err {
            MembershipError::NotFound => Self::NotFound,
            MembershipError::NotAssignable | MembershipError::LastOwner => {
                Self::BadRequest(err.to_string())
            }
            MembershipError::Db(e) => Self::DB(e),
        }
    }
}

/// Ganti role member `member_id` menjadi `role_id`. Role yang sama tidak menulis apa pun.
///
/// # Errors
///
/// [`MembershipError::NotFound`], [`MembershipError::NotAssignable`] (role asal atau tujuan),
/// [`MembershipError::LastOwner`], atau error database.
pub async fn change_role(
    ctx: &AppContext,
    acting: &Acting<'_>,
    member_id: Uuid,
    role_id: Uuid,
) -> Result<workspace_members::Model, MembershipError> {
    let wid = acting.workspace_id();
    let locked = Locked::begin(ctx, acting, Need::ManageMembers).await?;
    let (member, current) = find_member(&locked.txn, wid, member_id).await?;
    let target = roles::Model::find_in(&locked.txn, wid, role_id)
        .await?
        .ok_or(MembershipError::NotFound)?;
    if !roles::can_assign(&locked.actor_role, &current)
        || !roles::can_assign(&locked.actor_role, &target)
    {
        return Err(MembershipError::NotAssignable);
    }
    if current.id == target.id {
        locked.txn.rollback().await?;
        return Ok(member);
    }
    ensure_not_last_owner(&locked.txn, wid, &current, Some(&target)).await?;
    let user_id = member.user_id;
    let member = member.set_role(&locked.txn, target.id).await?;
    audit::record(
        &locked.txn,
        acting.actor().audit(
            AuditAction::MemberRoleChanged,
            (AuditSubject::Member, member.id),
            serde_json::json!({
                "user_id": user_id,
                "role": audit::change(&current.name, &target.name),
            }),
        ),
    )
    .await?;
    locked.txn.commit().await?;
    Ok(member)
}

/// Ganti jabatan member `member_id` (`None` = tanpa jabatan). Jabatan yang sama tidak menulis
/// apa pun. Jabatan adalah atribut member, jadi member yang role-nya tidak boleh disentuh
/// pelaku (mis. Owner bagi Admin) juga tidak boleh diubah jabatannya.
///
/// # Errors
///
/// [`MembershipError::NotFound`] (member atau jabatan), [`MembershipError::NotAssignable`],
/// atau error database.
pub async fn set_job_title(
    ctx: &AppContext,
    acting: &Acting<'_>,
    member_id: Uuid,
    job_title_id: Option<Uuid>,
) -> Result<workspace_members::Model, MembershipError> {
    let wid = acting.workspace_id();
    let locked = Locked::begin(ctx, acting, Need::ManageMembers).await?;
    let (member, current_role) = find_member(&locked.txn, wid, member_id).await?;
    if !roles::can_assign(&locked.actor_role, &current_role) {
        return Err(MembershipError::NotAssignable);
    }
    let to = title_name(&locked.txn, wid, job_title_id).await?;
    if member.job_title_id == job_title_id {
        locked.txn.rollback().await?;
        return Ok(member);
    }
    let from = title_name(&locked.txn, wid, member.job_title_id).await?;
    let member = member.set_job_title(&locked.txn, job_title_id).await?;
    audit::record(
        &locked.txn,
        acting.actor().audit(
            AuditAction::MemberJobTitleChanged,
            (AuditSubject::Member, member.id),
            serde_json::json!({
                "user_id": member.user_id,
                "job_title": { "from": from, "to": to },
            }),
        ),
    )
    .await?;
    locked.txn.commit().await?;
    Ok(member)
}

/// Keluarkan member `member_id`. Berlaku seketika: `Tenant` memeriksa keanggotaan di setiap
/// request, jadi request berikutnya dari user itu mendapat 404.
///
/// # Errors
///
/// [`MembershipError::NotFound`], [`MembershipError::NotAssignable`],
/// [`MembershipError::LastOwner`], atau error database.
pub async fn remove(
    ctx: &AppContext,
    acting: &Acting<'_>,
    member_id: Uuid,
) -> Result<(), MembershipError> {
    let wid = acting.workspace_id();
    let locked = Locked::begin(ctx, acting, Need::ManageMembers).await?;
    let (member, role) = find_member(&locked.txn, wid, member_id).await?;
    if !roles::can_assign(&locked.actor_role, &role) {
        return Err(MembershipError::NotAssignable);
    }
    ensure_not_last_owner(&locked.txn, wid, &role, None).await?;
    let changes = serde_json::json!({ "user_id": member.user_id, "role": role.name });
    let id = member.id;
    member.delete(&locked.txn).await?;
    audit::record(
        &locked.txn,
        acting.actor().audit(
            AuditAction::MemberRemoved,
            (AuditSubject::Member, id),
            changes,
        ),
    )
    .await?;
    locked.txn.commit().await?;
    Ok(())
}

/// Pelaku keluar dari workspace-nya sendiri. Terbuka untuk semua role, kecuali Owner terakhir.
///
/// # Errors
///
/// [`MembershipError::LastOwner`], [`MembershipError::NotFound`] bila pelaku sudah bukan
/// anggota, atau error database.
pub async fn leave(ctx: &AppContext, acting: &Acting<'_>) -> Result<(), MembershipError> {
    let wid = acting.workspace_id();
    let locked = Locked::begin(ctx, acting, Need::Membership).await?;
    ensure_not_last_owner(&locked.txn, wid, &locked.actor_role, None).await?;
    let changes = serde_json::json!({
        "user_id": locked.actor_member.user_id,
        "role": locked.actor_role.name,
    });
    let id = locked.actor_member.id;
    locked.actor_member.delete(&locked.txn).await?;
    audit::record(
        &locked.txn,
        acting
            .actor()
            .audit(AuditAction::MemberLeft, (AuditSubject::Member, id), changes),
    )
    .await?;
    locked.txn.commit().await?;
    Ok(())
}

/// Satu-satunya aturan "workspace tidak boleh kehilangan Owner terakhir": member ber-role
/// `current` yang akan ber-role `after` (`None` = keluar/dikeluarkan) ditolak bila ia Owner,
/// tidak lagi Owner sesudahnya, dan Owner lain tidak ada. Pemanggil harus sudah mengunci baris
/// workspace di transaksi `txn` ([`Locked::begin`]), supaya hitungannya tidak basi.
///
/// # Errors
///
/// [`MembershipError::LastOwner`] atau error database.
pub async fn ensure_not_last_owner(
    txn: &DatabaseTransaction,
    workspace_id: WorkspaceId,
    current: &roles::Model,
    after: Option<&roles::Model>,
) -> Result<(), MembershipError> {
    let stays_owner = after.is_some_and(roles::Model::is_owner);
    if !current.is_owner() || stays_owner {
        return Ok(());
    }
    if workspace_members::Model::owner_count(txn, workspace_id).await? <= 1 {
        return Err(MembershipError::LastOwner);
    }
    Ok(())
}

/// Izin pelaku yang dicek ulang [`Locked::begin`] setelah kunci.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Need {
    /// Kelola anggota lain (`ManageMembers`).
    ManageMembers,
    /// Cukup masih anggota (keluar sendiri).
    Membership,
}

/// Transaksi dengan baris workspace terkunci, beserta keanggotaan dan role pelaku terbaru
/// (dibaca setelah kunci).
struct Locked {
    txn: DatabaseTransaction,
    actor_member: workspace_members::Model,
    actor_role: roles::Model,
}

impl Locked {
    /// `NotFound` bila pelaku sudah bukan anggota, atau role terbarunya tidak memenuhi `need`.
    async fn begin(
        ctx: &AppContext,
        acting: &Acting<'_>,
        need: Need,
    ) -> Result<Self, MembershipError> {
        let wid = acting.workspace_id();
        let txn = ctx.db.begin().await?;
        workspaces::Model::lock_for_membership(&txn, wid).await?;
        let (actor_member, actor_role) =
            workspace_members::Model::find_with_role(&txn, wid, acting.user.id)
                .await?
                .ok_or(MembershipError::NotFound)?;
        if need == Need::ManageMembers && !actor_role.has(Permission::ManageMembers) {
            return Err(MembershipError::NotFound);
        }
        Ok(Self {
            txn,
            actor_member,
            actor_role,
        })
    }
}

async fn find_member(
    txn: &DatabaseTransaction,
    workspace_id: WorkspaceId,
    member_id: Uuid,
) -> Result<(workspace_members::Model, roles::Model), MembershipError> {
    workspace_members::Model::find_in(txn, workspace_id, member_id)
        .await?
        .ok_or(MembershipError::NotFound)
}

/// Nama jabatan `id` di workspace ini (`None` untuk tanpa jabatan), atau `NotFound`.
async fn title_name(
    txn: &DatabaseTransaction,
    workspace_id: WorkspaceId,
    id: Option<Uuid>,
) -> Result<Option<String>, MembershipError> {
    let Some(id) = id else {
        return Ok(None);
    };
    job_titles::Model::find_in(txn, workspace_id, id)
        .await?
        .map(|title| Some(title.name))
        .ok_or(MembershipError::NotFound)
}
