#![allow(elided_lifetimes_in_paths)]
#![allow(clippy::wildcard_imports)]
pub use sea_orm_migration::prelude::*;
mod m20261111_000001_enable_extensions;
mod m20261112_000001_create_users_and_auth;
mod m20261112_000002_create_workspaces;
mod m20261112_000003_create_teams_projects;
mod m20261112_000004_ui_preferences_and_project_logo;
mod m20261113_000001_create_tickets;
pub mod m20261114_000001_create_workflows;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20261111_000001_enable_extensions::Migration),
            Box::new(m20261112_000001_create_users_and_auth::Migration),
            Box::new(m20261112_000002_create_workspaces::Migration),
            Box::new(m20261112_000003_create_teams_projects::Migration),
            Box::new(m20261112_000004_ui_preferences_and_project_logo::Migration),
            Box::new(m20261113_000001_create_tickets::Migration),
            Box::new(m20261114_000001_create_workflows::Migration),
            // inject-above (do not remove this comment)
        ]
    }
}
