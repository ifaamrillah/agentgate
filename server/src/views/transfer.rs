//! Data view bersama halaman Ekspor data dan Impor CSV (M11).
use std::collections::HashMap;

use sea_orm::{ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter};
use uuid::Uuid;

use super::tickets::PersonView;
use crate::models::users;

/// Orang per id (pembuat ekspor/impor) untuk kolom "Oleh".
///
/// # Errors
///
/// Error database.
pub async fn people_by_id<C: ConnectionTrait>(
    db: &C,
    ids: Vec<Uuid>,
) -> Result<HashMap<Uuid, PersonView>, DbErr> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    Ok(users::Entity::find()
        .filter(users::Column::Id.is_in(ids))
        .all(db)
        .await?
        .iter()
        .map(|u| (u.id, PersonView::of(u)))
        .collect())
}
