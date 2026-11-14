//! Kode perangkat device code flow (RFC 8628) untuk `agentgate login`. Kode perangkat hanya
//! disimpan sebagai digest; `user_code` ditampilkan di terminal dan diketik user di `/device`.
pub use super::_entities::oauth_device_codes::{ActiveModel, Column, Entity, Model};

standard_behavior!(ActiveModel);

/// `WDJBMJHT` → `WDJB-MJHT` (tampilan kode di terminal dan halaman `/device`).
#[must_use]
pub fn format_user_code(code: &str) -> String {
    if code.len() == 8 {
        format!("{}-{}", &code[..4], &code[4..])
    } else {
        code.to_string()
    }
}
