//! Route autentikasi (rules §7). Adapter tipis: logika ada di `services::auth`.
use loco_rs::prelude::Routes;

#[cfg(debug_assertions)]
pub mod dev_login;
pub mod login;
pub mod logout;
pub mod password;
pub mod register;
pub mod two_factor;
pub mod verify_email;

/// "Cek email kamu": tujuan `303` setelah daftar, lupa password, dan kirim ulang verifikasi.
pub const CHECK_EMAIL: &str = "/check-email";

pub fn routes() -> Vec<Routes> {
    vec![
        login::routes().add("/logout", loco_rs::prelude::post(logout::submit)),
        register::routes(),
        verify_email::routes(),
        password::routes(),
        two_factor::routes(),
    ]
}
