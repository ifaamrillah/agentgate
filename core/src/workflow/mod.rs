//! Workflow: definisi stage dan aturannya, dokumen YAML, dan template bawaan.

pub mod definition;
pub mod templates;
pub mod yaml;

pub use definition::{Category, Definition, Exit, Gate, Issue, Mode, People, Stage};
