//! Edge code graph (M7): `calls`, `inherits`, `imports`, `handles` dengan keyakinan dan sumber.
pub use super::_entities::graph_edges::{ActiveModel, Column, Entity, Model};

standard_behavior!(ActiveModel, append_only);
