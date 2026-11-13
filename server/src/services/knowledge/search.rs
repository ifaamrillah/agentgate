//! Pencarian dokumen knowledge: full-text Postgres (`simple`, indeks GIN judul + isi) dengan
//! cadangan `ILIKE` untuk potongan kata, diurutkan judul cocok dulu lalu peringkat FTS.

use sea_orm::{
    ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    sea_query::{Expr, SimpleExpr},
};
use uuid::Uuid;

use crate::{
    common::markdown,
    models::{knowledge_docs, workspaces::WorkspaceId},
};

/// Satu hasil pencarian.
#[derive(Debug, Clone)]
pub struct Hit {
    pub doc: knowledge_docs::Model,
    /// Cuplikan teks polos di sekitar kata yang dicari.
    pub snippet: String,
}

fn like_pattern(q: &str) -> String {
    let escaped = q
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

/// Cuplikan ±`width` karakter di sekitar kemunculan pertama `q` (tanpa beda huruf besar/kecil).
#[must_use]
pub fn snippet(body: &str, q: &str, width: usize) -> String {
    let text = markdown::plain(body, 20_000);
    let lower = text.to_lowercase();
    let needle = q.trim().to_lowercase();
    let chars: Vec<char> = text.chars().collect();
    let pos = if needle.is_empty() {
        None
    } else {
        lower.find(&needle).map(|b| lower[..b].chars().count())
    };
    let Some(pos) = pos else {
        return markdown::plain(body, width * 2);
    };
    let start = pos.saturating_sub(width);
    let end = (pos + needle.chars().count() + width).min(chars.len());
    let mut out: String = chars[start..end].iter().collect();
    if start > 0 {
        out = format!("…{}", out.trim_start());
    }
    if end < chars.len() {
        out = format!("{}…", out.trim_end());
    }
    out
}

/// Cari dokumen aktif. `project` = dokumen project itu + dokumen workspace yang berlaku untuknya;
/// `None` = semua dokumen workspace (halaman Knowledge workspace).
///
/// # Errors
///
/// Error database.
pub async fn search<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project: Option<Uuid>,
    q: &str,
    limit: u64,
) -> Result<Vec<Hit>, DbErr> {
    let q = q.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let q: String = q.chars().take(200).collect();
    let fts: SimpleExpr = Expr::cust_with_values(
        "to_tsvector('simple', title || ' ' || body) @@ plainto_tsquery('simple', $1)",
        [q.clone()],
    );
    let like = like_pattern(&q);
    let matches = sea_orm::Condition::any()
        .add(fts)
        .add(Expr::cust_with_values("title ILIKE $1", [like.clone()]))
        .add(Expr::cust_with_values("body ILIKE $1", [like.clone()]));
    let scope = match project {
        Some(p) => sea_orm::Condition::any()
            .add(knowledge_docs::Column::ProjectId.eq(p))
            .add(
                sea_orm::Condition::all()
                    .add(knowledge_docs::Column::ProjectId.is_null())
                    .add(Expr::cust_with_values(
                        "(cardinality(applies_to) = 0 OR $1 = ANY(applies_to))",
                        [p],
                    )),
            ),
        None => sea_orm::Condition::all(),
    };
    let docs = knowledge_docs::Entity::find()
        .filter(knowledge_docs::Column::WorkspaceId.eq(wid.get()))
        .filter(knowledge_docs::Column::ArchivedAt.is_null())
        .filter(scope)
        .filter(matches)
        .order_by_desc(Expr::cust_with_values("(title ILIKE $1)", [like]))
        .order_by_desc(Expr::cust_with_values(
            "ts_rank(to_tsvector('simple', title || ' ' || body), plainto_tsquery('simple', $1))",
            [q.clone()],
        ))
        .order_by_asc(knowledge_docs::Column::Title)
        .limit(limit)
        .all(db)
        .await?;
    Ok(docs
        .into_iter()
        .map(|doc| Hit {
            snippet: snippet(&doc.body, &q, 70),
            doc,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snippet_centers_on_match() {
        let body = format!(
            "{} kata kunci pembayaran ada di sini {}",
            "a ".repeat(100),
            "b ".repeat(100)
        );
        let s = snippet(&body, "Pembayaran", 10);
        assert!(s.contains("pembayaran"), "{s}");
        assert!(s.starts_with('…') && s.ends_with('…'), "{s}");
        assert_eq!(like_pattern("50%_a"), "%50\\%\\_a%");
    }
}
