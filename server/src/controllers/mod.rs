//! Adapter HTTP tipis. Semua route didaftarkan di `routes`.
use loco_rs::{app::AppContext, controller::AppRoutes, environment::Environment};

pub mod account;
pub mod agents;
pub mod api_routes;
pub mod auth;
pub mod connect;
#[cfg(debug_assertions)]
pub mod dev;
pub mod device;
pub mod extractors;
pub mod flash;
pub mod git;
pub mod graph;
pub mod graph_api;
pub mod invitations;
pub mod knowledge;
pub mod knowledge_api;
pub mod knowledge_settings;
pub mod mcp;
pub mod middleware;
pub mod oauth;
pub mod projects;
pub mod public_routes;
pub mod render;
pub mod runner;
pub mod runner_api;
pub mod shell;
pub mod tickets;
pub mod tokens;
pub mod webhooks;
pub mod workspaces;

pub fn routes(ctx: &AppContext) -> AppRoutes {
    let routes = AppRoutes::with_default_routes()
        .add_routes(auth::routes())
        .add_route(account::routes())
        .add_route(invitations::routes())
        .add_route(workspaces::routes())
        .add_route(projects::routes())
        .add_route(tickets::routes())
        .add_route(agents::routes())
        .add_route(oauth::routes())
        .add_route(connect::routes())
        .add_route(device::routes())
        .add_route(runner::routes())
        .add_route(runner_api::routes())
        .add_route(graph_api::routes())
        .add_route(knowledge_api::routes())
        .add_route(git::routes())
        .add_route(graph::routes())
        .add_route(knowledge::routes())
        .add_route(knowledge_settings::routes())
        .add_route(tokens::routes())
        .add_route(webhooks::routes())
        .add_route(mcp::routes(ctx));
    add_dev_routes(routes, &ctx.environment)
}

#[cfg(debug_assertions)]
fn add_dev_routes(routes: AppRoutes, env: &Environment) -> AppRoutes {
    match env {
        Environment::Development => routes
            .add_route(dev::routes())
            .add_route(auth::dev_login::routes()),
        Environment::Test => routes.add_route(dev::routes()),
        _ => routes,
    }
}

#[cfg(not(debug_assertions))]
fn add_dev_routes(routes: AppRoutes, _env: &Environment) -> AppRoutes {
    routes
}
