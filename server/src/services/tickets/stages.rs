//! Gate engine: semua perpindahan stage tiket (spec "Workflow, stage & gate").
//!
//! **Aturan.** Stage aturan (gate, transisi keluar, batas gagal) dibaca dari versi workflow
//! saat tiket masuk stage itu (`tickets.stage_version`): mengubah workflow tidak mengubah aturan
//! stage yang sedang berjalan ("gate versi baru berlaku sejak tiket masuk stage berikutnya").
//! Urutan dan daftar stage tujuan dibaca dari versi aktif.
//!
//! - **Maju** (board, tombol "Pindah ke …", ubah status): stage sekarang harus `active`, transisi
//!   keluarnya bukan "tunggu approve", semua gate-nya terpenuhi, dan stage yang dilewati tidak
//!   menegakkan apa pun (`Stage::enforces`). Selain itu = lompat stage (Admin).
//! - **Mundur**: boleh untuk `EditProject`; gate tidak berlaku ke belakang.
//! - **Minta review** (transisi tunggu approve): gate dicek, keadaan menjadi `review`, versi
//!   artifact stage ini yang masih draf menjadi `pending`, approver mendapat notifikasi.
//! - **Approve**: satu approver cukup; gate dicek ulang; artifact `pending` → `approved`;
//!   tiket masuk stage berikutnya.
//! - **Reject**: wajib catatan; satu putaran gagal. Tiket kembali ke `fail_to` (mis. Testing →
//!   Development) atau tetap di stage. Putaran gagal mencapai `fail_limit` → `decision`.
//! - **Keputusan** (Butuh keputusan): lima pilihan spec, lihat [`Decision`].
//! - **Lompat stage / paksa transisi**: `AdministerProject`, alasan wajib, audit log.
//! - **Batas WIP** (R3): pindah ke stage yang beban kolomnya (tiket aktif project di stage itu)
//!   sudah mencapai `wip` ditolak (`ticket-wip-full`) di semua jalur pindah kartu (board, menu,
//!   ubah status, "tandai selesai", API, MCP, sinkron Jira). Admin project bisa memaksa dengan
//!   alasan ([`move_over_wip`], audit `ticket.wip_overridden`). Approve, keputusan, dan lompat
//!   Admin tidak dicek: itu alur gate, bukan pindah kartu.
//!
//! Approver = penugasan approver stage sekarang di tiket; stage tanpa approver boleh diputuskan
//! siapa pun yang memegang `EditProject`. Relasi *blocks* tetap ditegakkan di semua jalur
//! ([`super::ensure_can_move`], dinilai dari kategori stage).

use agentgate_core::workflow::{Category, Definition, Exit, Stage};
use sea_orm::{
    ActiveEnum, ActiveModelTrait, ActiveValue::Set, ConnectionTrait, DatabaseConnection, DbErr,
    EntityTrait, IntoActiveModel,
};
use uuid::Uuid;

use super::{
    TicketFailure, Working,
    create::{self, TicketForm},
    ensure_can_move,
    gates::{self, GateCheck},
    notify, watch_if_member,
};
use crate::{
    models::{
        artifact_versions::{self, VersionStatus},
        notifications::NotificationKind,
        permission::Permission,
        project_access::EffectiveRole,
        ticket_artifacts,
        ticket_events::EventKind,
        ticket_links::{self, LinkKind},
        ticket_stage_people::{self, StagePeople},
        ticket_types,
        tickets::{self, StageState, TicketInput, TicketStatus},
        workflow_versions, workflows, workspace_members,
        workspaces::WorkspaceId,
    },
    services::{
        audit::{AuditAction, AuditSubject},
        workspaces::Acting,
    },
};

pub const STAGE_UNKNOWN: &str = "ticket-stage-unknown";
pub const NEEDS_APPROVAL: &str = "ticket-needs-approval";
pub const JUMP_ADMIN_ONLY: &str = "ticket-jump-admin-only";
pub const GATES_UNMET: &str = "ticket-gates-unmet";
pub const STAGE_BUSY: &str = "ticket-stage-busy";
pub const NOT_APPROVER: &str = "ticket-not-approver";
pub const NOT_IN_REVIEW: &str = "ticket-not-in-review";
pub const NOT_IN_DECISION: &str = "ticket-not-in-decision";
pub const NO_APPROVAL_STEP: &str = "ticket-no-approval-step";
pub const LAST_STAGE: &str = "ticket-last-stage";
pub const NOTE_REQUIRED: &str = "ticket-note-required";
pub const NOTE_TOO_LONG: &str = "ticket-note-too-long";
pub const REASON_REQUIRED: &str = "ticket-reason-required";
pub const FORCE_DISABLED: &str = "ticket-force-disabled";
pub const DECISION_INVALID: &str = "ticket-decision-invalid";
pub const DECISION_TARGET_INVALID: &str = "ticket-decision-target-invalid";
pub const PEOPLE_INVALID: &str = "ticket-stage-people-invalid";
/// Kolom tujuan sudah mencapai batas WIP (R3); parameter `stage`, `wip`.
pub const WIP_FULL: &str = "ticket-wip-full";
pub const ERROR_CODES: &[&str] = &[
    STAGE_UNKNOWN,
    NEEDS_APPROVAL,
    JUMP_ADMIN_ONLY,
    GATES_UNMET,
    STAGE_BUSY,
    NOT_APPROVER,
    NOT_IN_REVIEW,
    NOT_IN_DECISION,
    NO_APPROVAL_STEP,
    LAST_STAGE,
    NOTE_REQUIRED,
    NOTE_TOO_LONG,
    REASON_REQUIRED,
    FORCE_DISABLED,
    DECISION_INVALID,
    DECISION_TARGET_INVALID,
    PEOPLE_INVALID,
    WIP_FULL,
];
/// Batas catatan reject/keputusan dan alasan lompat/paksa.
pub const NOTE_MAX_LEN: usize = 2_000;

/// Workflow tiket: versi aktif (urutan stage) dan versi aturan stage sekarang.
#[derive(Debug, Clone)]
pub struct Flow {
    pub workflow: workflows::Model,
    pub current: Definition,
    pub rules: Definition,
}

impl Flow {
    /// # Errors
    ///
    /// Error database; workflow tiket tidak ada → `RecordNotFound`.
    pub async fn load<C: ConnectionTrait>(
        db: &C,
        wid: WorkspaceId,
        ticket: &tickets::Model,
    ) -> Result<Self, DbErr> {
        let workflow = workflows::Model::find_in(db, wid, ticket.workflow_id)
            .await?
            .ok_or_else(|| DbErr::RecordNotFound("workflow".into()))?;
        let current = workflow.definition(db).await?;
        let rules = if ticket.stage_version == workflow.version {
            current.clone()
        } else {
            workflow_versions::Model::definition_of(
                db,
                wid.get(),
                workflow.id,
                ticket.stage_version,
            )
            .await
            .unwrap_or_else(|_| current.clone())
        };
        Ok(Self {
            workflow,
            current,
            rules,
        })
    }

    /// Aturan stage `key`: dari versi tiket masuk, lalu versi aktif.
    #[must_use]
    pub fn rule(&self, key: &str) -> Option<&Stage> {
        self.rules.stage(key).or_else(|| self.current.stage(key))
    }

    /// Posisi stage `key` di versi aktif. Stage yang sudah dihapus (tiket belum dimigrasi)
    /// dianggap di posisi stage aktif pertama yang kategorinya sama atau lebih tinggi.
    #[must_use]
    pub fn position(&self, key: &str) -> usize {
        self.current.index_of(key).unwrap_or_else(|| {
            let category = self.rule(key).map(|s| s.category);
            self.current
                .stages
                .iter()
                .position(|s| Some(s.category) >= category)
                .unwrap_or(0)
        })
    }

    /// Stage berikutnya setelah stage sekarang (versi aktif).
    #[must_use]
    pub fn next(&self, key: &str) -> Option<&Stage> {
        let i = self.position(key);
        let i = if self.current.index_of(key).is_some() {
            i + 1
        } else {
            i
        };
        self.current.stages.get(i)
    }
}

fn invalid(code: &'static str) -> TicketFailure {
    TicketFailure::field("stage", code)
}

fn note_checked(note: &str, required: bool) -> Result<String, TicketFailure> {
    let note = note.trim();
    if required && note.is_empty() {
        return Err(TicketFailure::field("note", NOTE_REQUIRED));
    }
    if note.chars().count() > NOTE_MAX_LEN {
        return Err(TicketFailure::field("note", NOTE_TOO_LONG));
    }
    Ok(note.to_string())
}

/// Ringkasan perpindahan stage untuk riwayat.
fn stage_change(flow: &Flow, from: &str, to: &Stage) -> serde_json::Value {
    serde_json::json!({
        "stage": {
            "from": flow.rule(from).map_or(from, |s| s.name.as_str()),
            "to": to.name,
            "from_key": from,
            "to_key": to.key,
        }
    })
}

fn merge(mut a: serde_json::Value, b: serde_json::Value) -> serde_json::Value {
    if let (Some(a), serde_json::Value::Object(b)) = (a.as_object_mut(), b) {
        a.extend(b);
    }
    a
}

/// Masukkan tiket ke stage `to` (versi aktif): kategori → `status`, keadaan `active`, posisi
/// paling bawah kolom, penugasan default disalin bila stage ini belum punya, watcher stage
/// mengikuti tiket. Hitungan gagal tidak diubah (urusan pemanggil). Tidak menulis riwayat.
///
/// # Errors
///
/// Error database.
pub async fn enter_stage<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    ticket: tickets::Model,
    flow: &Flow,
    to: &Stage,
) -> Result<tickets::Model, DbErr> {
    let status = TicketStatus::parse(to.category.as_str()).unwrap_or(TicketStatus::Todo);
    let was_done = ticket.status == TicketStatus::Done.to_value();
    let ticket = ticket
        .set_status(db, status, tickets::bottom_position())
        .await?;
    let mut model = ticket.into_active_model();
    model.stage_key = Set(to.key.clone());
    model.stage_version = Set(flow.workflow.version);
    model.stage_state = Set(StageState::Active.to_value());
    model.stage_entered_at = Set(chrono::Utc::now().into());
    let ticket = model.update(db).await?;
    seed_people(db, wid, ticket.id, to).await?;
    crate::services::jobs::on_stage_entered(db, wid, &ticket, to).await?;
    if status == TicketStatus::Done && !was_done {
        crate::services::knowledge::triggers::on_ticket_done(db, wid, &ticket).await?;
    }
    if (status == TicketStatus::Done) != was_done {
        crate::services::planning::scope::done_changed(db, wid, &ticket, !was_done).await?;
    }
    Ok(ticket)
}

/// Salin penugasan default stage `to` ke tiket bila stage itu belum punya penugasan, lalu
/// watcher stage mengikuti tiket.
///
/// # Errors
///
/// Error database.
pub async fn seed_people<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    ticket_id: Uuid,
    to: &Stage,
) -> Result<(), DbErr> {
    if !ticket_stage_people::Model::has_stage(db, wid, ticket_id, &to.key).await? {
        let people = members_only(
            db,
            wid,
            StagePeople {
                executors: to.people.executors.clone(),
                approvers: to.people.approvers.clone(),
                watchers: to.people.watchers.clone(),
            },
        )
        .await?;
        if !people.is_empty() {
            ticket_stage_people::ActiveModel::replace(db, wid, ticket_id, &to.key, &people).await?;
        }
    }
    let people = ticket_stage_people::Model::of_stage(db, wid, ticket_id, &to.key).await?;
    for user in &people.watchers {
        watch_if_member(db, wid, ticket_id, *user).await?;
    }
    Ok(())
}

/// Buang user yang bukan (lagi) anggota workspace.
async fn members_only<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    people: StagePeople,
) -> Result<StagePeople, DbErr> {
    let mut out = StagePeople::default();
    for (src, dst) in [
        (people.executors, &mut out.executors),
        (people.approvers, &mut out.approvers),
        (people.watchers, &mut out.watchers),
    ] {
        for id in src {
            if !dst.contains(&id)
                && workspace_members::Model::find_with_role(db, wid, id)
                    .await?
                    .is_some()
            {
                dst.push(id);
            }
        }
    }
    Ok(out)
}

/// Stage pertama untuk tiket baru: `stage` (kunci) bila ada di workflow ini, selain itu stage
/// pertama berkategori `status`, selain itu stage pertama. Membuat tiket langsung di stage yang
/// didahului stage yang menegakkan sesuatu sama dengan lompat stage → ditolak.
///
/// # Errors
///
/// [`TicketFailure::Invalid`] (`stage`).
pub fn initial_stage<'a>(
    definition: &'a Definition,
    stage: &str,
    status: TicketStatus,
) -> Result<&'a Stage, TicketFailure> {
    let by_status = || {
        definition
            .first_in(category(&status).unwrap_or(Category::Todo))
            .or(definition.first())
    };
    // Kunci stage dari kolom board yang tidak ada di workflow tipe ini → menurut status.
    let chosen = match stage.trim() {
        "" => by_status(),
        key => definition.stage(key).or_else(by_status),
    }
    .ok_or_else(|| invalid(STAGE_UNKNOWN))?;
    let i = definition.index_of(&chosen.key).unwrap_or(0);
    if definition.stages[..i].iter().any(Stage::enforces) {
        return Err(invalid(JUMP_ADMIN_ONLY));
    }
    Ok(chosen)
}

/// Gate stage sekarang tiket ini (aturan versi tiket).
///
/// # Errors
///
/// Error database.
pub async fn current_gates<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    ticket: &tickets::Model,
    flow: &Flow,
) -> Result<Vec<GateCheck>, DbErr> {
    match flow.rule(&ticket.stage_key) {
        Some(stage) => gates::check(db, wid, ticket, stage).await,
        None => Ok(Vec::new()),
    }
}

async fn require_gates<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    ticket: &tickets::Model,
    flow: &Flow,
) -> Result<(), TicketFailure> {
    if gates::all_ok(&current_gates(db, wid, ticket, flow).await?) {
        Ok(())
    } else {
        Err(invalid(GATES_UNMET))
    }
}

/// Aturan pindah biasa dari stage sekarang ke `to` (tanpa menulis apa pun).
///
/// # Errors
///
/// [`TicketFailure::Invalid`] dengan kode alasan.
pub async fn check_move<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    ticket: &tickets::Model,
    flow: &Flow,
    to: &Stage,
) -> Result<(), TicketFailure> {
    let from = flow.position(&ticket.stage_key);
    let target = flow
        .current
        .index_of(&to.key)
        .ok_or_else(|| invalid(STAGE_UNKNOWN))?;
    let removed = flow.current.index_of(&ticket.stage_key).is_none();
    if target < from || (target == from && removed) {
        return Ok(());
    }
    if ticket.stage_state()? != StageState::Active {
        return Err(invalid(STAGE_BUSY));
    }
    if removed {
        return Ok(());
    }
    if flow
        .rule(&ticket.stage_key)
        .is_some_and(|s| s.exit == Exit::Approval)
    {
        return Err(invalid(NEEDS_APPROVAL));
    }
    if flow.current.stages[from + 1..target]
        .iter()
        .any(Stage::enforces)
    {
        return Err(invalid(JUMP_ADMIN_ONLY));
    }
    require_gates(db, wid, ticket, flow).await
}

/// Tujuan pindah: kunci stage (board per stage, tombol "Pindah ke …"), atau kategori (jalur
/// status M2: stage pertama berkategori itu; kategori yang sama dengan stage sekarang = tetap).
#[derive(Debug, Clone)]
pub enum Target {
    Stage(String),
    Category(TicketStatus),
}

/// Pindah stage lewat board atau tombol. `before` = kartu yang akan berada di bawah kartu ini
/// (urutan di kolom). Stage yang sama = hanya urutan.
///
/// # Errors
///
/// [`TicketFailure::Invalid`] (kode di modul ini atau `ticket-blocked`), `NotFound`, atau error
/// database.
pub async fn move_to(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    ticket_id: Uuid,
    target: Target,
    before: Option<Uuid>,
) -> Result<tickets::Model, TicketFailure> {
    move_with(db, acting, ticket_id, target, before, None).await
}

/// Pindah walau kolom tujuan sudah mencapai batas WIP (R3): Admin project, alasan wajib. Bila
/// kolom ternyata belum penuh, sama dengan pindah biasa (tanpa audit). Aturan lain (gate,
/// approval, blocks) tetap berlaku.
///
/// # Errors
///
/// [`TicketFailure::Invalid`] (`ticket-reason-required`, kode [`move_to`]), `NotFound` (bukan
/// Admin), atau error database.
pub async fn move_over_wip(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    ticket_id: Uuid,
    stage_key: &str,
    reason: &str,
) -> Result<tickets::Model, TicketFailure> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err(TicketFailure::field("reason", REASON_REQUIRED));
    }
    if reason.chars().count() > NOTE_MAX_LEN {
        return Err(TicketFailure::field("reason", NOTE_TOO_LONG));
    }
    move_with(
        db,
        acting,
        ticket_id,
        Target::Stage(stage_key.trim().to_string()),
        None,
        Some(reason),
    )
    .await
}

async fn move_with(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    ticket_id: Uuid,
    target: Target,
    before: Option<Uuid>,
    wip_reason: Option<&str>,
) -> Result<tickets::Model, TicketFailure> {
    let permission = if wip_reason.is_some() {
        Permission::AdministerProject
    } else {
        Permission::EditProject
    };
    let work = Working::begin(db, acting, ticket_id, permission, false).await?;
    let flow = Flow::load(&work.txn, work.wid, &work.ticket).await?;
    let to = match target {
        Target::Stage(key) => flow.current.stage(&key).cloned(),
        Target::Category(status) if status.to_value() == work.ticket.status => {
            flow.rule(&work.ticket.stage_key).cloned()
        }
        Target::Category(status) => category(&status)
            .and_then(|c| flow.current.first_in(c))
            .cloned(),
    }
    .ok_or_else(|| invalid(STAGE_UNKNOWN))?;
    let position = match before {
        Some(id) if id != ticket_id => tickets::Model::find_in(&work.txn, work.wid, id)
            .await?
            .filter(|t| t.project_id == work.ticket.project_id && !t.is_trashed())
            .map_or_else(tickets::bottom_position, |t| t.position - 1),
        _ => tickets::bottom_position(),
    };
    let ticket = if to.key == work.ticket.stage_key {
        work.ticket.clone()
    } else {
        move_in(&work, acting, &flow, &to, wip_reason).await?
    };
    let mut model = ticket.into_active_model();
    model.position = Set(position);
    let ticket = model.update(&work.txn).await?;
    work.txn.commit().await?;
    Ok(ticket)
}

/// Kategori workflow untuk status tiket.
#[must_use]
pub fn category(status: &TicketStatus) -> Option<Category> {
    Category::parse(&status.to_value())
}

/// Beban kolom `to` melebihi batas WIP-nya bila `ticket` masuk: `Some((jumlah sekarang, batas))`.
/// Tiket yang sudah di stage itu tidak dihitung dua kali.
///
/// # Errors
///
/// Error database.
pub(crate) async fn wip_overflow<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    ticket: &tickets::Model,
    to: &Stage,
) -> Result<Option<(i64, u16)>, DbErr> {
    let Some(limit) = to.wip.filter(|w| *w > 0) else {
        return Ok(None);
    };
    if ticket.stage_key == to.key {
        return Ok(None);
    }
    let load = tickets::Model::count_by_stage(db, wid, ticket.project_id)
        .await?
        .into_iter()
        .find(|(k, _)| *k == to.key)
        .map_or(0, |(_, n)| n);
    Ok((load >= i64::from(limit)).then_some((load, limit)))
}

/// Error `ticket-wip-full` dengan nama stage dan batasnya.
pub(crate) fn wip_full(to: &Stage, limit: u16) -> TicketFailure {
    let mut errors = validator::ValidationErrors::new();
    let mut e = validator::ValidationError::new(WIP_FULL);
    e.add_param("stage".into(), &to.name);
    e.add_param("wip".into(), &limit.to_string());
    // Kalimat lengkap untuk API/MCP/flash ([`TicketFailure::message`]); template memakai kode.
    e.message = Some(
        crate::common::i18n::text(
            "ticket-wip-full-detail",
            &[("stage", &to.name), ("wip", &limit.to_string())],
        )
        .into(),
    );
    errors.add("stage", e);
    TicketFailure::errors(errors)
}

/// Langkah [`move_to`] di transaksi `work` (juga "tandai selesai"). `wip_reason` = Admin memaksa
/// melewati batas WIP ([`move_over_wip`]).
pub(crate) async fn move_in(
    work: &Working,
    acting: &Acting<'_>,
    flow: &Flow,
    to: &Stage,
    wip_reason: Option<&str>,
) -> Result<tickets::Model, TicketFailure> {
    check_move(&work.txn, work.wid, &work.ticket, flow, to).await?;
    let to_status = TicketStatus::parse(to.category.as_str()).unwrap_or(TicketStatus::Todo);
    ensure_can_move(&work.txn, work.wid, &work.ticket, to_status).await?;
    let overflow = wip_overflow(&work.txn, work.wid, &work.ticket, to).await?;
    if let Some((_, limit)) = overflow
        && wip_reason.is_none()
    {
        return Err(wip_full(to, limit));
    }
    let from_status = work.ticket.status.clone();
    let from = work.ticket.stage_key.clone();
    let ticket = enter_stage(&work.txn, work.wid, work.ticket.clone(), flow, to).await?;
    let mut changes = stage_change(flow, &from, to);
    if from_status != ticket.status {
        changes = merge(
            changes,
            serde_json::json!({ "status": { "from": from_status, "to": ticket.status } }),
        );
    }
    if let (Some((load, limit)), Some(reason)) = (overflow, wip_reason) {
        changes = merge(
            changes,
            serde_json::json!({ "wip_override": { "reason": reason, "wip": limit, "load": load } }),
        );
        crate::services::audit::record(
            &work.txn,
            acting.actor().audit(
                AuditAction::TicketWipOverridden,
                (AuditSubject::Ticket, ticket.id),
                merge(changes.clone(), serde_json::json!({ "code": ticket.code })),
            ),
        )
        .await?;
    }
    work.event(acting.user.id, EventKind::StageChanged, changes)
        .await?;
    Ok(ticket)
}

/// Penugasan stage sekarang dan apakah pelaku boleh memutuskan (approve/reject/keputusan).
pub(crate) async fn may_decide<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    ticket: &tickets::Model,
    user: Uuid,
    access: &EffectiveRole,
) -> Result<StagePeople, TicketFailure> {
    let people =
        ticket_stage_people::Model::of_stage(db, wid, ticket.id, &ticket.stage_key).await?;
    let allowed = if people.approvers.is_empty() {
        access.has(Permission::EditProject)
    } else {
        people.approvers.contains(&user)
    };
    if allowed {
        Ok(people)
    } else {
        Err(invalid(NOT_APPROVER))
    }
}

/// Pelaku boleh memutuskan stage sekarang tiket ini (untuk menampilkan tombol).
///
/// # Errors
///
/// Error database.
pub async fn can_decide<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    ticket: &tickets::Model,
    user: Uuid,
    access: &EffectiveRole,
) -> Result<bool, DbErr> {
    match may_decide(db, wid, ticket, user, access).await {
        Ok(_) => Ok(true),
        Err(TicketFailure::Db(e)) => Err(e),
        Err(_) => Ok(false),
    }
}

async fn stage_artifact_ids<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    ticket: &tickets::Model,
) -> Result<Vec<Uuid>, DbErr> {
    Ok(ticket_artifacts::Model::of_ticket(db, wid, ticket.id)
        .await?
        .into_iter()
        .filter(|a| a.stage_key == ticket.stage_key)
        .map(|a| a.id)
        .collect())
}

async fn set_state<C: ConnectionTrait>(
    db: &C,
    ticket: tickets::Model,
    state: StageState,
) -> Result<tickets::Model, DbErr> {
    let mut model = ticket.into_active_model();
    model.stage_state = Set(state.to_value());
    model.update(db).await
}

pub(crate) async fn notify_all<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    actor: Uuid,
    users: impl IntoIterator<Item = Uuid>,
    kind: NotificationKind,
    ticket_id: Uuid,
) -> Result<(), DbErr> {
    let mut seen = Vec::new();
    for user in users {
        if seen.contains(&user) {
            continue;
        }
        seen.push(user);
        if workspace_members::Model::find_with_role(db, wid, user)
            .await?
            .is_some()
        {
            notify(db, wid, actor, user, kind, ticket_id, None).await?;
        }
    }
    Ok(())
}

/// "Selesaikan stage": transisi tunggu approve → minta review; selain itu → pindah ke stage
/// berikutnya (gate dicek).
///
/// # Errors
///
/// [`TicketFailure::Invalid`], `NotFound`, atau error database.
pub async fn complete(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    ticket_id: Uuid,
) -> Result<tickets::Model, TicketFailure> {
    let work = Working::begin(db, acting, ticket_id, Permission::EditProject, false).await?;
    let flow = Flow::load(&work.txn, work.wid, &work.ticket).await?;
    if work.ticket.stage_state()? != StageState::Active {
        return Err(invalid(STAGE_BUSY));
    }
    let rule = flow.rule(&work.ticket.stage_key).cloned();
    let ticket = if rule.as_ref().is_some_and(|r| r.exit == Exit::Approval) {
        require_gates(&work.txn, work.wid, &work.ticket, &flow).await?;
        let ids = stage_artifact_ids(&work.txn, work.wid, &work.ticket).await?;
        artifact_versions::ActiveModel::decide_latest(
            &work.txn,
            work.wid,
            &ids,
            &[VersionStatus::Draft],
            VersionStatus::Pending,
            None,
            "",
        )
        .await?;
        let ticket = set_state(&work.txn, work.ticket.clone(), StageState::Review).await?;
        work.event(
            acting.user.id,
            EventKind::ReviewRequested,
            serde_json::json!({ "stage": rule.as_ref().map(|r| r.name.clone()) }),
        )
        .await?;
        let people =
            ticket_stage_people::Model::of_stage(&work.txn, work.wid, ticket.id, &ticket.stage_key)
                .await?;
        notify_all(
            &work.txn,
            work.wid,
            acting.user.id,
            people.approvers,
            NotificationKind::ReviewRequested,
            ticket.id,
        )
        .await?;
        ticket
    } else {
        let next = flow
            .next(&work.ticket.stage_key)
            .cloned()
            .ok_or_else(|| invalid(LAST_STAGE))?;
        move_in(&work, acting, &flow, &next, None).await?
    };
    work.txn.commit().await?;
    Ok(ticket)
}

/// Approve stage yang menunggu review. `note` opsional.
///
/// # Errors
///
/// [`TicketFailure::Invalid`] (`ticket-not-in-review`, `ticket-not-approver`,
/// `ticket-gates-unmet`, `ticket-blocked`), `NotFound`, atau error database.
pub async fn approve(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    ticket_id: Uuid,
    note: &str,
) -> Result<tickets::Model, TicketFailure> {
    let note = note_checked(note, false)?;
    let work = Working::begin(db, acting, ticket_id, Permission::ViewProject, false).await?;
    if work.ticket.stage_state()? != StageState::Review {
        return Err(invalid(NOT_IN_REVIEW));
    }
    may_decide(
        &work.txn,
        work.wid,
        &work.ticket,
        acting.user.id,
        &work.access,
    )
    .await?;
    let flow = Flow::load(&work.txn, work.wid, &work.ticket).await?;
    require_gates(&work.txn, work.wid, &work.ticket, &flow).await?;
    let ids = stage_artifact_ids(&work.txn, work.wid, &work.ticket).await?;
    artifact_versions::ActiveModel::decide_latest(
        &work.txn,
        work.wid,
        &ids,
        &[VersionStatus::Pending, VersionStatus::Draft],
        VersionStatus::Approved,
        Some(acting.user.id),
        &note,
    )
    .await?;
    let from = work.ticket.stage_key.clone();
    let mut ticket = work.ticket.clone();
    if ticket.fail_stage_key.as_deref() == Some(from.as_str()) {
        let mut model = ticket.into_active_model();
        model.fail_count = Set(0);
        model.fail_stage_key = Set(None);
        ticket = model.update(&work.txn).await?;
    }
    let ticket = match flow.next(&from).cloned() {
        Some(next) => {
            let status = TicketStatus::parse(next.category.as_str()).unwrap_or(TicketStatus::Todo);
            ensure_can_move(&work.txn, work.wid, &ticket, status).await?;
            let entered = enter_stage(&work.txn, work.wid, ticket, &flow, &next).await?;
            work.event(
                acting.user.id,
                EventKind::Approved,
                merge(
                    stage_change(&flow, &from, &next),
                    serde_json::json!({ "note": note }),
                ),
            )
            .await?;
            entered
        }
        None => {
            let t = set_state(&work.txn, ticket, StageState::Active).await?;
            work.event(
                acting.user.id,
                EventKind::Approved,
                serde_json::json!({ "note": note }),
            )
            .await?;
            t
        }
    };
    let executors =
        ticket_stage_people::Model::of_stage(&work.txn, work.wid, ticket.id, &from).await?;
    notify_all(
        &work.txn,
        work.wid,
        acting.user.id,
        executors.executors.into_iter().chain(ticket.assignee_id),
        NotificationKind::Approved,
        ticket.id,
    )
    .await?;
    work.txn.commit().await?;
    Ok(ticket)
}

/// Reject stage yang menunggu review: satu putaran gagal.
///
/// # Errors
///
/// [`TicketFailure::Invalid`] (`ticket-note-required`, `ticket-not-in-review`,
/// `ticket-not-approver`), `NotFound`, atau error database.
pub async fn reject(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    ticket_id: Uuid,
    note: &str,
) -> Result<tickets::Model, TicketFailure> {
    let note = note_checked(note, true)?;
    let work = Working::begin(db, acting, ticket_id, Permission::ViewProject, false).await?;
    if work.ticket.stage_state()? != StageState::Review {
        return Err(invalid(NOT_IN_REVIEW));
    }
    let people = may_decide(
        &work.txn,
        work.wid,
        &work.ticket,
        acting.user.id,
        &work.access,
    )
    .await?;
    let flow = Flow::load(&work.txn, work.wid, &work.ticket).await?;
    let stage_key = work.ticket.stage_key.clone();
    let rule = flow.rule(&stage_key).cloned();
    let ids = stage_artifact_ids(&work.txn, work.wid, &work.ticket).await?;
    artifact_versions::ActiveModel::decide_latest(
        &work.txn,
        work.wid,
        &ids,
        &[VersionStatus::Pending, VersionStatus::Draft],
        VersionStatus::Rejected,
        Some(acting.user.id),
        &note,
    )
    .await?;
    let round = if work.ticket.fail_stage_key.as_deref() == Some(stage_key.as_str()) {
        work.ticket.fail_count + 1
    } else {
        1
    };
    let mut model = work.ticket.clone().into_active_model();
    model.fail_count = Set(round);
    model.fail_stage_key = Set(Some(stage_key.clone()));
    let ticket = model.update(&work.txn).await?;
    let limit = i32::from(rule.as_ref().map_or(3, |r| r.fail_limit));
    work.event(
        acting.user.id,
        EventKind::Rejected,
        serde_json::json!({
            "note": note,
            "round": round,
            "limit": limit,
            "stage": rule.as_ref().map(|r| r.name.clone()),
        }),
    )
    .await?;
    let ticket = if round >= limit {
        let t = set_state(&work.txn, ticket, StageState::Decision).await?;
        work.event(
            acting.user.id,
            EventKind::DecisionNeeded,
            serde_json::json!({ "round": round, "limit": limit }),
        )
        .await?;
        notify_all(
            &work.txn,
            work.wid,
            acting.user.id,
            people.approvers.iter().copied().chain(t.assignee_id),
            NotificationKind::DecisionNeeded,
            t.id,
        )
        .await?;
        t
    } else {
        let back = rule
            .as_ref()
            .and_then(|r| r.fail_to.as_deref())
            .and_then(|k| flow.current.stage(k))
            .cloned();
        let t = match back {
            Some(back) => {
                let entered = enter_stage(&work.txn, work.wid, ticket, &flow, &back).await?;
                work.event(
                    acting.user.id,
                    EventKind::StageChanged,
                    stage_change(&flow, &stage_key, &back),
                )
                .await?;
                entered
            }
            None => {
                let t = set_state(&work.txn, ticket, StageState::Active).await?;
                // AI merevisi di stage yang sama: job stage diumumkan ulang.
                if let Some(r) = rule.as_ref() {
                    crate::services::jobs::on_stage_entered(&work.txn, work.wid, &t, r).await?;
                }
                t
            }
        };
        notify_all(
            &work.txn,
            work.wid,
            acting.user.id,
            people.executors.iter().copied().chain(t.assignee_id),
            NotificationKind::Rejected,
            t.id,
        )
        .await?;
        t
    };
    work.txn.commit().await?;
    Ok(ticket)
}

/// Lima pilihan saat tiket berstatus Butuh keputusan (spec "Testing gagal").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Beri petunjuk, lalu lanjutkan; hitungan gagal kembali ke 0.
    Hint,
    /// Perbaiki sendiri (pelaku jadi penerima tugas), lalu lanjut ke stage pengujian lagi.
    SelfFix,
    /// Ubah requirement: kembali ke stage pilihan (mis. PRD).
    Requirement,
    /// Terima dengan catatan: lanjut ke stage berikutnya, tiket Bug terpisah dibuat.
    Accept,
    /// Batalkan tiket: ke stage terakhir, `cancelled_at` diisi.
    Cancel,
}

impl Decision {
    pub const ALL: [Self; 5] = [
        Self::Hint,
        Self::SelfFix,
        Self::Requirement,
        Self::Accept,
        Self::Cancel,
    ];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hint => "hint",
            Self::SelfFix => "self",
            Self::Requirement => "requirement",
            Self::Accept => "accept",
            Self::Cancel => "cancel",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|d| d.as_str() == value.trim())
    }

    #[must_use]
    pub fn needs_note(self) -> bool {
        matches!(self, Self::Hint | Self::Accept)
    }
}

/// Ambil keputusan untuk tiket berstatus Butuh keputusan. `target` = kunci stage untuk
/// "Ubah requirement". Mengembalikan tiket dan kode tiket Bug baru (pilihan "Terima dengan
/// catatan").
///
/// # Errors
///
/// [`TicketFailure::Invalid`], `NotFound`, atau error database.
pub async fn decide(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    ticket_id: Uuid,
    choice: &str,
    note: &str,
    target: &str,
) -> Result<(tickets::Model, Option<String>), TicketFailure> {
    let choice =
        Decision::parse(choice).ok_or_else(|| TicketFailure::field("choice", DECISION_INVALID))?;
    let note = note_checked(note, choice.needs_note())?;
    let work = Working::begin(db, acting, ticket_id, Permission::ViewProject, false).await?;
    if work.ticket.stage_state()? != StageState::Decision {
        return Err(invalid(NOT_IN_DECISION));
    }
    may_decide(
        &work.txn,
        work.wid,
        &work.ticket,
        acting.user.id,
        &work.access,
    )
    .await?;
    if !work.access.has(Permission::EditProject) {
        return Err(TicketFailure::NotFound);
    }
    let flow = Flow::load(&work.txn, work.wid, &work.ticket).await?;
    let from = work.ticket.stage_key.clone();
    let rule = flow.rule(&from).cloned();
    let mut model = work.ticket.clone().into_active_model();
    model.fail_count = Set(0);
    model.fail_stage_key = Set(None);
    if choice == Decision::SelfFix {
        model.assignee_id = Set(Some(acting.user.id));
    }
    if choice == Decision::Cancel {
        model.cancelled_at = Set(Some(chrono::Utc::now().into()));
    }
    let ticket = model.update(&work.txn).await?;
    let fail_to = rule
        .as_ref()
        .and_then(|r| r.fail_to.as_deref())
        .and_then(|k| flow.current.stage(k))
        .cloned();
    let destination: Option<Stage> = match choice {
        Decision::Hint | Decision::SelfFix => fail_to,
        Decision::Requirement => {
            let i = flow.position(&from);
            let stage = flow
                .current
                .stage(target.trim())
                .filter(|s| flow.current.index_of(&s.key).is_some_and(|t| t < i))
                .cloned()
                .ok_or_else(|| TicketFailure::field("target", DECISION_TARGET_INVALID))?;
            Some(stage)
        }
        Decision::Accept => Some(
            flow.next(&from)
                .cloned()
                .ok_or_else(|| invalid(LAST_STAGE))?,
        ),
        Decision::Cancel => flow.current.final_stage().cloned(),
    };
    let ticket = match &destination {
        Some(to) if to.key != from => {
            if choice != Decision::Cancel {
                let status =
                    TicketStatus::parse(to.category.as_str()).unwrap_or(TicketStatus::Todo);
                ensure_can_move(&work.txn, work.wid, &ticket, status).await?;
            }
            enter_stage(&work.txn, work.wid, ticket, &flow, to).await?
        }
        _ => set_state(&work.txn, ticket, StageState::Active).await?,
    };
    let bug = if choice == Decision::Accept {
        Some(create_follow_up_bug(&work, acting, &ticket, &note).await?)
    } else {
        None
    };
    let mut changes = serde_json::json!({
        "choice": choice.as_str(),
        "note": note,
        "bug": bug.as_ref().map(|b| b.code.clone()),
    });
    if let Some(to) = destination.as_ref().filter(|t| t.key != from) {
        changes = merge(changes, stage_change(&flow, &from, to));
    }
    work.event(acting.user.id, EventKind::Decided, changes)
        .await?;
    if choice == Decision::SelfFix {
        watch_if_member(&work.txn, work.wid, ticket.id, acting.user.id).await?;
    }
    work.txn.commit().await?;
    Ok((ticket, bug.map(|b| b.code)))
}

/// Tiket Bug lanjutan ("Terima dengan catatan"): project sama, deskripsi = catatan, relasi
/// "terkait dengan" ke tiket asal.
async fn create_follow_up_bug(
    work: &Working,
    acting: &Acting<'_>,
    origin: &tickets::Model,
    note: &str,
) -> Result<tickets::Model, TicketFailure> {
    let types = ticket_types::Model::all_in(&work.txn, work.wid).await?;
    let bug_type = types
        .iter()
        .find(|t| t.key == "bug")
        .or_else(|| types.iter().find(|t| t.key == "task"))
        .or(types.first())
        .ok_or(TicketFailure::NotFound)?;
    let title: String = format!(
        "Lanjutan {}: {}",
        origin.code,
        note.lines().next().unwrap_or("")
    )
    .chars()
    .take(tickets::TITLE_MAX_LEN)
    .collect();
    let form = TicketForm {
        input: TicketInput {
            title,
            description: format!("{note}\n\nDiterima dengan catatan dari {}.", origin.code),
            priority: origin.priority.clone(),
            ..TicketInput::default()
        },
        type_id: bug_type.id.to_string(),
        ..TicketForm::default()
    };
    let bug = create::run_in(&work.txn, acting, origin.project_id, &form).await?;
    ticket_links::ActiveModel::link(
        &work.txn,
        work.wid,
        bug.id,
        origin.id,
        LinkKind::Relates,
        acting.user.id,
    )
    .await?;
    Ok(bug)
}

/// Jenis pindah paksa Admin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdminMove {
    /// Lompat stage: melewati satu atau lebih stage (gate stage yang dilewati tidak dicek).
    Jump,
    /// Paksa transisi: pindah walau gate belum terpenuhi.
    Force,
}

/// Lompat stage atau paksa transisi (Admin project, alasan wajib, audit log).
///
/// # Errors
///
/// [`TicketFailure::Invalid`] (`ticket-reason-required`, `ticket-force-disabled`,
/// `ticket-stage-unknown`, `ticket-blocked`), `NotFound` (bukan Admin), atau error database.
pub async fn admin_move(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    ticket_id: Uuid,
    to_key: &str,
    reason: &str,
    kind: AdminMove,
) -> Result<tickets::Model, TicketFailure> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err(TicketFailure::field("reason", REASON_REQUIRED));
    }
    if reason.chars().count() > NOTE_MAX_LEN {
        return Err(TicketFailure::field("reason", NOTE_TOO_LONG));
    }
    let work = Working::begin(db, acting, ticket_id, Permission::AdministerProject, false).await?;
    if kind == AdminMove::Force && !work.project.allow_force_transition {
        return Err(invalid(FORCE_DISABLED));
    }
    let flow = Flow::load(&work.txn, work.wid, &work.ticket).await?;
    let to = flow
        .current
        .stage(to_key)
        .filter(|s| s.key != work.ticket.stage_key)
        .cloned()
        .ok_or_else(|| invalid(STAGE_UNKNOWN))?;
    let status = TicketStatus::parse(to.category.as_str()).unwrap_or(TicketStatus::Todo);
    ensure_can_move(&work.txn, work.wid, &work.ticket, status).await?;
    let unmet: Vec<&str> = current_gates(&work.txn, work.wid, &work.ticket, &flow)
        .await?
        .iter()
        .filter(|c| !c.ok)
        .map(|c| c.gate.as_str())
        .collect();
    let from = work.ticket.stage_key.clone();
    let changes = merge(
        stage_change(&flow, &from, &to),
        serde_json::json!({ "reason": reason, "unmet_gates": unmet }),
    );
    let ticket = enter_stage(&work.txn, work.wid, work.ticket.clone(), &flow, &to).await?;
    let (event, action) = match kind {
        AdminMove::Jump => (EventKind::StageJumped, AuditAction::TicketStageJumped),
        AdminMove::Force => (
            EventKind::TransitionForced,
            AuditAction::TicketTransitionForced,
        ),
    };
    work.event(acting.user.id, event, changes.clone()).await?;
    crate::services::audit::record(
        &work.txn,
        acting.actor().audit(
            action,
            (AuditSubject::Ticket, ticket.id),
            merge(changes, serde_json::json!({ "code": ticket.code })),
        ),
    )
    .await?;
    work.txn.commit().await?;
    Ok(ticket)
}

/// Ubah penugasan stage `stage_key` tiket. Orang harus punya akses project.
///
/// # Errors
///
/// [`TicketFailure::Invalid`] (`ticket-stage-people-invalid`, `ticket-stage-unknown`),
/// `NotFound`, atau error database.
pub async fn set_people(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    ticket_id: Uuid,
    stage_key: &str,
    people: StagePeople,
) -> Result<(), TicketFailure> {
    let work = Working::begin(db, acting, ticket_id, Permission::EditProject, false).await?;
    let flow = Flow::load(&work.txn, work.wid, &work.ticket).await?;
    if flow.current.stage(stage_key).is_none() && stage_key != work.ticket.stage_key {
        return Err(invalid(STAGE_UNKNOWN));
    }
    let before =
        ticket_stage_people::Model::of_stage(&work.txn, work.wid, ticket_id, stage_key).await?;
    for id in people
        .executors
        .iter()
        .chain(&people.approvers)
        .chain(&people.watchers)
    {
        let known = before.executors.contains(id)
            || before.approvers.contains(id)
            || before.watchers.contains(id)
            || create::has_access(&work.txn, work.wid, work.project.id, *id).await?;
        if !known {
            return Err(TicketFailure::field("people", PEOPLE_INVALID));
        }
    }
    let people = members_only(&work.txn, work.wid, people).await?;
    if people == before {
        work.txn.rollback().await?;
        return Ok(());
    }
    ticket_stage_people::ActiveModel::replace(&work.txn, work.wid, ticket_id, stage_key, &people)
        .await?;
    if stage_key == work.ticket.stage_key {
        for user in &people.watchers {
            watch_if_member(&work.txn, work.wid, ticket_id, *user).await?;
        }
    }
    work.event(
        acting.user.id,
        EventKind::PeopleChanged,
        serde_json::json!({ "stage": flow.rule(stage_key).map(|s| s.name.clone()) }),
    )
    .await?;
    work.txn.commit().await?;
    Ok(())
}

/// Tiket yang menunggu keputusan pelaku di workspace ini: keadaan `review`/`decision` dan
/// pelaku approver stage-nya (stage tanpa approver: penerima tugas atau pelapor).
///
/// # Errors
///
/// Error database.
pub async fn awaiting<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    user: Uuid,
    project_ids: &[Uuid],
) -> Result<Vec<tickets::Model>, DbErr> {
    use sea_orm::{ColumnTrait, QueryFilter, QueryOrder};
    let candidates = tickets::Entity::find()
        .filter(tickets::Column::WorkspaceId.eq(wid.get()))
        .filter(tickets::active())
        .filter(tickets::Column::ProjectId.is_in(project_ids.iter().copied()))
        .filter(tickets::Column::StageState.ne(StageState::Active.to_value()))
        .order_by_desc(tickets::Column::StageEnteredAt)
        .order_by_desc(tickets::Column::Id)
        .all(db)
        .await?;
    let mut out = Vec::new();
    for t in candidates {
        let people = ticket_stage_people::Model::of_stage(db, wid, t.id, &t.stage_key).await?;
        let mine = if people.approvers.is_empty() {
            t.assignee_id == Some(user) || t.reporter_id == Some(user)
        } else {
            people.approvers.contains(&user)
        };
        if mine {
            out.push(t);
        }
    }
    Ok(out)
}

/// Stage padanan `key` (kategori `category`) di `definition`: kunci yang sama, lalu stage
/// pertama berkategori sama, lalu stage pertama.
#[must_use]
pub fn counterpart<'a>(
    definition: &'a Definition,
    key: &str,
    category: Category,
) -> Option<&'a Stage> {
    definition
        .stage(key)
        .or_else(|| definition.first_in(category))
        .or_else(|| definition.first())
}

/// Pindahkan tiket ke workflow `workflow_id` (tipe tiket berganti, atau penetapan workflow per
/// tipe berubah): stage padanan menurut kunci lalu kategori, keadaan `active`, hitungan gagal
/// kembali ke 0. Status tetap mengikuti kategori stage baru. Tidak menulis riwayat; `None` bila
/// workflow-nya sama.
///
/// # Errors
///
/// Error database; workflow tidak ada → `RecordNotFound`.
pub async fn follow_workflow<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    ticket: tickets::Model,
    workflow_id: Uuid,
) -> Result<(tickets::Model, Option<(String, String)>), DbErr> {
    if ticket.workflow_id == workflow_id {
        return Ok((ticket, None));
    }
    let workflow = workflows::Model::find_in(db, wid, workflow_id)
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("workflow".into()))?;
    let definition = workflow.definition(db).await?;
    let old = Flow::load(db, wid, &ticket).await?;
    let from_name = old
        .rule(&ticket.stage_key)
        .map_or_else(|| ticket.stage_key.clone(), |s| s.name.clone());
    let cat = Category::parse(&ticket.status).unwrap_or(Category::Todo);
    let Some(to) = counterpart(&definition, &ticket.stage_key, cat).cloned() else {
        return Ok((ticket, None));
    };
    let mut model = ticket.into_active_model();
    model.workflow_id = Set(workflow.id);
    model.fail_count = Set(0);
    model.fail_stage_key = Set(None);
    let ticket = model.update(db).await?;
    let flow = Flow {
        current: definition.clone(),
        rules: definition,
        workflow,
    };
    let keep_position = ticket.position;
    let status_before = ticket.status.clone();
    let mut ticket = enter_stage(db, wid, ticket, &flow, &to).await?;
    if ticket.status == status_before {
        let mut model = ticket.into_active_model();
        model.position = Set(keep_position);
        ticket = model.update(db).await?;
    }
    Ok((ticket, Some((from_name, to.name))))
}
