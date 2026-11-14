//! Relasi lintas repo per project (M7): HTTP call ↔ route, event, shared package.
use agentgate_graph::cross::{LinkConfidence, LinkKind};

pub use super::_entities::graph_links::{ActiveModel, Column, Entity, Model};

standard_behavior!(ActiveModel, append_only);

impl Model {
    #[must_use]
    pub fn kind(&self) -> LinkKind {
        LinkKind::parse(&self.kind).unwrap_or(LinkKind::Http)
    }

    #[must_use]
    pub fn confidence(&self) -> LinkConfidence {
        LinkConfidence::parse(&self.confidence).unwrap_or(LinkConfidence::Inferred)
    }
}
