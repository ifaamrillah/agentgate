//! Bukti gate manual (R1): screenshot, log, atau teks yang diunggah anggota untuk gate evidence
//! (`tests_passed`, `screenshot_exists`) stage sekarang. Berbeda dengan `job_evidence` (bertanda
//! tangan runner, langsung dihitung gate), bukti manual baru memenuhi gate setelah approver stage
//! menyetujuinya (`status = approved`). Berkas di storage `manual-evidence/<ticket>/<id>`; teks
//! langsung di `body`.
use sea_orm::entity::prelude::*;

pub use super::_entities::manual_evidence::{ActiveModel, Column, Entity, Model};

standard_behavior!(ActiveModel);

/// Gate yang dituju bukti manual. Dua tempat harus sepakat: CHECK `manual_evidence.gate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, serde::Serialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum ManualGate {
    #[sea_orm(string_value = "tests_passed")]
    TestsPassed,
    #[sea_orm(string_value = "screenshot_exists")]
    ScreenshotExists,
}

/// Jenis bukti. Dua tempat harus sepakat: CHECK `manual_evidence.kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, serde::Serialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum ManualKind {
    #[sea_orm(string_value = "screenshot")]
    Screenshot,
    #[sea_orm(string_value = "log")]
    Log,
    #[sea_orm(string_value = "text")]
    Text,
}

/// Status persetujuan. Dua tempat harus sepakat: CHECK `manual_evidence.status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, serde::Serialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum ManualStatus {
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "approved")]
    Approved,
    #[sea_orm(string_value = "rejected")]
    Rejected,
}

impl Model {
    /// Key storage berkas bukti manual tiket-tiket ini (purge tiket).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn keys_of<C: sea_orm::ConnectionTrait>(
        db: &C,
        ticket_ids: &[uuid::Uuid],
    ) -> Result<Vec<String>, DbErr> {
        Ok(Entity::find()
            .filter(Column::TicketId.is_in(ticket_ids.iter().copied()))
            .all(db)
            .await?
            .into_iter()
            .filter_map(|e| e.storage_key)
            .collect())
    }

    #[must_use]
    pub fn gate(&self) -> Option<ManualGate> {
        ManualGate::try_from_value(&self.gate).ok()
    }

    #[must_use]
    pub fn kind(&self) -> Option<ManualKind> {
        ManualKind::try_from_value(&self.kind).ok()
    }

    #[must_use]
    pub fn status(&self) -> ManualStatus {
        ManualStatus::try_from_value(&self.status).unwrap_or(ManualStatus::Pending)
    }
}
