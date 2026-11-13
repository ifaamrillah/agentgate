//! `bin/loco task webhooks`: sapu delivery webhook yang tertahan atau jatuh tempo retry
//! (`services::webhooks::dispatch::sweep`). Jalur utama tetap worker `WebhookDeliveryWorker`
//! yang di-enqueue setelah commit; scheduler menjalankan task ini tiap menit sebagai jaring
//! pengaman (worker mati, enqueue gagal, perubahan dari luar request HTTP).
use async_trait::async_trait;
use loco_rs::{
    Result,
    app::AppContext,
    task::{Task, TaskInfo, Vars},
};

use super::{banner, ok, summary};
use crate::{common::settings::Settings, services::webhooks::dispatch};

/// Maksimal delivery per sapuan; sisanya diambil sapuan berikutnya.
const SWEEP_LIMIT: u64 = 200;

pub struct Webhooks;

#[async_trait]
impl Task for Webhooks {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "webhooks".to_string(),
            detail: "Kirim delivery webhook yang tertahan atau jatuh tempo retry".to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, _vars: &Vars) -> Result<()> {
        banner("webhooks");
        let settings = Settings::from_ctx(ctx)?;
        let sent = dispatch::sweep(&ctx.db, &settings, chrono::Utc::now(), SWEEP_LIMIT).await?;
        ok(&format!("{sent} delivery dikirim"));
        summary(1, 0, 0);
        Ok(())
    }
}
