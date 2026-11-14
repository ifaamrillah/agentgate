//! Persona agent per workspace (spec "Persona agent, skill pack & model"): peran AI di stage
//! workflow, dengan instruksi, tool MCP dan tools client yang diizinkan, izin aksi berbahaya,
//! gaya ringkas, dan model per client.
//!
//! - **Kunci** (`key`) stabil seumur workspace: definisi workflow menyimpan kunci persona di
//!   stage. Kunci unik per workspace termasuk persona yang sudah dihapus, supaya kunci lama di
//!   versi workflow tidak pernah menunjuk persona lain.
//! - **Hapus** = `deleted_at` + `replaced_by`. Stage yang menyebut kunci persona terhapus
//!   memakai penggantinya ([`Model::resolve`], [`Model::labels`]); versi workflow lama tidak
//!   perlu ditulis ulang.
//! - 9 persona bawaan (`agentgate_core::persona::BUILTINS`) dibuat bersama workspace
//!   ([`ActiveModel::seed_builtins`]); setelah itu sepenuhnya milik workspace.

use std::collections::{BTreeMap, HashMap};

use agentgate_core::persona::{
    self as core_persona, ALWAYS_BLOCKED, BUILTINS, CLIENT_TOOLS, DANGER_ACTIONS, Level, MCP_TOOLS,
    ModelChoice,
};
use loco_rs::model::{TenantEntity, TenantQueryExt};
use sea_orm::{ActiveValue::Set, QueryOrder, entity::prelude::*};
use serde::Serialize;
use validator::{ValidationError, ValidationErrors};

pub use super::_entities::personas::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;
use crate::common::text;

/// Label persona untuk tampilan stage: nama, ikon, dan warna persona workspace. Kunci persona
/// yang tidak dikenal workspace (seharusnya tidak terjadi) tampil sebagai kuncinya.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PersonaLabel {
    pub key: String,
    pub name: String,
    pub icon: String,
    pub color: String,
}

/// Label persona per kunci (`models::personas::Model::labels`).
pub type PersonaLabels = std::collections::HashMap<String, PersonaLabel>;

pub const SUMMARY_MAX_LEN: usize = 120;
pub const INSTRUCTIONS_MAX_LEN: usize = 20_000;
pub const MODEL_MAX_LEN: usize = 80;
/// Batas rantai pengganti yang diikuti (persona dihapus berkali-kali).
const REPLACEMENT_DEPTH: usize = 8;

pub const NAME_TAKEN: &str = "persona-name-taken";
pub const SUMMARY_TOO_LONG: &str = "persona-summary-too-long";
pub const COLOR_INVALID: &str = "persona-color-invalid";
pub const ICON_INVALID: &str = "persona-icon-invalid";
pub const INSTRUCTIONS_TOO_LONG: &str = "persona-instructions-too-long";
pub const TOOL_UNKNOWN: &str = "persona-tool-unknown";
pub const LEVEL_INVALID: &str = "persona-level-invalid";
pub const MODEL_INVALID: &str = "persona-model-invalid";
pub const REPLACEMENT_INVALID: &str = "persona-replacement-invalid";
pub const ERROR_CODES: &[&str] = &[
    NAME_TAKEN,
    SUMMARY_TOO_LONG,
    COLOR_INVALID,
    ICON_INVALID,
    INSTRUCTIONS_TOO_LONG,
    TOOL_UNKNOWN,
    LEVEL_INVALID,
    MODEL_INVALID,
    REPLACEMENT_INVALID,
];

/// Ikon yang bisa dipilih untuk persona (ikon persona bawaan + umum).
pub const ICONS: &[&str] = &[
    "robot",
    "bulb",
    "file-text",
    "layout-sidebar",
    "hierarchy-2",
    "code",
    "git-compare",
    "test-pipe",
    "rocket",
    "writing",
    "shield-lock",
    "chart-bar",
    "sparkles",
    "user",
];

impl TenantEntity for Entity {
    type TenantId = WorkspaceId;

    fn tenant_column() -> Column {
        Column::WorkspaceId
    }
}

standard_behavior!(ActiveModel);

/// Isian identitas dan izin persona yang sudah dinormalisasi (dipakai buat dan ubah).
#[derive(Debug, Clone, Default)]
pub struct Identity {
    pub name: String,
    pub summary: String,
    pub color: String,
    pub icon: String,
}

impl Model {
    #[must_use]
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }

    /// Persona aktif workspace: bawaan dulu (urutan spec), lalu custom urut dibuat.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn active_in<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
    ) -> Result<Vec<Self>, DbErr> {
        let mut rows = Entity::find()
            .in_tenant(wid)
            .filter(Column::DeletedAt.is_null())
            .order_by_asc(Column::CreatedAt)
            .order_by_asc(Column::Id)
            .all(db)
            .await?;
        rows.sort_by_key(|p| {
            (
                !p.builtin,
                BUILTINS
                    .iter()
                    .position(|b| b.key == p.key)
                    .unwrap_or(usize::MAX),
            )
        });
        Ok(rows)
    }

    /// Semua persona workspace termasuk yang dihapus.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn all_in<C: ConnectionTrait>(db: &C, wid: WorkspaceId) -> Result<Vec<Self>, DbErr> {
        Entity::find().in_tenant(wid).all(db).await
    }

    /// Persona aktif dengan kunci `key`.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn find_active<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        key: &str,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find()
            .in_tenant(wid)
            .filter(Column::Key.eq(key))
            .filter(Column::DeletedAt.is_null())
            .one(db)
            .await
    }

    /// Persona aktif `id` di workspace.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn find_active_by_id<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        id: Uuid,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(id)
            .in_tenant(wid)
            .filter(Column::DeletedAt.is_null())
            .one(db)
            .await
    }

    /// Persona untuk kunci `key` di stage: persona itu sendiri, atau pengganti terakhirnya bila
    /// sudah dihapus. `None` bila kunci tidak pernah ada di workspace.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn resolve<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        key: &str,
    ) -> Result<Option<Self>, DbErr> {
        let all = Self::all_in(db, wid).await?;
        Ok(resolve_in(&all, key).cloned())
    }

    /// Label semua kunci persona workspace (kunci persona terhapus → label penggantinya).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn labels<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
    ) -> Result<PersonaLabels, DbErr> {
        let all = Self::all_in(db, wid).await?;
        Ok(all
            .iter()
            .filter_map(|p| resolve_in(&all, &p.key).map(|r| (p.key.clone(), r.label())))
            .collect::<HashMap<_, _>>())
    }

    /// Label persona aktif (pilihan persona di builder).
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn active_labels<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
    ) -> Result<Vec<PersonaLabel>, DbErr> {
        Ok(Self::active_in(db, wid)
            .await?
            .iter()
            .map(Self::label)
            .collect())
    }

    /// Semua kunci persona workspace (aktif maupun terhapus): kunci yang boleh dipakai stage.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn known_keys<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
    ) -> Result<Vec<String>, DbErr> {
        Ok(Self::all_in(db, wid)
            .await?
            .into_iter()
            .map(|p| p.key)
            .collect())
    }

    #[must_use]
    pub fn label(&self) -> PersonaLabel {
        PersonaLabel {
            key: self.key.clone(),
            name: self.name.clone(),
            icon: self.icon.clone(),
            color: self.color.clone(),
        }
    }

    /// Tingkat izin per aksi berbahaya (aksi yang tidak tersimpan memakai bawaan).
    #[must_use]
    pub fn danger_levels(&self) -> Vec<(&'static str, Level)> {
        DANGER_ACTIONS
            .iter()
            .map(|(action, default)| {
                let level = self
                    .danger
                    .get(*action)
                    .and_then(serde_json::Value::as_str)
                    .and_then(Level::parse)
                    .unwrap_or(*default);
                (*action, level)
            })
            .collect()
    }

    /// Model per client (kunci client katalog → pilihan). Client tanpa pilihan tidak ada.
    #[must_use]
    pub fn model_choices(&self) -> BTreeMap<String, ModelChoice> {
        serde_json::from_value(self.models.clone()).unwrap_or_default()
    }

    /// Model rekomendasi untuk client `client_key`, bila diisi.
    #[must_use]
    pub fn model_for(&self, client_key: &str) -> Option<ModelChoice> {
        self.model_choices()
            .remove(client_key)
            .filter(|c| !c.model.is_empty() || !c.effort.is_empty())
    }
}

/// Persona untuk `key`, mengikuti rantai pengganti.
fn resolve_in<'a>(all: &'a [Model], key: &str) -> Option<&'a Model> {
    let mut current = all.iter().find(|p| p.key == key)?;
    for _ in 0..REPLACEMENT_DEPTH {
        if current.deleted_at.is_none() {
            return Some(current);
        }
        let next = current.replaced_by?;
        current = all.iter().find(|p| p.id == next)?;
    }
    current.deleted_at.is_none().then_some(current)
}

impl ActiveModel {
    /// Buat 9 persona bawaan di workspace baru.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn seed_builtins<C: ConnectionTrait>(db: &C, wid: WorkspaceId) -> Result<(), DbErr> {
        for b in &BUILTINS {
            let danger: serde_json::Map<String, serde_json::Value> = b
                .danger_levels()
                .into_iter()
                .map(|(a, l)| (a.to_string(), l.as_str().into()))
                .collect();
            Self {
                workspace_id: Set(wid.get()),
                key: Set(b.key.to_string()),
                name: Set(b.name.to_string()),
                summary: Set(b.summary.to_string()),
                color: Set(b.color.to_string()),
                icon: Set(b.icon.to_string()),
                builtin: Set(true),
                instructions: Set(b.instructions.to_string()),
                concise: Set(false),
                mcp_tools: Set(b.mcp_tools.iter().map(|t| (*t).to_string()).collect()),
                client_tools: Set(b.client_tools.iter().map(|t| (*t).to_string()).collect()),
                danger: Set(serde_json::Value::Object(danger)),
                models: Set(serde_json::json!({})),
                ..Default::default()
            }
            .insert(db)
            .await?;
        }
        Ok(())
    }
}

fn err(errors: &mut ValidationErrors, field: &'static str, code: &'static str) {
    errors.add(field, ValidationError::new(code));
}

/// Validasi identitas (nama, tugas, warna, ikon). Nama dinormalisasi.
///
/// # Errors
///
/// Error per field.
pub fn validate_identity(input: &Identity) -> Result<Identity, ValidationErrors> {
    let mut errors = ValidationErrors::new();
    let name = text::normalize_name(&input.name);
    if let Err(e) = text::validate_name("persona", &name) {
        errors.add("name", e);
    }
    let summary = input.summary.trim().to_string();
    if summary.chars().count() > SUMMARY_MAX_LEN || summary.chars().any(char::is_control) {
        err(&mut errors, "summary", SUMMARY_TOO_LONG);
    }
    let color = input.color.trim().to_lowercase();
    if !text::is_hex_color(&color) {
        err(&mut errors, "color", COLOR_INVALID);
    }
    let icon = input.icon.trim().to_string();
    if !ICONS.contains(&icon.as_str()) {
        err(&mut errors, "icon", ICON_INVALID);
    }
    if errors.is_empty() {
        Ok(Identity {
            name,
            summary,
            color,
            icon,
        })
    } else {
        Err(errors)
    }
}

/// Instruksi persona: CRLF → LF, batas panjang.
///
/// # Errors
///
/// `persona-instructions-too-long` di field `instructions`.
pub fn validate_instructions(value: &str) -> Result<String, ValidationErrors> {
    let value = value.replace("\r\n", "\n").trim_end().to_string();
    if value.chars().count() > INSTRUCTIONS_MAX_LEN {
        let mut errors = ValidationErrors::new();
        err(&mut errors, "instructions", INSTRUCTIONS_TOO_LONG);
        return Err(errors);
    }
    Ok(value)
}

/// Tool MCP dan tools client yang diizinkan, serta tingkat aksi berbahaya. Urutan mengikuti
/// katalog; duplikat dibuang.
///
/// # Errors
///
/// `persona-tool-unknown` (`mcp_tools`/`client_tools`) atau `persona-level-invalid` (`danger`).
pub fn validate_tools(
    mcp: &[String],
    client: &[String],
    danger: &BTreeMap<String, String>,
) -> Result<(Vec<String>, Vec<String>, serde_json::Value), ValidationErrors> {
    let mut errors = ValidationErrors::new();
    let pick = |items: &[String], catalog: &[&str]| -> Option<Vec<String>> {
        items
            .iter()
            .all(|t| catalog.contains(&t.as_str()))
            .then(|| {
                catalog
                    .iter()
                    .filter(|c| items.iter().any(|t| t == *c))
                    .map(|c| (*c).to_string())
                    .collect()
            })
    };
    let mcp = pick(mcp, MCP_TOOLS).unwrap_or_else(|| {
        err(&mut errors, "mcp_tools", TOOL_UNKNOWN);
        Vec::new()
    });
    let client = pick(client, CLIENT_TOOLS).unwrap_or_else(|| {
        err(&mut errors, "client_tools", TOOL_UNKNOWN);
        Vec::new()
    });
    let mut levels = serde_json::Map::new();
    for (action, default) in DANGER_ACTIONS {
        let level = match danger.get(*action) {
            None => *default,
            Some(v) => Level::parse(v).unwrap_or_else(|| {
                err(&mut errors, "danger", LEVEL_INVALID);
                *default
            }),
        };
        levels.insert((*action).to_string(), level.as_str().into());
    }
    if danger.keys().any(|k| {
        !DANGER_ACTIONS.iter().any(|(a, _)| a == k) || ALWAYS_BLOCKED.contains(&k.as_str())
    }) {
        err(&mut errors, "danger", LEVEL_INVALID);
    }
    if errors.is_empty() {
        Ok((mcp, client, serde_json::Value::Object(levels)))
    } else {
        Err(errors)
    }
}

/// Model + effort per client katalog. Model boleh di luar katalog (diketik bebas), effort
/// harus dikenal client itu.
///
/// # Errors
///
/// `persona-model-invalid` di field `models`.
pub fn validate_models(
    input: &BTreeMap<String, ModelChoice>,
) -> Result<serde_json::Value, ValidationErrors> {
    let mut out = BTreeMap::new();
    let mut ok = true;
    for (client_key, choice) in input {
        let Some(client) = core_persona::client(client_key) else {
            ok = false;
            continue;
        };
        let model = choice.model.trim().to_string();
        let effort = choice.effort.trim().to_string();
        if model.chars().count() > MODEL_MAX_LEN
            || model.chars().any(|c| c.is_whitespace() || c.is_control())
            || (!effort.is_empty() && !client.efforts.contains(&effort.as_str()))
        {
            ok = false;
            continue;
        }
        if !model.is_empty() || !effort.is_empty() {
            out.insert(client_key.clone(), ModelChoice { model, effort });
        }
    }
    if ok {
        Ok(serde_json::to_value(out).unwrap_or_else(|_| serde_json::json!({})))
    } else {
        let mut errors = ValidationErrors::new();
        err(&mut errors, "models", MODEL_INVALID);
        Err(errors)
    }
}

/// Kunci baru dari nama (slug) yang belum dipakai di `taken`.
#[must_use]
pub fn fresh_key(name: &str, taken: &[String]) -> String {
    let base = agentgate_core::workflow::definition::slug(name).replace('_', "-");
    // `slug` membuang angka di depan; nama tanpa huruf → "persona".
    let base = if base.is_empty() {
        "persona".to_string()
    } else {
        base
    };
    let base: String = base.chars().take(32).collect();
    if !taken.contains(&base) {
        return base;
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|k| !taken.contains(k))
        .unwrap_or(base)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_key_is_unique_slug() {
        let taken = vec!["security-auditor".to_string()];
        assert_eq!(fresh_key("Security Auditor", &taken), "security-auditor-2");
        assert_eq!(fresh_key("Analis Data", &taken), "analis-data");
        assert_eq!(fresh_key("123", &taken), "persona");
        assert_eq!(fresh_key("2 QA", &taken), "qa");
    }

    #[test]
    fn tools_keep_catalog_order_and_reject_unknown() {
        let (mcp, client, danger) = validate_tools(
            &["ag_comment".into(), "ag_context".into()],
            &["read".into()],
            &BTreeMap::from([("deploy".into(), "ask".into())]),
        )
        .unwrap_or_else(|e| panic!("{e:?}"));
        assert_eq!(mcp, vec!["ag_context", "ag_comment"]);
        assert_eq!(client, vec!["read"]);
        assert_eq!(danger["deploy"], "ask");
        assert_eq!(danger["db_migrate"], "ask");
        assert!(validate_tools(&["rm_rf".into()], &[], &BTreeMap::new()).is_err());
        assert!(
            validate_tools(
                &[],
                &[],
                &BTreeMap::from([("force_push_main".into(), "allow".into())])
            )
            .is_err()
        );
    }

    #[test]
    fn models_accept_free_text_but_known_efforts() {
        let ok = BTreeMap::from([(
            "claude".to_string(),
            ModelChoice {
                model: "claude-next".into(),
                effort: "high".into(),
            },
        )]);
        assert!(validate_models(&ok).is_ok());
        let bad = BTreeMap::from([(
            "cursor".to_string(),
            ModelChoice {
                model: String::new(),
                effort: "high".into(),
            },
        )]);
        assert!(validate_models(&bad).is_err());
    }
}
