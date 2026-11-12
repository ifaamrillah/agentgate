//! Field custom workspace (spec "field custom"): pustaka field yang bisa dipakai beberapa tipe
//! tiket sekaligus (`ticket_type_fields`). Nilai per tiket disimpan di
//! `tickets.custom_values` (lihat `tickets::CustomValues`), divalidasi menurut [`FieldKind`] di
//! [`Model::parse_value`]. Field `required` wajib diisi di tiket bertipe yang memakainya.

use loco_rs::model::{TenantEntity, TenantQueryExt};
use sea_orm::{ActiveValue::Set, IntoActiveModel, QueryOrder, SqlErr, entity::prelude::*};
use validator::{ValidationError, ValidationErrors};

pub use super::_entities::custom_fields::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;
use crate::common::text;

pub const FIELD_NAME_TAKEN: &str = "custom-field-name-taken";
pub const FIELD_OPTIONS_REQUIRED: &str = "custom-field-options-required";
pub const FIELD_VALUE_REQUIRED: &str = "custom-field-value-required";
pub const FIELD_VALUE_INVALID: &str = "custom-field-value-invalid";
pub const ERROR_CODES: &[&str] = &[
    FIELD_NAME_TAKEN,
    FIELD_OPTIONS_REQUIRED,
    FIELD_VALUE_REQUIRED,
    FIELD_VALUE_INVALID,
];
/// Panjang maksimal nilai field teks/URL.
pub const VALUE_MAX_LEN: usize = 500;
/// Jumlah maksimal pilihan field `select`.
pub const OPTIONS_MAX: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum FieldKind {
    #[sea_orm(string_value = "text")]
    Text,
    #[sea_orm(string_value = "number")]
    Number,
    #[sea_orm(string_value = "select")]
    Select,
    #[sea_orm(string_value = "date")]
    Date,
    #[sea_orm(string_value = "url")]
    Url,
}

#[derive(Debug, thiserror::Error)]
pub enum CustomFieldError {
    #[error("invalid custom field")]
    Invalid(loco_rs::validation::ModelValidationErrors),
    #[error(transparent)]
    Db(#[from] DbErr),
}

#[derive(Debug, Clone)]
pub struct FieldInput {
    pub name: String,
    pub kind: FieldKind,
    /// Pilihan field `select` (satu per baris di form); diabaikan jenis lain.
    pub options: Vec<String>,
    pub required: bool,
}

impl TenantEntity for Entity {
    type TenantId = WorkspaceId;

    fn tenant_column() -> Column {
        Column::WorkspaceId
    }
}

standard_behavior!(ActiveModel);

impl Model {
    /// # Errors
    ///
    /// Nilai kolom bukan varian [`FieldKind`].
    pub fn kind(&self) -> Result<FieldKind, DbErr> {
        FieldKind::try_from_value(&self.kind)
    }

    /// Semua field workspace, urut nama.
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
            .order_by_asc(Column::Name)
            .order_by_asc(Column::Id)
            .all(db)
            .await
    }

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

    /// Nilai isian untuk field ini, dinormalisasi: `Ok(None)` = kosong. Wajib-isi dicek
    /// pemanggil (bergantung tipe tiket).
    ///
    /// # Errors
    ///
    /// [`FIELD_VALUE_INVALID`].
    pub fn parse_value(&self, raw: &str) -> Result<Option<String>, ValidationError> {
        let value = raw.trim();
        if value.is_empty() {
            return Ok(None);
        }
        let invalid = || ValidationError::new(FIELD_VALUE_INVALID);
        let ok = match self.kind().map_err(|_| invalid())? {
            FieldKind::Text => value.chars().count() <= VALUE_MAX_LEN,
            FieldKind::Number => value.parse::<f64>().is_ok_and(f64::is_finite),
            FieldKind::Select => self.options.iter().any(|o| o == value),
            FieldKind::Date => chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").is_ok(),
            FieldKind::Url => {
                value.len() <= VALUE_MAX_LEN
                    && (value.starts_with("https://") || value.starts_with("http://"))
                    && !value.contains(char::is_whitespace)
            }
        };
        if ok {
            Ok(Some(value.to_string()))
        } else {
            Err(invalid())
        }
    }

    /// Ubah field. Jenis tidak berubah setelah dibuat (nilai lama bisa tidak cocok).
    ///
    /// # Errors
    ///
    /// [`CustomFieldError::Invalid`] atau error database.
    pub async fn update_details<C: ConnectionTrait>(
        self,
        db: &C,
        input: &FieldInput,
    ) -> Result<Self, CustomFieldError> {
        let kind = self.kind()?;
        let (name, options) = validated(input, kind)?;
        let mut model = self.into_active_model();
        model.name = Set(name);
        model.options = Set(options);
        model.required = Set(input.required);
        model.update(db).await.map_err(name_taken)
    }
}

impl ActiveModel {
    /// # Errors
    ///
    /// [`CustomFieldError::Invalid`] atau error database.
    pub async fn create<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        input: &FieldInput,
    ) -> Result<Model, CustomFieldError> {
        let (name, options) = validated(input, input.kind)?;
        Self {
            workspace_id: Set(workspace_id.get()),
            name: Set(name),
            kind: Set(input.kind.to_value()),
            options: Set(options),
            required: Set(input.required),
            ..Default::default()
        }
        .insert(db)
        .await
        .map_err(name_taken)
    }
}

fn validated(
    input: &FieldInput,
    kind: FieldKind,
) -> Result<(String, Vec<String>), CustomFieldError> {
    let mut errors = ValidationErrors::new();
    let name = text::normalize_name(&input.name);
    if let Err(e) = text::validate_name("custom_field", &name) {
        errors.add("name", e);
    }
    let mut options: Vec<String> = Vec::new();
    if kind == FieldKind::Select {
        for o in &input.options {
            let o = o.trim();
            if !o.is_empty() && !options.iter().any(|x| x == o) && o.chars().count() <= 100 {
                options.push(o.to_string());
            }
        }
        options.truncate(OPTIONS_MAX);
        if options.is_empty() {
            errors.add("options", ValidationError::new(FIELD_OPTIONS_REQUIRED));
        }
    }
    if errors.is_empty() {
        Ok((name, options))
    } else {
        Err(CustomFieldError::Invalid(errors.into()))
    }
}

fn name_taken(e: DbErr) -> CustomFieldError {
    if matches!(e.sql_err(), Some(SqlErr::UniqueConstraintViolation(_))) {
        let mut errors = ValidationErrors::new();
        errors.add("name", ValidationError::new(FIELD_NAME_TAKEN));
        return CustomFieldError::Invalid(errors.into());
    }
    CustomFieldError::Db(e)
}
