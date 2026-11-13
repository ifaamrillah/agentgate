//! Evidence bertanda tangan runner (append-only). Berkas di storage (`evidence/<job>/<id>`).
use agentgate_core::runner::protocol::EvidenceKind;

pub use super::_entities::job_evidence::{ActiveModel, Column, Entity, Model};

standard_behavior!(ActiveModel, append_only);

impl Model {
    #[must_use]
    pub fn kind(&self) -> Option<EvidenceKind> {
        EvidenceKind::parse(&self.kind)
    }
}
