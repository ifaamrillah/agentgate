//! Undangan email ke workspace. Token mentah hanya ada di link email; database menyimpan
//! digest SHA-256-nya (rules §7). Satu undangan tertunda (belum diterima, belum dibatalkan,
//! termasuk yang sudah kedaluwarsa) per email per workspace, dijaga partial unique index:
//! undangan baru untuk email yang sama membatalkan yang lama lebih dulu
//! ([`Model::revoke_pending_for`]).
//!
//! Token dicari lintas workspace ([`Model::find_usable`], [`Model::claim`]): pemegang token
//! belum tentu anggota mana pun, jadi tidak ada tenant untuk membatasi query. Digest token
//! unik, jadi hasilnya paling banyak satu baris.
//!
//! "Berlaku" = belum diterima, belum dibatalkan, dan belum kedaluwarsa ([`usable`]), satu
//! definisi untuk halaman publik dan claim.

use chrono::{Duration, Utc};
use loco_rs::model::{TenantEntity, TenantQueryExt};
use sea_orm::{
    ActiveValue::{self, Set},
    Condition, IntoActiveModel, QueryOrder, QuerySelect,
    entity::prelude::*,
    sea_query::Expr,
};

pub use super::_entities::invitations::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;
use crate::common::crypto;

/// Data undangan baru untuk [`ActiveModel::issue`]. `email` dinormalisasi saat disimpan.
#[derive(Debug, Clone)]
pub struct NewInvitation {
    pub email: String,
    pub role_id: Uuid,
    pub job_title_id: Option<Uuid>,
    pub invited_by: Uuid,
    pub ttl: Duration,
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
        if insert && let ActiveValue::Set(email) = &self.email {
            self.email = Set(super::users::normalize_email(email));
        }
        if !insert && self.updated_at.is_unchanged() {
            self.updated_at = Set(chrono::Utc::now().into());
        }
        Ok(self)
    }
}

impl Model {
    /// Undangan yang tokennya `raw` dan masih berlaku, tanpa mengonsumsinya (halaman publik:
    /// pemindai link email membuka link tanpa diklik user).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn find_usable<C: ConnectionTrait>(db: &C, raw: &str) -> Result<Option<Self>, DbErr> {
        Entity::find().filter(usable(raw)).one(db).await
    }

    /// Terima undangan: tandai `accepted_at` bila token `raw` masih berlaku (claim bersyarat,
    /// rules §5). `None` bila token tidak berlaku lagi, termasuk bila request lain menerimanya
    /// lebih dulu.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn claim<C: ConnectionTrait>(db: &C, raw: &str) -> Result<Option<Self>, DbErr> {
        let now = Utc::now();
        let claimed = Entity::update_many()
            .col_expr(Column::AcceptedAt, Expr::value(now))
            .col_expr(Column::UpdatedAt, Expr::value(now))
            .filter(usable(raw))
            .exec_with_returning(db)
            .await?;
        Ok(match <[Self; 1]>::try_from(claimed) {
            Ok([invitation]) => Some(invitation),
            Err(_) => None,
        })
    }

    /// Undangan tertunda (belum diterima, belum dibatalkan) di workspace ini, terbaru dulu.
    /// Yang sudah kedaluwarsa ikut tampil supaya bisa dikirim ulang.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn pending_in<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .in_tenant(workspace_id)
            .filter(pending())
            .order_by_desc(Column::CreatedAt)
            .order_by_desc(Column::Id)
            .all(db)
            .await
    }

    /// Undangan tertunda `id` di workspace ini, dikunci (`FOR UPDATE`) sampai transaksi `txn`
    /// selesai. Undangan workspace lain, yang sudah diterima, atau dibatalkan tidak ditemukan.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn lock_pending_in<C: ConnectionTrait>(
        txn: &C,
        workspace_id: WorkspaceId,
        id: Uuid,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(id)
            .in_tenant(workspace_id)
            .filter(pending())
            .lock_exclusive()
            .one(txn)
            .await
    }

    /// Batalkan semua undangan tertunda untuk `email` di workspace ini, supaya undangan baru
    /// lolos partial unique index. Mengembalikan jumlah yang dibatalkan.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn revoke_pending_for<C: ConnectionTrait>(
        txn: &C,
        workspace_id: WorkspaceId,
        email: &str,
    ) -> Result<u64, DbErr> {
        let now = Utc::now();
        Entity::update_many()
            .col_expr(Column::RevokedAt, Expr::value(now))
            .col_expr(Column::UpdatedAt, Expr::value(now))
            .filter(Column::WorkspaceId.eq(workspace_id))
            .filter(Column::Email.eq(super::users::normalize_email(email)))
            .filter(pending())
            .exec(txn)
            .await
            .map(|res| res.rows_affected)
    }

    /// Batalkan undangan ini (`revoked_at`).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn revoke<C: ConnectionTrait>(self, db: &C) -> Result<Self, DbErr> {
        let mut invitation = self.into_active_model();
        invitation.revoked_at = Set(Some(Utc::now().into()));
        invitation.update(db).await
    }

    /// Hapus undangan `role_id` yang sudah selesai (diterima, dibatalkan, atau kedaluwarsa),
    /// supaya tidak menahan penghapusan role lewat FK. Undangan aktif tidak disentuh dan tetap
    /// menahan role (`role-in-use`). Dipanggil di transaksi hapus role.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn delete_finished_for_role<C: ConnectionTrait>(
        txn: &C,
        workspace_id: WorkspaceId,
        role_id: Uuid,
    ) -> Result<u64, DbErr> {
        let now: DateTimeWithTimeZone = chrono::Utc::now().into();
        Entity::delete_many()
            .in_tenant(workspace_id)
            .filter(Column::RoleId.eq(role_id))
            .filter(
                Condition::any()
                    .add(Column::AcceptedAt.is_not_null())
                    .add(Column::RevokedAt.is_not_null())
                    .add(Column::ExpiresAt.lte(now)),
            )
            .exec(txn)
            .await
            .map(|res| res.rows_affected)
    }
}

impl ActiveModel {
    /// Buat undangan baru di workspace ini dengan token acak. Mengembalikan undangan dan token
    /// mentah untuk link email; yang tersimpan hanya digest-nya. Undangan tertunda lain untuk
    /// email yang sama harus sudah dibatalkan ([`Model::revoke_pending_for`]).
    ///
    /// # Errors
    ///
    /// Error database: unique index undangan tertunda bila masih ada untuk email ini, FK bila
    /// role atau jabatan milik workspace lain.
    pub async fn issue<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        new: NewInvitation,
    ) -> Result<(Model, String), DbErr> {
        let raw = crypto::random_token();
        let invitation = Self {
            workspace_id: Set(workspace_id.get()),
            email: Set(new.email),
            role_id: Set(new.role_id),
            job_title_id: Set(new.job_title_id),
            token_digest: Set(crypto::sha256_hex(&raw)),
            invited_by: Set(Some(new.invited_by)),
            expires_at: Set((Utc::now() + new.ttl).into()),
            ..Default::default()
        }
        .insert(db)
        .await?;
        Ok((invitation, raw))
    }
}

/// Belum diterima dan belum dibatalkan (kedaluwarsa atau tidak).
fn pending() -> Condition {
    Condition::all()
        .add(Column::AcceptedAt.is_null())
        .add(Column::RevokedAt.is_null())
}

/// Satu definisi "token undangan berlaku", dipakai [`Model::find_usable`] dan [`Model::claim`].
fn usable(raw: &str) -> Condition {
    pending()
        .add(Column::TokenDigest.eq(crypto::sha256_hex(raw)))
        .add(Column::ExpiresAt.gt(Utc::now()))
}
