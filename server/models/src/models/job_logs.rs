//! Log job dari runner (append-only).
pub use super::_entities::job_logs::{ActiveModel, Column, Entity, Model};

standard_behavior!(ActiveModel, append_only);
