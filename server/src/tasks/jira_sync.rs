//! `bin/loco task jira_sync`: satu putaran sinkron untuk setiap koneksi Jira mirror/dua arah
//! yang aktif (`services::jira::sync`). Scheduler menjalankannya tiap 5 menit: dorong outbox
//! yang tertinggal dan tarik perubahan Jira (mode dua arah).
use async_trait::async_trait;
use loco_rs::{
    Result,
    app::AppContext,
    task::{Task, TaskInfo, Vars},
};

use super::{banner, fail, ok, skip, summary};
use crate::{
    common::settings::Settings,
    models::jira_connections,
    services::jira::sync::{self, Outcome, RunKind},
};

pub struct JiraSync;

#[async_trait]
impl Task for JiraSync {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "jira_sync".to_string(),
            detail: "Sinkron Jira berkala (dorong outbox, tarik perubahan dua arah)".to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, _vars: &Vars) -> Result<()> {
        banner("jira_sync");
        let settings = Settings::from_ctx(ctx)?;
        let (mut done, mut failed, mut skipped) = (0, 0, 0);
        for conn in jira_connections::Model::due_for_sync(&ctx.db).await? {
            let label = format!("{} ({})", conn.jira_project_key, conn.id);
            match sync::run(&ctx.db, &ctx.storage, &settings, conn.id, RunKind::Cycle).await? {
                Outcome::Done => {
                    ok(&label);
                    done += 1;
                }
                Outcome::Failed => {
                    fail(&label);
                    failed += 1;
                }
                Outcome::Skipped | Outcome::Busy => {
                    skip(&label);
                    skipped += 1;
                }
            }
        }
        summary(done, failed, skipped);
        Ok(())
    }
}
