//! Workspace mewajibkan 2FA (Pengaturan → Zona berbahaya). Penegakannya ada di extractor
//! `Tenant`: anggota tanpa 2FA diarahkan ke setup 2FA di setiap halaman workspace.
//!
//! Hanya role bawaan Owner (spec: "bisa diwajibkan Owner"), bukan sekadar `ManageWorkspace`:
//! role custom pemegang `ManageWorkspace` tidak boleh bisa mematikan kebijakan keamanan ini.
//! Role pelaku dibaca ulang setelah baris workspace dikunci.
//!
//! Mewajibkan 2FA butuh pelaku yang sudah ber-2FA, supaya ia tidak mengunci dirinya sendiri
//! dari workspace. Mematikannya selalu boleh. Nilai yang sama tidak menulis dan tidak diaudit.

use sea_orm::{DatabaseConnection, DbErr, TransactionTrait};

use super::{Acting, lock_as};
use crate::{
    models::{roles, workspaces},
    services::audit::{self, AuditAction, AuditSubject},
};

/// Pelaku belum mengaktifkan 2FA di akunnya sendiri.
pub const REQUIRE_2FA_ACTOR_MISSING: &str = "require-2fa-actor-missing";
/// Kode error milik service ini (dijaga tes i18n).
pub const ERROR_CODES: &[&str] = &[REQUIRE_2FA_ACTOR_MISSING];

#[derive(Debug, thiserror::Error)]
pub enum RequireTwoFactorError {
    /// Pelaku sudah bukan anggota, bukan Owner, atau workspace sudah dihapus permanen.
    #[error("not found")]
    NotFound,
    #[error("actor has no 2FA")]
    ActorMissing,
    #[error(transparent)]
    Db(#[from] DbErr),
}

/// Wajibkan (`enabled`) atau berhenti mewajibkan 2FA, dengan audit
/// `workspace.require_2fa_changed` di transaksi yang sama.
///
/// # Errors
///
/// [`RequireTwoFactorError::ActorMissing`] bila mewajibkan tanpa 2FA sendiri,
/// [`RequireTwoFactorError::NotFound`], atau error database.
pub async fn set(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    enabled: bool,
) -> Result<workspaces::Model, RequireTwoFactorError> {
    if enabled && acting.user.totp_enabled_at.is_none() {
        return Err(RequireTwoFactorError::ActorMissing);
    }
    let txn = db.begin().await?;
    let workspace = lock_as(&txn, acting, roles::Model::is_owner)
        .await?
        .ok_or(RequireTwoFactorError::NotFound)?;
    let before = workspace.require_2fa_at.is_some();
    if before == enabled {
        txn.rollback().await?;
        return Ok(workspace);
    }
    let workspace = workspace.set_require_two_factor(&txn, enabled).await?;
    audit::record(
        &txn,
        acting.actor().audit(
            AuditAction::WorkspaceRequire2faChanged,
            (AuditSubject::Workspace, workspace.id),
            serde_json::json!({ "enabled": audit::change(&before, &enabled) }),
        ),
    )
    .await?;
    txn.commit().await?;
    Ok(workspace)
}
