#![allow(elided_lifetimes_in_paths)]
#![allow(clippy::wildcard_imports)]
pub use sea_orm_migration::prelude::*;
mod m20261111_000001_enable_extensions;
mod m20261112_000001_create_users_and_auth;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20261111_000001_enable_extensions::Migration),
            Box::new(m20261112_000001_create_users_and_auth::Migration),
            // inject-above (do not remove this comment)
        ]
    }
}
