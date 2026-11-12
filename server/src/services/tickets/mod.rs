//! Semua perubahan tiket (M2): buat, ubah, pindah status, relasi, komentar, lampiran, watcher,
//! sampah, dan pengaturan tipe/field/kode.
//!
//! **Izin dibaca ulang di dalam transaksi** ([`Working::begin`]), seperti `Administering` di
//! service project: baris tiket dikunci, lalu role workspace dan akses project pelaku dimuat
//! lagi. Pelaku yang aksesnya dicabut di tengah jalan mendapat `NotFound` (sama dengan
//! extractor), dan perubahan pada satu tiket berjalan berurutan.
//!
//! Riwayat (`ticket_events`) dan notifikasi (`notifications`) ditulis di transaksi yang sama
//! dengan perubahannya: keduanya baris database, bukan efek samping keluar proses. Satu-satunya
//! efek samping keluar proses adalah file lampiran di storage (lihat [`attachments`]).

pub mod artifacts;
pub mod attachments;
pub mod comments;
pub mod create;
pub mod gates;
pub mod links;
pub mod mentions;
pub mod settings;
pub mod stages;
pub mod trash;
pub mod update;

use loco_rs::validation::ModelValidationErrors;
use sea_orm::{DatabaseConnection, DatabaseTransaction, DbErr, TransactionTrait};
use uuid::Uuid;
use validator::{ValidationError, ValidationErrors};

use crate::{
    models::{
        notifications::{self, NewNotification, NotificationKind},
        permission::Permission,
        project_access::{self, Accessor, EffectiveRole},
        projects,
        ticket_events::{self, EventKind},
        ticket_links, ticket_watchers,
        tickets::{self, TicketStatus},
        workspace_members,
        workspaces::WorkspaceId,
    },
    services::workspaces::Acting,
};

#[derive(Debug, thiserror::Error)]
pub enum TicketFailure {
    /// Tiket/project/komentar/lampiran tidak ada di workspace ini, di sampah, atau pelaku tidak
    /// (lagi) memegang izin yang dibutuhkan.
    #[error("not found")]
    NotFound,
    #[error("invalid: {0}")]
    Invalid(ModelValidationErrors),
    #[error(transparent)]
    Db(#[from] DbErr),
}

impl TicketFailure {
    /// Satu error field.
    #[must_use]
    pub fn field(field: &'static str, code: &'static str) -> Self {
        let mut errors = ValidationErrors::new();
        errors.add(field, ValidationError::new(code));
        Self::Invalid(errors.into())
    }

    #[must_use]
    pub fn errors(errors: ValidationErrors) -> Self {
        Self::Invalid(errors.into())
    }
}

impl From<TicketFailure> for loco_rs::Error {
    fn from(err: TicketFailure) -> Self {
        match err {
            TicketFailure::NotFound => Self::NotFound,
            TicketFailure::Invalid(errors) => Self::Validation(errors),
            TicketFailure::Db(e) => Self::DB(e),
        }
    }
}

/// Transaksi dengan baris tiket terkunci, project-nya, dan akses project pelaku yang dibaca
/// setelah kunci.
pub(crate) struct Working {
    pub wid: WorkspaceId,
    pub txn: DatabaseTransaction,
    pub ticket: tickets::Model,
    pub project: projects::Model,
    pub access: EffectiveRole,
}

impl Working {
    /// `NotFound` bila tiket tidak ada di workspace pelaku, di sampah (kecuali `trashed`), pelaku
    /// bukan lagi anggota, atau akses project terbarunya tidak memegang `permission`.
    pub(crate) async fn begin(
        db: &DatabaseConnection,
        acting: &Acting<'_>,
        ticket_id: Uuid,
        permission: Permission,
        trashed: bool,
    ) -> Result<Self, TicketFailure> {
        let wid = acting.workspace_id();
        let txn = db.begin().await?;
        let ticket = tickets::Model::lock_in(&txn, wid, ticket_id)
            .await?
            .filter(|t| t.is_trashed() == trashed)
            .ok_or(TicketFailure::NotFound)?;
        let project = projects::Model::find_in(&txn, wid, ticket.project_id)
            .await?
            .ok_or(TicketFailure::NotFound)?;
        let access = access_in(&txn, acting, project.id, permission).await?;
        Ok(Self {
            wid,
            txn,
            ticket,
            project,
            access,
        })
    }

    /// Catat riwayat tiket ini.
    pub(crate) async fn event(
        &self,
        actor: Uuid,
        kind: EventKind,
        changes: serde_json::Value,
    ) -> Result<(), DbErr> {
        ticket_events::ActiveModel::record(
            &self.txn,
            self.wid,
            self.ticket.id,
            Some(actor),
            kind,
            changes,
        )
        .await?;
        Ok(())
    }
}

/// Akses project pelaku, dibaca ulang dari database (role workspace terbaru), yang memegang
/// `permission`; selain itu `NotFound`.
pub(crate) async fn access_in<C: sea_orm::ConnectionTrait>(
    db: &C,
    acting: &Acting<'_>,
    project_id: Uuid,
    permission: Permission,
) -> Result<EffectiveRole, TicketFailure> {
    let wid = acting.workspace_id();
    let (_, role) = workspace_members::Model::find_with_role(db, wid, acting.user.id)
        .await?
        .ok_or(TicketFailure::NotFound)?;
    let who = Accessor {
        workspace_id: wid,
        user_id: acting.user.id,
        role: &role,
    };
    project_access::effective_role(db, &who, project_id)
        .await?
        .filter(|a| a.has(permission))
        .ok_or(TicketFailure::NotFound)
}

/// Satu-satunya aturan *blocks* (spec: "tiket yang diblokir tidak bisa mulai sebelum tiket
/// pemblokir selesai"): pindah dari belum mulai ke `in_progress`/`done` ditolak selama ada
/// pemblokir yang belum selesai. Dipanggil semua jalur yang mengubah status.
pub(crate) async fn ensure_can_move<C: sea_orm::ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    ticket: &tickets::Model,
    to: TicketStatus,
) -> Result<(), TicketFailure> {
    let from = ticket.status()?;
    if from.is_started() || !to.is_started() {
        return Ok(());
    }
    let blockers = ticket_links::open_blockers(db, wid, ticket.id).await?;
    if blockers.is_empty() {
        return Ok(());
    }
    let mut e = ValidationError::new(tickets::BLOCKED);
    let codes: Vec<&str> = blockers.iter().map(|b| b.code.as_str()).collect();
    e.add_param("codes".into(), &codes.join(", "));
    let mut errors = ValidationErrors::new();
    errors.add("status", e);
    Err(TicketFailure::errors(errors))
}

/// Notifikasi untuk `user_id` (bukan pelakunya sendiri).
pub(crate) async fn notify<C: sea_orm::ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    actor: Uuid,
    user_id: Uuid,
    kind: NotificationKind,
    ticket_id: Uuid,
    comment_id: Option<Uuid>,
) -> Result<(), DbErr> {
    if user_id == actor {
        return Ok(());
    }
    notifications::ActiveModel::notify(
        db,
        wid,
        NewNotification {
            user_id,
            actor_id: Some(actor),
            kind,
            ticket_id: Some(ticket_id),
            comment_id,
        },
    )
    .await?;
    Ok(())
}

/// Ikuti tiket bila `user_id` anggota workspace (penerima tugas bisa user yang sudah keluar).
pub(crate) async fn watch_if_member<C: sea_orm::ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    ticket_id: Uuid,
    user_id: Uuid,
) -> Result<(), DbErr> {
    if workspace_members::Model::find_with_role(db, wid, user_id)
        .await?
        .is_some()
    {
        ticket_watchers::ActiveModel::watch(db, wid, ticket_id, user_id).await?;
    }
    Ok(())
}

/// Ikuti / berhenti mengikuti tiket. Semua yang bisa melihat tiket.
///
/// # Errors
///
/// [`TicketFailure::NotFound`] atau error database.
pub async fn set_watching(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    ticket_id: Uuid,
    watch: bool,
) -> Result<(), TicketFailure> {
    let work = Working::begin(db, acting, ticket_id, Permission::ViewProject, false).await?;
    if watch {
        ticket_watchers::ActiveModel::watch(&work.txn, work.wid, ticket_id, acting.user.id).await?;
    } else {
        ticket_watchers::ActiveModel::unwatch(&work.txn, work.wid, ticket_id, acting.user.id)
            .await?;
    }
    work.txn.commit().await?;
    Ok(())
}

/// Pindah status (jalur M2: board per status, tombol status): pindah ke stage pertama
/// berkategori `to` di workflow tiket, dengan aturan gate yang sama ([`stages::move_to`]).
/// `before` = kartu yang akan berada di bawah kartu ini di kolom tujuan (`None` = paling bawah).
///
/// # Errors
///
/// [`TicketFailure::Invalid`] (`ticket-blocked` pada field `status`, kode gate pada `stage`),
/// `NotFound`, atau error database.
pub async fn transition(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    ticket_id: Uuid,
    to: TicketStatus,
    before: Option<Uuid>,
) -> Result<tickets::Model, TicketFailure> {
    stages::move_to(db, acting, ticket_id, stages::Target::Category(to), before).await
}
