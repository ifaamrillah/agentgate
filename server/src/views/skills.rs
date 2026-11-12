//! Tampilan Pengaturan → Skill pack (`assets/views/skills/`).

use serde::Serialize;
use uuid::Uuid;

use crate::models::skill_packs::{self, SkillRisk, SkillSource, SkillStatus};

/// Satu skill pack di daftar (dan di bagian Skill persona).
#[derive(Debug, Clone, Serialize)]
pub struct SkillRow {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub version: String,
    pub source: &'static str,
    /// `owner/repo` untuk sumber repo; `""` untuk buatan sendiri.
    pub repo: String,
    pub status: &'static str,
    pub status_key: String,
    pub status_variant: &'static str,
    pub risk: &'static str,
    pub risk_key: String,
    pub risk_variant: &'static str,
    pub href: String,
}

#[must_use]
pub fn status_variant(s: SkillStatus) -> &'static str {
    match s {
        SkillStatus::Draft => "neutral",
        SkillStatus::Pending => "gate",
        SkillStatus::Approved => "success",
        SkillStatus::Rejected => "destructive",
    }
}

#[must_use]
pub fn risk_variant(r: SkillRisk) -> &'static str {
    match r {
        SkillRisk::Safe => "success",
        SkillRisk::Attention => "gate",
        SkillRisk::Danger => "destructive",
    }
}

impl SkillRow {
    #[must_use]
    pub fn of(p: &skill_packs::Model, base: &str) -> Self {
        use sea_orm::ActiveEnum;
        let status = p.status();
        let risk = p.risk();
        let repo = p
            .repo_url
            .as_deref()
            .map(|u| {
                u.trim_start_matches("https://github.com/")
                    .trim_end_matches(".git")
                    .to_string()
            })
            .unwrap_or_default();
        Self {
            id: p.id,
            name: p.name.clone(),
            description: p.description.clone(),
            version: p.version.clone(),
            source: match p.source() {
                SkillSource::Custom => "custom",
                SkillSource::Repo => "repo",
            },
            repo,
            status: status_str(status),
            status_key: format!("skill-status-{}", status.to_value()),
            status_variant: status_variant(status),
            risk: risk.map_or("", risk_str),
            risk_key: risk.map_or_else(String::new, |r| format!("skill-risk-{}", r.to_value())),
            risk_variant: risk.map_or("neutral", risk_variant),
            href: format!("{base}/{}", p.id),
        }
    }
}

fn status_str(s: SkillStatus) -> &'static str {
    match s {
        SkillStatus::Draft => "draft",
        SkillStatus::Pending => "pending",
        SkillStatus::Approved => "approved",
        SkillStatus::Rejected => "rejected",
    }
}

fn risk_str(r: SkillRisk) -> &'static str {
    match r {
        SkillRisk::Safe => "safe",
        SkillRisk::Attention => "attention",
        SkillRisk::Danger => "danger",
    }
}
