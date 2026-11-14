//! Pengaturan runner per user per workspace (auto-ambil, batas paralel, client default,
//! kebijakan kuota habis, program terminal per OS). Baris dibuat saat pertama disimpan; tanpa
//! baris = bawaan.
use sea_orm::entity::prelude::*;

pub use super::_entities::runner_prefs::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

standard_behavior!(ActiveModel);

/// Kebijakan saat kuota AI habis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, serde::Serialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum QuotaPolicy {
    /// Tunggu limit reset lalu lanjut di runner yang sama.
    #[sea_orm(string_value = "wait")]
    Wait,
    /// Lepas job ke antrean klaim.
    #[sea_orm(string_value = "release")]
    Release,
}

/// Program terminal per OS (kosong = deteksi otomatis).
pub const TERMINAL_OS: &[&str] = &["macos", "linux", "windows"];

/// Pengaturan efektif (bawaan bila belum disimpan).
#[derive(Debug, Clone, serde::Serialize)]
pub struct Prefs {
    pub auto_claim: bool,
    pub max_parallel: Option<i32>,
    pub default_client: String,
    pub quota_policy: QuotaPolicy,
    pub terminals: serde_json::Value,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            auto_claim: false,
            max_parallel: None,
            default_client: "claude".into(),
            quota_policy: QuotaPolicy::Wait,
            terminals: serde_json::json!({}),
        }
    }
}

impl Model {
    /// Pengaturan efektif `user` di workspace.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn of<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        user: Uuid,
    ) -> Result<Prefs, DbErr> {
        let row = Entity::find()
            .filter(Column::WorkspaceId.eq(wid.get()))
            .filter(Column::UserId.eq(user))
            .one(db)
            .await?;
        Ok(row.map_or_else(Prefs::default, |r| Prefs {
            auto_claim: r.auto_claim,
            max_parallel: r.max_parallel,
            quota_policy: QuotaPolicy::try_from_value(&r.quota_policy).unwrap_or(QuotaPolicy::Wait),
            default_client: r.default_client,
            terminals: r.terminals,
        }))
    }
}
