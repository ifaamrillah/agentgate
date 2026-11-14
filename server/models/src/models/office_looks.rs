//! Tampilan karakter AI Office (M10): satu baris per persona (`subject_key` = key persona) atau
//! manusia (`subject_key` = id user) per workspace. Berlaku untuk semua orang di workspace.
use sea_orm::{
    ActiveEnum, ColumnTrait, ConnectionTrait, DbErr, DeriveActiveEnum, EntityTrait, EnumIter,
    QueryFilter,
};
use serde::{Deserialize, Serialize};

pub use super::_entities::office_looks::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

standard_behavior!(ActiveModel);

pub const ALIAS_MAX_LEN: usize = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum LookSubject {
    #[sea_orm(string_value = "persona")]
    Persona,
    #[sea_orm(string_value = "user")]
    User,
}

/// Aksesori kepala karakter (sama dengan prototype A1).
pub const ACCESSORIES: &[&str] = &[
    "none", "headset", "glasses", "cap", "beanie", "beret", "scarf",
];
/// Gaya rambut manusia.
pub const HAIR_STYLES: &[&str] = &["short", "bob", "long", "hijab"];

/// Isi `look` (semua opsional; kosong = bawaan dari persona/warna user).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Look {
    pub alias: String,
    pub shirt: String,
    pub acc: String,
    pub skin: String,
    pub hair: String,
    pub style: String,
}

/// Kode error validasi (kunci Fluent).
pub const LOOK_INVALID: &str = "office-look-invalid";

fn is_hex(v: &str) -> bool {
    v.len() == 7 && v.starts_with('#') && v[1..].chars().all(|c| c.is_ascii_hexdigit())
}

impl Look {
    /// Normalisasi + validasi isian form. Nilai kosong = bawaan.
    ///
    /// # Errors
    /// [`LOOK_INVALID`] bila warna bukan `#rrggbb`, aksesori/gaya tidak dikenal, atau nama terlalu
    /// panjang.
    pub fn normalized(mut self) -> Result<Self, &'static str> {
        self.alias = self.alias.trim().to_string();
        for v in [&mut self.shirt, &mut self.skin, &mut self.hair] {
            *v = v.trim().to_ascii_lowercase();
        }
        self.acc = self.acc.trim().to_string();
        self.style = self.style.trim().to_string();
        let colors_ok = [&self.shirt, &self.skin, &self.hair]
            .iter()
            .all(|v| v.is_empty() || is_hex(v));
        if self.alias.chars().count() > ALIAS_MAX_LEN
            || !colors_ok
            || !(self.acc.is_empty() || ACCESSORIES.contains(&self.acc.as_str()))
            || !(self.style.is_empty() || HAIR_STYLES.contains(&self.style.as_str()))
        {
            return Err(LOOK_INVALID);
        }
        Ok(self)
    }
}

impl Model {
    #[must_use]
    pub fn look(&self) -> Look {
        serde_json::from_value(self.look.clone()).unwrap_or_default()
    }

    #[must_use]
    pub fn subject(&self) -> Option<LookSubject> {
        LookSubject::try_from_value(&self.subject).ok()
    }

    /// Semua tampilan di workspace.
    ///
    /// # Errors
    /// Error database.
    pub async fn of_workspace<C: ConnectionTrait>(
        db: &C,
        workspace: WorkspaceId,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::WorkspaceId.eq(workspace.get()))
            .all(db)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn look_validation() {
        let ok = Look {
            alias: "  Dev ".into(),
            shirt: "#5B5BD6".into(),
            acc: "headset".into(),
            ..Look::default()
        }
        .normalized()
        .unwrap();
        assert_eq!(ok.alias, "Dev");
        assert_eq!(ok.shirt, "#5b5bd6");
        for bad in [
            Look {
                shirt: "red".into(),
                ..Look::default()
            },
            Look {
                acc: "crown".into(),
                ..Look::default()
            },
            Look {
                style: "mohawk".into(),
                ..Look::default()
            },
            Look {
                alias: "x".repeat(25),
                ..Look::default()
            },
        ] {
            assert_eq!(bad.normalized(), Err(LOOK_INVALID));
        }
    }
}
