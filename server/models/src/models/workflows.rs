//! Workflow milik workspace (spec "Workflow, stage & gate").
//!
//! - Definisi stage ada di [`super::workflow_versions`]; `version` = versi aktif. Mengubah
//!   workflow selalu membuat versi baru (riwayat), tidak menimpa versi lama, karena tiket yang
//!   belum pindah stage masih memakai aturan stage dari versinya (`tickets.stage_version`).
//! - `draft` = definisi yang sedang disusun di builder (belum berlaku). `None` = tanpa draf.
//! - `key` = slug unik per workspace (nama berkas YAML ekspor), dibentuk dari nama.

use agentgate_core::workflow::{Definition, definition::slug};
use loco_rs::model::{TenantEntity, TenantQueryExt};
use sea_orm::{ActiveValue::Set, QueryOrder, QuerySelect, entity::prelude::*};

pub use super::_entities::workflows::{ActiveModel, Column, Entity, Model};
use super::{workflow_versions, workspaces::WorkspaceId};

pub const NAME_REQUIRED: &str = "workflow-name-required";
pub const NAME_TOO_LONG: &str = "workflow-name-too-long";
pub const NAME_TAKEN: &str = "workflow-name-taken";
pub const DESCRIPTION_TOO_LONG: &str = "workflow-description-too-long";
pub const IN_USE: &str = "workflow-in-use";
pub const ERROR_CODES: &[&str] = &[
    NAME_REQUIRED,
    NAME_TOO_LONG,
    NAME_TAKEN,
    DESCRIPTION_TOO_LONG,
    IN_USE,
];
pub const NAME_MAX_LEN: usize = 60;
pub const DESCRIPTION_MAX_LEN: usize = 300;

impl TenantEntity for Entity {
    type TenantId = WorkspaceId;

    fn tenant_column() -> Column {
        Column::WorkspaceId
    }
}

standard_behavior!(ActiveModel);

/// Validasi nama dan deskripsi (tanpa cek unik; unik dijaga index).
///
/// # Errors
///
/// `(field, kode)` pertama yang gagal.
pub fn validate_identity(
    name: &str,
    description: &str,
) -> Result<(), (&'static str, &'static str)> {
    let name = name.trim();
    if name.is_empty() {
        return Err(("name", NAME_REQUIRED));
    }
    if name.chars().count() > NAME_MAX_LEN || name.chars().any(char::is_control) {
        return Err(("name", NAME_TOO_LONG));
    }
    if description.trim().chars().count() > DESCRIPTION_MAX_LEN {
        return Err(("description", DESCRIPTION_TOO_LONG));
    }
    Ok(())
}

/// Workflow baru beserta versi 1.
#[derive(Debug)]
pub struct NewWorkflow<'a> {
    pub name: &'a str,
    pub description: &'a str,
    pub template_key: Option<&'a str>,
    pub definition: &'a Definition,
    pub created_by: Option<Uuid>,
}

impl Model {
    /// Semua workflow workspace, urut nama.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn all_in<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .in_tenant(workspace_id)
            .order_by_asc(Column::Name)
            .order_by_asc(Column::Id)
            .all(db)
            .await
    }

    /// # Errors
    ///
    /// Error database.
    pub async fn find_in<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        id: Uuid,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(id).in_tenant(workspace_id).one(db).await
    }

    /// Seperti [`Self::find_in`] dengan baris dikunci sampai transaksi selesai.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn lock_in<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        id: Uuid,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(id)
            .in_tenant(workspace_id)
            .lock_exclusive()
            .one(db)
            .await
    }

    /// Workflow tertua di workspace (Kanban bawaan): default tipe tiket baru.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn oldest_in<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find()
            .in_tenant(workspace_id)
            .order_by_asc(Column::CreatedAt)
            .order_by_asc(Column::Id)
            .one(db)
            .await
    }

    /// Definisi versi aktif.
    ///
    /// # Errors
    ///
    /// Error database; versi aktif yang hilang → `RecordNotFound`.
    pub async fn definition<C: ConnectionTrait>(&self, db: &C) -> Result<Definition, DbErr> {
        workflow_versions::Model::definition_of(db, self.workspace_id, self.id, self.version).await
    }

    /// Draf builder, bila ada.
    #[must_use]
    pub fn draft(&self) -> Option<Definition> {
        self.draft
            .clone()
            .and_then(|d| serde_json::from_value(d).ok())
    }
}

impl ActiveModel {
    /// Buat workflow + versi 1. Kunci dari nama, diberi akhiran bila sudah dipakai.
    ///
    /// # Errors
    ///
    /// Error database (nama kembar → unique violation, ditangani pemanggil).
    pub async fn create<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        new: NewWorkflow<'_>,
    ) -> Result<Model, DbErr> {
        let base = match slug(new.name) {
            s if s.is_empty() => "workflow".to_string(),
            s => s,
        };
        let taken: Vec<String> = Entity::find()
            .in_tenant(workspace_id)
            .all(db)
            .await?
            .into_iter()
            .map(|w| w.key)
            .collect();
        let key = if taken.contains(&base) {
            (2..)
                .map(|n| format!("{base}_{n}"))
                .find(|k| !taken.contains(k))
                .unwrap_or(base)
        } else {
            base
        };
        let workflow = Self {
            workspace_id: Set(workspace_id.get()),
            key: Set(key),
            name: Set(new.name.trim().to_string()),
            description: Set(new.description.trim().to_string()),
            template_key: Set(new.template_key.map(str::to_string)),
            version: Set(1),
            draft: Set(None),
            created_by: Set(new.created_by),
            ..Default::default()
        }
        .insert(db)
        .await?;
        workflow_versions::ActiveModel::append(db, &workflow, 1, new.definition, new.created_by)
            .await?;
        Ok(workflow)
    }
}
