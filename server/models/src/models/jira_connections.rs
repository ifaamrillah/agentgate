//! Koneksi Jira per workspace (M11, satu per workspace di MVP). Token Jira (API token Cloud atau
//! PAT Data Center) disimpan terenkripsi; hanya [`Model::token`] yang membukanya dan kolomnya
//! tidak ikut serialisasi.
//!
//! Pemetaan dan katalog Jira disimpan sebagai jsonb dan selalu dibaca lewat struct bertipe
//! ([`StatusMap`], [`FieldMap`], [`UserMap`], [`Catalog`], [`SyncStats`]). Katalog diperbarui
//! saat koneksi disimpan/diuji, sehingga halaman pengaturan (GET) tidak memanggil Jira.
//!
//! Sinkron untuk satu koneksi berjalan satu per satu: [`ActiveModel::claim_sync`] meng-claim
//! `sync_started_at` dengan update bersyarat (rules §5 race guard); claim yang lebih tua dari
//! [`CLAIM_MINUTES`] dianggap mati dan boleh diambil alih.

use chrono::{DateTime, Duration, Utc};
use sea_orm::{ActiveValue::Set, Condition, entity::prelude::*, sea_query::Expr};
use serde::{Deserialize, Serialize};

pub use super::_entities::jira_connections::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;
use crate::common::crypto;

standard_behavior!(ActiveModel);

/// Lama claim sinkron sebelum dianggap mati (menit).
pub const CLAIM_MINUTES: i64 = 15;

/// Jenis Jira (CHECK `jira_connections.env`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum JiraEnv {
    /// `*.atlassian.net`: email + API token, REST v3 (deskripsi ADF).
    #[sea_orm(string_value = "cloud")]
    Cloud,
    /// Server sendiri: Personal Access Token (Bearer), REST v2 (deskripsi teks wiki).
    #[sea_orm(string_value = "data_center")]
    DataCenter,
}

impl JiraEnv {
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::try_from_value(&value.trim().to_string()).ok()
    }

    #[must_use]
    pub fn as_str(self) -> String {
        self.to_value()
    }
}

/// Mode sinkron (CHECK `jira_connections.mode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum SyncMode {
    /// Impor sekali: issue → tiket, lalu koneksi selesai.
    #[sea_orm(string_value = "once")]
    Once,
    /// Agent Gate → Jira; Jira hanya cermin.
    #[sea_orm(string_value = "mirror")]
    Mirror,
    /// Dua arah: mirror + tarik perubahan Jira (JQL `updated >=`).
    #[sea_orm(string_value = "two_way")]
    TwoWay,
}

impl SyncMode {
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::try_from_value(&value.trim().to_string()).ok()
    }

    #[must_use]
    pub fn as_str(self) -> String {
        self.to_value()
    }

    /// Mode yang mendorong perubahan Agent Gate ke Jira.
    #[must_use]
    pub fn pushes(self) -> bool {
        matches!(self, Self::Mirror | Self::TwoWay)
    }
}

/// Kategori status Jira (`statusCategory.key`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusCategory {
    #[default]
    New,
    Indeterminate,
    Done,
}

impl StatusCategory {
    /// Dari `statusCategory.key` Jira (`new`, `indeterminate`, `done`; selain itu `new`).
    #[must_use]
    pub fn from_jira(key: &str) -> Self {
        match key {
            "done" => Self::Done,
            "indeterminate" => Self::Indeterminate,
            _ => Self::New,
        }
    }
}

/// Satu status Jira project dan stage tujuannya (`None` = belum dipetakan).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusEntry {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub category: StatusCategory,
    #[serde(default)]
    pub stage: Option<String>,
}

/// Status Jira ↔ stage project Agent Gate.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusMap {
    #[serde(default)]
    pub statuses: Vec<StatusEntry>,
}

impl StatusMap {
    /// Stage untuk status Jira `id`.
    #[must_use]
    pub fn stage_of(&self, id: &str) -> Option<&str> {
        self.statuses
            .iter()
            .find(|s| s.id == id)
            .and_then(|s| s.stage.as_deref())
    }

    /// Status Jira untuk `stage`: `prefer` bila status itu juga dipetakan ke stage ini, selain
    /// itu status pertama yang dipetakan ke sana.
    #[must_use]
    pub fn status_for(&self, stage: &str, prefer: &str) -> Option<&StatusEntry> {
        let mapped = |s: &&StatusEntry| s.stage.as_deref() == Some(stage);
        self.statuses
            .iter()
            .filter(mapped)
            .find(|s| s.id == prefer)
            .or_else(|| self.statuses.iter().find(mapped))
    }

    #[must_use]
    pub fn name_of(&self, id: &str) -> String {
        self.statuses
            .iter()
            .find(|s| s.id == id)
            .map_or_else(|| id.to_string(), |s| s.name.clone())
    }

    /// Status yang belum dipetakan.
    #[must_use]
    pub fn unmapped(&self) -> Vec<&StatusEntry> {
        self.statuses.iter().filter(|s| s.stage.is_none()).collect()
    }
}

/// Field Agent Gate tujuan pemetaan field Jira.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldTarget {
    Title,
    Description,
    StoryPoints,
    Sprint,
    Release,
    Component,
    DueOn,
    Labels,
    Priority,
    /// Field custom Agent Gate (`custom:<uuid>`).
    Custom(Uuid),
    /// Tidak disinkronkan.
    Skip,
}

impl FieldTarget {
    /// Target bawaan (tanpa field custom), urut tampil.
    pub const BUILTIN: [Self; 9] = [
        Self::Title,
        Self::Description,
        Self::StoryPoints,
        Self::Sprint,
        Self::Release,
        Self::Component,
        Self::DueOn,
        Self::Labels,
        Self::Priority,
    ];

    #[must_use]
    pub fn as_str(self) -> String {
        match self {
            Self::Title => "title".into(),
            Self::Description => "description".into(),
            Self::StoryPoints => "story_points".into(),
            Self::Sprint => "sprint".into(),
            Self::Release => "release".into(),
            Self::Component => "component".into(),
            Self::DueOn => "due_on".into(),
            Self::Labels => "labels".into(),
            Self::Priority => "priority".into(),
            Self::Custom(id) => format!("custom:{id}"),
            Self::Skip => "none".into(),
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        let v = value.trim();
        if let Some(id) = v.strip_prefix("custom:") {
            return Uuid::parse_str(id).ok().map(Self::Custom);
        }
        Self::BUILTIN
            .into_iter()
            .chain([Self::Skip])
            .find(|t| t.as_str() == v)
    }
}

impl Serialize for FieldTarget {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.as_str())
    }
}

impl<'de> Deserialize<'de> for FieldTarget {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Ok(Self::parse(&raw).unwrap_or(Self::Skip))
    }
}

/// Satu field Jira dan field Agent Gate tujuannya.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldEntry {
    /// Id field Jira (`summary`, `customfield_10016`).
    pub id: String,
    pub name: String,
    pub target: FieldTarget,
}

/// Field Jira ↔ field Agent Gate.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldMap {
    #[serde(default)]
    pub fields: Vec<FieldEntry>,
}

impl FieldMap {
    /// Id field Jira untuk `target` (yang pertama).
    #[must_use]
    pub fn jira_field(&self, target: FieldTarget) -> Option<&str> {
        self.fields
            .iter()
            .find(|f| f.target == target)
            .map(|f| f.id.as_str())
    }

    #[must_use]
    pub fn maps(&self, target: FieldTarget) -> bool {
        self.jira_field(target).is_some()
    }
}

/// Satu user Jira dan anggota Agent Gate pasangannya.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserEntry {
    /// `accountId` (Cloud) atau `name` (Data Center).
    pub account: String,
    pub name: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub user_id: Option<Uuid>,
}

/// User Jira ↔ anggota workspace.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserMap {
    #[serde(default)]
    pub users: Vec<UserEntry>,
}

impl UserMap {
    /// Anggota untuk akun Jira `account`.
    #[must_use]
    pub fn user_of(&self, account: &str) -> Option<Uuid> {
        self.users
            .iter()
            .find(|u| u.account == account)
            .and_then(|u| u.user_id)
    }

    /// Akun Jira untuk anggota `user_id`.
    #[must_use]
    pub fn account_of(&self, user_id: Uuid) -> Option<&str> {
        self.users
            .iter()
            .find(|u| u.user_id == Some(user_id))
            .map(|u| u.account.as_str())
    }

    #[must_use]
    pub fn mapped(&self) -> usize {
        self.users.iter().filter(|u| u.user_id.is_some()).count()
    }
}

/// Project Jira yang terlihat token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogProject {
    pub id: String,
    pub key: String,
    pub name: String,
}

/// Katalog Jira terakhir (diperbarui saat koneksi disimpan/diuji).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Catalog {
    #[serde(default)]
    pub projects: Vec<CatalogProject>,
}

/// Statistik impor/sinkron untuk Ringkasan.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncStats {
    #[serde(default)]
    pub tickets: u64,
    #[serde(default)]
    pub comments: u64,
    #[serde(default)]
    pub attachments: u64,
    #[serde(default)]
    pub sprints: u64,
}

fn typed<T: serde::de::DeserializeOwned + Default>(v: &serde_json::Value) -> T {
    serde_json::from_value(v.clone()).unwrap_or_default()
}

impl Model {
    #[must_use]
    pub fn env(&self) -> JiraEnv {
        JiraEnv::parse(&self.env).unwrap_or(JiraEnv::Cloud)
    }

    #[must_use]
    pub fn mode(&self) -> SyncMode {
        SyncMode::parse(&self.mode).unwrap_or(SyncMode::Mirror)
    }

    #[must_use]
    pub fn status_map(&self) -> StatusMap {
        typed(&self.status_map)
    }

    #[must_use]
    pub fn field_map(&self) -> FieldMap {
        typed(&self.field_map)
    }

    #[must_use]
    pub fn user_map(&self) -> UserMap {
        typed(&self.user_map)
    }

    #[must_use]
    pub fn catalog(&self) -> Catalog {
        typed(&self.catalog)
    }

    #[must_use]
    pub fn stats(&self) -> SyncStats {
        typed(&self.stats)
    }

    #[must_use]
    pub fn is_paused(&self) -> bool {
        self.paused_at.is_some()
    }

    /// Project Jira dan project Agent Gate sudah dipilih.
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.project_id.is_some() && !self.jira_project_key.is_empty()
    }

    /// Host URL Jira (`acme.atlassian.net`).
    #[must_use]
    pub fn host(&self) -> String {
        reqwest::Url::parse(&self.base_url)
            .ok()
            .and_then(|u| u.host_str().map(str::to_string))
            .unwrap_or_else(|| self.base_url.clone())
    }

    /// Token Jira (dibuka dengan `ENCRYPTION_KEY`).
    ///
    /// # Errors
    ///
    /// Data rusak atau kunci berbeda.
    pub fn token(&self, key: &[u8; 32]) -> Result<String, crypto::CryptoError> {
        let raw = crypto::decrypt(key, &self.token_encrypted)?;
        String::from_utf8(raw).map_err(|_| crypto::CryptoError::Decrypt)
    }

    /// Cursor outbox (mirror): event terakhir yang sudah diproses.
    #[must_use]
    pub fn outbox_cursor(&self) -> Option<(DateTimeWithTimeZone, Uuid)> {
        self.outbox_after_at.zip(self.outbox_after_id)
    }

    /// Koneksi workspace.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn of_workspace<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find()
            .filter(Column::WorkspaceId.eq(wid.get()))
            .one(db)
            .await
    }

    /// Koneksi yang perlu disinkron berkala: mode mirror/dua arah, tidak dijeda, siap.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn due_for_sync<C: ConnectionTrait>(db: &C) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::Mode.is_in([SyncMode::Mirror.as_str(), SyncMode::TwoWay.as_str()]))
            .filter(Column::PausedAt.is_null())
            .filter(Column::ProjectId.is_not_null())
            .filter(Column::JiraProjectKey.ne(""))
            .all(db)
            .await
    }
}

impl ActiveModel {
    /// Claim sinkron koneksi `id` pada `now`. `false` = sinkron lain masih berjalan.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn claim_sync<C: ConnectionTrait>(
        db: &C,
        id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<bool, DbErr> {
        let stale = DateTimeWithTimeZone::from(now - Duration::minutes(CLAIM_MINUTES));
        let res = Entity::update_many()
            .col_expr(
                Column::SyncStartedAt,
                Expr::value(DateTimeWithTimeZone::from(now)),
            )
            .filter(Column::Id.eq(id))
            .filter(
                Condition::any()
                    .add(Column::SyncStartedAt.is_null())
                    .add(Column::SyncStartedAt.lt(stale)),
            )
            .exec(db)
            .await?;
        Ok(res.rows_affected == 1)
    }

    /// Lepas claim sinkron.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn release_sync<C: ConnectionTrait>(db: &C, id: Uuid) -> Result<(), DbErr> {
        Entity::update_many()
            .col_expr(
                Column::SyncStartedAt,
                Expr::value(Option::<DateTimeWithTimeZone>::None),
            )
            .filter(Column::Id.eq(id))
            .exec(db)
            .await?;
        Ok(())
    }

    /// Simpan cursor outbox.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn advance_cursor<C: ConnectionTrait>(
        db: &C,
        id: Uuid,
        at: DateTimeWithTimeZone,
        event_id: Uuid,
    ) -> Result<(), DbErr> {
        Entity::update_many()
            .col_expr(Column::OutboxAfterAt, Expr::value(at))
            .col_expr(Column::OutboxAfterId, Expr::value(event_id))
            .filter(Column::Id.eq(id))
            .exec(db)
            .await?;
        Ok(())
    }

    /// Kunci kolom jsonb bertipe.
    #[must_use]
    pub fn json<T: Serialize>(value: &T) -> serde_json::Value {
        serde_json::to_value(value).unwrap_or_else(|_| serde_json::json!({}))
    }

    /// Set satu kolom jsonb bertipe.
    pub fn set_status_map(&mut self, map: &StatusMap) {
        self.status_map = Set(Self::json(map));
    }

    pub fn set_field_map(&mut self, map: &FieldMap) {
        self.field_map = Set(Self::json(map));
    }

    pub fn set_user_map(&mut self, map: &UserMap) {
        self.user_map = Set(Self::json(map));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_target_round_trips() {
        let id = Uuid::now_v7();
        for t in FieldTarget::BUILTIN
            .into_iter()
            .chain([FieldTarget::Skip, FieldTarget::Custom(id)])
        {
            assert_eq!(FieldTarget::parse(&t.as_str()), Some(t));
        }
        assert_eq!(FieldTarget::parse("custom:nope"), None);
    }

    #[test]
    fn status_for_prefers_known_status_of_same_stage() {
        let map = StatusMap {
            statuses: vec![
                StatusEntry {
                    id: "1".into(),
                    name: "In Review".into(),
                    category: StatusCategory::Indeterminate,
                    stage: Some("test".into()),
                },
                StatusEntry {
                    id: "2".into(),
                    name: "QA".into(),
                    category: StatusCategory::Indeterminate,
                    stage: Some("test".into()),
                },
            ],
        };
        assert_eq!(map.status_for("test", "2").unwrap().id, "2");
        assert_eq!(map.status_for("test", "9").unwrap().id, "1");
        assert!(map.status_for("dev", "1").is_none());
    }
}
