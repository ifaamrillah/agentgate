//! Component project (M9): bagian produk dengan pemilik dan modul code graph (prefix path).
use sea_orm::{ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

pub use super::_entities::components::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

standard_behavior!(ActiveModel);

pub const NAME_MAX_LEN: usize = 80;
pub const DESCRIPTION_MAX_LEN: usize = 300;
pub const MODULES_MAX: usize = 20;

impl Model {
    /// Component di workspace berdasarkan id.
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

    /// Component project urut nama.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn of_project<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        project_id: Uuid,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::WorkspaceId.eq(wid.get()))
            .filter(Column::ProjectId.eq(project_id))
            .order_by_asc(Column::Name)
            .order_by_asc(Column::Id)
            .all(db)
            .await
    }
}
