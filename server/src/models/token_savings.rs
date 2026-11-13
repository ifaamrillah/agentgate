//! Penghematan token yang terukur server (M10): satu baris per kejadian (mis. `ag_snippet`
//! mengirim potongan, bukan file utuh). Agregasinya tampil di halaman Penghemat token project.
use sea_orm::{ActiveEnum, DeriveActiveEnum, EnumIter};

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
