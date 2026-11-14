use agentgate_server::app::App;
use loco_rs::cli;
use migration::Migrator;

#[tokio::main]
async fn main() -> loco_rs::Result<()> {
    // Error tracking (R1): aktif hanya bila SENTRY_DSN diisi; guard hidup sepanjang proses
    // supaya event terkirim sebelum keluar.
    let _sentry = agentgate_server::services::error_tracking::init_from_env();
    cli::main::<App, Migrator>().await
}
