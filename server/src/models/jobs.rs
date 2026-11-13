//! Job: AI mengerjakan satu stage untuk satu tiket (atau `@agent` dari komentar) di runner
//! executor. Status mengikuti `agentgate_core::runner::protocol::JobStatus`.
use agentgate_core::runner::protocol::{JobKind, JobMode, JobStatus, Step};

pub use super::_entities::jobs::{ActiveModel, Column, Entity, Model};

standard_behavior!(ActiveModel);

/// Status job yang masih terbuka (nilai kolom).
pub const OPEN: &[&str] = &["queued", "claimed", "running", "paused", "quota"];

impl Model {
    #[must_use]
    pub fn status(&self) -> JobStatus {
        JobStatus::parse(&self.status).unwrap_or(JobStatus::Failed)
    }

    #[must_use]
    pub fn kind(&self) -> JobKind {
        JobKind::parse(&self.kind).unwrap_or(JobKind::Stage)
    }

    #[must_use]
    pub fn mode(&self) -> JobMode {
        JobMode::parse(&self.mode).unwrap_or(JobMode::Auto)
    }

    /// Langkah job (kosong bila belum dilaporkan).
    #[must_use]
    pub fn steps(&self) -> Vec<Step> {
        serde_json::from_value(self.steps.clone()).unwrap_or_default()
    }
}
