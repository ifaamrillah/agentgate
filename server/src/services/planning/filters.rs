//! Saved filter JQL: milik user, dibagikan ke satu tim (pemilik harus anggotanya) atau seluruh
//! workspace. Hanya pemilik yang bisa mengubah/menghapus.
use sea_orm::{
    ActiveEnum, ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait,
    QueryFilter,
};
use uuid::Uuid;

use super::{Failure, NAME_INVALID, QUERY_INVALID, TEAM_INVALID, jql};
use crate::{
    models::{
        saved_filters::{self, NAME_MAX_LEN, QUERY_MAX_LEN, Visibility},
        team_members,
    },
    services::workspaces::Acting,
};

/// Isian saved filter mentah.
#[derive(Debug, Clone, Default)]
pub struct FilterInput {
    pub name: String,
    pub query: String,
    /// `private`, `team`, `workspace`.
    pub visibility: String,
    /// Id tim bila `team`.
    pub team: String,
}

/// Tim tempat user menjadi anggota.
///
/// # Errors
///
/// Error database.
pub async fn team_ids(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
) -> Result<Vec<Uuid>, sea_orm::DbErr> {
    Ok(
        team_members::Model::in_teams_of(db, acting.workspace_id(), [acting.user.id])
            .await?
            .iter()
            .map(|m| m.team_id)
            .collect(),
    )
}

/// Saved filter yang boleh dilihat user.
///
/// # Errors
///
/// Error database.
pub async fn visible(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
) -> Result<Vec<saved_filters::Model>, sea_orm::DbErr> {
    let teams = team_ids(db, acting).await?;
    saved_filters::Model::visible_to(db, acting.workspace_id(), acting.user.id, &teams).await
}

/// Simpan saved filter baru.
///
/// # Errors
///
/// `planning-name-invalid`, `planning-query-invalid`, `planning-team-invalid`, error database.
pub async fn create(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    input: &FilterInput,
) -> Result<saved_filters::Model, Failure> {
    let name = input.name.trim().to_string();
    if name.is_empty() || name.chars().count() > NAME_MAX_LEN {
        return Err(Failure::field("name", NAME_INVALID));
    }
    let query = input.query.trim().to_string();
    if query.is_empty() || query.chars().count() > QUERY_MAX_LEN || jql::parse(&query).is_err() {
        return Err(Failure::field("query", QUERY_INVALID));
    }
    let (visibility, team_id) = match input.visibility.as_str() {
        "workspace" => (Visibility::Workspace, None),
        "team" => {
            let id = Uuid::parse_str(input.team.trim())
                .map_err(|_| Failure::field("team", TEAM_INVALID))?;
            if !team_ids(db, acting).await?.contains(&id) {
                return Err(Failure::field("team", TEAM_INVALID));
            }
            (Visibility::Team, Some(id))
        }
        _ => (Visibility::Private, None),
    };
    Ok(saved_filters::ActiveModel {
        workspace_id: Set(acting.workspace_id().get()),
        owner_id: Set(acting.user.id),
        name: Set(name),
        query: Set(query),
        visibility: Set(visibility.to_value()),
        team_id: Set(team_id),
        ..Default::default()
    }
    .insert(db)
    .await?)
}

/// Hapus saved filter milik user.
///
/// # Errors
///
/// `NotFound` (bukan pemilik), error database.
pub async fn delete(db: &DatabaseConnection, acting: &Acting<'_>, id: Uuid) -> Result<(), Failure> {
    let f = saved_filters::Entity::find_by_id(id)
        .filter(saved_filters::Column::WorkspaceId.eq(acting.workspace_id().get()))
        .filter(saved_filters::Column::OwnerId.eq(acting.user.id))
        .one(db)
        .await?
        .ok_or(Failure::NotFound)?;
    saved_filters::Entity::delete_by_id(f.id).exec(db).await?;
    Ok(())
}
