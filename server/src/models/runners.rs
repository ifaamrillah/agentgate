//! Runner: perangkat milik user di satu workspace yang menjalankan job. Online bila mengirim
//! heartbeat dalam [`ONLINE_SECONDS`] terakhir dan belum dicabut.
use chrono::{Duration, Utc};

pub use super::_entities::runners::{ActiveModel, Column, Entity, Model};

standard_behavior!(ActiveModel);

/// Batas umur heartbeat runner online.
pub const ONLINE_SECONDS: i64 = 60;

impl Model {
    #[must_use]
    pub fn is_online(&self) -> bool {
        self.revoked_at.is_none()
            && self.last_seen_at.is_some_and(|t| {
                Utc::now().signed_duration_since(t) < Duration::seconds(ONLINE_SECONDS)
            })
    }
}
