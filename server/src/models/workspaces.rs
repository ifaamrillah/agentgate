//! Workspace = tenant (perusahaan/organisasi). Setiap tabel domain punya `workspace_id`, dan
//! setiap query model atas tabel itu menerima [`WorkspaceId`] sebagai parameter wajib
//! (`TenantEntity` Loco, rules "Ditunda → Multi-tenancy"). Workspace di-resolve sekali per
//! request oleh extractor `Tenant` dari path `/w/{slug}`.
//!
//! Slug dinormalisasi ke huruf kecil di sini dan unik lewat expression index `lower(slug)`;
//! pencarian juga case-insensitive, jadi `/w/ACME` me-resolve workspace `acme`. Slug baru tanpa
//! isian dibentuk dari nama (`text::slugify`). Daftar slug terlarang menjaga path statis di
//! bawah `/w/` (`/w/new`) dan path yang mungkin dipakai kelak.

use loco_rs::validation::ModelValidationErrors;
use sea_orm::{
    ActiveValue::Set,
    IntoActiveModel, QueryOrder, QuerySelect, SqlErr,
    entity::prelude::*,
    sea_query::{ExprTrait, Func, LockType},
};
use validator::{ValidationError, ValidationErrors};

pub use super::_entities::workspaces::{ActiveModel, Column, Entity, Model};
use super::{roles, users};
use crate::common::text;

/// Slug yang tidak boleh dipakai workspace: bentrok dengan route statis di bawah `/w/`, atau
/// dicadangkan untuk area aplikasi.
pub const RESERVED_SLUGS: &[&str] = &["new", "settings", "admin", "api", "auth"];

/// Masa tenggang hapus workspace (spec "Keamanan, privasi & akun"): selama itu Owner bisa
/// membatalkan; setelahnya job pembersihan menghapus workspace permanen.
pub const DELETION_GRACE_DAYS: i64 = 30;

/// Jam (UTC) job pembersihan harian berjalan: `scheduler.jobs.cleanup.schedule` di
/// `config/{development,staging,production}.yaml` (dijaga tes kontrak config). Banner hapus
/// workspace memakainya untuk menampilkan kapan penghapusan permanen mulai.
pub const PURGE_HOUR_UTC: u32 = 3;

pub const SLUG_TAKEN: &str = "slug-taken";
pub const SLUG_RESERVED: &str = "slug-reserved";
/// Kode error field milik model ini (kode slug umum ada di `common::text`).
pub const ERROR_CODES: &[&str] = &[SLUG_TAKEN, SLUG_RESERVED];

/// Id workspace bertipe: parameter wajib setiap query tabel milik workspace, sehingga query
/// tanpa tenant tidak bisa ditulis tanpa sengaja melewatkan argumen. Field-nya privat: nilai
/// ini hanya dibuat dari workspace yang sudah dimuat ([`Model::workspace_id`], dan lewat itu
/// `Tenant`), tidak dari UUID sembarang (mis. input request).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorkspaceId(Uuid);

impl WorkspaceId {
    #[must_use]
    pub fn get(self) -> Uuid {
        self.0
    }
}

impl From<WorkspaceId> for sea_orm::Value {
    fn from(id: WorkspaceId) -> Self {
        id.0.into()
    }
}

impl std::fmt::Display for WorkspaceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Data untuk [`ActiveModel::create`]. Slug kosong = dibentuk dari nama.
#[derive(Debug)]
pub struct CreateWorkspace {
    pub name: String,
    pub slug: String,
    pub created_by: Option<Uuid>,
}

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    /// Validasi field (`name`, `slug`) gagal, termasuk slug yang sudah dipakai.
    #[error("invalid workspace: {0}")]
    Invalid(ModelValidationErrors),
    #[error(transparent)]
    Db(#[from] DbErr),
}

impl From<WorkspaceError> for loco_rs::Error {
    fn from(err: WorkspaceError) -> Self {
        match err {
            WorkspaceError::Invalid(errors) => Self::Validation(errors),
            WorkspaceError::Db(e) => Self::DB(e),
        }
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
    #[must_use]
    pub fn workspace_id(&self) -> WorkspaceId {
        WorkspaceId(self.id)
    }

    /// Workspace dengan slug ini, tanpa membedakan huruf besar.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn find_by_slug<C: ConnectionTrait>(
        db: &C,
        slug: &str,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find()
            .filter(lower_slug().eq(slug.trim().to_lowercase()))
            .one(db)
            .await
    }

    /// Kunci baris workspace ini (`FOR NO KEY UPDATE`) sampai transaksi `txn` selesai. Dipakai
    /// setiap perubahan keanggotaan yang bisa mengurangi Owner, supaya dua perubahan bersamaan
    /// berjalan berurutan dan aturan "Owner terakhir" dihitung dari data terbaru. `NO KEY`:
    /// insert anggota dan undangan (FK ke `workspaces`, `FOR KEY SHARE`) tidak ikut tertahan.
    ///
    /// # Errors
    ///
    /// Error database, termasuk `RecordNotFound` bila workspace sudah tidak ada.
    pub async fn lock_for_membership<C: ConnectionTrait>(
        txn: &C,
        workspace_id: WorkspaceId,
    ) -> Result<Self, DbErr> {
        Entity::find_by_id(workspace_id.get())
            .lock(LockType::NoKeyUpdate)
            .one(txn)
            .await?
            .ok_or_else(|| DbErr::RecordNotFound("workspace".to_string()))
    }

    /// Workspace dengan id ini (mis. dari undangan). Hanya untuk jalur yang tidak punya
    /// `Tenant`; halaman workspace memakai slug.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn find_by_id<C: ConnectionTrait>(db: &C, id: Uuid) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(id).one(db).await
    }

    /// Workspace boleh dibuka anggota dengan role `role`: selama dijadwalkan hapus, hanya
    /// Owner yang masih melihatnya (untuk membatalkan). Satu aturan untuk extractor `Tenant`
    /// dan daftar workspace user.
    #[must_use]
    pub fn is_open_to(&self, role: &roles::Model) -> bool {
        self.deletion_scheduled_at.is_none() || role.is_owner()
    }

    /// Workspace mewajibkan 2FA dan `user` belum mengaktifkannya.
    #[must_use]
    pub fn requires_two_factor_from(&self, user: &users::Model) -> bool {
        self.require_2fa_at.is_some() && user.totp_enabled_at.is_none()
    }

    /// Saat workspace terjadwal hapus mulai dihapus permanen: run pertama job pembersihan
    /// (`PURGE_HOUR_UTC`) setelah tenggat (jadwal + masa tenggang).
    #[must_use]
    pub fn purge_starts_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.deletion_scheduled_at.map(|at| {
            first_cleanup_after(
                at.with_timezone(&chrono::Utc) + chrono::Duration::days(DELETION_GRACE_DAYS),
            )
        })
    }

    /// Set atau kosongkan `require_2fa_at`. Mengembalikan workspace tanpa menulis bila nilainya
    /// sudah sama.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn set_require_two_factor<C: ConnectionTrait>(
        self,
        db: &C,
        enabled: bool,
    ) -> Result<Self, DbErr> {
        if self.require_2fa_at.is_some() == enabled {
            return Ok(self);
        }
        let mut workspace = self.into_active_model();
        workspace.require_2fa_at = Set(enabled.then(|| chrono::Utc::now().into()));
        workspace.update(db).await
    }

    /// Jadwalkan hapus sekarang (`scheduled`) atau batalkan. Mengembalikan workspace tanpa
    /// menulis bila status jadwalnya sudah sama: jadwal ulang tidak memperpanjang tenggat.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn set_deletion_scheduled<C: ConnectionTrait>(
        self,
        db: &C,
        scheduled: bool,
    ) -> Result<Self, DbErr> {
        if self.deletion_scheduled_at.is_some() == scheduled {
            return Ok(self);
        }
        let mut workspace = self.into_active_model();
        workspace.deletion_scheduled_at = Set(scheduled.then(|| chrono::Utc::now().into()));
        workspace.update(db).await
    }

    /// Id workspace yang masa tenggangnya sudah lewat pada `now` (job pembersihan). Lintas
    /// tenant, sengaja: tidak ada workspace "aktif" untuk membatasi query ini.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn due_for_purge<C: ConnectionTrait>(
        db: &C,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<Uuid>, DbErr> {
        Entity::find()
            .select_only()
            .column(Column::Id)
            .filter(purge_due(now))
            .order_by_asc(Column::DeletionScheduledAt)
            .into_tuple()
            .all(db)
            .await
    }

    /// Hapus permanen workspace `id` bila masa tenggangnya masih lewat pada `now` (dicek ulang
    /// di `DELETE` yang sama, supaya pembatalan yang menang lebih dulu tidak ikut terhapus).
    /// Semua data tenant ikut terhapus di statement yang sama lewat FK `ON DELETE CASCADE`
    /// dari `workspaces`; FK `role_id` tanpa ON DELETE (anggota, undangan, grant) tidak
    /// menghalangi karena baris yang merujuk role ikut terhapus di statement itu. Audit log
    /// workspace ikut terhapus, disengaja. `true` bila ada yang terhapus.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn purge_if_due<C: ConnectionTrait>(
        db: &C,
        id: Uuid,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<bool, DbErr> {
        let res = Entity::delete_many()
            .filter(Column::Id.eq(id))
            .filter(purge_due(now))
            .exec(db)
            .await?;
        Ok(res.rows_affected > 0)
    }

    /// Ganti nama dan slug setelah normalisasi dan validasi yang sama dengan
    /// [`ActiveModel::create`]. Slug kosong berarti tetap.
    ///
    /// # Errors
    ///
    /// [`WorkspaceError::Invalid`] untuk field tidak valid atau slug terpakai,
    /// [`WorkspaceError::Db`] untuk error database lain.
    pub async fn update_details<C: ConnectionTrait>(
        self,
        db: &C,
        name: &str,
        slug: &str,
    ) -> Result<Self, WorkspaceError> {
        let slug = if slug.trim().is_empty() {
            self.slug.clone()
        } else {
            slug.to_string()
        };
        let (name, slug) = normalize_and_validate(name, &slug)?;
        let mut workspace = self.into_active_model();
        workspace.name = Set(name);
        workspace.slug = Set(slug);
        workspace.update(db).await.map_err(slug_taken_or_db)
    }
}

impl ActiveModel {
    /// Buat workspace (tanpa role dan anggota; workflow lengkapnya
    /// `services::workspaces::create`).
    ///
    /// # Errors
    ///
    /// [`WorkspaceError::Invalid`] untuk field tidak valid, slug terlarang, atau slug yang
    /// sudah dipakai (unique index `lower(slug)`), [`WorkspaceError::Db`] untuk error lain.
    pub async fn create<C: ConnectionTrait>(
        db: &C,
        new: CreateWorkspace,
    ) -> Result<Model, WorkspaceError> {
        let slug = if new.slug.trim().is_empty() {
            text::slugify(&new.name)
        } else {
            new.slug
        };
        let (name, slug) = normalize_and_validate(&new.name, &slug)?;
        Self {
            name: Set(name),
            slug: Set(slug),
            created_by: Set(new.created_by),
            ..Default::default()
        }
        .insert(db)
        .await
        .map_err(slug_taken_or_db)
    }
}

/// Terjadwal hapus lebih dari masa tenggang sebelum `now`.
fn purge_due(now: chrono::DateTime<chrono::Utc>) -> sea_orm::Condition {
    sea_orm::Condition::all()
        .add(Column::DeletionScheduledAt.lt(now - chrono::Duration::days(DELETION_GRACE_DAYS)))
}

/// `lower(slug)`: bentuk yang sama dengan unique index, sehingga pencarian memakai index itu.
fn lower_slug() -> Expr {
    Expr::expr(Func::lower(Expr::col(Column::Slug)))
}

/// Normalisasi nama (`trim`) dan slug (`trim` + huruf kecil), lalu validasi keduanya sekaligus
/// supaya form menampilkan semua error.
fn normalize_and_validate(name: &str, slug: &str) -> Result<(String, String), WorkspaceError> {
    let name = text::normalize_name(name);
    let slug = slug.trim().to_lowercase();
    let mut errors = ValidationErrors::new();
    if let Err(e) = text::validate_name("workspace", &name) {
        errors.add("name", e);
    }
    if let Err(e) = text::validate_slug(&slug) {
        errors.add("slug", e);
    } else if RESERVED_SLUGS.contains(&slug.as_str()) {
        errors.add("slug", ValidationError::new(SLUG_RESERVED));
    }
    if errors.is_empty() {
        Ok((name, slug))
    } else {
        Err(WorkspaceError::Invalid(errors.into()))
    }
}

fn slug_taken_or_db(e: DbErr) -> WorkspaceError {
    if matches!(e.sql_err(), Some(SqlErr::UniqueConstraintViolation(_))) {
        let mut errors = ValidationErrors::new();
        errors.add("slug", ValidationError::new(SLUG_TAKEN));
        WorkspaceError::Invalid(errors.into())
    } else {
        WorkspaceError::Db(e)
    }
}

/// Run job pembersihan pertama yang menghapus workspace dengan tenggat `deadline`. Job
/// menghapus bila `deletion_scheduled_at < now - tenggang` (`purge_if_due`), jadi run tepat di
/// tenggat belum menghapus: run itu harus sesudah tenggat.
fn first_cleanup_after(deadline: chrono::DateTime<chrono::Utc>) -> chrono::DateTime<chrono::Utc> {
    let same_day = (deadline.date_naive().and_time(chrono::NaiveTime::MIN)
        + chrono::Duration::hours(i64::from(PURGE_HOUR_UTC)))
    .and_utc();
    if same_day > deadline {
        same_day
    } else {
        same_day + chrono::Duration::days(1)
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::first_cleanup_after;

    #[test]
    fn purge_starts_at_first_cleanup_run_after_deadline() {
        let at = |d, h, m, s| Utc.with_ymd_and_hms(2026, 12, d, h, m, s).unwrap();
        assert_eq!(first_cleanup_after(at(11, 2, 59, 59)), at(11, 3, 0, 0));
        assert_eq!(first_cleanup_after(at(11, 3, 0, 0)), at(12, 3, 0, 0));
        assert_eq!(first_cleanup_after(at(11, 15, 30, 0)), at(12, 3, 0, 0));
        assert_eq!(first_cleanup_after(at(11, 0, 0, 0)), at(11, 3, 0, 0));
    }
}
