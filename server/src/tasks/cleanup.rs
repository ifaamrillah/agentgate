//! `bin/loco task cleanup [confirm:yes]`: pembersihan harian (`services::cleanup`) sekarang.
//! Dry run secara default (hanya menghitung); hanya `confirm:yes` yang menghapus (rules §8).
//! Scheduler menjalankannya setiap hari dengan `confirm:yes` (`config/*.yaml`).
use async_trait::async_trait;
use loco_rs::{
    Error, Result,
    app::AppContext,
    task::{Task, TaskInfo, Vars},
};

use super::{banner, fail, ok, skip, summary};
use crate::services::cleanup::{self, CleanupMode};

pub struct Cleanup;

#[async_trait]
impl Task for Cleanup {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "cleanup".to_string(),
            detail: "Bersihkan sesi, rate limit, token, undangan lama, dan workspace yang masa \
                     tenggang hapusnya lewat. Dry run kecuali confirm:yes"
                .to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, vars: &Vars) -> Result<()> {
        let confirmed = vars.cli_arg("confirm").is_ok_and(|v| v.trim() == "yes");
        let mode = if confirmed {
            CleanupMode::Apply
        } else {
            CleanupMode::DryRun
        };
        banner(if confirmed {
            "cleanup"
        } else {
            "cleanup (dry run)"
        });
        let report = cleanup::run(ctx, mode).await;
        let (mut done, mut failed, mut skipped) = (0, 0, 0);
        for step in &report.steps {
            let name = step.step.name();
            if step.failed > 0 {
                fail(&format!(
                    "{name}: {} dibersihkan, {} gagal (lihat log)",
                    step.count, step.failed
                ));
                failed += 1;
            } else if confirmed {
                ok(&format!("{name}: {} dibersihkan", step.count));
                done += 1;
            } else {
                skip(&format!("{name}: {} akan dibersihkan", step.count));
                skipped += 1;
            }
        }
        summary(done, failed, skipped);
        if !confirmed {
            println!("Jalankan ulang untuk menghapus: bin/loco task cleanup confirm:yes");
        }
        if report.failed() > 0 {
            // Exit code bukan nol: kegagalan terlihat di log scheduler, bukan hanya di baris FAIL.
            return Err(Error::string("cleanup: ada langkah yang gagal"));
        }
        Ok(())
    }
}
