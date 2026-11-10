//! Adapter HTTP tipis. Semua route didaftarkan di `routes`.
use loco_rs::{app::AppContext, controller::AppRoutes};

pub mod home;

pub fn routes(_ctx: &AppContext) -> AppRoutes {
    AppRoutes::with_default_routes().add_route(home::routes())
}
