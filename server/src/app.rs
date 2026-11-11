use std::path::Path;

use async_trait::async_trait;
use axum::Router as AxumRouter;
use loco_rs::{
    Result,
    app::{AppContext, Hooks, Initializer},
    bgworker::Queue,
    boot::{BootResult, StartMode, create_app},
    config::Config,
    controller::AppRoutes,
    environment::Environment,
    task::Tasks,
};
use migration::Migrator;
use sea_orm::{ConnectionTrait, DbBackend, Statement, Value};

use crate::{common::settings::Settings, controllers, initializers};

pub struct App;

/// Semua tabel yang dikosongkan hook `truncate`; tabel baru didaftarkan di sini.
const TRUNCATE_TABLES: &[&str] = &[
    "users",
    "user_tokens",
    "login_throttles",
    "identities",
    "recovery_codes",
    "loco_sessions",
    "loco_remember_tokens",
];

#[async_trait]
impl Hooks for App {
    fn app_name() -> &'static str {
        "agentgate"
    }

    fn app_version() -> String {
        format!(
            "{} ({})",
            agentgate_core::VERSION,
            option_env!("BUILD_SHA")
                .or(option_env!("GITHUB_SHA"))
                .unwrap_or("dev")
        )
    }

    async fn boot(
        mode: StartMode,
        environment: &Environment,
        config: Config,
    ) -> Result<BootResult> {
        // Settings divalidasi sebelum apa pun jalan, supaya config rusak gagal saat boot.
        Settings::from_json(config.settings.as_ref())?;
        create_app::<Self, Migrator>(mode, environment, config).await
    }

    async fn initializers(_ctx: &AppContext) -> Result<Vec<Box<dyn Initializer>>> {
        Ok(vec![Box::new(
            initializers::view_engine::ViewEngineInitializer,
        )])
    }

    fn routes(ctx: &AppContext) -> AppRoutes {
        controllers::routes(ctx)
    }

    async fn before_routes(_ctx: &AppContext) -> Result<AxumRouter<AppContext>> {
        // Fallback dipasang di sini (sebelum middleware Loco diterapkan) supaya respons 404
        // ikut membawa CSP, request id, dan header keamanan lain.
        Ok(AxumRouter::new().fallback(controllers::middleware::html_errors::not_found))
    }

    async fn after_routes(router: AxumRouter, _ctx: &AppContext) -> Result<AxumRouter> {
        // CSRF membungkus semua route; penolakannya dirender sebagai halaman HTML oleh
        // layer html_errors yang dipasang view engine di luar layer ini.
        Ok(router.layer(axum::middleware::from_fn(
            controllers::middleware::csrf::layer,
        )))
    }

    async fn connect_workers(_ctx: &AppContext, _queue: &Queue) -> Result<()> {
        Ok(())
    }

    fn register_tasks(_tasks: &mut Tasks) {}

    /// Kosongkan tabel sebelum tes (`dangerously_truncate`). Satu `TRUNCATE` untuk semua tabel
    /// supaya urutan FK tidak penting. Tabel sesi Loco dibuat malas saat sesi pertama dipakai,
    /// jadi hanya tabel yang sudah ada yang ikut (`to_regclass`).
    async fn truncate(ctx: &AppContext) -> Result<()> {
        let existing = ctx
            .db
            .query_all_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT t FROM unnest($1::text[]) AS t WHERE to_regclass(t) IS NOT NULL",
                [Value::from(
                    TRUNCATE_TABLES
                        .iter()
                        .map(|t| (*t).to_string())
                        .collect::<Vec<_>>(),
                )],
            ))
            .await?
            .iter()
            .map(|row| row.try_get_by_index::<String>(0))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if !existing.is_empty() {
            // Nama tabel berasal dari konstanta TRUNCATE_TABLES, bukan input user.
            ctx.db
                .execute_unprepared(&format!("TRUNCATE {} CASCADE", existing.join(", ")))
                .await?;
        }
        Ok(())
    }

    async fn seed(_ctx: &AppContext, _base: &Path) -> Result<()> {
        Ok(())
    }
}
