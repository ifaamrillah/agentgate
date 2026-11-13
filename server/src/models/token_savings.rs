//! Penghematan token yang terukur server (M10): satu baris per kejadian (mis. `ag_snippet`
//! mengirim potongan, bukan file utuh). Agregasinya tampil di halaman Penghemat token project.
use chrono::{DateTime, FixedOffset, NaiveDate};
use sea_orm::{ActiveEnum, DeriveActiveEnum, EnumIter, FromQueryResult};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::projects;

pub use super::_entities::token_savings::{ActiveModel, Column, Entity, Model};

standard_behavior!(ActiveModel, append_only);

/// Fitur penghemat yang penghematannya bisa diukur dari sisi server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum SavingFeature {
    /// Code graph built-in: potongan/ringkasan simbol, bukan membaca file utuh.
    #[sea_orm(string_value = "code_graph")]
    CodeGraph,
    /// Respons MCP berlapis: detail (isi artifact, skill) tidak dikirim sampai diminta.
    #[sea_orm(string_value = "layered")]
    Layered,
    /// Aturan lewat MCP: hanya instruksi stage sekarang, bukan semua stage.
    #[sea_orm(string_value = "mcp_rules")]
    McpRules,
    /// Serah terima ringkas: ringkasan artifact stage sebelumnya, bukan transkrip.
    #[sea_orm(string_value = "handoff")]
    Handoff,
}

impl Model {
    #[must_use]
    pub fn feature(&self) -> Option<SavingFeature> {
        SavingFeature::try_from_value(&self.feature).ok()
    }
}

/// Toggle fitur opsional per project (`projects.token_saver`). Fitur wajib tidak bisa dimatikan
/// dan tidak disimpan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenSaver {
    /// Gaya ringkas persona (chat dan log); artifact tetap lengkap.
    #[serde(default = "yes")]
    pub concise: bool,
    /// Model per persona (model murah untuk tugas ringan).
    #[serde(default = "yes")]
    pub persona_models: bool,
}

fn yes() -> bool {
    true
}

impl Default for TokenSaver {
    fn default() -> Self {
        Self {
            concise: true,
            persona_models: true,
        }
    }
}

impl TokenSaver {
    #[must_use]
    pub fn of(project: &projects::Model) -> Self {
        serde_json::from_value(project.token_saver.clone()).unwrap_or_default()
    }
}

/// Pemakaian token dalam satu rentang (job yang selesai).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, FromQueryResult)]
pub struct Usage {
    pub input: i64,
    pub output: i64,
    pub cache_read: i64,
    pub cache_write: i64,
    /// Total = jumlah keempatnya, atau `jobs.tokens` bila client hanya melapor total.
    pub total: i64,
    pub cost_micros: i64,
    /// Job yang melaporkan pemakaian.
    pub jobs: i64,
    /// Client berbeda yang melapor.
    pub clients: i64,
}

/// Satu minggu (mulai Senin) di grafik tren.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Week {
    pub start: NaiveDate,
    pub used: i64,
    pub saved: i64,
}

/// Pemakaian satu tiket.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, FromQueryResult)]
pub struct TicketUsage {
    pub code: String,
    pub title: String,
    pub input: i64,
    pub output: i64,
    pub cache: i64,
    pub total: i64,
    pub cost_micros: i64,
    pub jobs: i64,
}

/// Satu job di tabel pemakaian.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, FromQueryResult)]
pub struct JobUsage {
    pub id: Uuid,
    pub code: Option<String>,
    pub persona: String,
    pub client: String,
    pub model: String,
    pub status: String,
    pub input: Option<i64>,
    pub output: Option<i64>,
    pub cache: Option<i64>,
    pub total: Option<i64>,
    pub cost_micros: Option<i64>,
    pub finished_at: Option<DateTime<FixedOffset>>,
    pub saved: i64,
}
