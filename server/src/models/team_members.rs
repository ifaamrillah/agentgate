//! Anggota tim: satu baris per `(team_id, user_id)`. Orangnya harus anggota workspace yang
//! sama: FK `(workspace_id, user_id)` ke `workspace_members` dengan `ON DELETE CASCADE`, jadi
//! anggota yang keluar dari workspace otomatis keluar dari semua timnya.

use loco_rs::model::{TenantEntity, TenantQueryExt};
use sea_orm::{ActiveValue::Set, QueryOrder, entity::prelude::*, sea_query::OnConflict};

pub use super::_entities::team_members::{ActiveModel, Column, Entity, Model};
use super::{users, workspaces::WorkspaceId};

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
    /// Anggota tim `team_id` beserta user-nya, urut nama.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn of_team<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        team_id: Uuid,
    ) -> Result<Vec<(Self, users::Model)>, DbErr> {
        let members = Entity::find()
            .in_tenant(workspace_id)
            .filter(Column::TeamId.eq(team_id))
            .all(db)
            .await?;
        // Tanpa relasi langsung ke `users` (FK lewat `workspace_members`): satu query lagi.
        let users = users::Entity::find()
            .filter(users::Column::Id.is_in(members.iter().map(|m| m.user_id)))
            .order_by_asc(users::Column::Name)
            .order_by_asc(users::Column::Id)
            .all(db)
            .await?;
        Ok(users
            .into_iter()
            .filter_map(|user| {
                let member = members.iter().find(|m| m.user_id == user.id)?.clone();
                Some((member, user))
            })
            .collect())
    }

    /// Semua keanggotaan tim di workspace (jumlah anggota per tim di daftar tim).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn all_in<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find().in_tenant(workspace_id).all(db).await
    }

    /// Keanggotaan tim para user `user_ids` (kolom Tim di tabel anggota).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn in_teams_of<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        user_ids: impl IntoIterator<Item = Uuid>,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .in_tenant(workspace_id)
            .filter(Column::UserId.is_in(user_ids))
            .all(db)
            .await
    }

    /// Keanggotaan `user_id` di tim `team_id`, bila ada.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn find<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        team_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find()
            .in_tenant(workspace_id)
            .filter(Column::TeamId.eq(team_id))
            .filter(Column::UserId.eq(user_id))
            .one(db)
            .await
    }
}

impl ActiveModel {
    /// Masukkan `user_id` ke tim `team_id`. Sudah anggota bukan error: `false`, tanpa
    /// perubahan (`ON CONFLICT DO NOTHING`, jadi transaksi tetap bisa dipakai).
    ///
    /// # Errors
    ///
    /// Error database, termasuk FK bila tim atau user bukan milik/anggota workspace ini.
    pub async fn add<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        team_id: Uuid,
        user_id: Uuid,
    ) -> Result<bool, DbErr> {
        let inserted = Entity::insert(Self {
            id: Set(Uuid::now_v7()),
            workspace_id: Set(workspace_id.get()),
            team_id: Set(team_id),
            user_id: Set(user_id),
            ..Default::default()
        })
        .on_conflict(
            OnConflict::columns([Column::TeamId, Column::UserId])
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(db)
        .await?;
        Ok(inserted == 1)
    }
}
