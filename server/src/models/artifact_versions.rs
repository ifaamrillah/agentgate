//! Versi artifact (v1, v2, …). Status:
//! - `draft`: disimpan, belum diajukan untuk review;
//! - `pending`: menunggu review (diajukan bersama stage-nya, atau disimpan saat stage sedang
//!   menunggu review);
//! - `approved` / `rejected`: diputuskan approver lewat approve/reject stage.
//!
//! Isi versi tidak pernah diubah; perubahan selalu menjadi versi baru.

use loco_rs::model::{TenantEntity, TenantQueryExt};
use sea_orm::{ActiveValue::Set, QueryOrder, entity::prelude::*};
use serde::Serialize;

pub use super::_entities::artifact_versions::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, DeriveActiveEnum, Serialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum VersionStatus {
    #[sea_orm(string_value = "draft")]
    Draft,
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "approved")]
    Approved,
    #[sea_orm(string_value = "rejected")]
    Rejected,
}

impl TenantEntity for Entity {
    type TenantId = WorkspaceId;

    fn tenant_column() -> Column {
        Column::WorkspaceId
    }
}

standard_behavior!(ActiveModel);

impl Model {
    /// # Errors
    ///
    /// Nilai kolom bukan varian [`VersionStatus`].
    pub fn status(&self) -> Result<VersionStatus, DbErr> {
        VersionStatus::try_from_value(&self.status)
    }

    /// Semua versi artifact-artifact ini, terbaru dulu per artifact.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn of_artifacts<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        artifact_ids: &[Uuid],
    ) -> Result<Vec<Self>, DbErr> {
        if artifact_ids.is_empty() {
            return Ok(Vec::new());
        }
        Entity::find()
            .in_tenant(workspace_id)
            .filter(Column::ArtifactId.is_in(artifact_ids.iter().copied()))
            .order_by_asc(Column::ArtifactId)
            .order_by_desc(Column::Number)
            .all(db)
            .await
    }

    /// Versi terbaru artifact ini.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn latest<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        artifact_id: Uuid,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find()
            .in_tenant(workspace_id)
            .filter(Column::ArtifactId.eq(artifact_id))
            .order_by_desc(Column::Number)
            .one(db)
            .await
    }
}

impl ActiveModel {
    /// Simpan versi berikutnya (nomor = terbaru + 1).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn append<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        artifact_id: Uuid,
        body: &str,
        status: VersionStatus,
        author_id: Uuid,
    ) -> Result<Model, DbErr> {
        let number = Model::latest(db, workspace_id, artifact_id)
            .await?
            .map_or(1, |v| v.number + 1);
        Self {
            workspace_id: Set(workspace_id.get()),
            artifact_id: Set(artifact_id),
            number: Set(number),
            body: Set(body.to_string()),
            status: Set(status.to_value()),
            author_id: Set(Some(author_id)),
            ..Default::default()
        }
        .insert(db)
        .await
    }

    /// Ubah status versi terbaru setiap artifact `artifact_ids` yang berstatus salah satu
    /// `from` menjadi `to` (keputusan approver: `decided_by`, catatan).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn decide_latest<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        artifact_ids: &[Uuid],
        from: &[VersionStatus],
        to: VersionStatus,
        decided_by: Option<Uuid>,
        note: &str,
    ) -> Result<u64, DbErr> {
        let mut n = 0;
        for id in artifact_ids {
            let Some(latest) = Model::latest(db, workspace_id, *id).await? else {
                continue;
            };
            if !latest.status().is_ok_and(|s| from.contains(&s)) {
                continue;
            }
            let mut m: Self = latest.into();
            m.status = Set(to.to_value());
            if to == VersionStatus::Approved || to == VersionStatus::Rejected {
                m.decided_by = Set(decided_by);
                m.decided_at = Set(Some(chrono::Utc::now().into()));
                m.note = Set(note.to_string());
            }
            m.update(db).await?;
            n += 1;
        }
        Ok(n)
    }
}
