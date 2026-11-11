//! Undangan email ke workspace (alur lengkapnya di Task 11). Token mentah hanya ada di link
//! email; database menyimpan digest SHA-256-nya (rules §7). Satu undangan aktif (belum
//! diterima, belum dibatalkan) per email per workspace, dijaga partial unique index.

use loco_rs::model::TenantEntity;
use sea_orm::{
    ActiveValue::{self, Set},
    entity::prelude::*,
};

pub use super::_entities::invitations::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

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
        if insert && let ActiveValue::Set(email) = &self.email {
            self.email = Set(super::users::normalize_email(email));
        }
        if !insert && self.updated_at.is_unchanged() {
            self.updated_at = Set(chrono::Utc::now().into());
        }
        Ok(self)
    }
}
