//! Status index code graph per repo (M7). Satu baris per repo, dibuat saat index pertama.
use agentgate_graph::{
    build::{BuildStats, ScipData},
    history::GitStats,
};
use sea_orm::{QueryOrder, entity::prelude::*};
use serde::{Deserialize, Serialize};

pub use super::_entities::graph_repos::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if !insert && self.updated_at.is_unchanged() {
            self.updated_at = sea_orm::ActiveValue::Set(chrono::Utc::now().into());
        }
        Ok(self)
    }
}

/// Status index (CHECK `graph_repos.status`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum IndexStatus {
    #[sea_orm(string_value = "none")]
    None,
    #[sea_orm(string_value = "queued")]
    Queued,
    #[sea_orm(string_value = "running")]
    Running,
    #[sea_orm(string_value = "ready")]
    Ready,
    #[sea_orm(string_value = "failed")]
    Failed,
}

/// Asal index terakhir (CHECK `graph_repos.source`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum IndexSource {
    /// `agentgate graph index` di laptop user.
    #[sea_orm(string_value = "cli")]
    Cli,
    /// Clone oleh server (Index ulang, merge ke branch utama).
    #[sea_orm(string_value = "server")]
    Server,
}

/// Jenis index (CHECK `graph_repos.mode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum IndexMode {
    #[sea_orm(string_value = "incremental")]
    Incremental,
    #[sea_orm(string_value = "full")]
    Full,
}

/// Statistik build (`graph_repos.stats`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoStats {
    #[serde(default)]
    pub build: BuildStats,
    #[serde(default)]
    pub todos: u32,
    /// Fungsi dengan kompleksitas > 15.
    #[serde(default)]
    pub complex: u32,
    #[serde(default)]
    pub dead: u32,
    #[serde(default)]
    pub parse_errors: u32,
    #[serde(default)]
    pub lines: u32,
}

/// Bahasa: jumlah file + baris.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LangCount {
    pub files: u32,
    pub lines: u32,
}

/// File dilewati per alasan.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkipCount {
    pub count: u32,
    pub examples: Vec<String>,
}

/// Modul hasil clustering (`graph_repos.modules`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleRow {
    pub name: String,
    pub files: u32,
    pub symbols: u32,
    #[serde(default)]
    pub central: Option<Uuid>,
    #[serde(default)]
    pub central_name: String,
}

/// Permukaan lintas repo dengan id simbol (`graph_repos.surface`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceRow {
    #[serde(default)]
    pub routes: Vec<(Uuid, String, String)>,
    /// (simbol, method, path, exact, host)
    #[serde(default)]
    pub http: Vec<(Uuid, String, String, bool, String)>,
    #[serde(default)]
    pub events: Vec<(Uuid, String, bool)>,
    #[serde(default)]
    pub packages: Vec<String>,
    #[serde(default)]
    pub deps: Vec<String>,
}

impl Model {
    /// Baris kosong (repo belum pernah di-index).
    #[must_use]
    pub fn empty() -> Self {
        let t: DateTimeWithTimeZone = chrono::Utc::now().into();
        Self {
            repo_id: Uuid::nil(),
            workspace_id: Uuid::nil(),
            project_id: Uuid::nil(),
            status: "none".into(),
            source: String::new(),
            mode: String::new(),
            commit_sha: String::new(),
            branch: String::new(),
            files_indexed: 0,
            files_skipped: 0,
            files_parsed: 0,
            symbols: 0,
            edges: 0,
            edges_certain: 0,
            stats: serde_json::json!({}),
            languages: serde_json::json!({}),
            skipped: serde_json::json!({}),
            modules: serde_json::json!([]),
            surface: serde_json::json!({}),
            git_stats: serde_json::json!({}),
            scip: None,
            scip_indexer: String::new(),
            scip_by: String::new(),
            scip_at: None,
            error: String::new(),
            requested_by: None,
            started_at: None,
            indexed_at: None,
            created_at: t,
            updated_at: t,
            fetch_via: String::new(),
            fetch_note: String::new(),
            embed_symbols: 0,
            embed_ms: 0,
            embedded_at: None,
            embed_error: String::new(),
        }
    }

    #[must_use]
    pub fn status(&self) -> IndexStatus {
        IndexStatus::try_from_value(&self.status).unwrap_or(IndexStatus::None)
    }

    #[must_use]
    pub fn source(&self) -> Option<IndexSource> {
        IndexSource::try_from_value(&self.source).ok()
    }

    #[must_use]
    pub fn stats(&self) -> RepoStats {
        serde_json::from_value(self.stats.clone()).unwrap_or_default()
    }

    #[must_use]
    pub fn languages(&self) -> Vec<(String, LangCount)> {
        let map: std::collections::BTreeMap<String, LangCount> =
            serde_json::from_value(self.languages.clone()).unwrap_or_default();
        let mut v: Vec<(String, LangCount)> = map.into_iter().collect();
        v.sort_by(|a, b| b.1.lines.cmp(&a.1.lines).then(a.0.cmp(&b.0)));
        v
    }

    #[must_use]
    pub fn skipped(&self) -> Vec<(String, SkipCount)> {
        let map: std::collections::BTreeMap<String, SkipCount> =
            serde_json::from_value(self.skipped.clone()).unwrap_or_default();
        map.into_iter().collect()
    }

    #[must_use]
    pub fn modules(&self) -> Vec<ModuleRow> {
        serde_json::from_value(self.modules.clone()).unwrap_or_default()
    }

    #[must_use]
    pub fn surface(&self) -> SurfaceRow {
        serde_json::from_value(self.surface.clone()).unwrap_or_default()
    }

    #[must_use]
    pub fn git(&self) -> GitStats {
        serde_json::from_value(self.git_stats.clone()).unwrap_or_default()
    }

    #[must_use]
    pub fn scip(&self) -> Option<ScipData> {
        self.scip
            .as_ref()
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// Cakupan index (persen file yang masuk graph).
    #[must_use]
    pub fn coverage(&self) -> f64 {
        let total = self.files_indexed + self.files_skipped;
        if total == 0 {
            0.0
        } else {
            f64::from(self.files_indexed) * 100.0 / f64::from(total)
        }
    }

    /// Index repo project (urut menurut repo dibuat).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn of_project<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        project: Uuid,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::WorkspaceId.eq(wid.get()))
            .filter(Column::ProjectId.eq(project))
            .order_by_asc(Column::CreatedAt)
            .all(db)
            .await
    }

    /// Index satu repo.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn of_repo<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        repo: Uuid,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find()
            .filter(Column::WorkspaceId.eq(wid.get()))
            .filter(Column::RepoId.eq(repo))
            .one(db)
            .await
    }
}
