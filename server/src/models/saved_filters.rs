//! Saved filter JQL (M9): milik user, bisa dibagikan ke satu tim atau seluruh workspace.
use sea_orm::{
    ActiveEnum, ColumnTrait, Condition, ConnectionTrait, DbErr, DeriveActiveEnum, EntityTrait,
    EnumIter, QueryFilter, QueryOrder,
};
use uuid::Uuid;

pub use super::_entities::saved_filters::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

standard_behavior!(ActiveModel);

pub const NAME_MAX_LEN: usize = 80;
pub const QUERY_MAX_LEN: usize = 2000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum Visibility {
    #[sea_orm(string_value = "private")]
    Private,
    #[sea_orm(string_value = "team")]
    Team,
    #[sea_orm(string_value = "workspace")]
    Workspace,
}

impl Model {
    #[must_use]
    pub fn visibility(&self) -> Visibility {
        Visibility::try_from_value(&self.visibility).unwrap_or(Visibility::Private)
    }

    /// Filter yang boleh dilihat user: miliknya, tim yang ia ikuti, dan filter workspace.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn visible_to<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        user_id: Uuid,
        team_ids: &[Uuid],
    ) -> Result<Vec<Self>, DbErr> {
        let mut cond = Condition::any()
            .add(Column::OwnerId.eq(user_id))
            .add(Column::Visibility.eq(Visibility::Workspace.to_value()));
        if !team_ids.is_empty() {
            cond = cond.add(
                Condition::all()
                    .add(Column::Visibility.eq(Visibility::Team.to_value()))
                    .add(Column::TeamId.is_in(team_ids.to_vec())),
            );
        }
        Entity::find()
            .filter(Column::WorkspaceId.eq(wid.get()))
            .filter(cond)
            .order_by_asc(Column::Name)
            .order_by_asc(Column::Id)
            .all(db)
            .await
    }
}
