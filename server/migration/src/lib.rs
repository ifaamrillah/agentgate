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
mod m20261115_000001_create_mcp_personas;
mod m20261116_000001_create_runners_jobs;
mod m20261117_000001_create_git_prs;
mod m20261118_000001_create_code_graph;
mod m20261119_000001_create_knowledge;
mod m20261120_000001_create_planning;
mod m20261121_000001_create_visual;
mod m20261122_000001_create_api_webhooks;
mod m20261123_000001_create_exports_imports;
mod m20261124_000001_create_jira;
mod m20261125_000001_realtime_notify;

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
            Box::new(m20261115_000001_create_mcp_personas::Migration),
            Box::new(m20261116_000001_create_runners_jobs::Migration),
            Box::new(m20261117_000001_create_git_prs::Migration),
            Box::new(m20261118_000001_create_code_graph::Migration),
            Box::new(m20261119_000001_create_knowledge::Migration),
            Box::new(m20261120_000001_create_planning::Migration),
            Box::new(m20261121_000001_create_visual::Migration),
            Box::new(m20261122_000001_create_api_webhooks::Migration),
            Box::new(m20261123_000001_create_exports_imports::Migration),
            Box::new(m20261124_000001_create_jira::Migration),
            Box::new(m20261125_000001_realtime_notify::Migration),
            // inject-above (do not remove this comment)
        ]
    }
}
