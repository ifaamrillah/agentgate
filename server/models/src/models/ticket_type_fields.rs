//! Pemakaian field custom oleh tipe tiket (banyak-ke-banyak). Satu field bisa dipakai beberapa
//! tipe; field yang tidak dipakai tipe tiket tidak tampil di form tiket itu.

use loco_rs::model::{TenantEntity, TenantQueryExt};
use sea_orm::{ActiveValue::Set, QueryOrder, QuerySelect, entity::prelude::*};

pub use super::_entities::ticket_type_fields::{ActiveModel, Column, Entity, Model};
use super::{custom_fields, workspaces::WorkspaceId};

impl TenantEntity for Entity {
    type TenantId = WorkspaceId;

    fn tenant_column() -> Column {
        Column::WorkspaceId
    }
}

standard_behavior!(ActiveModel);

impl Model {
    /// Field yang dipakai tipe `type_id`, urut nama.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn fields_of<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        type_id: Uuid,
    ) -> Result<Vec<custom_fields::Model>, DbErr> {
        custom_fields::Entity::find()
            .filter(custom_fields::Column::WorkspaceId.eq(workspace_id.get()))
            .filter(
                custom_fields::Column::Id.in_subquery(sea_orm::QueryTrait::into_query(
                    Entity::find()
                        .select_only()
                        .column(Column::FieldId)
                        .filter(Column::TypeId.eq(type_id)),
                )),
            )
            .order_by_asc(custom_fields::Column::Name)
            .order_by_asc(custom_fields::Column::Id)
            .all(db)
            .await
    }

    /// Semua pasangan (tipe, field) workspace.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn all_in<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find().in_tenant(workspace_id).all(db).await
    }

    /// Ganti himpunan field tipe `type_id` dengan `field_ids` (yang bukan milik workspace ini
    /// ditolak FK komposit).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn replace_for_type<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        type_id: Uuid,
        field_ids: &[Uuid],
    ) -> Result<(), DbErr> {
        Entity::delete_many()
            .filter(Column::WorkspaceId.eq(workspace_id.get()))
            .filter(Column::TypeId.eq(type_id))
            .exec(db)
            .await?;
        for field_id in field_ids {
            ActiveModel {
                workspace_id: Set(workspace_id.get()),
                type_id: Set(type_id),
                field_id: Set(*field_id),
                ..Default::default()
            }
            .insert(db)
            .await?;
        }
        Ok(())
    }

    /// Ganti himpunan tipe yang memakai field `field_id`.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn replace_for_field<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        field_id: Uuid,
        type_ids: &[Uuid],
    ) -> Result<(), DbErr> {
        Entity::delete_many()
            .filter(Column::WorkspaceId.eq(workspace_id.get()))
            .filter(Column::FieldId.eq(field_id))
            .exec(db)
            .await?;
        for type_id in type_ids {
            ActiveModel {
                workspace_id: Set(workspace_id.get()),
                type_id: Set(*type_id),
                field_id: Set(field_id),
                ..Default::default()
            }
            .insert(db)
            .await?;
        }
        Ok(())
    }
}
