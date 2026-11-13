//! Koneksi provider git per workspace (M6): GitHub atau GitHub Enterprise Server. Token akses
//! dan secret webhook disimpan terenkripsi; hanya [`Model::token`]/[`Model::webhook_secret`]
//! yang membukanya, dan keduanya tidak pernah ikut `Debug`/serialisasi.

use sea_orm::{QueryOrder, entity::prelude::*};

pub use super::_entities::git_connections::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;
use crate::common::crypto;

standard_behavior!(ActiveModel);

/// Jenis provider (CHECK `git_connections.provider`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, serde::Serialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum GitProviderKind {
    #[sea_orm(string_value = "github")]
    Github,
    #[sea_orm(string_value = "github_enterprise")]
    GithubEnterprise,
}

impl GitProviderKind {
    /// Nama tampilan provider.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Github => "GitHub",
            Self::GithubEnterprise => "GitHub Enterprise Server",
        }
    }
}

impl Model {
    #[must_use]
    pub fn kind(&self) -> GitProviderKind {
        GitProviderKind::try_from_value(&self.provider).unwrap_or(GitProviderKind::Github)
    }

    /// Token akses provider (dibuka dengan `ENCRYPTION_KEY`).
    ///
    /// # Errors
    ///
    /// Data rusak atau kunci berbeda.
    pub fn token(&self, key: &[u8; 32]) -> Result<String, crypto::CryptoError> {
        let raw = crypto::decrypt(key, &self.token_encrypted)?;
        String::from_utf8(raw).map_err(|_| crypto::CryptoError::Decrypt)
    }

    /// Secret HMAC webhook.
    ///
    /// # Errors
    ///
    /// Data rusak atau kunci berbeda.
    pub fn webhook_secret(&self, key: &[u8; 32]) -> Result<String, crypto::CryptoError> {
        let raw = crypto::decrypt(key, &self.webhook_secret_encrypted)?;
        String::from_utf8(raw).map_err(|_| crypto::CryptoError::Decrypt)
    }

    /// Semua koneksi workspace (urut dibuat).
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
            .all(db)
            .await
    }

    /// Koneksi `id` milik workspace.
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

    /// Koneksi workspace untuk host repo (`github.com`, `ghe.corp.local`).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn for_host<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        host: &str,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find()
            .filter(Column::WorkspaceId.eq(wid.get()))
            .filter(Column::Host.eq(host.to_ascii_lowercase()))
            .one(db)
            .await
    }
}
