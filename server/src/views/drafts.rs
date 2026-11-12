//! Tampilan Project → Draft tiket (`assets/views/tickets/drafts.html`): draft hasil
//! brainstorming dalam pohon induk-anak, dicentang lalu dibuat menjadi tiket.
use serde::Serialize;
use uuid::Uuid;

use super::{tickets::TypeView, ui::Stat};
use crate::models::{ticket_drafts, ticket_types};

pub const FLASH_CREATED: &str = "draft-flash-created";
pub const FLASH_SAVED: &str = "draft-flash-saved";
pub const FLASH_DELETED: &str = "draft-flash-deleted";

/// Satu baris draft di pohon.
#[derive(Debug, Clone, Serialize)]
pub struct DraftRow {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub type_key: String,
    pub ticket_type: Option<TypeView>,
    pub story_points: String,
    pub approved: bool,
    /// Kedalaman di pohon (0 = akar).
    pub depth: usize,
    /// Indentasi CSS (px).
    pub indent: usize,
    pub edit_action: String,
    pub delete_action: String,
}

/// Urutkan draft menjadi pohon (induk lalu anak-anaknya, urut posisi) beserta kedalamannya.
#[must_use]
pub fn tree(
    drafts: &[ticket_drafts::Model],
    types: &[ticket_types::Model],
    base: &str,
) -> Vec<DraftRow> {
    fn walk(
        parent: Option<Uuid>,
        depth: usize,
        drafts: &[ticket_drafts::Model],
        types: &[ticket_types::Model],
        base: &str,
        out: &mut Vec<DraftRow>,
    ) {
        for d in drafts.iter().filter(|d| {
            d.parent_id == parent
                || (parent.is_none()
                    && d.parent_id
                        .is_some_and(|p| !drafts.iter().any(|x| x.id == p)))
        }) {
            if out.iter().any(|r| r.id == d.id) || depth > 4 {
                continue;
            }
            out.push(DraftRow {
                id: d.id,
                title: d.title.clone(),
                description: d.description.clone(),
                type_key: d.type_key.clone(),
                ticket_type: types.iter().find(|t| t.key == d.type_key).map(TypeView::of),
                story_points: d.story_points.map(|p| p.to_string()).unwrap_or_default(),
                approved: d.approved,
                depth,
                indent: depth * 22,
                edit_action: format!("{base}/{}", d.id),
                delete_action: format!("{base}/{}/delete", d.id),
            });
            walk(Some(d.id), depth + 1, drafts, types, base, out);
        }
    }
    let mut out = Vec::new();
    walk(None, 0, drafts, types, base, &mut out);
    out
}

#[must_use]
pub fn stats(rows: &[DraftRow]) -> Vec<Stat> {
    let approved = rows.iter().filter(|r| r.approved).count();
    let points: i64 = rows
        .iter()
        .filter(|r| r.type_key != "epic")
        .filter_map(|r| r.story_points.parse::<i64>().ok())
        .sum();
    vec![
        Stat::new("draft-stat-count", rows.len(), "ticket"),
        Stat::new("draft-stat-approved", approved, "circle-check").tone("success"),
        Stat::new("draft-stat-points", points, "sparkles").tone("accent"),
    ]
}

/// Kunci Fluent yang dibangun di kode (dites kontrak i18n).
#[must_use]
pub fn label_keys() -> Vec<String> {
    let mut keys: Vec<String> = stats(&[])
        .into_iter()
        .map(|s| s.label_key.to_string())
        .collect();
    keys.extend([FLASH_CREATED, FLASH_SAVED, FLASH_DELETED].map(str::to_string));
    keys
}
