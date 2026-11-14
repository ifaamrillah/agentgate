//! Client OAuth (RFC 7591, tidak milik workspace). Pendaftaran dinamis terbuka (dibatasi per IP),
//! tetapi client baru boleh mendapat token hanya setelah seorang user menyetujuinya di halaman
//! consent atau persetujuan device (`approved_at`, R1). Client yang tidak pernah disetujui
//! ("yatim") dihapus langkah cleanup setelah [`ORPHAN_HOURS`] jam.
use chrono::{DateTime, Duration, Utc};
use sea_orm::{ColumnTrait, ConnectionTrait, DbErr, EntityTrait, PaginatorTrait, QueryFilter};

pub use super::_entities::oauth_clients::{ActiveModel, Column, Entity, Model};

standard_behavior!(ActiveModel);

/// Umur client tanpa persetujuan sebelum dihapus.
pub const ORPHAN_HOURS: i64 = 24;

impl Model {
    #[must_use]
    pub fn is_approved(&self) -> bool {
        self.approved_at.is_some()
    }

    /// Tandai client disetujui (sekali; persetujuan berikutnya tidak mengubah waktunya).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn mark_approved<C: ConnectionTrait>(db: &C, id: uuid::Uuid) -> Result<(), DbErr> {
        Entity::update_many()
            .col_expr(
                Column::ApprovedAt,
                sea_orm::sea_query::Expr::value(DateTime::<chrono::FixedOffset>::from(Utc::now())),
            )
            .filter(Column::Id.eq(id))
            .filter(Column::ApprovedAt.is_null())
            .exec(db)
            .await?;
        Ok(())
    }

    /// Hapus (atau hitung, bila `dry`) client tanpa persetujuan yang lebih tua dari
    /// [`ORPHAN_HOURS`], kecuali `keep` (client bawaan server).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn purge_orphans<C: ConnectionTrait>(
        db: &C,
        now: DateTime<Utc>,
        keep: &str,
        dry: bool,
    ) -> Result<u64, DbErr> {
        let cutoff: DateTime<chrono::FixedOffset> = (now - Duration::hours(ORPHAN_HOURS)).into();
        let filter = sea_orm::Condition::all()
            .add(Column::ApprovedAt.is_null())
            .add(Column::CreatedAt.lt(cutoff))
            .add(Column::ClientId.ne(keep));
        if dry {
            return Entity::find().filter(filter).count(db).await;
        }
        Ok(Entity::delete_many()
            .filter(filter)
            .exec(db)
            .await?
            .rows_affected)
    }
}
