//! Tipe tiket per workspace (spec: Epic, Story, Task, Bug, Subtask, plus tipe custom). Tipe
//! bawaan ([`BUILTIN_TYPES`]) dibuat bersama workspace dan tidak bisa dihapus; nama, ikon, warna,
//! dan field-nya tetap bisa diubah. Tipe custom bisa dihapus selama tidak dipakai tiket (FK
//! `tickets.type_id` tanpa `ON DELETE`). `key` (huruf kecil) dipakai token `{TYPE}` dan tidak
//! berubah setelah dibuat.

use loco_rs::model::{TenantEntity, TenantQueryExt};
use sea_orm::{ActiveValue::Set, IntoActiveModel, QueryOrder, SqlErr, entity::prelude::*};
use validator::{ValidationError, ValidationErrors};

pub use super::_entities::ticket_types::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;
use crate::common::text;

pub const TYPE_KEY_INVALID: &str = "ticket-type-key-invalid";
pub const TYPE_KEY_TAKEN: &str = "ticket-type-key-taken";
pub const TYPE_NAME_TAKEN: &str = "ticket-type-name-taken";
pub const TYPE_ICON_INVALID: &str = "ticket-type-icon-invalid";
pub const TYPE_COLOR_INVALID: &str = "ticket-type-color-invalid";
pub const TYPE_BUILTIN: &str = "ticket-type-builtin";
pub const TYPE_IN_USE: &str = "ticket-type-in-use";
pub const ERROR_CODES: &[&str] = &[
    TYPE_KEY_INVALID,
    TYPE_KEY_TAKEN,
    TYPE_NAME_TAKEN,
    TYPE_ICON_INVALID,
    TYPE_COLOR_INVALID,
    TYPE_BUILTIN,
    TYPE_IN_USE,
];
/// Panjang deskripsi tipe.
pub const DESCRIPTION_MAX_LEN: usize = 200;

/// Ikon yang bisa dipilih untuk tipe (nama ikon Tabler di `assets/icons/`).
pub const TYPE_ICONS: &[&str] = &[
    "bookmark",
    "square-check",
    "bug",
    "bolt",
    "subtask",
    "flag",
    "bulb",
    "target",
    "book",
    "rocket",
    "star",
    "tag",
    "palette",
    "code",
];

/// Tipe bawaan: key, nama, deskripsi, ikon, warna. Sama dengan backfill di migrasi tiket.
pub const BUILTIN_TYPES: &[(&str, &str, &str, &str, &str)] = &[
    (
        "epic",
        "Epic",
        "Kumpulan story besar lintas sprint",
        "bolt",
        "#7c3aed",
    ),
    (
        "story",
        "Story",
        "Kebutuhan dari sisi pengguna",
        "bookmark",
        "#0f8a5f",
    ),
    (
        "task",
        "Task",
        "Pekerjaan teknis atau operasional",
        "square-check",
        "#2563eb",
    ),
    (
        "bug",
        "Bug",
        "Perilaku yang salah di aplikasi",
        "bug",
        "#dc2626",
    ),
    (
        "subtask",
        "Subtask",
        "Pecahan kecil dari tiket lain",
        "subtask",
        "#64748b",
    ),
];

#[derive(Debug, thiserror::Error)]
pub enum TicketTypeError {
    #[error("invalid ticket type")]
    Invalid(loco_rs::validation::ModelValidationErrors),
    #[error(transparent)]
    Db(#[from] DbErr),
}

/// Isian tipe (buat/ubah). `key` hanya dipakai saat membuat.
#[derive(Debug, Clone, Default)]
pub struct TypeInput {
    pub key: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub color: String,
}

impl TenantEntity for Entity {
    type TenantId = WorkspaceId;

    fn tenant_column() -> Column {
        Column::WorkspaceId
    }
}

standard_behavior!(ActiveModel);

impl Model {
    /// Semua tipe workspace, urut posisi lalu nama.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn all_in<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .in_tenant(workspace_id)
            .order_by_asc(Column::Position)
            .order_by_asc(Column::Name)
            .order_by_asc(Column::Id)
            .all(db)
            .await
    }

    /// Tipe `id` di workspace ini.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn find_in<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        id: Uuid,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(id).in_tenant(workspace_id).one(db).await
    }

    /// Ubah nama, deskripsi, ikon, warna.
    ///
    /// # Errors
    ///
    /// [`TicketTypeError::Invalid`] atau error database.
    pub async fn update_details<C: ConnectionTrait>(
        self,
        db: &C,
        input: &TypeInput,
    ) -> Result<Self, TicketTypeError> {
        let mut errors = ValidationErrors::new();
        let fields = validated(input, &mut errors);
        if !errors.is_empty() {
            return Err(TicketTypeError::Invalid(errors.into()));
        }
        let mut model = self.into_active_model();
        model.name = Set(fields.name);
        model.description = Set(fields.description);
        model.icon = Set(fields.icon);
        model.color = Set(fields.color);
        model.update(db).await.map_err(name_taken)
    }
}

impl ActiveModel {
    /// Buat tipe (custom bila `builtin == false`).
    ///
    /// # Errors
    ///
    /// [`TicketTypeError::Invalid`] (termasuk key/nama terpakai) atau error database.
    pub async fn create<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        input: &TypeInput,
        builtin: bool,
        position: i32,
    ) -> Result<Model, TicketTypeError> {
        let mut errors = ValidationErrors::new();
        let key = input.key.trim().to_lowercase();
        if !valid_key(&key) {
            errors.add("key", ValidationError::new(TYPE_KEY_INVALID));
        }
        let fields = validated(input, &mut errors);
        if !errors.is_empty() {
            return Err(TicketTypeError::Invalid(errors.into()));
        }
        Self {
            workspace_id: Set(workspace_id.get()),
            key: Set(key),
            name: Set(fields.name),
            description: Set(fields.description),
            icon: Set(fields.icon),
            color: Set(fields.color),
            builtin: Set(builtin),
            position: Set(position),
            ..Default::default()
        }
        .insert(db)
        .await
        .map_err(name_taken)
    }

    /// Tipe bawaan untuk workspace baru.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn create_builtin_set<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
    ) -> Result<Vec<Model>, DbErr> {
        let mut out = Vec::with_capacity(BUILTIN_TYPES.len());
        for (position, (key, name, description, icon, color)) in BUILTIN_TYPES.iter().enumerate() {
            let input = TypeInput {
                key: (*key).to_string(),
                name: (*name).to_string(),
                description: (*description).to_string(),
                icon: (*icon).to_string(),
                color: (*color).to_string(),
            };
            let position = i32::try_from(position).unwrap_or(i32::MAX);
            out.push(
                Self::create(db, workspace_id, &input, true, position)
                    .await
                    .map_err(|e| match e {
                        TicketTypeError::Db(e) => e,
                        TicketTypeError::Invalid(_) => DbErr::Custom("builtin type".into()),
                    })?,
            );
        }
        Ok(out)
    }
}

/// Key tipe: huruf kecil diawali huruf, 2–20 karakter `[a-z0-9_]`.
#[must_use]
pub fn valid_key(key: &str) -> bool {
    (2..=20).contains(&key.len())
        && key.bytes().next().is_some_and(|b| b.is_ascii_lowercase())
        && key
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

struct Fields {
    name: String,
    description: String,
    icon: String,
    color: String,
}

fn validated(input: &TypeInput, errors: &mut ValidationErrors) -> Fields {
    let name = text::normalize_name(&input.name);
    if let Err(e) = text::validate_name("ticket_type", &name) {
        errors.add("name", e);
    }
    let description = input.description.trim().to_string();
    if description.chars().count() > DESCRIPTION_MAX_LEN {
        let mut e = ValidationError::new(text::NAME_TOO_LONG);
        e.add_param("entity".into(), &"ticket_type");
        e.add_param("max".into(), &DESCRIPTION_MAX_LEN);
        errors.add("description", e);
    }
    let icon = input.icon.trim().to_string();
    if !TYPE_ICONS.contains(&icon.as_str()) {
        errors.add("icon", ValidationError::new(TYPE_ICON_INVALID));
    }
    let color = input.color.trim().to_ascii_lowercase();
    if !text::is_hex_color(&color) {
        errors.add("color", ValidationError::new(TYPE_COLOR_INVALID));
    }
    Fields {
        name,
        description,
        icon,
        color,
    }
}

fn name_taken(e: DbErr) -> TicketTypeError {
    if let Some(SqlErr::UniqueConstraintViolation(msg)) = e.sql_err() {
        let mut errors = ValidationErrors::new();
        if msg.contains("key_lower") {
            errors.add("key", ValidationError::new(TYPE_KEY_TAKEN));
        } else {
            errors.add("name", ValidationError::new(TYPE_NAME_TAKEN));
        }
        return TicketTypeError::Invalid(errors.into());
    }
    TicketTypeError::Db(e)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_lowercase_identifiers() {
        assert!(valid_key("story"));
        assert!(valid_key("spike_2"));
        assert!(!valid_key("S"));
        assert!(!valid_key("2fa"));
        assert!(!valid_key("Story"));
    }

    #[test]
    fn builtin_icons_are_selectable() {
        for (_, _, _, icon, _) in BUILTIN_TYPES {
            assert!(TYPE_ICONS.contains(icon));
        }
    }
}
