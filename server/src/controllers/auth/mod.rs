//! Route autentikasi (rules §7). Adapter tipis: logika ada di `services::auth`.
use loco_rs::prelude::Routes;

pub mod login;
pub mod logout;

pub fn routes() -> Routes {
    login::routes().add("/logout", loco_rs::prelude::post(logout::submit))
}
