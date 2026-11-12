//! Keanggotaan user di workspace: satu baris per `(workspace_id, user_id)`, dengan satu role
//! dan jabatan opsional. Role dan jabatan dijaga FK komposit `(…, workspace_id)`, jadi member
//! tidak bisa memegang role atau jabatan milik workspace lain.

use loco_rs::model::{TenantEntity, TenantQueryExt};
use sea_orm::{
    ActiveValue::Set,
    Condition, DatabaseTransaction, IntoActiveModel, Order, QueryOrder, QuerySelect, QueryTrait,
    SqlErr, TransactionTrait,
    entity::prelude::*,
    sea_query::{ExprTrait, Func},
};

pub use super::_entities::workspace_members::{ActiveModel, Column, Entity, Model};
use super::{
    roles::{self, RoleKey},
    team_members, users,
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

    /// Jumlah anggota workspace (statistik beranda).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn count_in<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
    ) -> Result<u64, DbErr> {
        Entity::find().in_tenant(workspace_id).count(db).await
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

    /// Satu halaman anggota untuk tabel Pengaturan → Anggota: filter, urutan, dan potongan
    /// dikerjakan database. Hasil: baris halaman ini dan total baris yang cocok.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn page_in<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        query: &MemberQuery<'_>,
    ) -> Result<(Vec<(Self, users::Model, roles::Model)>, u64), DbErr> {
        let mut select = Entity::find()
            .in_tenant(workspace_id)
            .inner_join(users::Entity)
            .inner_join(roles::Entity);
        if let Some(q) = query.q.filter(|q| !q.trim().is_empty()) {
            let pattern = format!("%{}%", escape_like(&q.trim().to_lowercase()));
            select = select.filter(
                Condition::any()
                    .add(
                        Expr::expr(Func::lower(Expr::col((users::Entity, users::Column::Name))))
                            .like(pattern.clone()),
                    )
                    .add(
                        Expr::expr(Func::lower(Expr::col((
                            users::Entity,
                            users::Column::Email,
                        ))))
                        .like(pattern),
                    ),
            );
        }
        if !query.role_ids.is_empty() {
            select = select.filter(Column::RoleId.is_in(query.role_ids.iter().copied()));
        }
        if !query.team_ids.is_empty() {
            let in_teams = team_members::Entity::find()
                .select_only()
                .column(team_members::Column::UserId)
                .filter(team_members::Column::WorkspaceId.eq(workspace_id.get()))
                .filter(team_members::Column::TeamId.is_in(query.team_ids.iter().copied()))
                .into_query();
            select = select.filter(Column::UserId.in_subquery(in_teams));
        }
        match query.two_factor {
            Some(true) => select = select.filter(users::Column::TotpEnabledAt.is_not_null()),
            Some(false) => select = select.filter(users::Column::TotpEnabledAt.is_null()),
            None => {}
        }
        let total = select.clone().count(db).await?;
        let order = if query.desc { Order::Desc } else { Order::Asc };
        select = match query.sort {
            MemberSort::Role => select.order_by(roles::Column::Name, order.clone()),
            MemberSort::Joined => select.order_by(Column::JoinedAt, order.clone()),
            MemberSort::Name => select.order_by(users::Column::Name, order.clone()),
        };
        let members = select
            .order_by(Column::Id, order)
            .offset(query.offset)
            .limit(query.limit)
            .all(db)
            .await?;
        let users = users::Entity::find()
            .filter(users::Column::Id.is_in(members.iter().map(|m| m.user_id)))
            .all(db)
            .await?;
        let roles = roles::Model::in_workspace(db, workspace_id).await?;
        let rows = members
            .into_iter()
            .filter_map(|member| {
                let user = users.iter().find(|u| u.id == member.user_id)?.clone();
                let role = roles.iter().find(|r| r.id == member.role_id)?.clone();
                Some((member, user, role))
            })
            .collect();
        Ok((rows, total))
    }

    /// Jumlah anggota tanpa 2FA aktif (Pengaturan → Keamanan).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn without_two_factor_count<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
    ) -> Result<u64, DbErr> {
        Entity::find()
            .in_tenant(workspace_id)
            .inner_join(users::Entity)
            .filter(users::Column::TotpEnabledAt.is_null())
            .count(db)
            .await
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

/// Urutan tabel anggota.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberSort {
    Name,
    Role,
    Joined,
}

/// Filter dan potongan halaman tabel anggota (`Model::page_in`).
#[derive(Debug, Clone)]
pub struct MemberQuery<'a> {
    pub q: Option<&'a str>,
    pub role_ids: Vec<Uuid>,
    pub team_ids: Vec<Uuid>,
    /// `Some(true)` = hanya yang 2FA aktif, `Some(false)` = hanya yang belum.
    pub two_factor: Option<bool>,
    pub sort: MemberSort,
    pub desc: bool,
    pub offset: u64,
    pub limit: u64,
}

/// Escape `%`, `_`, dan `\` untuk pola `LIKE`.
fn escape_like(raw: &str) -> String {
    raw.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}
