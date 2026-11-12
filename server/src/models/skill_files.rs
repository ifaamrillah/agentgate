//! Berkas skill pack (`SKILL.md`, `scripts/…`, `references/…`): teks UTF-8, path relatif unik
//! per skill pack.
use loco_rs::model::TenantEntity;
use sea_orm::{QueryOrder, entity::prelude::*};
use uuid::Uuid;

pub use super::_entities::skill_files::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

impl TenantEntity for Entity {
    type TenantId = WorkspaceId;

    fn tenant_column() -> Column {
        Column::WorkspaceId
    }
}

standard_behavior!(ActiveModel);

/// Path berkas skill pack yang boleh dibuat sendiri: `SKILL.md`, atau berkas di bawah
/// `scripts/` / `references/` (huruf, angka, `.`, `-`, `_`, `/`; tanpa `..`).
#[must_use]
pub fn valid_path(path: &str) -> bool {
    let ok_chars = path
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '/'));
    let segments: Vec<&str> = path.split('/').collect();
    ok_chars
        && path.len() <= 120
        && segments
            .iter()
            .all(|s| !s.is_empty() && *s != "." && *s != "..")
        && (path == "SKILL.md"
            || (segments.len() >= 2 && matches!(segments[0], "scripts" | "references")))
}

impl Model {
    /// Berkas skill pack, `SKILL.md` dulu lalu urut path.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn of_pack<C: ConnectionTrait>(db: &C, pack_id: Uuid) -> Result<Vec<Self>, DbErr> {
        let mut files = Entity::find()
            .filter(Column::PackId.eq(pack_id))
            .order_by_asc(Column::Path)
            .all(db)
            .await?;
        files.sort_by_key(|f| (f.path != "SKILL.md", f.path.clone()));
        Ok(files)
    }

    /// Jenis berkas untuk tampilan: `md` atau `code`.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        if self.path.ends_with(".md") {
            "md"
        } else {
            "code"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::valid_path;

    #[test]
    fn paths_stay_in_skill_folders() {
        for ok in [
            "SKILL.md",
            "scripts/cek.py",
            "references/format-struk.md",
            "scripts/a/b.sh",
        ] {
            assert!(valid_path(ok), "{ok}");
        }
        for bad in [
            "README.md",
            "scripts/",
            "scripts/../x",
            "/SKILL.md",
            "src/x.rs",
            "scripts/a b.sh",
            "scripts",
        ] {
            assert!(!valid_path(bad), "{bad}");
        }
    }
}
