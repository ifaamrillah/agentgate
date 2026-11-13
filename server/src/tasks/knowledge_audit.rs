//! `bin/loco task knowledge_audit [force:yes]`: audit knowledge mingguan
//! (`services::knowledge::triggers::weekly_audit`). Scheduler menjalankannya setiap hari; hanya
//! workspace yang hari auditnya = hari ini (dan belum diaudit 6 hari terakhir) yang mendapat run.
//! `force:yes` mengabaikan hari dan jeda.
use async_trait::async_trait;
use loco_rs::{
    Result,
    app::AppContext,
    task::{Task, TaskInfo, Vars},
};

use super::{banner, ok, summary};
use crate::services::knowledge::triggers;

pub struct KnowledgeAudit;

#[async_trait]
impl Task for KnowledgeAudit {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "knowledge_audit".to_string(),
            detail: "Minta run audit knowledge untuk workspace yang hari auditnya hari ini. \
                     force:yes mengabaikan jadwal"
                .to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, vars: &Vars) -> Result<()> {
        let force = vars.cli_arg("force").is_ok_and(|v| v.trim() == "yes");
        banner("knowledge_audit");
        let report = triggers::weekly_audit(&ctx.db, chrono::Utc::now(), force).await?;
        ok(&format!(
            "{} project diperiksa, {} run audit dibuat",
            report.projects, report.runs
        ));
        summary(1, 0, 0);
        Ok(())
    }
}
