//! Extractor aplikasi (rules §6): langkah yang berulang di banyak handler ditulis sekali di sini.
pub mod client_ip;
pub mod current_user;
pub mod valid_form;

pub use client_ip::ClientIp;
pub use current_user::{CurrentUser, MaybeCurrentUser, sign_in_redirect};
pub use valid_form::ValidForm;
