//! Autentikasi milik aplikasi (rules §7): password, login, rate limit, dan aturan sesi
//! tambahan. Sesi itu sendiri (cookie, store, rotasi id, revoke) memakai `loco_rs::auth`.
pub mod password;
pub mod sign_in;
pub mod throttle;
