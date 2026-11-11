//! Data error form untuk komponen `field`: satu entri per field yang dirender, selalu ada
//! (kosong bila tanpa error), karena Tera 2 gagal pada variabel yang tidak didefinisikan.

use std::collections::BTreeMap;

use loco_rs::validation::ModelValidationErrors;
use serde::Serialize;

/// Error pertama satu field: kode (= kunci Fluent) dan argumen `min`/`max` untuk pesannya.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct FieldError {
    pub code: String,
    pub min: i64,
    pub max: i64,
}

/// `field -> FieldError` untuk setiap nama di `fields`.
pub type FieldErrors = BTreeMap<&'static str, FieldError>;

/// Error per field dari hasil validasi; field tanpa error mendapat entri kosong.
#[must_use]
pub fn field_errors(
    fields: &[&'static str],
    errors: Option<&ModelValidationErrors>,
) -> FieldErrors {
    fields
        .iter()
        .map(|field| {
            let first = errors
                .and_then(|e| e.errors.get(*field))
                .and_then(|list| list.first());
            let error = first.map_or_else(FieldError::default, |e| {
                let int = |key: &str| {
                    e.params
                        .get(key)
                        .and_then(serde_json::Value::as_i64)
                        .unwrap_or(0)
                };
                FieldError {
                    code: e.code.clone(),
                    min: int("min"),
                    max: int("max"),
                }
            });
            (*field, error)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use validator::{ValidationError, ValidationErrors};

    use super::*;

    #[test]
    fn every_field_gets_an_entry_and_only_the_first_error_is_kept() {
        let mut errors = ValidationErrors::new();
        let mut short = ValidationError::new("password-too-short");
        short.add_param("min".into(), &12);
        errors.add("password", short);
        errors.add("password", ValidationError::new("second"));
        let errors = ModelValidationErrors::from(errors);

        let map = field_errors(&["email", "password"], Some(&errors));
        assert_eq!(map["email"], FieldError::default());
        assert_eq!(
            map["password"],
            FieldError {
                code: "password-too-short".into(),
                min: 12,
                max: 0
            }
        );
        assert_eq!(
            field_errors(&["email"], None)["email"],
            FieldError::default()
        );
    }
}
