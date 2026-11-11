//! Tim (Pengaturan → Tim): buat, ganti nama, hapus, tambah dan keluarkan anggota, masing-masing
//! dengan audit `team.*` di transaksi yang sama. Hapus tim mencabut keanggotaan tim dan grant
//! project tim itu lewat FK `ON DELETE CASCADE` (migrasi teams_projects); grant yang hilang
//! dicatat di audit-nya.
//!
//! Dua aturan dijaga di sini:
//! - **Izin pelaku dibaca ulang setelah kunci** ([`Managing::begin`]), bukan dari awal request:
//!   baris workspace dikunci (berurutan dengan ganti role dan keluarkan anggota, lihat
//!   `membership`) dan role pelaku dibaca ulang dengan kunci baca. Pelaku yang tidak lagi
//!   memegang `ManageTeams` mendapat `NotFound`, sama dengan guard `Require<ManageTeams>`.
//! - **Menambah anggota tidak boleh menaikkan akses** ([`Managing::ensure_can_join`]): anggota
//!   tim mewarisi semua grant tim, jadi menambah orang ke tim sama dengan memberinya grant-grant
//!   itu. Pelaku harus Admin di semua project (`project_access::is_admin_everywhere`), atau
//!   untuk setiap grant tim boleh memberikan role grant itu di project-nya
//!   (`project_access::can_grant` dengan akses project pelaku sendiri). Tanpa aturan ini,
//!   pemegang `ManageTeams` bisa memasukkan dirinya ke tim yang Admin di project mana pun.
//!   Mengeluarkan anggota hanya mengurangi akses, jadi tidak dibatasi.

use loco_rs::validation::ModelValidationErrors;
use sea_orm::{DatabaseConnection, DatabaseTransaction, DbErr, ModelTrait, TransactionTrait};
use uuid::Uuid;

use super::Actor;
use crate::{
    models::{
        permission::Permission,
        project_access::{self, Accessor},
        project_grants, roles, team_members,
        teams::{self, TeamError},
        workspace_members,
        workspaces::{self, WorkspaceId},
    },
    services::audit::{self, AuditAction, AuditSubject},
};

/// Menambah anggota ke tim yang grant-nya melebihi akses project pelaku.
pub const TEAM_MEMBER_NOT_ASSIGNABLE: &str = "team-member-not-assignable";
/// Kode error milik service ini; masing-masing punya pesan di Fluent (dijaga tes i18n).
pub const ERROR_CODES: &[&str] = &[TEAM_MEMBER_NOT_ASSIGNABLE];

#[derive(Debug, thiserror::Error)]
pub enum TeamFailure {
    /// Tim atau orang tidak ada di workspace ini, atau pelaku tidak (lagi) memegang
    /// `ManageTeams`.
    #[error("not found")]
    NotFound,
    /// Tim punya grant di atas akses project pelaku (`team-member-not-assignable`, padanan
    /// tim untuk `member-role-not-assignable`).
    #[error("team access exceeds actor access")]
    NotAssignable,
    /// Validasi field (`name`) gagal.
    #[error("invalid team: {0}")]
    Invalid(ModelValidationErrors),
    #[error(transparent)]
    Db(DbErr),
}

impl TeamFailure {
    /// Kode Fluent untuk penolakan aturan yang ditampilkan sebagai pesan halaman.
    #[must_use]
    pub fn code(&self) -> Option<&'static str> {
        match self {
            Self::NotAssignable => Some(TEAM_MEMBER_NOT_ASSIGNABLE),
            Self::NotFound | Self::Invalid(_) | Self::Db(_) => None,
        }
    }
}

impl From<DbErr> for TeamFailure {
    fn from(err: DbErr) -> Self {
        match err {
            DbErr::RecordNotFound(_) => Self::NotFound,
            other => Self::Db(other),
        }
    }
}

impl From<TeamError> for TeamFailure {
    fn from(err: TeamError) -> Self {
        match err {
            TeamError::Invalid(errors) => Self::Invalid(errors),
            TeamError::Db(e) => e.into(),
        }
    }
}

impl From<TeamFailure> for loco_rs::Error {
    fn from(err: TeamFailure) -> Self {
        match err {
            TeamFailure::NotFound => Self::NotFound,
            TeamFailure::Invalid(errors) => Self::Validation(errors),
            TeamFailure::NotAssignable => Self::BadRequest(err.to_string()),
            TeamFailure::Db(e) => Self::DB(e),
        }
    }
}

/// # Errors
///
/// [`TeamFailure::NotFound`] bila pelaku tidak lagi memegang `ManageTeams`, lalu seperti
/// `teams::ActiveModel::create`.
pub async fn create(
    db: &DatabaseConnection,
    actor: &Actor,
    name: &str,
) -> Result<teams::Model, TeamFailure> {
    let managing = Managing::begin(db, actor).await?;
    let team = teams::ActiveModel::create(&managing.txn, actor.workspace_id, name).await?;
    audit::record(
        &managing.txn,
        actor.audit(
            AuditAction::TeamCreated,
            (AuditSubject::Team, team.id),
            serde_json::json!({ "name": team.name }),
        ),
    )
    .await?;
    managing.txn.commit().await?;
    Ok(team)
}

/// # Errors
///
/// [`TeamFailure::NotFound`] bila tim `id` sudah tidak ada di workspace ini atau pelaku tidak
/// lagi memegang `ManageTeams`, lalu seperti `teams::ActiveModel::rename`.
pub async fn rename(
    db: &DatabaseConnection,
    actor: &Actor,
    id: Uuid,
    name: &str,
) -> Result<teams::Model, TeamFailure> {
    let managing = Managing::begin(db, actor).await?;
    let current = managing.team(id).await?;
    let old_name = current.name.clone();
    let updated = teams::ActiveModel::rename(&managing.txn, current, name).await?;
    let Some(change) = audit::change(&old_name, &updated.name) else {
        managing.txn.rollback().await?;
        return Ok(updated);
    };
    audit::record(
        &managing.txn,
        actor.audit(
            AuditAction::TeamUpdated,
            (AuditSubject::Team, updated.id),
            serde_json::json!({ "name": change }),
        ),
    )
    .await?;
    managing.txn.commit().await?;
    Ok(updated)
}

/// Hapus tim; grant project tim itu ikut hilang (CASCADE) dan dicatat di audit (`grants`).
///
/// # Errors
///
/// [`TeamFailure::NotFound`] bila tim `id` sudah tidak ada di workspace ini atau pelaku tidak
/// lagi memegang `ManageTeams`, error database lain.
pub async fn delete(db: &DatabaseConnection, actor: &Actor, id: Uuid) -> Result<(), TeamFailure> {
    let managing = Managing::begin(db, actor).await?;
    let current = managing.team(id).await?;
    let grants = project_grants::Model::of_team(&managing.txn, managing.workspace_id, id).await?;
    let changes = serde_json::json!({ "name": current.name, "grants": grant_refs(&grants) });
    current.delete(&managing.txn).await?;
    audit::record(
        &managing.txn,
        actor.audit(AuditAction::TeamDeleted, (AuditSubject::Team, id), changes),
    )
    .await?;
    managing.txn.commit().await?;
    Ok(())
}

/// Masukkan anggota workspace `user_id` ke tim `team_id`. Sudah anggota tim: tanpa perubahan
/// dan tanpa audit.
///
/// # Errors
///
/// [`TeamFailure::NotFound`] bila tim tidak ada, `user_id` bukan anggota workspace ini, atau
/// pelaku tidak lagi memegang `ManageTeams`; [`TeamFailure::NotAssignable`] bila grant tim
/// melebihi akses pelaku; error database lain.
pub async fn add_member(
    db: &DatabaseConnection,
    actor: &Actor,
    team_id: Uuid,
    user_id: Uuid,
) -> Result<(), TeamFailure> {
    let managing = Managing::begin(db, actor).await?;
    let team = managing.team(team_id).await?;
    if workspace_members::Model::find_with_role(&managing.txn, managing.workspace_id, user_id)
        .await?
        .is_none()
    {
        return Err(TeamFailure::NotFound);
    }
    managing.ensure_can_join(team.id).await?;
    if !team_members::ActiveModel::add(&managing.txn, managing.workspace_id, team.id, user_id)
        .await?
    {
        managing.txn.rollback().await?;
        return Ok(());
    }
    audit::record(
        &managing.txn,
        actor.audit(
            AuditAction::TeamMemberAdded,
            (AuditSubject::Team, team.id),
            serde_json::json!({ "user_id": user_id }),
        ),
    )
    .await?;
    managing.txn.commit().await?;
    Ok(())
}

/// Keluarkan `user_id` dari tim `team_id`. Bukan anggota tim: tanpa perubahan dan tanpa audit.
/// Grant tim yang tidak lagi menjangkaunya dicatat di audit (`grants`).
///
/// # Errors
///
/// [`TeamFailure::NotFound`] bila tim tidak ada atau pelaku tidak lagi memegang `ManageTeams`,
/// error database lain.
pub async fn remove_member(
    db: &DatabaseConnection,
    actor: &Actor,
    team_id: Uuid,
    user_id: Uuid,
) -> Result<(), TeamFailure> {
    let managing = Managing::begin(db, actor).await?;
    let wid = managing.workspace_id;
    let team = managing.team(team_id).await?;
    let Some(member) = team_members::Model::find(&managing.txn, wid, team.id, user_id).await?
    else {
        managing.txn.rollback().await?;
        return Ok(());
    };
    let grants = project_grants::Model::of_team(&managing.txn, wid, team.id).await?;
    member.delete(&managing.txn).await?;
    audit::record(
        &managing.txn,
        actor.audit(
            AuditAction::TeamMemberRemoved,
            (AuditSubject::Team, team.id),
            serde_json::json!({ "user_id": user_id, "grants": grant_refs(&grants) }),
        ),
    )
    .await?;
    managing.txn.commit().await?;
    Ok(())
}

/// Id grant dan project-nya, untuk audit akses yang hilang.
fn grant_refs(grants: &[project_grants::Model]) -> serde_json::Value {
    grants
        .iter()
        .map(|g| serde_json::json!({ "id": g.id, "project_id": g.project_id }))
        .collect()
}

/// Transaksi dengan baris workspace terkunci dan role pelaku terbaru (kunci baca). Ada hanya
/// bila pelaku masih anggota dan role-nya memegang `ManageTeams`.
struct Managing {
    txn: DatabaseTransaction,
    workspace_id: WorkspaceId,
    user_id: Uuid,
    actor_role: roles::Model,
}

impl Managing {
    async fn begin(db: &DatabaseConnection, actor: &Actor) -> Result<Self, TeamFailure> {
        let wid = actor.workspace_id;
        let txn = db.begin().await?;
        workspaces::Model::lock_for_membership(&txn, wid).await?;
        let (_, role) = workspace_members::Model::find_with_role(&txn, wid, actor.user_id)
            .await?
            .ok_or(TeamFailure::NotFound)?;
        let actor_role = roles::Model::lock_shared_in(&txn, wid, role.id)
            .await?
            .filter(|role| role.has(Permission::ManageTeams))
            .ok_or(TeamFailure::NotFound)?;
        Ok(Self {
            txn,
            workspace_id: wid,
            user_id: actor.user_id,
            actor_role,
        })
    }

    /// Tim `id` di workspace pelaku, dikunci (`FOR UPDATE`): berurutan dengan grant baru ke tim
    /// ini (`projects::access`).
    async fn team(&self, id: Uuid) -> Result<teams::Model, TeamFailure> {
        teams::Model::lock_in(&self.txn, self.workspace_id, id)
            .await?
            .ok_or(TeamFailure::NotFound)
    }

    /// Aturan tambah anggota (lihat doc modul): setiap grant tim harus boleh diberikan pelaku.
    async fn ensure_can_join(&self, team_id: Uuid) -> Result<(), TeamFailure> {
        if project_access::is_admin_everywhere(&self.actor_role) {
            return Ok(());
        }
        let who = Accessor {
            workspace_id: self.workspace_id,
            user_id: self.user_id,
            role: &self.actor_role,
        };
        for grant in project_grants::Model::of_team(&self.txn, self.workspace_id, team_id).await? {
            let role = roles::Model::find_in(&self.txn, self.workspace_id, grant.role_id)
                .await?
                .ok_or(TeamFailure::NotFound)?;
            let allowed = project_access::effective_role(&self.txn, &who, grant.project_id)
                .await?
                .is_some_and(|access| project_access::can_grant(&self.actor_role, &access, &role));
            if !allowed {
                return Err(TeamFailure::NotAssignable);
            }
        }
        Ok(())
    }
}
