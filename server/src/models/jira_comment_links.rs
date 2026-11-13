//! Tautan komentar tiket ↔ komentar Jira (M11) beserta asalnya. Komentar yang sudah bertaut
//! tidak disalin lagi ke arah mana pun: komentar dari Jira tidak didorong balik oleh mirror, dan
//! komentar yang didorong mirror tidak ditarik lagi oleh sinkron dua arah.

use sea_orm::{
    ActiveValue::Set,
    entity::prelude::*,
    sea_query::OnConflict,
};

pub use super::_entities::jira_comment_links::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

standard_behavior!(ActiveModel);

/// Asal komentar (CHECK `jira_comment_links.origin`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, serde::Serialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum CommentOrigin {
    #[sea_orm(string_value = "agent_gate")]
    AgentGate,
    #[sea_orm(string_value = "jira")]
    Jira,
}

impl Model {
    /// # Errors
    ///
    /// Error database.
    pub async fn of_comment<C: ConnectionTrait>(
        db: &C,
        comment_id: Uuid,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find()
            .filter(Column::CommentId.eq(comment_id))
            .one(db)
            .await
    }

    /// # Errors
    ///
    /// Error database.
    pub async fn of_jira<C: ConnectionTrait>(
        db: &C,
        connection_id: Uuid,
        jira_comment_id: &str,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find()
            .filter(Column::ConnectionId.eq(connection_id))
            .filter(Column::JiraCommentId.eq(jira_comment_id))
            .one(db)
            .await
    }
}

impl ActiveModel {
    /// Catat tautan; sudah ada → diabaikan.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn record<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        connection_id: Uuid,
        comment_id: Uuid,
        jira_comment_id: &str,
        origin: CommentOrigin,
    ) -> Result<(), DbErr> {
        let row = Self {
            id: Set(Uuid::now_v7()),
            workspace_id: Set(wid.get()),
            connection_id: Set(connection_id),
            comment_id: Set(comment_id),
            jira_comment_id: Set(jira_comment_id.to_string()),
            origin: Set(origin.to_value()),
            ..Default::default()
        };
        Entity::insert(row)
            .on_conflict(OnConflict::new().do_nothing().to_owned())
            .exec_without_returning(db)
            .await?;
        Ok(())
    }
}
