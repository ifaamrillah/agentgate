//! Autentikasi milik aplikasi (rules §7): password, login, pendaftaran, verifikasi email,
//! reset dan ganti password, rate limit, dan aturan sesi tambahan. Sesi itu sendiri (cookie,
//! store, rotasi id, revoke) memakai `loco_rs::auth`.
#[cfg(debug_assertions)]
pub mod dev_login;
pub mod password;
pub mod password_change;
pub mod password_reset;
pub mod registration;
pub mod sign_in;
pub mod throttle;
pub mod totp;
pub mod verification;
