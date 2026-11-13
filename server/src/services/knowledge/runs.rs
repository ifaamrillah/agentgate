//! Run knowledge: scan project, update dokumen tertentu, atau audit. Setiap run = satu job
//! `knowledge` (tanpa tiket) di runner. Agent membaca code graph + kode dan menulis hasilnya
//! lewat MCP `ag_knowledge propose`, jadi semua hasil masuk sebagai usulan.

use agentgate_core::runner::protocol::{JobKind, JobMode, JobStatus};
use sea_orm::{
    ActiveEnum, ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr,
    EntityTrait, IntoActiveModel, QueryFilter, QueryOrder, Set, TransactionTrait,
};
use uuid::Uuid;

use super::{Failure, RUN_OPEN, catalog, now};
use crate::{
    models::{
        graph_repos, jobs, knowledge_catalog, knowledge_docs,
        knowledge_runs::{self, Depth, RunKind, RunStatus, RunTrigger},
        notifications::{self, NewNotification, NotificationKind},
        permission::Permission,
        projects,
        workspaces::WorkspaceId,
    },
    services::{tickets::access_in, workspaces::Acting},
};

/// Persona bawaan yang menjalankan job knowledge (Tech Writer).
pub const PERSONA: &str = "writer";

/// Estimasi scan dari ukuran code graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct Estimate {
    pub depth: &'static str,
    pub files: i64,
    pub symbols: i64,
    /// Perkiraan jumlah dokumen yang dihasilkan.
    pub docs: i64,
    pub tokens: i64,
    pub minutes: i64,
}

/// Label kedalaman (tampilan).
#[must_use]
pub fn depth_label(d: Depth) -> &'static str {
    match d {
        Depth::Cepat => "Cepat",
        Depth::Standar => "Standar",
        Depth::Mendalam => "Mendalam",
    }
}

/// Estimasi untuk satu kedalaman dari jumlah file, simbol, dan dokumen relevan.
#[must_use]
pub fn estimate_for(
    depth: Depth,
    files: i64,
    symbols: i64,
    relevant: i64,
    modules: i64,
) -> Estimate {
    let (per_file, per_symbol, base, docs) = match depth {
        Depth::Cepat => (60, 2, 15_000, relevant.min(8)),
        Depth::Standar => (180, 6, 40_000, relevant),
        Depth::Mendalam => (420, 14, 80_000, relevant + modules),
    };
    let tokens = base + files * per_file + symbols * per_symbol + docs * 3_000;
    Estimate {
        depth: match depth {
            Depth::Cepat => "cepat",
            Depth::Standar => "standar",
            Depth::Mendalam => "mendalam",
        },
        files,
        symbols,
        docs,
        tokens,
        minutes: (tokens / 30_000).max(2),
    }
}

/// Estimasi tiga kedalaman untuk project.
///
/// # Errors
///
/// Error database.
pub async fn estimates<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project_id: Uuid,
) -> Result<Vec<Estimate>, DbErr> {
    let graphs = graph_repos::Model::of_project(db, wid, project_id).await?;
    let files: i64 = graphs.iter().map(|g| i64::from(g.files_indexed)).sum();
    let symbols: i64 = graphs.iter().map(|g| i64::from(g.symbols)).sum();
    let modules: i64 = graphs
        .iter()
        .map(|g| i64::try_from(g.modules().len()).unwrap_or(0))
        .sum();
    let relevant =
        i64::try_from(catalog::relevant_types(db, wid, project_id).await?.len()).unwrap_or(0);
    Ok([Depth::Cepat, Depth::Standar, Depth::Mendalam]
        .into_iter()
        .map(|d| estimate_for(d, files, symbols, relevant, modules.min(20)))
        .collect())
}

/// Permintaan run dari UI.
#[derive(Debug, Clone)]
pub struct RunRequest {
    pub kind: RunKind,
    pub depth: Depth,
    /// Kosong = seluruh project; selain itu nama repo atau folder.
    pub scope: String,
    pub doc_ids: Vec<Uuid>,
    /// Runner pelaku yang langsung mengambil job (opsional).
    pub runner: Option<Uuid>,
}

/// Run terbuka (antre/berjalan) project dengan jenis `kind`.
///
/// # Errors
///
/// Error database.
pub async fn open_run<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project_id: Uuid,
    kind: Option<RunKind>,
) -> Result<Option<knowledge_runs::Model>, DbErr> {
    let q = knowledge_runs::Entity::find()
        .filter(knowledge_runs::Column::WorkspaceId.eq(wid.get()))
        .filter(knowledge_runs::Column::ProjectId.eq(project_id))
        .filter(
            knowledge_runs::Column::Status
                .is_in([RunStatus::Queued.to_value(), RunStatus::Running.to_value()]),
        );
    let q = match kind {
        Some(k) => q.filter(knowledge_runs::Column::Kind.eq(k.to_value())),
        None => q,
    };
    q.order_by_desc(knowledge_runs::Column::CreatedAt)
        .one(db)
        .await
}

/// Run terbaru project.
///
/// # Errors
///
/// Error database.
pub async fn latest<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project_id: Uuid,
) -> Result<Option<knowledge_runs::Model>, DbErr> {
    knowledge_runs::Entity::find()
        .filter(knowledge_runs::Column::WorkspaceId.eq(wid.get()))
        .filter(knowledge_runs::Column::ProjectId.eq(project_id))
        .order_by_desc(knowledge_runs::Column::CreatedAt)
        .one(db)
        .await
}

fn note_for(kind: RunKind, depth: Depth, docs: usize) -> String {
    match kind {
        RunKind::Scan => format!("Scan knowledge · {}", depth_label(depth)),
        RunKind::Update => format!("Perbarui {docs} dokumen knowledge"),
        RunKind::Audit => "Audit knowledge mingguan".into(),
    }
}

struct NewRun {
    kind: RunKind,
    trigger: RunTrigger,
    depth: Depth,
    scope: String,
    doc_ids: Vec<Uuid>,
    ticket_id: Option<Uuid>,
    trigger_ref: String,
    requested_by: Option<Uuid>,
}

async fn insert_run<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project: &projects::Model,
    new: NewRun,
) -> Result<(knowledge_runs::Model, jobs::Model), DbErr> {
    let estimate = estimates(db, wid, project.id)
        .await?
        .into_iter()
        .find(|e| e.depth == new.depth.to_value())
        .map_or(0, |e| e.tokens);
    let estimate = if new.kind == RunKind::Scan {
        estimate
    } else {
        i64::try_from(new.doc_ids.len()).unwrap_or(1).max(1) * 12_000
    };
    let note = note_for(new.kind, new.depth, new.doc_ids.len());
    let run = knowledge_runs::ActiveModel {
        workspace_id: Set(wid.get()),
        project_id: Set(project.id),
        kind: Set(new.kind.to_value()),
        trigger: Set(new.trigger.to_value()),
        depth: Set(new.depth.to_value()),
        scope: Set(new.scope.chars().take(300).collect()),
        doc_ids: Set(new.doc_ids),
        ticket_id: Set(new.ticket_id),
        trigger_ref: Set(new.trigger_ref.chars().take(200).collect()),
        estimate_tokens: Set(estimate),
        status: Set(RunStatus::Queued.to_value()),
        proposals: Set(0),
        requested_by: Set(new.requested_by),
        ..Default::default()
    }
    .insert(db)
    .await?;
    let job = jobs::ActiveModel {
        workspace_id: Set(wid.get()),
        project_id: Set(project.id),
        ticket_id: Set(None),
        kind: Set(JobKind::Knowledge.as_str().into()),
        mode: Set(JobMode::Auto.as_str().into()),
        stage_key: Set(new.kind.to_value()),
        persona: Set(PERSONA.into()),
        status: Set(JobStatus::Queued.as_str().into()),
        prompt_note: Set(note),
        requested_by: Set(new.requested_by),
        steps: Set(serde_json::json!([])),
        knowledge_run_id: Set(Some(run.id)),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok((run, job))
}

/// Minta run oleh user (`EditProject`). Scan kedua saat scan masih terbuka ditolak.
///
/// # Errors
///
/// `knowledge-run-open`, `NotFound`, error jobs (runner), error database.
pub async fn request(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    project_id: Uuid,
    req: RunRequest,
) -> Result<(knowledge_runs::Model, jobs::Model), Failure> {
    let wid = acting.workspace_id();
    access_in(db, acting, project_id, Permission::EditProject).await?;
    let project = projects::Model::find_in(db, wid, project_id)
        .await?
        .ok_or(Failure::NotFound)?;
    if open_run(db, wid, project.id, Some(req.kind))
        .await?
        .is_some()
    {
        return Err(Failure::field("run", RUN_OPEN));
    }
    let mut doc_ids = Vec::new();
    for id in req.doc_ids {
        if knowledge_docs::Model::find_in(db, wid, id)
            .await?
            .is_some_and(|d| d.project_id == Some(project.id))
            && !doc_ids.contains(&id)
        {
            doc_ids.push(id);
        }
    }
    let txn = db.begin().await?;
    let (run, job) = insert_run(
        &txn,
        wid,
        &project,
        NewRun {
            kind: req.kind,
            trigger: RunTrigger::Manual,
            depth: req.depth,
            scope: req.scope,
            doc_ids,
            ticket_id: None,
            trigger_ref: String::new(),
            requested_by: Some(acting.user.id),
        },
    )
    .await?;
    txn.commit().await?;
    let job = match req.runner {
        Some(r) => crate::services::jobs::claim(db, acting, job.id, Some(r)).await?,
        None => job,
    };
    Ok((run, job))
}

/// Minta run oleh sistem (pemicu Done/merge/audit). Executor = pemilik knowledge project (dapat
/// notifikasi; runner-nya auto-ambil bila diaktifkan). Update yang masih antre digabung.
///
/// # Errors
///
/// Error database.
#[allow(clippy::too_many_arguments)]
pub async fn request_system<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project: &projects::Model,
    kind: RunKind,
    trigger: RunTrigger,
    doc_ids: Vec<Uuid>,
    ticket_id: Option<Uuid>,
    trigger_ref: &str,
) -> Result<Option<knowledge_runs::Model>, DbErr> {
    if let Some(open) = open_run(db, wid, project.id, Some(kind)).await? {
        if open.status() == RunStatus::Queued {
            let mut ids = open.doc_ids.clone();
            for id in doc_ids {
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
            let mut m = open.into_active_model();
            m.doc_ids = Set(ids);
            return Ok(Some(m.update(db).await?));
        }
        return Ok(None);
    }
    let settings = crate::models::knowledge_settings::Model::of(db, wid).await?;
    let depth = Depth::try_from_value(&settings.default_depth).unwrap_or(Depth::Standar);
    let (run, _) = insert_run(
        db,
        wid,
        project,
        NewRun {
            kind,
            trigger,
            depth,
            scope: String::new(),
            doc_ids,
            ticket_id,
            trigger_ref: trigger_ref.to_string(),
            requested_by: project.knowledge_owner_id,
        },
    )
    .await?;
    if let Some(owner) = project.knowledge_owner_id {
        notifications::ActiveModel::notify(
            db,
            wid,
            NewNotification {
                user_id: owner,
                actor_id: None,
                kind: NotificationKind::KnowledgeJob,
                ticket_id: None,
                comment_id: None,
                project_id: Some(project.id),
            },
        )
        .await?;
    }
    Ok(Some(run))
}

/// Run job knowledge.
async fn run_of<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    job: &jobs::Model,
) -> Result<Option<knowledge_runs::Model>, DbErr> {
    match job.knowledge_run_id {
        Some(id) => knowledge_runs::Model::find_in(db, wid, id).await,
        None => Ok(None),
    }
}

/// Prompt job knowledge: tugas, aturan, katalog relevan, dan dokumen yang ada.
///
/// # Errors
///
/// Error database.
pub async fn prompt<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    job: &jobs::Model,
    project: &projects::Model,
) -> Result<String, DbErr> {
    let run = run_of(db, wid, job).await?;
    let existing = knowledge_docs::Model::of_project(db, wid, project.id).await?;
    let mut p = String::new();
    let (kind, depth) = run
        .as_ref()
        .map_or((RunKind::Scan, Depth::Standar), |r| (r.kind(), r.depth()));
    p.push_str(&format!(
        "# Tugas knowledge project {} ({})\n\n",
        project.name, project.key
    ));
    p.push_str(
        "Anda menyusun knowledge base project ini untuk anggota tim dan agent AI. Baca code graph \
         (`ag_graph`) dan kode repo, lalu tulis setiap dokumen sebagai **usulan** lewat tool MCP \
         `ag_knowledge` dengan `action: \"propose\"` (isi Markdown lengkap, `reason` singkat, dan \
         `sources` berisi repo + path yang Anda baca). Jangan mengubah file repo, jangan membuat \
         branch atau PR. Usulan baru tampil setelah disetujui pemilik knowledge.\n\n\
         Aturan isi: bahasa Indonesia yang jelas, fakta dari kode (bukan tebakan), sebut path file \
         penting, diagram memakai blok ```mermaid, tanpa nilai secret.\n\n",
    );
    if let Some(run) = &run {
        p.push_str(&format!(
            "Id run: `{}`  (isi `run` saat propose)\n\n",
            run.id
        ));
    }
    match kind {
        RunKind::Scan => {
            p.push_str(&format!("## Scan · kedalaman {}\n\n", depth_label(depth)));
            p.push_str(match depth {
                Depth::Cepat => "Tulis dokumen inti saja (gambaran umum, arsitektur, setup), masing-masing ringkas.\n\n",
                Depth::Standar => "Tulis semua jenis dokumen relevan di bawah dengan kedalaman wajar.\n\n",
                Depth::Mendalam => "Tulis semua jenis relevan secara rinci, termasuk satu dokumen `modul` per modul/service penting.\n\n",
            });
            if let Some(scope) = run
                .as_ref()
                .map(|r| r.scope.as_str())
                .filter(|s| !s.is_empty())
            {
                p.push_str(&format!("Cakupan: hanya `{scope}`.\n\n"));
            }
            p.push_str("### Jenis dokumen relevan\n\n");
            for t in catalog::relevant_types(db, wid, project.id).await? {
                p.push_str(&format!(
                    "- `{}` · {} ({}): {}\n",
                    t.key, t.name, t.group, t.hint
                ));
                if !t.instructions.is_empty() {
                    p.push_str(&format!("  Instruksi: {}\n", t.instructions.trim()));
                }
            }
            p.push('\n');
        }
        RunKind::Update | RunKind::Audit => {
            let ids = run.as_ref().map(|r| r.doc_ids.clone()).unwrap_or_default();
            let targets: Vec<&knowledge_docs::Model> = if ids.is_empty() {
                existing.iter().collect()
            } else {
                existing.iter().filter(|d| ids.contains(&d.id)).collect()
            };
            if kind == RunKind::Audit {
                p.push_str("## Audit mingguan\n\nPeriksa setiap dokumen terhadap kode sekarang. Usulkan perbaikan hanya untuk dokumen yang sudah tidak sesuai.\n\n");
            } else {
                p.push_str("## Perbarui dokumen\n\nBaca dokumen (`ag_knowledge read`) lalu kode terkait; usulkan versi baru bila isinya sudah tidak sesuai.\n\n");
            }
            if let Some(r) = run.as_ref().filter(|r| !r.trigger_ref.is_empty()) {
                p.push_str(&format!("Pemicu: {}\n\n", r.trigger_ref));
            }
            for d in targets {
                p.push_str(&format!("- `{}` · {} (v{})", d.slug, d.title, d.version));
                if d.is_stale() {
                    p.push_str(&format!(" · basi: {}", d.stale_reason));
                }
                p.push('\n');
            }
            p.push('\n');
        }
    }
    if !existing.is_empty() && kind == RunKind::Scan {
        p.push_str("### Dokumen yang sudah ada (perbarui lewat slug yang sama)\n\n");
        for d in &existing {
            p.push_str(&format!(
                "- `{}` · {} (`{}`)\n",
                d.slug, d.title, d.type_key
            ));
        }
        p.push('\n');
    }
    p.push_str("Selesai bila semua usulan sudah dikirim; tulis ringkasan singkat jumlah usulan.\n");
    Ok(p)
}

/// Runner mulai menjalankan job knowledge.
///
/// # Errors
///
/// Error database.
pub async fn job_started<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    job: &jobs::Model,
) -> Result<(), DbErr> {
    if let Some(run) = run_of(db, wid, job).await?.filter(|r| r.is_open()) {
        let mut m = run.into_active_model();
        m.status = Set(RunStatus::Running.to_value());
        m.update(db).await?;
    }
    Ok(())
}

/// Job knowledge selesai (sukses/gagal/batal): tutup run.
///
/// # Errors
///
/// Error database.
pub async fn job_finished<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    job: &jobs::Model,
) -> Result<(), DbErr> {
    let Some(run) = run_of(db, wid, job).await?.filter(|r| r.is_open()) else {
        return Ok(());
    };
    let status = match job.status() {
        JobStatus::Succeeded => RunStatus::Done,
        JobStatus::Cancelled => RunStatus::Cancelled,
        _ => RunStatus::Failed,
    };
    let mut m = run.into_active_model();
    m.status = Set(status.to_value());
    m.finished_at = Set(Some(now()));
    m.update(db).await?;
    Ok(())
}

/// Tipe relevan untuk tampilan dialog scan: (kelompok, jumlah jenis).
#[must_use]
pub fn group_counts(
    types: &[catalog::TypeInfo],
) -> Vec<(&'static knowledge_catalog::Group, usize)> {
    knowledge_catalog::GROUPS
        .iter()
        .map(|g| (g, types.iter().filter(|t| t.group == g.key).count()))
        .filter(|(_, n)| *n > 0)
        .collect()
}
