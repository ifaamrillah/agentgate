//! Grant = akses satu subjek (tim **atau** orang, CHECK `num_nonnulls = 1`) ke satu project
//! dengan satu role workspace. Satu grant per subjek per project (partial unique index).
//!
//! Aturan hapus ada di database (migrasi teams_projects): tim dihapus → grant-nya hilang
//! (CASCADE); orang keluar dari workspace → grant langsungnya hilang (CASCADE lewat
//! `workspace_members`); role yang masih dipakai grant tidak bisa dihapus (FK NO ACTION →
//! `role-in-use`). Siapa boleh mengubah grant diputuskan service `projects::access`.

use loco_rs::model::{TenantEntity, TenantQueryExt};
use sea_orm::{ActiveValue::Set, IntoActiveModel, QueryOrder, QuerySelect, entity::prelude::*};

pub use super::_entities::project_grants::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

/// Subjek sudah punya grant di project ini (partial unique index per subjek).
pub const PROJECT_GRANT_EXISTS: &str = "project-grant-exists";
/// Role grant tanpa izin project (`ViewProject`/`EditProject`/`AdministerProject`): tidak
/// memberi akses apa pun, jadi ditolak.
pub const GRANT_ROLE_NO_PROJECT_ACCESS: &str = "grant-role-no-project-access";
/// Kode error milik model ini; masing-masing punya pesan di Fluent (dijaga tes i18n).
pub const ERROR_CODES: &[&str] = &[PROJECT_GRANT_EXISTS, GRANT_ROLE_NO_PROJECT_ACCESS];

/// Subjek grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrantSubject {
    Team(Uuid),
    User(Uuid),
}

impl TenantEntity for Entity {
    type TenantId = WorkspaceId;

    fn tenant_column() -> Column {
        Column::WorkspaceId
    }
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if insert && self.id.is_not_set() {
            self.id = Set(Uuid::now_v7());
        }
        if !insert && self.updated_at.is_unchanged() {
            self.updated_at = Set(chrono::Utc::now().into());
        }
        Ok(self)
    }
}

impl Model {
    /// Subjek grant ini. Gagal hanya bila CHECK dilewati.
    ///
    /// # Errors
    ///
    /// Baris tanpa subjek atau dengan dua subjek.
    pub fn subject(&self) -> Result<GrantSubject, DbErr> {
        match (self.team_id, self.user_id) {
            (Some(team), None) => Ok(GrantSubject::Team(team)),
            (None, Some(user)) => Ok(GrantSubject::User(user)),
            _ => Err(DbErr::Custom(format!(
                "grant {} has no single subject",
                self.id
            ))),
        }
    }

    /// Semua grant project `project_id`, urut waktu dibuat.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn of_project<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        project_id: Uuid,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .in_tenant(workspace_id)
            .filter(Column::ProjectId.eq(project_id))
            .order_by_asc(Column::CreatedAt)
            .order_by_asc(Column::Id)
            .all(db)
            .await
    }

    /// Semua grant tim `team_id` (di project mana pun), urut waktu dibuat.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn of_team<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        team_id: Uuid,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .in_tenant(workspace_id)
            .filter(Column::TeamId.eq(team_id))
            .order_by_asc(Column::CreatedAt)
            .order_by_asc(Column::Id)
            .all(db)
            .await
    }

    /// Grant `id` milik project `project_id` di workspace ini, dikunci (`FOR UPDATE`) sampai
    /// transaksi `txn` selesai; grant project lain tidak ditemukan.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn lock_in<C: ConnectionTrait>(
        txn: &C,
        workspace_id: WorkspaceId,
        project_id: Uuid,
        id: Uuid,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(id)
            .in_tenant(workspace_id)
            .filter(Column::ProjectId.eq(project_id))
            .lock_exclusive()
            .one(txn)
            .await
    }

    /// Ganti role grant (role harus milik workspace yang sama, dijaga FK komposit).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn set_role<C: ConnectionTrait>(self, db: &C, role_id: Uuid) -> Result<Self, DbErr> {
        let mut grant = self.into_active_model();
        grant.role_id = Set(role_id);
        grant.update(db).await
    }
}

impl ActiveModel {
    /// Beri `subject` akses ke project `project_id` dengan role `role_id`.
    ///
    /// # Errors
    ///
    /// Error database: unique bila subjek sudah punya grant di project ini, FK bila project,
    /// tim, role, atau orang bukan milik/anggota workspace ini.
    pub async fn grant<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        project_id: Uuid,
        subject: GrantSubject,
        role_id: Uuid,
    ) -> Result<Model, DbErr> {
        let (team_id, user_id) = match subject {
            GrantSubject::Team(id) => (Some(id), None),
            GrantSubject::User(id) => (None, Some(id)),
        };
        Self {
            workspace_id: Set(workspace_id.get()),
            project_id: Set(project_id),
            team_id: Set(team_id),
            user_id: Set(user_id),
            role_id: Set(role_id),
            ..Default::default()
        }
        .insert(db)
        .await
    }
}
