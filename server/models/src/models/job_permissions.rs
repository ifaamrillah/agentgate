//! Permintaan izin aksi berbahaya dari job yang sedang berjalan.
use agentgate_core::runner::protocol::PermissionStatus;

pub use super::_entities::job_permissions::{ActiveModel, Column, Entity, Model};

standard_behavior!(ActiveModel);

impl Model {
    #[must_use]
    pub fn status(&self) -> PermissionStatus {
        PermissionStatus::parse(&self.status).unwrap_or(PermissionStatus::Pending)
    }
}
