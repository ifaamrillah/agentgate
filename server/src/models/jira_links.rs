//! Tautan tiket ↔ issue Jira (M11). `snapshot` adalah nilai field terakhir yang disepakati kedua
//! sisi ([`Snapshot`]): mirror tidak mendorong field yang sama dengan snapshot (perubahan yang
//! baru ditulis dari Jira tidak memantul balik), dan tarik dua arah menggabungkan tiga arah
//! (snapshot = dasar; sisi yang berubah menang; keduanya berubah → perubahan terakhir menang).

use sea_orm::{ActiveValue::Set, PaginatorTrait, entity::prelude::*};
use serde::{Deserialize, Serialize};

pub use super::_entities::jira_links::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

standard_behavior!(ActiveModel);

/// Nilai field yang disinkronkan, dalam bentuk Agent Gate (prioritas = kunci Agent Gate,
/// tanggal `YYYY-MM-DD`, label urut). Field yang tidak dipetakan tetap ada tetapi diabaikan.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub priority: String,
    #[serde(default)]
    pub due_on: Option<String>,
    #[serde(default)]
    pub story_points: Option<i32>,
}

impl Snapshot {
    /// Normalisasi supaya dua sisi bisa dibandingkan (label urut tanpa duplikat, teks di-trim).
    #[must_use]
    pub fn normalized(mut self) -> Self {
        self.title = self.title.trim().to_string();
        self.description = self.description.trim().replace("\r\n", "\n");
        self.labels.sort();
        self.labels.dedup();
        self
    }
}

/// Data tautan baru.
#[derive(Debug, Clone)]
pub struct NewLink {
    pub workspace_id: WorkspaceId,
    pub connection_id: Uuid,
    pub ticket_id: Uuid,
    pub issue_id: String,
    pub issue_key: String,
    pub jira_status_id: String,
    pub snapshot: Snapshot,
}

impl Model {
    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        serde_json::from_value(self.snapshot.clone()).unwrap_or_default()
    }

    /// # Errors
    ///
    /// Error database.
    pub async fn of_ticket<C: ConnectionTrait>(
        db: &C,
        connection_id: Uuid,
        ticket_id: Uuid,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find()
            .filter(Column::ConnectionId.eq(connection_id))
            .filter(Column::TicketId.eq(ticket_id))
            .one(db)
            .await
    }

    /// # Errors
    ///
    /// Error database.
    pub async fn of_issue<C: ConnectionTrait>(
        db: &C,
        connection_id: Uuid,
        issue_id: &str,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find()
            .filter(Column::ConnectionId.eq(connection_id))
            .filter(Column::IssueId.eq(issue_id))
            .one(db)
            .await
    }

    /// # Errors
    ///
    /// Error database.
    pub async fn count_of<C: ConnectionTrait>(db: &C, connection_id: Uuid) -> Result<u64, DbErr> {
        Entity::find()
            .filter(Column::ConnectionId.eq(connection_id))
            .count(db)
            .await
    }

    /// # Errors
    ///
    /// Error database.
    pub async fn all_of<C: ConnectionTrait>(
        db: &C,
        connection_id: Uuid,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::ConnectionId.eq(connection_id))
            .all(db)
            .await
    }
}

impl ActiveModel {
    /// # Errors
    ///
    /// Error database (termasuk bentrok unik: issue/tiket sudah tertaut).
    pub async fn link<C: ConnectionTrait>(db: &C, new: NewLink) -> Result<Model, DbErr> {
        Self {
            workspace_id: Set(new.workspace_id.get()),
            connection_id: Set(new.connection_id),
            ticket_id: Set(new.ticket_id),
            issue_id: Set(new.issue_id),
            issue_key: Set(new.issue_key),
            jira_status_id: Set(new.jira_status_id),
            snapshot: Set(serde_json::to_value(new.snapshot.normalized()).unwrap_or_default()),
            synced_at: Set(Some(chrono::Utc::now().into())),
            ..Default::default()
        }
        .insert(db)
        .await
    }

    /// Simpan snapshot dan status Jira terakhir yang disepakati.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn agree<C: ConnectionTrait>(
        db: &C,
        link: Model,
        snapshot: Snapshot,
        jira_status_id: &str,
    ) -> Result<Model, DbErr> {
        let mut m: Self = link.into();
        m.snapshot = Set(serde_json::to_value(snapshot.normalized()).unwrap_or_default());
        m.jira_status_id = Set(jira_status_id.to_string());
        m.synced_at = Set(Some(chrono::Utc::now().into()));
        m.update(db).await
    }
}
