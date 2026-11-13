//! Code graph Agent Gate: parsing kode dengan tree-sitter dan pembangunan graph.
//! Dipisah dari `server` karena grammar tree-sitter berat di-compile, dan dipakai server maupun CLI.
pub mod extract;
pub mod facts;
pub mod lang;
pub mod manifest;
pub mod walk;

pub use extract::extract;
pub use facts::{FileFacts, SymbolKind};
pub use lang::Lang;
