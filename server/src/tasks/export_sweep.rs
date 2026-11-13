//! `bin/loco task export_sweep`: sapuan kelas "wajib terjadi" untuk ekspor data dan impor CSV
//! (M11). Baris `queued`/`running` yang macet (enqueue gagal, worker mati) di-enqueue ulang;
//! ekspor yang terlalu sering gagal ditandai `failed`. Scheduler menjalankannya tiap 10 menit.
use async_trait::async_trait;
use loco_rs::{
    Result,
    app::AppContext,
    task::{Task, TaskInfo, Vars},
};

use super::{banner, ok, summary};
use crate::services::{exports, imports};

pub struct ExportSweep;

#[async_trait]
impl Task for ExportSweep {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "export_sweep".to_string(),
            detail: "Antrekan ulang ekspor data dan impor CSV yang macet".to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, _vars: &Vars) -> Result<()> {
        banner("export_sweep");
        let now = chrono::Utc::now();
        let exports = exports::sweep(ctx, now).await?;
        ok(&format!(
            "ekspor: {exports} diantrekan ulang atau ditandai gagal"
        ));
        let imports = imports::sweep(ctx, now).await?;
        ok(&format!("impor: {imports} diantrekan ulang"));
        summary(2, 0, 0);
        Ok(())
    }
}
