//! Adapter HTTP tipis. Semua route didaftarkan di `routes`.
use loco_rs::{app::AppContext, controller::AppRoutes, environment::Environment};

pub mod account;
pub mod auth;
#[cfg(debug_assertions)]
pub mod dev;
pub mod extractors;
pub mod flash;
pub mod home;
pub mod middleware;
pub mod public_routes;
pub mod render;

pub fn routes(ctx: &AppContext) -> AppRoutes {
    let routes = AppRoutes::with_default_routes()
        .add_routes(auth::routes())
        .add_route(account::routes())
        .add_route(home::routes());
    add_dev_routes(routes, &ctx.environment)
}

#[cfg(debug_assertions)]
fn add_dev_routes(routes: AppRoutes, env: &Environment) -> AppRoutes {
    match env {
        Environment::Development | Environment::Test => routes.add_route(dev::routes()),
        _ => routes,
    }
}

#[cfg(not(debug_assertions))]
fn add_dev_routes(routes: AppRoutes, _env: &Environment) -> AppRoutes {
    routes
}
