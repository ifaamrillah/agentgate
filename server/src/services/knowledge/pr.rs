//! PR knowledge (R2): setelah usulan di-approve, server menulis dokumen ke repo lewat provider
//! git dan membuka PR. Tidak pernah push ke branch utama.
//!
//! - Repo knowledge = repo project pertama yang tertaut provider (urut dibuat).
//! - Branch `agentgate/knowledge-<slug>-v<versi>`; selama PR dokumen itu masih terbuka, versi
//!   berikutnya ditambahkan sebagai commit baru di branch PR yang sama.
//! - File `.agentgate/knowledge/<slug>.md` dengan format yang sama dengan `agentgate knowledge
//!   pull` (`agentgate_core::knowledge`).
//! - Status PR dilacak lewat `pull_requests` (diperbarui webhook PR seperti PR lain).
//!
//! `agentgate knowledge sync` dari laptop tetap jalan dan tidak bergantung pada ini.
use agentgate_core::{git::PrState, knowledge as kfile};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr,
    EntityTrait, IntoActiveModel, QueryFilter, QueryOrder,
};
use uuid::Uuid;

use crate::{
    common::settings::Settings,
    models::{
        knowledge_docs,
        knowledge_prs::{self, KnowledgePrStatus},
        pull_requests, repos,
        workspaces::WorkspaceId,
    },
    services::git::{
        self,
        provider::{NewPull, PutFile},
        provider_code, pulls,
    },
};

/// Awalan semua branch PR knowledge.
pub const BRANCH_PREFIX: &str = "agentgate/knowledge-";
/// Branch tidak aman (bukan awalan knowledge, atau sama dengan branch utama).
pub const UNSAFE_BRANCH: &str = "git-unsafe-branch";

/// Nama branch PR knowledge untuk dokumen + versi.
#[must_use]
pub fn branch_for(slug: &str, version: i32) -> String {
    let slug: String = slug
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .take(60)
        .collect();
    format!("{BRANCH_PREFIX}{slug}-v{version}")
}

/// Branch boleh ditulis server: berawalan knowledge dan bukan branch utama repo.
#[must_use]
pub fn safe_branch(branch: &str, default_branch: &str) -> bool {
    branch.starts_with(BRANCH_PREFIX)
        && branch.len() > BRANCH_PREFIX.len()
        && branch != default_branch
        && !branch.contains("..")
}

/// Repo knowledge project: repo pertama yang tertaut provider.
///
/// # Errors
///
/// Error database.
pub async fn knowledge_repo<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project: Uuid,
) -> Result<Option<repos::Model>, DbErr> {
    Ok(repos::Model::of_project(db, wid, project)
        .await?
        .into_iter()
        .filter(|r| r.connection_id.is_some() && !r.full_name.is_empty())
        .min_by_key(|r| r.created_at))
}

/// Catat PR knowledge untuk versi dokumen yang baru di-approve. Project tanpa repo tertaut →
/// baris `skipped` (tidak ada PR; sync dari laptop tetap bisa) dan `None`.
///
/// # Errors
///
/// Error database.
pub async fn queue<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    doc: &knowledge_docs::Model,
    proposal: Option<Uuid>,
) -> Result<Option<knowledge_prs::Model>, DbErr> {
    let Some(project) = doc.project_id else {
        return Ok(None);
    };
    let repo = knowledge_repo(db, wid, project).await?;
    let row = knowledge_prs::ActiveModel {
        workspace_id: Set(wid.get()),
        project_id: Set(project),
        doc_id: Set(doc.id),
        proposal_id: Set(proposal),
        repo_id: Set(repo.as_ref().map(|r| r.id)),
        version: Set(doc.version),
        status: Set(if repo.is_some() {
            KnowledgePrStatus::Queued
        } else {
            KnowledgePrStatus::Skipped
        }
        .to_value()),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(repo.map(|_| row))
}

use sea_orm::ActiveEnum;

/// PR knowledge dokumen yang masih terbuka di repo (untuk menambah commit ke branch-nya).
async fn open_pr_of_doc(
    db: &DatabaseConnection,
    row: &knowledge_prs::Model,
    repo: Uuid,
) -> Result<Option<(String, pull_requests::Model)>, DbErr> {
    let prev = knowledge_prs::Entity::find()
        .filter(knowledge_prs::Column::DocId.eq(row.doc_id))
        .filter(knowledge_prs::Column::RepoId.eq(repo))
        .filter(knowledge_prs::Column::Id.ne(row.id))
        .filter(knowledge_prs::Column::PullRequestId.is_not_null())
        .order_by_desc(knowledge_prs::Column::CreatedAt)
        .all(db)
        .await?;
    for p in prev {
        let Some(pr_id) = p.pull_request_id else {
            continue;
        };
        if let Some(pr) = pull_requests::Entity::find_by_id(pr_id).one(db).await?
            && matches!(pr.state(), PrState::Open | PrState::Draft)
        {
            return Ok(Some((p.branch, pr)));
        }
    }
    Ok(None)
}

async fn finish(
    db: &DatabaseConnection,
    row: knowledge_prs::Model,
    status: KnowledgePrStatus,
    branch: &str,
    pr: Option<Uuid>,
    error: &str,
) -> Result<knowledge_prs::Model, DbErr> {
    let mut m = row.into_active_model();
    m.status = Set(status.to_value());
    m.branch = Set(branch.to_string());
    m.pull_request_id = Set(pr);
    m.error = Set(error.chars().take(200).collect());
    m.update(db).await
}

/// Jalankan PR knowledge yang antre (worker). Kegagalan provider dicatat di baris (`failed` +
/// kode), bukan error.
///
/// # Errors
///
/// Error database.
pub async fn run(
    db: &DatabaseConnection,
    settings: &Settings,
    wid: WorkspaceId,
    id: Uuid,
) -> Result<Option<knowledge_prs::Model>, DbErr> {
    let Some(row) = knowledge_prs::Entity::find_by_id(id)
        .filter(knowledge_prs::Column::WorkspaceId.eq(wid.get()))
        .one(db)
        .await?
    else {
        return Ok(None);
    };
    if row.status() != KnowledgePrStatus::Queued {
        return Ok(Some(row));
    }
    let Some(repo) = (match row.repo_id {
        Some(r) => repos::Model::find_in(db, wid, r).await?,
        None => None,
    }) else {
        return finish(
            db,
            row,
            KnowledgePrStatus::Failed,
            "",
            None,
            git::NOT_CONNECTED,
        )
        .await
        .map(Some);
    };
    let Some(doc) = knowledge_docs::Model::find_in(db, wid, row.doc_id).await? else {
        return Ok(Some(row));
    };
    let Some(conn) = pulls::connection_of(db, wid, &repo).await? else {
        return finish(
            db,
            row,
            KnowledgePrStatus::Failed,
            "",
            None,
            git::NOT_CONNECTED,
        )
        .await
        .map(Some);
    };
    let provider = match git::provider_for(&conn, settings).await {
        Ok(p) => p,
        Err(_) => {
            return finish(
                db,
                row,
                KnowledgePrStatus::Failed,
                "",
                None,
                git::TOKEN_INVALID,
            )
            .await
            .map(Some);
        }
    };
    let existing = open_pr_of_doc(db, &row, repo.id).await?;
    let branch = existing
        .as_ref()
        .map_or_else(|| branch_for(&doc.slug, doc.version), |(b, _)| b.clone());
    if !safe_branch(&branch, &repo.default_branch) {
        return finish(
            db,
            row,
            KnowledgePrStatus::Failed,
            &branch,
            None,
            UNSAFE_BRANCH,
        )
        .await
        .map(Some);
    }
    let full = repo.full_name.clone();
    let result: Result<pull_requests::Model, &'static str> = async {
        if existing.is_none() {
            let base = provider
                .branch_head(&full, &repo.default_branch)
                .await
                .map_err(|e| provider_code(&e))?;
            match provider
                .create_ref(&full, &format!("refs/heads/{branch}"), &base)
                .await
            {
                Ok(()) => {}
                // Branch tersisa dari run sebelumnya (mis. worker diulang): tulis di atasnya.
                Err(git::provider::ProviderError::Rejected(m)) if m == "already-exists" => {}
                Err(e) => return Err(provider_code(&e)),
            }
        }
        let path = kfile::path(&doc.slug);
        let sha = provider
            .file_sha(&full, &path, &branch)
            .await
            .map_err(|e| provider_code(&e))?;
        let group = doc.group_key.clone();
        provider
            .put_file(
                &full,
                &PutFile {
                    path,
                    branch: branch.clone(),
                    message: format!(
                        "docs(knowledge): perbarui {} (v{})",
                        doc.title.replace('\n', " "),
                        doc.version
                    ),
                    content: kfile::render(&doc.title, &doc.type_key, &group, doc.version, &doc.body),
                    sha,
                },
            )
            .await
            .map_err(|e| provider_code(&e))?;
        if let Some((_, pr)) = &existing {
            return Ok(pr.clone());
        }
        let info = provider
            .create_pull(
                &full,
                &NewPull {
                    head: branch.clone(),
                    base: repo.default_branch.clone(),
                    title: format!("docs(knowledge): {} v{}", doc.title.replace('\n', " "), doc.version),
                    body: format!(
                        "Dokumen knowledge **{}** diperbarui di Agent Gate (versi {}) setelah usulan \
                         disetujui. PR ini menyalin isinya ke `{}` supaya repo tetap menjadi sumber \
                         kebenaran.\n\nDibuat otomatis oleh Agent Gate. Perubahan berikutnya pada \
                         dokumen yang sama ditambahkan ke PR ini selama masih terbuka.",
                        doc.title,
                        doc.version,
                        kfile::path(&doc.slug)
                    ),
                },
            )
            .await
            .map_err(|e| provider_code(&e))?;
        let (pr, _) = pulls::upsert(db, wid, &repo, &info, None, None)
            .await
            .map_err(|_| git::UNREACHABLE)?;
        Ok(pr)
    }
    .await;
    match result {
        Ok(pr) => finish(db, row, KnowledgePrStatus::Open, &branch, Some(pr.id), "")
            .await
            .map(Some),
        Err(code) => {
            tracing::warn!(code, doc = %doc.id, "knowledge PR failed");
            finish(db, row, KnowledgePrStatus::Failed, &branch, None, code)
                .await
                .map(Some)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branches_are_prefixed_and_never_the_default_branch() {
        assert_eq!(
            branch_for("aturan-bisnis", 3),
            "agentgate/knowledge-aturan-bisnis-v3"
        );
        assert_eq!(branch_for("a b/c", 1), "agentgate/knowledge-a-b-c-v1");
        assert!(safe_branch("agentgate/knowledge-x-v1", "main"));
        assert!(!safe_branch("main", "main"));
        assert!(!safe_branch("agentgate/knowledge-", "main"));
        assert!(!safe_branch(
            "agentgate/knowledge-x",
            "agentgate/knowledge-x"
        ));
        assert!(!safe_branch("feature/x", "main"));
    }
}
