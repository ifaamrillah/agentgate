//! Webhook keluar per workspace (M11): URL tujuan, secret HMAC terenkripsi, event yang
//! dilanggan, dan filter project (kosong = semua). Nonaktif bila `disabled_at` terisi: manual,
//! atau otomatis setelah [`MAX_CONSECUTIVE_FAILURES`] percobaan gagal beruntun (pengirim:
//! `services::webhooks::dispatch`). Secret hanya dibuka [`Model::secret`].

use sea_orm::{QueryOrder, entity::prelude::*};

pub use super::_entities::webhooks::{ActiveModel, Column, Entity, Model};
use super::{outbox_events::WebhookEvent, workspaces::WorkspaceId};
use crate::common::crypto;

standard_behavior!(ActiveModel);

/// Percobaan gagal beruntun yang menonaktifkan webhook.
pub const MAX_CONSECUTIVE_FAILURES: i32 = 5;

impl Model {
    /// Secret HMAC (dibuka dengan `ENCRYPTION_KEY`).
    ///
    /// # Errors
    ///
    /// Data rusak atau kunci berbeda.
    pub fn secret(&self, key: &[u8; 32]) -> Result<String, crypto::CryptoError> {
        let raw = crypto::decrypt(key, &self.secret_encrypted)?;
        String::from_utf8(raw).map_err(|_| crypto::CryptoError::Decrypt)
    }

    #[must_use]
    pub fn is_active(&self) -> bool {
        self.disabled_at.is_none()
    }

    /// Dinonaktifkan otomatis karena gagal beruntun (bukan dimatikan manual).
    #[must_use]
    pub fn is_failing(&self) -> bool {
        self.disabled_at.is_some() && self.consecutive_failures >= MAX_CONSECUTIVE_FAILURES
    }

    /// Event yang dilanggan (nilai tak dikenal dilewati; CHECK menjaga isinya).
    #[must_use]
    pub fn events(&self) -> Vec<WebhookEvent> {
        self.events
            .iter()
            .filter_map(|e| WebhookEvent::parse(e))
            .collect()
    }

    /// Host URL tujuan (untuk tampilan dan filter log).
    #[must_use]
    pub fn host(&self) -> String {
        reqwest::Url::parse(&self.url)
            .ok()
            .and_then(|u| u.host_str().map(str::to_string))
            .unwrap_or_else(|| self.url.clone())
    }

    /// Semua webhook workspace (urut dibuat).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn of_workspace<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::WorkspaceId.eq(wid.get()))
            .order_by_asc(Column::CreatedAt)
            .order_by_asc(Column::Id)
            .all(db)
            .await
    }

    /// Webhook `id` milik workspace.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn find_in<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        id: Uuid,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(id)
            .filter(Column::WorkspaceId.eq(wid.get()))
            .one(db)
            .await
    }

    /// Webhook aktif yang melanggan `event` untuk `project_id` (filter project kosong = semua;
    /// event tanpa project hanya ke webhook tanpa filter project).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn subscribed<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        event: WebhookEvent,
        project_id: Option<Uuid>,
    ) -> Result<Vec<Self>, DbErr> {
        let value = event.as_str();
        Ok(Entity::find()
            .filter(Column::WorkspaceId.eq(wid.get()))
            .filter(Column::DisabledAt.is_null())
            .order_by_asc(Column::CreatedAt)
            .all(db)
            .await?
            .into_iter()
            .filter(|h| h.events.contains(&value))
            .filter(|h| {
                h.project_ids.is_empty() || project_id.is_some_and(|p| h.project_ids.contains(&p))
            })
            .collect())
    }
}
