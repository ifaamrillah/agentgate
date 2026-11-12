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
    pub icon: &'static str,
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
            icon: match (status, p.source()) {
                (SkillStatus::Rejected, _) => "shield-x",
                (_, SkillSource::Custom) => "pencil",
                (_, SkillSource::Repo) => "package",
            },
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

pub const FLASH_CREATED: &str = "skill-flash-created";
pub const FLASH_SAVED: &str = "skill-flash-saved";
pub const FLASH_SUBMITTED: &str = "skill-flash-submitted";
pub const FLASH_APPROVED: &str = "skill-flash-approved";
pub const FLASH_REJECTED: &str = "skill-flash-rejected";
pub const FLASH_DELETED: &str = "skill-flash-deleted";
pub const FLASH_IMPORTED: &str = "skill-flash-imported";

/// Satu berkas di pohon berkas skill pack.
#[derive(Debug, Clone, Serialize)]
pub struct FileView {
    pub path: String,
    pub kind: &'static str,
    pub href: String,
    pub current: bool,
    pub lines: usize,
    pub icon: &'static str,
}

/// Ikon berkas menurut jenisnya (`md` | `code`).
#[must_use]
pub fn file_icon(kind: &str) -> &'static str {
    if kind == "md" {
        "markdown"
    } else {
        "file-code"
    }
}

/// Satu pemeriksaan laporan pindai.
#[derive(Debug, Clone, Serialize)]
pub struct CheckView {
    pub key: String,
    pub label_key: String,
    pub level: String,
    pub level_key: String,
    pub variant: &'static str,
    pub icon: &'static str,
    pub findings: Vec<FindingView>,
    pub tools: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FindingView {
    pub file: String,
    pub line: usize,
    pub excerpt: String,
    pub reason_key: String,
    pub level: String,
}

/// Laporan pindai tersimpan (`skill_packs.report`) untuk tampilan.
#[must_use]
pub fn checks(report: &serde_json::Value) -> Vec<CheckView> {
    let list: Vec<skill_packs::ScanCheck> =
        serde_json::from_value(report.clone()).unwrap_or_default();
    list.into_iter()
        .map(|c| CheckView {
            label_key: format!("skill-check-{}", c.key),
            level_key: format!("skill-risk-{}", c.level),
            variant: match c.level.as_str() {
                "danger" => "destructive",
                "attention" => "gate",
                _ => "success",
            },
            icon: if c.level == "safe" {
                "check"
            } else {
                "alert-triangle"
            },
            key: c.key,
            level: c.level,
            findings: c
                .findings
                .into_iter()
                .map(|f| FindingView {
                    file: f.file,
                    line: f.line,
                    excerpt: f.excerpt,
                    reason_key: f.reason,
                    level: f.level,
                })
                .collect(),
            tools: c.tools,
        })
        .collect()
}

/// Kunci Fluent yang dibangun di kode untuk halaman skill pack (dites kontrak i18n).
#[must_use]
pub fn label_keys() -> Vec<String> {
    use sea_orm::{ActiveEnum, Iterable};
    let mut keys: Vec<String> = SkillStatus::iter()
        .map(|s| format!("skill-status-{}", s.to_value()))
        .chain(SkillRisk::iter().map(|r| format!("skill-risk-{}", r.to_value())))
        .collect();
    for c in ["commands", "injection", "secrets", "tools"] {
        keys.push(format!("skill-check-{c}"));
    }
    keys.extend(
        [
            "skill-scan-pipe-shell",
            "skill-scan-rm-root",
            "skill-scan-disk",
            "skill-scan-force-push",
            "skill-scan-upload",
            "skill-scan-obfuscated",
            "skill-scan-network",
            "skill-scan-sudo",
            "skill-scan-permissions",
            "skill-scan-force",
            "skill-scan-rm",
            "skill-scan-drop",
            "skill-scan-override",
            "skill-scan-permissions-bypass",
            "skill-scan-no-confirm",
            "skill-scan-hide",
            "skill-scan-env",
            "skill-scan-key",
            "skill-scan-credential",
            "skill-scan-token",
            FLASH_CREATED,
            FLASH_SAVED,
            FLASH_SUBMITTED,
            FLASH_APPROVED,
            FLASH_REJECTED,
            FLASH_DELETED,
            FLASH_IMPORTED,
        ]
        .map(str::to_string),
    );
    keys
}
