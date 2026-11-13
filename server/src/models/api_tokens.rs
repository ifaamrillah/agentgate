//! Token API pribadi (M11, `ag_pat_…`): milik satu anggota workspace, dibatasi cakupan
//! ([`ApiScope`]) dan opsional satu project. Token bertindak sebagai pembuatnya: izin role dan
//! akses project pembuatnya tetap dicek di setiap request (`services::api_tokens`). Hanya
//! digest SHA-256 yang disimpan; token mentah ditampilkan sekali saat dibuat.

use chrono::{DateTime, Duration, Utc};
use sea_orm::{ActiveValue::Set, Iterable, QueryOrder, entity::prelude::*, sea_query::Expr};
use serde::Serialize;

pub use super::_entities::api_tokens::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;
use crate::common::crypto;

standard_behavior!(ActiveModel);

/// Awalan token mentah (dikenali scanner secret, mis. GitHub secret scanning pola kustom).
pub const TOKEN_PREFIX: &str = "ag_pat_";
/// Panjang `prefix` yang disimpan untuk tampilan (`ag_pat_` + 4 karakter).
pub const DISPLAY_PREFIX_LEN: usize = 11;

/// Cakupan token (CHECK `api_tokens.scopes`). Teks UI lewat Fluent `api-scope-<nilai>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, DeriveActiveEnum, Serialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum ApiScope {
    /// Membaca project, tiket, komentar, riwayat.
    #[sea_orm(string_value = "tiket:baca")]
    #[serde(rename = "tiket:baca")]
    TicketsRead,
    /// Membuat dan mengubah tiket (pindah stage tetap lewat gate).
    #[sea_orm(string_value = "tiket:tulis")]
    #[serde(rename = "tiket:tulis")]
    TicketsWrite,
    #[sea_orm(string_value = "komentar:tulis")]
    #[serde(rename = "komentar:tulis")]
    CommentsWrite,
    /// Membaca sprint dan laporan.
    #[sea_orm(string_value = "laporan:baca")]
    #[serde(rename = "laporan:baca")]
    ReportsRead,
    #[sea_orm(string_value = "knowledge:baca")]
    #[serde(rename = "knowledge:baca")]
    KnowledgeRead,
    #[sea_orm(string_value = "webhook:kelola")]
    #[serde(rename = "webhook:kelola")]
    WebhooksManage,
}

impl ApiScope {
    #[must_use]
    pub fn as_str(self) -> String {
        self.to_value()
    }

    /// Cakupan dari string; tidak dikenal → `None`.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::try_from_value(&value.to_string()).ok()
    }

    /// Kunci Fluent deskripsi (`api-scope-tiket-baca`).
    #[must_use]
    pub fn label_key(self) -> String {
        format!("api-scope-{}", self.as_str().replace(':', "-"))
    }
}

/// Pilihan masa berlaku token (form; bukan kolom).
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter)]
pub enum TokenExpiry {
    Days30,
    Days90,
    Days365,
    Never,
}

impl TokenExpiry {
    /// Nilai form.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::Days30 => "30",
            Self::Days90 => "90",
            Self::Days365 => "365",
            Self::Never => "never",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::iter().find(|e| e.key() == value)
    }

    /// Waktu kedaluwarsa dari `now`; `None` = tanpa batas.
    #[must_use]
    pub fn expires_at(self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        match self {
            Self::Days30 => Some(now + Duration::days(30)),
            Self::Days90 => Some(now + Duration::days(90)),
            Self::Days365 => Some(now + Duration::days(365)),
            Self::Never => None,
        }
    }
}

/// Data token baru (sudah divalidasi service).
#[derive(Debug, Clone)]
pub struct NewApiToken {
    pub workspace_id: WorkspaceId,
    pub user_id: Uuid,
    pub project_id: Option<Uuid>,
    pub name: String,
    pub scopes: Vec<ApiScope>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl Model {
    /// Cakupan token (nilai tak dikenal dilewati; CHECK menjaga isinya).
    #[must_use]
    pub fn scopes(&self) -> Vec<ApiScope> {
        self.scopes.iter().filter_map(|s| ApiScope::parse(s)).collect()
    }

    #[must_use]
    pub fn has(&self, scope: ApiScope) -> bool {
        self.scopes.contains(&scope.as_str())
    }

    /// Belum dicabut dan belum kedaluwarsa pada `now`.
    #[must_use]
    pub fn is_usable(&self, now: DateTime<Utc>) -> bool {
        self.revoked_at.is_none() && self.expires_at.is_none_or(|e| e > now)
    }

    /// Token dari token mentah (digest), apa pun keadaannya.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn find_by_raw<C: ConnectionTrait>(db: &C, raw: &str) -> Result<Option<Self>, DbErr> {
        if !raw.starts_with(TOKEN_PREFIX) {
            return Ok(None);
        }
        Entity::find()
            .filter(Column::Digest.eq(crypto::sha256_hex(raw)))
            .one(db)
            .await
    }

    /// Token workspace yang belum dicabut, terbaru dulu.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn active_of_workspace<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::WorkspaceId.eq(wid.get()))
            .filter(Column::RevokedAt.is_null())
            .order_by_desc(Column::CreatedAt)
            .order_by_desc(Column::Id)
            .all(db)
            .await
    }

    /// Token `id` milik workspace.
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

    /// Catat pemakaian: `last_used_at` diperbarui paling sering sekali per menit supaya
    /// request beruntun tidak menulis baris yang sama terus-menerus.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn touch<C: ConnectionTrait>(
        db: &C,
        id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<(), DbErr> {
        let stale = DateTimeWithTimeZone::from(now - Duration::minutes(1));
        Entity::update_many()
            .col_expr(Column::LastUsedAt, Expr::value(DateTimeWithTimeZone::from(now)))
            .filter(Column::Id.eq(id))
            .filter(
                sea_orm::Condition::any()
                    .add(Column::LastUsedAt.is_null())
                    .add(Column::LastUsedAt.lt(stale)),
            )
            .exec(db)
            .await?;
        Ok(())
    }
}

impl ActiveModel {
    /// Simpan token baru; kembalikan baris dan token mentah (satu-satunya kesempatan melihatnya).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn create<C: ConnectionTrait>(
        db: &C,
        new: NewApiToken,
    ) -> Result<(Model, String), DbErr> {
        let raw = format!("{TOKEN_PREFIX}{}", crypto::random_token());
        let prefix: String = raw.chars().take(DISPLAY_PREFIX_LEN).collect();
        let model = Self {
            workspace_id: Set(new.workspace_id.get()),
            user_id: Set(new.user_id),
            project_id: Set(new.project_id),
            name: Set(new.name),
            prefix: Set(prefix),
            digest: Set(crypto::sha256_hex(&raw)),
            scopes: Set(new.scopes.iter().map(|s| s.as_str()).collect()),
            expires_at: Set(new.expires_at.map(Into::into)),
            ..Default::default()
        }
        .insert(db)
        .await?;
        Ok((model, raw))
    }

    /// Cabut token (idempoten: yang sudah dicabut tidak berubah).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn revoke<C: ConnectionTrait>(db: &C, token: Model) -> Result<Model, DbErr> {
        if token.revoked_at.is_some() {
            return Ok(token);
        }
        let mut m: Self = token.into();
        m.revoked_at = Set(Some(Utc::now().into()));
        m.update(db).await
    }
}
