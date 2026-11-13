//! Usulan perubahan knowledge dari AI (atau dari repo/`agentgate init`). Tidak pernah menimpa
//! dokumen sendiri: pemilik knowledge (atau Admin project) menyetujui, menolak dengan alasan, atau
//! membatalkan keputusannya selama dokumen belum berubah lagi.

use sea_orm::{
    ActiveEnum, ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait,
    IntoActiveModel, QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait,
};
use uuid::Uuid;

use super::{
    CONFLICT, Failure, NOT_OWNER, NOT_PENDING, NOTE_REQUIRED, TYPE_INVALID, UNCHANGED,
    UNDO_BLOCKED, can_approve, clean_body, clean_title, docs, now,
};
use crate::{
    models::{
        knowledge_docs,
        knowledge_proposals::{self, ProposalStatus, ProposalTrigger},
        knowledge_versions::{self, VersionSource},
        notifications::{self, NewNotification, NotificationKind},
        projects,
        workspaces::WorkspaceId,
    },
    services::{tickets::artifacts::DiffLine, workspaces::Acting},
};

/// Usulan baru.
#[derive(Debug, Clone)]
pub struct NewProposal {
    pub project_id: Uuid,
    /// Slug dokumen yang diubah; tidak ada di project = usulan dokumen baru.
    pub slug: String,
    pub title: String,
    /// Jenis (wajib untuk dokumen baru).
    pub type_key: String,
    pub body: String,
    pub reason: String,
    pub trigger: ProposalTrigger,
    pub ticket_id: Option<Uuid>,
    pub trigger_ref: String,
    pub run_id: Option<Uuid>,
    pub persona: String,
    pub tokens: Option<i64>,
    pub sources: Vec<docs::SourceRef>,
    pub created_by: Option<Uuid>,
}

impl NewProposal {
    #[must_use]
    pub fn new(project_id: Uuid, trigger: ProposalTrigger) -> Self {
        Self {
            project_id,
            slug: String::new(),
            title: String::new(),
            type_key: String::new(),
            body: String::new(),
            reason: String::new(),
            trigger,
            ticket_id: None,
            trigger_ref: String::new(),
            run_id: None,
            persona: String::new(),
            tokens: None,
            sources: Vec::new(),
            created_by: None,
        }
    }
}

/// Simpan usulan (tanpa cek izin; pemanggil sudah memeriksa). Usulan pending lama untuk dokumen
/// (atau slug dokumen baru) yang sama digantikan. Pemilik knowledge mendapat notifikasi.
///
/// # Errors
///
/// `Invalid` (judul, isi, jenis, sama dengan dokumen), error database.
pub async fn propose(
    db: &DatabaseConnection,
    wid: WorkspaceId,
    new: NewProposal,
) -> Result<knowledge_proposals::Model, Failure> {
    let txn = db.begin().await?;
    let project = projects::Model::find_in(&txn, wid, new.project_id)
        .await?
        .ok_or(Failure::NotFound)?;
    let body = clean_body(&new.body)?;
    let slug = super::slugify(if new.slug.trim().is_empty() {
        &new.title
    } else {
        &new.slug
    });
    let doc = knowledge_docs::Model::by_slug(&txn, wid, Some(project.id), &slug)
        .await?
        .filter(|d| d.archived_at.is_none());
    let (title, group_key, type_key, base_version) = match &doc {
        Some(d) => {
            let title = if new.title.trim().is_empty() {
                d.title.clone()
            } else {
                clean_title(&new.title)?
            };
            if title == d.title && body == d.body {
                return Err(Failure::field("body", UNCHANGED));
            }
            (title, d.group_key.clone(), d.type_key.clone(), d.version)
        }
        None => {
            let title = clean_title(&new.title)?;
            let group = docs::group_of(&txn, wid, &new.type_key)
                .await?
                .ok_or_else(|| Failure::field("type_key", TYPE_INVALID))?;
            (title, group, new.type_key.clone(), 0)
        }
    };
    // Gantikan usulan pending lama untuk dokumen yang sama.
    let superseded = knowledge_proposals::Entity::update_many()
        .col_expr(
            knowledge_proposals::Column::Status,
            sea_orm::sea_query::Expr::value(ProposalStatus::Superseded.to_value()),
        )
        .col_expr(
            knowledge_proposals::Column::UpdatedAt,
            sea_orm::sea_query::Expr::value(now()),
        )
        .filter(knowledge_proposals::Column::WorkspaceId.eq(wid.get()))
        .filter(knowledge_proposals::Column::ProjectId.eq(project.id))
        .filter(knowledge_proposals::Column::Slug.eq(slug.clone()))
        .filter(knowledge_proposals::Column::Status.eq(ProposalStatus::Pending.to_value()));
    superseded.exec(&txn).await?;
    let reason: String = new.reason.trim().chars().take(2000).collect();
    let proposal = knowledge_proposals::ActiveModel {
        workspace_id: Set(wid.get()),
        project_id: Set(project.id),
        doc_id: Set(doc.as_ref().map(|d| d.id)),
        slug: Set(slug),
        title: Set(title),
        group_key: Set(group_key),
        type_key: Set(type_key),
        base_version: Set(base_version),
        body: Set(body),
        reason: Set(reason),
        trigger: Set(new.trigger.to_value()),
        ticket_id: Set(new.ticket_id),
        trigger_ref: Set(new.trigger_ref.chars().take(200).collect()),
        run_id: Set(new.run_id),
        persona: Set(new.persona.clone()),
        tokens: Set(new.tokens),
        sources: Set(serde_json::to_value(&new.sources).unwrap_or_default()),
        status: Set(ProposalStatus::Pending.to_value()),
        decision_note: Set(String::new()),
        created_by: Set(new.created_by),
        ..Default::default()
    }
    .insert(&txn)
    .await?;
    if let Some(run) = new.run_id {
        crate::models::knowledge_runs::Entity::update_many()
            .col_expr(
                crate::models::knowledge_runs::Column::Proposals,
                sea_orm::sea_query::Expr::cust("proposals + 1"),
            )
            .filter(crate::models::knowledge_runs::Column::WorkspaceId.eq(wid.get()))
            .filter(crate::models::knowledge_runs::Column::Id.eq(run))
            .exec(&txn)
            .await?;
    }
    if let Some(owner) = project.knowledge_owner_id
        && Some(owner) != new.created_by
    {
        notifications::ActiveModel::notify(
            &txn,
            wid,
            NewNotification {
                user_id: owner,
                actor_id: new.created_by,
                kind: NotificationKind::KnowledgeProposal,
                ticket_id: None,
                comment_id: None,
                project_id: Some(project.id),
            },
        )
        .await?;
    }
    txn.commit().await?;
    Ok(proposal)
}

/// Usulan terkunci + project-nya, dengan pelaku boleh memutuskan.
async fn lock_for_decision<C: ConnectionTrait>(
    db: &C,
    acting: &Acting<'_>,
    id: Uuid,
) -> Result<(knowledge_proposals::Model, projects::Model), Failure> {
    let wid = acting.workspace_id();
    let proposal = knowledge_proposals::Entity::find_by_id(id)
        .filter(knowledge_proposals::Column::WorkspaceId.eq(wid.get()))
        .lock_exclusive()
        .one(db)
        .await?
        .ok_or(Failure::NotFound)?;
    super::super::tickets::access_in(
        db,
        acting,
        proposal.project_id,
        crate::models::permission::Permission::ViewProject,
    )
    .await?;
    let project = projects::Model::find_in(db, wid, proposal.project_id)
        .await?
        .ok_or(Failure::NotFound)?;
    if !can_approve(db, acting, &project).await? {
        return Err(Failure::field("proposal", NOT_OWNER));
    }
    Ok((proposal, project))
}

/// Setujui usulan: dokumen baru dibuat atau versi baru ditambahkan. `edited` = isi yang sudah
/// diedit penyetuju ("Edit dulu"), dengan `edited_base` = versi dokumen saat diedit.
///
/// Dokumen yang berubah sejak usulan dibuat → `knowledge-conflict` (kecuali isi sudah diedit dari
/// versi terbaru).
///
/// # Errors
///
/// `knowledge-not-owner`, `knowledge-not-pending`, `knowledge-conflict`, `NotFound`, error database.
pub async fn approve(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
    edited: Option<(&str, i32)>,
) -> Result<(knowledge_proposals::Model, knowledge_docs::Model), Failure> {
    let wid = acting.workspace_id();
    let txn = db.begin().await?;
    let (proposal, project) = lock_for_decision(&txn, acting, id).await?;
    if !proposal.is_pending() {
        return Err(Failure::field("proposal", NOT_PENDING));
    }
    let body = edited.map_or(proposal.body.as_str(), |(b, _)| b);
    let message = if proposal.reason.is_empty() {
        format!("Usulan {}", trigger_label(proposal.trigger()))
    } else {
        proposal.reason.clone()
    };
    let existing = knowledge_docs::Model::by_slug(&txn, wid, Some(project.id), &proposal.slug)
        .await?
        .filter(|d| d.archived_at.is_none());
    let doc = match existing {
        Some(d) => {
            let doc = docs::lock(&txn, wid, d.id).await?;
            let base = edited.map_or(proposal.base_version, |(_, v)| v);
            if doc.version != base {
                return Err(Failure::field("body", CONFLICT));
            }
            docs::apply(
                &txn,
                wid,
                doc,
                &docs::Change {
                    title: &proposal.title,
                    body,
                    message: &message,
                    source: VersionSource::Proposal,
                    author: Some(acting.user.id),
                    persona: &proposal.persona,
                    proposal_id: Some(proposal.id),
                },
            )
            .await?
        }
        None => {
            let doc = docs::insert(
                &txn,
                wid,
                Some(acting.user.id),
                &docs::NewDoc {
                    project_id: Some(project.id),
                    title: proposal.title.clone(),
                    type_key: proposal.type_key.clone(),
                    body: body.to_string(),
                    slug: proposal.slug.clone(),
                },
                VersionSource::Proposal,
                Some(proposal.id),
                &proposal.persona,
            )
            .await?;
            let mut m = doc.into_active_model();
            m.owner_id = Set(project.knowledge_owner_id);
            m.update(&txn).await?
        }
    };
    let sources: Vec<docs::SourceRef> =
        serde_json::from_value(proposal.sources.clone()).unwrap_or_default();
    if !sources.is_empty() {
        docs::set_sources(&txn, wid, &doc, &sources).await?;
    }
    let mut m = proposal.into_active_model();
    m.status = Set(ProposalStatus::Approved.to_value());
    m.doc_id = Set(Some(doc.id));
    m.decided_by = Set(Some(acting.user.id));
    m.decided_at = Set(Some(now()));
    let proposal = m.update(&txn).await?;
    txn.commit().await?;
    Ok((proposal, doc))
}

/// Tolak usulan dengan alasan (wajib); pembuat usulan mendapat notifikasi.
///
/// # Errors
///
/// `knowledge-note-required`, `knowledge-not-owner`, `knowledge-not-pending`, `NotFound`, error
/// database.
pub async fn reject(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
    note: &str,
) -> Result<knowledge_proposals::Model, Failure> {
    let note = note.trim();
    if note.is_empty() {
        return Err(Failure::field("note", NOTE_REQUIRED));
    }
    let wid = acting.workspace_id();
    let txn = db.begin().await?;
    let (proposal, project) = lock_for_decision(&txn, acting, id).await?;
    if !proposal.is_pending() {
        return Err(Failure::field("proposal", NOT_PENDING));
    }
    let author = proposal.created_by;
    let mut m = proposal.into_active_model();
    m.status = Set(ProposalStatus::Rejected.to_value());
    m.decided_by = Set(Some(acting.user.id));
    m.decided_at = Set(Some(now()));
    m.decision_note = Set(note.chars().take(1000).collect());
    let proposal = m.update(&txn).await?;
    if let Some(user) = author.filter(|u| *u != acting.user.id) {
        notifications::ActiveModel::notify(
            &txn,
            wid,
            NewNotification {
                user_id: user,
                actor_id: Some(acting.user.id),
                kind: NotificationKind::KnowledgeRejected,
                ticket_id: None,
                comment_id: None,
                project_id: Some(project.id),
            },
        )
        .await?;
    }
    txn.commit().await?;
    Ok(proposal)
}

/// Batalkan keputusan: usulan kembali pending. Approve hanya bisa dibatalkan selama versi yang
/// dibuatnya masih versi terbaru dan belum ada usulan pending lain untuk dokumen itu.
///
/// # Errors
///
/// `knowledge-undo-blocked`, `knowledge-not-owner`, `NotFound`, error database.
pub async fn undo(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
) -> Result<knowledge_proposals::Model, Failure> {
    let wid = acting.workspace_id();
    let txn = db.begin().await?;
    let (proposal, project) = lock_for_decision(&txn, acting, id).await?;
    match proposal.status() {
        ProposalStatus::Rejected => {}
        ProposalStatus::Approved => {
            let other_pending = knowledge_proposals::Entity::find()
                .filter(knowledge_proposals::Column::WorkspaceId.eq(wid.get()))
                .filter(knowledge_proposals::Column::ProjectId.eq(project.id))
                .filter(knowledge_proposals::Column::Slug.eq(proposal.slug.clone()))
                .filter(knowledge_proposals::Column::Status.eq(ProposalStatus::Pending.to_value()))
                .one(&txn)
                .await?
                .is_some();
            let doc_id = proposal
                .doc_id
                .ok_or_else(|| Failure::field("proposal", UNDO_BLOCKED))?;
            let doc = docs::lock(&txn, wid, doc_id).await?;
            let latest = knowledge_versions::Model::number(&txn, wid, doc.id, doc.version)
                .await?
                .filter(|v| v.proposal_id == Some(proposal.id));
            if other_pending || latest.is_none() {
                return Err(Failure::field("proposal", UNDO_BLOCKED));
            }
            knowledge_versions::Entity::delete_many()
                .filter(knowledge_versions::Column::WorkspaceId.eq(wid.get()))
                .filter(knowledge_versions::Column::DocId.eq(doc.id))
                .filter(knowledge_versions::Column::Number.eq(doc.version))
                .exec(&txn)
                .await?;
            if doc.version == 1 {
                // Usulan dokumen baru: lepas dulu dari dokumen (FK cascade), lalu hapus dokumen.
                knowledge_proposals::Entity::update_many()
                    .col_expr(
                        knowledge_proposals::Column::DocId,
                        sea_orm::sea_query::Expr::value(Option::<Uuid>::None),
                    )
                    .filter(knowledge_proposals::Column::DocId.eq(doc.id))
                    .exec(&txn)
                    .await?;
                knowledge_docs::Entity::delete_by_id(doc.id)
                    .exec(&txn)
                    .await?;
            } else {
                let prev = knowledge_versions::Model::number(&txn, wid, doc.id, doc.version - 1)
                    .await?
                    .ok_or_else(|| Failure::field("proposal", UNDO_BLOCKED))?;
                let mut m = doc.into_active_model();
                m.title = Set(prev.title);
                m.body = Set(prev.body);
                m.version = Set(prev.number);
                m.update(&txn).await?;
            }
        }
        _ => return Err(Failure::field("proposal", UNDO_BLOCKED)),
    }
    let new_doc = proposal.base_version == 0;
    let mut m = proposal.into_active_model();
    if new_doc {
        m.doc_id = Set(None);
    }
    m.status = Set(ProposalStatus::Pending.to_value());
    m.decided_by = Set(None);
    m.decided_at = Set(None);
    m.decision_note = Set(String::new());
    let proposal = m.update(&txn).await?;
    txn.commit().await?;
    Ok(proposal)
}

/// Usulan pending project (antrean, terlama dulu) dan yang baru diputuskan.
///
/// # Errors
///
/// Error database.
pub async fn queue<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project_id: Uuid,
) -> Result<
    (
        Vec<knowledge_proposals::Model>,
        Vec<knowledge_proposals::Model>,
    ),
    sea_orm::DbErr,
> {
    let pending =
        knowledge_proposals::Model::of_project(db, wid, project_id, ProposalStatus::Pending, 200)
            .await?;
    let decided = knowledge_proposals::Entity::find()
        .filter(knowledge_proposals::Column::WorkspaceId.eq(wid.get()))
        .filter(knowledge_proposals::Column::ProjectId.eq(project_id))
        .filter(knowledge_proposals::Column::Status.is_in([
            ProposalStatus::Approved.to_value(),
            ProposalStatus::Rejected.to_value(),
        ]))
        .order_by_desc(knowledge_proposals::Column::DecidedAt)
        .limit(10)
        .all(db)
        .await?;
    Ok((pending, decided))
}

/// Jumlah usulan pending project.
///
/// # Errors
///
/// Error database.
pub async fn pending_count<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project_id: Uuid,
) -> Result<u64, sea_orm::DbErr> {
    use sea_orm::PaginatorTrait;
    knowledge_proposals::Entity::find()
        .filter(knowledge_proposals::Column::WorkspaceId.eq(wid.get()))
        .filter(knowledge_proposals::Column::ProjectId.eq(project_id))
        .filter(knowledge_proposals::Column::Status.eq(ProposalStatus::Pending.to_value()))
        .count(db)
        .await
}

/// Label pemicu untuk tampilan dan pesan versi.
#[must_use]
pub fn trigger_label(t: ProposalTrigger) -> &'static str {
    match t {
        ProposalTrigger::Scan => "scan",
        ProposalTrigger::Done => "tiket Done",
        ProposalTrigger::Merge => "merge",
        ProposalTrigger::Audit => "audit mingguan",
        ProposalTrigger::Manual => "manual",
        ProposalTrigger::Init => "aturan lama repo",
        ProposalTrigger::Repo => "perubahan di repo",
    }
}

/// Satu baris diff berdampingan.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SideRow {
    /// `same`, `del`, `add`, `mod` (berubah), atau `gap` (sisi kosong).
    pub left_kind: &'static str,
    pub left: String,
    pub right_kind: &'static str,
    pub right: String,
}

/// Diff berdampingan: baris yang dihapus dipasangkan dengan baris tambahan sesudahnya.
#[must_use]
pub fn side_by_side(old: &str, new: &str) -> Vec<SideRow> {
    let lines = crate::services::tickets::artifacts::diff(old, new);
    let mut out = Vec::new();
    let mut dels: Vec<String> = Vec::new();
    let mut adds: Vec<String> = Vec::new();
    let flush = |dels: &mut Vec<String>, adds: &mut Vec<String>, out: &mut Vec<SideRow>| {
        let n = dels.len().max(adds.len());
        for i in 0..n {
            let l = dels.get(i);
            let r = adds.get(i);
            out.push(SideRow {
                left_kind: if l.is_some() { "del" } else { "gap" },
                left: l.cloned().unwrap_or_default(),
                right_kind: if r.is_some() { "add" } else { "gap" },
                right: r.cloned().unwrap_or_default(),
            });
        }
        dels.clear();
        adds.clear();
    };
    for DiffLine { kind, text } in lines {
        match kind {
            "del" => {
                if !adds.is_empty() {
                    flush(&mut dels, &mut adds, &mut out);
                }
                dels.push(text);
            }
            "add" => adds.push(text),
            _ => {
                flush(&mut dels, &mut adds, &mut out);
                out.push(SideRow {
                    left_kind: "same",
                    left: text.clone(),
                    right_kind: "same",
                    right: text,
                });
            }
        }
    }
    flush(&mut dels, &mut adds, &mut out);
    out
}

/// Ringkasan diff: (+tambah, −hapus).
#[must_use]
pub fn stat(old: &str, new: &str) -> (usize, usize) {
    let lines = crate::services::tickets::artifacts::diff(old, new);
    (
        lines.iter().filter(|l| l.kind == "add").count(),
        lines.iter().filter(|l| l.kind == "del").count(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn side_by_side_pairs_changes() {
        let rows = side_by_side("a\nb\nc\n", "a\nB\nc\nd\n");
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].left_kind, "same");
        assert_eq!((rows[1].left_kind, rows[1].right_kind), ("del", "add"));
        assert_eq!((rows[1].left.as_str(), rows[1].right.as_str()), ("b", "B"));
        assert_eq!((rows[3].left_kind, rows[3].right_kind), ("gap", "add"));
        assert_eq!(stat("a\nb\n", "a\nc\nd\n"), (2, 1));
    }
}
