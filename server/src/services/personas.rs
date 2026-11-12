//! Kelola persona agent (Pengaturan → Persona agent, `ManageTicketSettings`): buat (kosong atau
//! clone), ubah per bagian, hapus dengan pengganti. Setiap perubahan mengunci workspace dan
//! membaca ulang role pelaku, menaikkan `revision` persona, dan mencatat audit `persona.*` di
//! transaksi yang sama. Ubah tanpa perubahan tidak menulis apa pun.

use std::collections::BTreeMap;

use agentgate_core::persona::ModelChoice;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, DatabaseTransaction,
    EntityTrait, IntoActiveModel, QueryFilter, QuerySelect, TransactionTrait,
};
use uuid::Uuid;

use crate::{
    models::{
        permission::Permission,
        persona_skills,
        personas::{self, Identity},
        skill_packs,
    },
    services::{
        audit::{self, AuditAction, AuditSubject},
        tickets::TicketFailure,
        workspaces::{Acting, lock_as},
    },
};

/// Kegagalan kelola persona (bentuknya sama dengan kegagalan tiket).
pub type PersonaFailure = TicketFailure;

/// Satu bagian halaman Edit persona.
#[derive(Debug, Clone)]
pub enum Edit {
    Identity(Identity),
    Instructions {
        instructions: String,
        concise: bool,
    },
    Tools {
        mcp: Vec<String>,
        client: Vec<String>,
        danger: BTreeMap<String, String>,
    },
    Models(BTreeMap<String, ModelChoice>),
    /// Skill pack terpasang (hanya yang sudah di-approve).
    Skills(Vec<Uuid>),
}

impl Edit {
    fn section(&self) -> &'static str {
        match self {
            Self::Identity(_) => "identity",
            Self::Instructions { .. } => "instructions",
            Self::Tools { .. } => "tools",
            Self::Models(_) => "models",
            Self::Skills(_) => "skills",
        }
    }
}

async fn begin(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
) -> Result<DatabaseTransaction, PersonaFailure> {
    let txn = db.begin().await?;
    lock_as(&txn, acting, |r| r.has(Permission::ManageTicketSettings))
        .await?
        .ok_or(TicketFailure::NotFound)?;
    Ok(txn)
}

async fn locked(
    txn: &DatabaseTransaction,
    acting: &Acting<'_>,
    id: Uuid,
) -> Result<personas::Model, PersonaFailure> {
    personas::Entity::find_by_id(id)
        .filter(personas::Column::WorkspaceId.eq(acting.workspace_id().get()))
        .filter(personas::Column::DeletedAt.is_null())
        .lock_exclusive()
        .one(txn)
        .await?
        .ok_or(TicketFailure::NotFound)
}

fn put(
    changes: &mut serde_json::Map<String, serde_json::Value>,
    field: &str,
    c: Option<serde_json::Value>,
) {
    if let Some(c) = c {
        changes.insert(field.to_string(), c);
    }
}

async fn name_free(
    txn: &DatabaseTransaction,
    acting: &Acting<'_>,
    name: &str,
    except: Option<Uuid>,
) -> Result<(), PersonaFailure> {
    let taken = personas::Model::active_in(txn, acting.workspace_id())
        .await?
        .iter()
        .any(|p| Some(p.id) != except && p.name.to_lowercase() == name.to_lowercase());
    if taken {
        Err(TicketFailure::field("name", personas::NAME_TAKEN))
    } else {
        Ok(())
    }
}

/// Buat persona baru: kosong (`source` = `None`) atau salinan persona aktif lain (instruksi,
/// skill, tools, izin, model, dan warna/ikon ikut disalin bila isian kosong).
///
/// # Errors
///
/// `Invalid` (identitas, nama dipakai), `NotFound` (pelaku/persona sumber), error database.
pub async fn create(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    input: &Identity,
    source: Option<Uuid>,
) -> Result<personas::Model, PersonaFailure> {
    let txn = begin(db, acting).await?;
    let wid = acting.workspace_id();
    let src = match source {
        Some(id) => Some(locked(&txn, acting, id).await?),
        None => None,
    };
    let mut input = input.clone();
    if let Some(s) = &src {
        if input.color.trim().is_empty() {
            input.color.clone_from(&s.color);
        }
        if input.icon.trim().is_empty() {
            input.icon.clone_from(&s.icon);
        }
    }
    if input.color.trim().is_empty() {
        input.color = "#64748b".to_string();
    }
    if input.icon.trim().is_empty() {
        input.icon = "robot".to_string();
    }
    let identity = personas::validate_identity(&input).map_err(TicketFailure::errors)?;
    name_free(&txn, acting, &identity.name, None).await?;
    let taken = personas::Model::known_keys(&txn, wid).await?;
    let key = personas::fresh_key(&identity.name, &taken);
    let mut row = personas::ActiveModel {
        workspace_id: Set(wid.get()),
        key: Set(key),
        name: Set(identity.name),
        summary: Set(identity.summary),
        color: Set(identity.color),
        icon: Set(identity.icon),
        builtin: Set(false),
        instructions: Set(String::new()),
        concise: Set(false),
        mcp_tools: Set(Vec::new()),
        client_tools: Set(Vec::new()),
        danger: Set(serde_json::json!({})),
        models: Set(serde_json::json!({})),
        created_by: Set(Some(acting.user.id)),
        ..Default::default()
    };
    if let Some(s) = &src {
        row.instructions = Set(s.instructions.clone());
        row.concise = Set(s.concise);
        row.mcp_tools = Set(s.mcp_tools.clone());
        row.client_tools = Set(s.client_tools.clone());
        row.danger = Set(s.danger.clone());
        row.models = Set(s.models.clone());
    } else {
        let (_, _, danger) =
            personas::validate_tools(&[], &[], &BTreeMap::new()).map_err(TicketFailure::errors)?;
        row.danger = Set(danger);
    }
    let created = row.insert(&txn).await?;
    if let Some(s) = &src {
        for link in persona_skills::Model::of_persona(&txn, s.id).await? {
            persona_skills::ActiveModel {
                workspace_id: Set(wid.get()),
                persona_id: Set(created.id),
                pack_id: Set(link.pack_id),
                ..Default::default()
            }
            .insert(&txn)
            .await?;
        }
    }
    audit::record(
        &txn,
        acting.actor().audit(
            AuditAction::PersonaCreated,
            (AuditSubject::Persona, created.id),
            serde_json::json!({
                "name": created.name,
                "key": created.key,
                "source": src.as_ref().map(|s| s.key.clone()),
            }),
        ),
    )
    .await?;
    txn.commit().await?;
    Ok(created)
}

/// Ubah satu bagian persona. Perubahan berlaku untuk job berikutnya (`revision` naik).
///
/// # Errors
///
/// `Invalid` per field, `NotFound`, error database.
pub async fn update(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
    edit: Edit,
) -> Result<personas::Model, PersonaFailure> {
    let txn = begin(db, acting).await?;
    let current = locked(&txn, acting, id).await?;
    let section = edit.section();
    let mut changes = serde_json::Map::new();
    let mut row = current.clone().into_active_model();
    match edit {
        Edit::Identity(input) => {
            let identity = personas::validate_identity(&input).map_err(TicketFailure::errors)?;
            name_free(&txn, acting, &identity.name, Some(current.id)).await?;
            put(
                &mut changes,
                "name",
                audit::change(&current.name, &identity.name),
            );
            put(
                &mut changes,
                "summary",
                audit::change(&current.summary, &identity.summary),
            );
            put(
                &mut changes,
                "color",
                audit::change(&current.color, &identity.color),
            );
            put(
                &mut changes,
                "icon",
                audit::change(&current.icon, &identity.icon),
            );
            row.name = Set(identity.name);
            row.summary = Set(identity.summary);
            row.color = Set(identity.color);
            row.icon = Set(identity.icon);
        }
        Edit::Instructions {
            instructions,
            concise,
        } => {
            let instructions =
                personas::validate_instructions(&instructions).map_err(TicketFailure::errors)?;
            if instructions != current.instructions {
                changes.insert(
                    "instructions".to_string(),
                    serde_json::json!({ "length": instructions.chars().count() }),
                );
            }
            put(
                &mut changes,
                "concise",
                audit::change(&current.concise, &concise),
            );
            row.instructions = Set(instructions);
            row.concise = Set(concise);
        }
        Edit::Tools {
            mcp,
            client,
            danger,
        } => {
            let (mcp, client, danger) =
                personas::validate_tools(&mcp, &client, &danger).map_err(TicketFailure::errors)?;
            put(
                &mut changes,
                "mcp_tools",
                audit::change(&current.mcp_tools, &mcp),
            );
            put(
                &mut changes,
                "client_tools",
                audit::change(&current.client_tools, &client),
            );
            put(
                &mut changes,
                "danger",
                audit::change(&current.danger, &danger),
            );
            row.mcp_tools = Set(mcp);
            row.client_tools = Set(client);
            row.danger = Set(danger);
        }
        Edit::Models(models) => {
            let models = personas::validate_models(&models).map_err(TicketFailure::errors)?;
            put(
                &mut changes,
                "models",
                audit::change(&current.models, &models),
            );
            row.models = Set(models);
        }
        Edit::Skills(packs) => {
            let before: Vec<Uuid> = persona_skills::Model::of_persona(&txn, current.id)
                .await?
                .iter()
                .map(|l| l.pack_id)
                .collect();
            let mut wanted: Vec<Uuid> = Vec::new();
            for pack in packs {
                if !wanted.contains(&pack) {
                    wanted.push(pack);
                }
            }
            let approved = skill_packs::Model::approved_in(&txn, acting.workspace_id()).await?;
            if wanted.iter().any(|p| !approved.iter().any(|a| a.id == *p)) {
                return Err(TicketFailure::field("skills", skill_packs::NOT_APPROVED));
            }
            let mut sorted_before = before.clone();
            sorted_before.sort();
            let mut sorted_wanted = wanted.clone();
            sorted_wanted.sort();
            if sorted_before != sorted_wanted {
                persona_skills::Entity::delete_many()
                    .filter(persona_skills::Column::PersonaId.eq(current.id))
                    .exec(&txn)
                    .await?;
                for pack in &wanted {
                    persona_skills::ActiveModel {
                        workspace_id: Set(acting.workspace_id().get()),
                        persona_id: Set(current.id),
                        pack_id: Set(*pack),
                        ..Default::default()
                    }
                    .insert(&txn)
                    .await?;
                }
                let names = |ids: &[Uuid]| -> Vec<String> {
                    ids.iter()
                        .filter_map(|id| approved.iter().find(|a| a.id == *id))
                        .map(|a| a.name.clone())
                        .collect()
                };
                changes.insert(
                    "skills".to_string(),
                    serde_json::json!({ "from": names(&before), "to": names(&wanted) }),
                );
            }
        }
    }
    if changes.is_empty() {
        txn.rollback().await?;
        return Ok(current);
    }
    row.revision = Set(current.revision + 1);
    let updated = row.update(&txn).await?;
    changes.insert("section".to_string(), section.into());
    audit::record(
        &txn,
        acting.actor().audit(
            AuditAction::PersonaUpdated,
            (AuditSubject::Persona, updated.id),
            serde_json::Value::Object(changes),
        ),
    )
    .await?;
    txn.commit().await?;
    Ok(updated)
}

/// Hapus persona: `deleted_at` + `replaced_by`. Stage yang menyebut kuncinya memakai pengganti.
/// Persona lain yang sudah dihapus dengan pengganti persona ini ikut menunjuk pengganti baru.
///
/// # Errors
///
/// `persona-replacement-invalid` (pengganti tidak aktif atau sama), `NotFound`, error database.
pub async fn delete(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
    replacement: Option<Uuid>,
) -> Result<(), PersonaFailure> {
    let txn = begin(db, acting).await?;
    let current = locked(&txn, acting, id).await?;
    let replacement = match replacement.filter(|r| *r != current.id) {
        Some(r) => personas::Model::find_active_by_id(&txn, acting.workspace_id(), r)
            .await?
            .ok_or_else(|| TicketFailure::field("replacement", personas::REPLACEMENT_INVALID))?,
        None => {
            return Err(TicketFailure::field(
                "replacement",
                personas::REPLACEMENT_INVALID,
            ));
        }
    };
    personas::Entity::update_many()
        .col_expr(
            personas::Column::ReplacedBy,
            sea_orm::sea_query::Expr::value(replacement.id),
        )
        .filter(personas::Column::WorkspaceId.eq(acting.workspace_id().get()))
        .filter(personas::Column::ReplacedBy.eq(current.id))
        .exec(&txn)
        .await?;
    let mut row = current.clone().into_active_model();
    row.deleted_at = Set(Some(chrono::Utc::now().into()));
    row.replaced_by = Set(Some(replacement.id));
    row.update(&txn).await?;
    persona_skills::Entity::delete_many()
        .filter(persona_skills::Column::PersonaId.eq(current.id))
        .exec(&txn)
        .await?;
    audit::record(
        &txn,
        acting.actor().audit(
            AuditAction::PersonaDeleted,
            (AuditSubject::Persona, current.id),
            serde_json::json!({
                "name": current.name,
                "key": current.key,
                "replacement": replacement.name,
            }),
        ),
    )
    .await?;
    txn.commit().await?;
    Ok(())
}
