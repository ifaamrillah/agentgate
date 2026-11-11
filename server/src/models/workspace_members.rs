//! Keanggotaan user di workspace: satu baris per `(workspace_id, user_id)`, dengan satu role
//! dan jabatan opsional. Role dan jabatan dijaga FK komposit `(…, workspace_id)`, jadi member
//! tidak bisa memegang role atau jabatan milik workspace lain.

use loco_rs::model::{TenantEntity, TenantQueryExt};
use sea_orm::{
    ActiveValue::Set, DatabaseTransaction, IntoActiveModel, QueryOrder, SqlErr, TransactionTrait,
    entity::prelude::*,
};

pub use super::_entities::workspace_members::{ActiveModel, Column, Entity, Model};
use super::{
    roles::{self, RoleKey},
    users,
    workspaces::{self, WorkspaceId},
};

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
    /// Keanggotaan `user_id` di workspace ini beserta role-nya, atau `None` bila bukan anggota.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn find_with_role<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        user_id: Uuid,
    ) -> Result<Option<(Self, roles::Model)>, DbErr> {
        let found = Entity::find()
            .in_tenant(workspace_id)
            .filter(Column::UserId.eq(user_id))
            .find_also_related(roles::Entity)
            .one(db)
            .await?;
        Ok(found.and_then(|(member, role)| role.map(|role| (member, role))))
    }

    /// Member `id` di workspace ini beserta role-nya; member workspace lain tidak ditemukan.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn find_in<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        id: Uuid,
    ) -> Result<Option<(Self, roles::Model)>, DbErr> {
        let found = Entity::find_by_id(id)
            .in_tenant(workspace_id)
            .find_also_related(roles::Entity)
            .one(db)
            .await?;
        Ok(found.and_then(|(member, role)| role.map(|role| (member, role))))
    }

    /// Semua anggota workspace beserta user dan role-nya, urut waktu bergabung.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn list_in<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
    ) -> Result<Vec<(Self, users::Model, roles::Model)>, DbErr> {
        let members = Entity::find()
            .in_tenant(workspace_id)
            .find_also_related(users::Entity)
            .order_by_asc(Column::JoinedAt)
            .order_by_asc(Column::Id)
            .all(db)
            .await?;
        let roles = roles::Model::in_workspace(db, workspace_id).await?;
        Ok(members
            .into_iter()
            .filter_map(|(member, user)| {
                let role = roles.iter().find(|r| r.id == member.role_id)?.clone();
                Some((member, user?, role))
            })
            .collect())
    }

    /// Jumlah anggota ber-role Owner di workspace ini.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn owner_count<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
    ) -> Result<u64, DbErr> {
        Entity::find()
            .in_tenant(workspace_id)
            .inner_join(roles::Entity)
            .filter(roles::Column::Key.eq(RoleKey::Owner.to_value()))
            .count(db)
            .await
    }

    /// Workspace yang boleh dibuka `user_id` (lihat `workspaces::Model::is_open_to`) beserta
    /// role-nya, urut nama. Satu-satunya query keanggotaan lintas workspace: dibatasi user,
    /// bukan tenant, karena dipakai untuk memilih tenant (switcher, `/`).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn workspaces_of<C: ConnectionTrait>(
        db: &C,
        user_id: Uuid,
    ) -> Result<Vec<(workspaces::Model, roles::Model)>, DbErr> {
        let memberships = Entity::find()
            .filter(Column::UserId.eq(user_id))
            .find_also_related(roles::Entity)
            .all(db)
            .await?;
        let ids: Vec<Uuid> = memberships.iter().map(|(m, _)| m.workspace_id).collect();
        let found = workspaces::Entity::find()
            .filter(workspaces::Column::Id.is_in(ids))
            .order_by_asc(workspaces::Column::Name)
            .order_by_asc(workspaces::Column::Id)
            .all(db)
            .await?;
        Ok(found
            .into_iter()
            .filter_map(|workspace| {
                memberships
                    .iter()
                    .find(|(m, _)| m.workspace_id == workspace.id)
                    .and_then(|(_, role)| role.clone())
                    .map(|role| (workspace, role))
            })
            .filter(|(workspace, role)| workspace.is_open_to(role))
            .collect())
    }

    /// Ganti jabatan member (`None` = tanpa jabatan).
    ///
    /// # Errors
    ///
    /// Error database, termasuk FK bila jabatan milik workspace lain.
    pub async fn set_job_title<C: ConnectionTrait>(
        self,
        db: &C,
        job_title_id: Option<Uuid>,
    ) -> Result<Self, DbErr> {
        let mut member = self.into_active_model();
        member.job_title_id = Set(job_title_id);
        member.update(db).await
    }

    /// Ganti role member (role harus milik workspace yang sama, dijaga FK komposit).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn set_role<C: ConnectionTrait>(self, db: &C, role_id: Uuid) -> Result<Self, DbErr> {
        let mut member = self.into_active_model();
        member.role_id = Set(role_id);
        member.update(db).await
    }
}

impl ActiveModel {
    /// Tambahkan `user_id` ke workspace dengan role `role_id` (harus role workspace ini).
    ///
    /// # Errors
    ///
    /// Error database: unique `(workspace_id, user_id)` bila sudah anggota, FK bila role milik
    /// workspace lain.
    pub async fn add<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        user_id: Uuid,
        role_id: Uuid,
    ) -> Result<Model, DbErr> {
        Self {
            workspace_id: Set(workspace_id.get()),
            user_id: Set(user_id),
            role_id: Set(role_id),
            ..Default::default()
        }
        .insert(db)
        .await
    }

    /// Seperti [`Self::add`] dengan jabatan, tetapi user yang sudah anggota bukan error:
    /// `None`, keanggotaan yang ada tidak diubah. Keunikan ditegakkan unique index lewat
    /// savepoint (rules §5), bukan SELECT lebih dulu, sehingga transaksi `txn` tetap bisa
    /// dipakai setelahnya.
    ///
    /// # Errors
    ///
    /// Error database selain unique `(workspace_id, user_id)`.
    pub async fn add_unless_member(
        txn: &DatabaseTransaction,
        workspace_id: WorkspaceId,
        user_id: Uuid,
        role_id: Uuid,
        job_title_id: Option<Uuid>,
    ) -> Result<Option<Model>, DbErr> {
        let savepoint = txn.begin().await?;
        let inserted = Self {
            workspace_id: Set(workspace_id.get()),
            user_id: Set(user_id),
            role_id: Set(role_id),
            job_title_id: Set(job_title_id),
            ..Default::default()
        }
        .insert(&savepoint)
        .await;
        match inserted {
            Ok(member) => {
                savepoint.commit().await?;
                Ok(Some(member))
            }
            Err(e) if matches!(e.sql_err(), Some(SqlErr::UniqueConstraintViolation(_))) => {
                savepoint.rollback().await?;
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }
}
