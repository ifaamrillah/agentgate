//! Webhook keluar (M11): kelola tujuan (buat, ubah, aktif/nonaktif, hapus, rotasi secret),
//! kirim ulang, kirim event uji. Pengiriman ada di [`dispatch`], penjaga URL di [`url_guard`].
//! Event domain ditulis ke outbox oleh `models::outbox_events::publish` dari titik pusat
//! perubahan (riwayat tiket, komentar, sprint, rilis, job), bukan dari controller.
//!
//! Semua aksi butuh `ManageIntegrations` (UI) atau token API bercakupan `webhook:kelola` milik
//! pemegang izin itu (`/api/v1/webhooks`), dicek ulang di dalam transaksi (`lock_as`).

pub mod dispatch;
pub mod url_guard;

use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, DatabaseConnection, IntoActiveModel, ModelTrait,
    TransactionTrait,
};
use uuid::Uuid;

use crate::{
    common::{crypto, settings::Settings},
    models::{
        outbox_events::WebhookEvent,
        permission::Permission,
        projects,
        webhook_deliveries::{self},
        webhooks,
    },
    services::{
        audit::{self, AuditAction, AuditSubject},
        tickets::TicketFailure,
        workspaces::{Acting, lock_as},
    },
};

pub const EVENTS_REQUIRED: &str = "webhook-events-required";
pub const PROJECT_INVALID: &str = "webhook-project-invalid";
pub const SECRET_UNAVAILABLE: &str = "webhook-secret-unavailable";
pub const INACTIVE: &str = "webhook-inactive";
/// Kode error yang dipakai dari Rust (kontrak i18n).
pub const ERROR_CODES: &[&str] = &[
    url_guard::URL_INVALID,
    url_guard::URL_FORBIDDEN,
    EVENTS_REQUIRED,
    PROJECT_INVALID,
    SECRET_UNAVAILABLE,
    INACTIVE,
];

/// Awalan secret webhook (mudah dikenali di secret manager).
pub const SECRET_PREFIX: &str = "whsec_";

/// Isian form/API webhook.
#[derive(Debug, Clone, Default)]
pub struct WebhookInput {
    pub url: String,
    pub events: Vec<String>,
    /// Kunci project; kosong atau berisi `all` = semua project.
    pub projects: Vec<String>,
}

fn key(settings: &Settings) -> Result<[u8; 32], TicketFailure> {
    settings
        .auth
        .encryption_key_bytes()
        .map_err(|_| TicketFailure::field("secret", SECRET_UNAVAILABLE))
}

fn new_secret() -> String {
    format!("{SECRET_PREFIX}{}", crypto::random_token())
}

/// Isian tervalidasi: (url, event, id project).
async fn validated(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    settings: &Settings,
    input: &WebhookInput,
) -> Result<(String, Vec<String>, Vec<Uuid>), TicketFailure> {
    let url = url_guard::validate(&input.url, settings.api.webhook_allow_loopback)
        .map_err(|code| TicketFailure::field("url", code))?;
    let mut events: Vec<WebhookEvent> = Vec::new();
    for e in &input.events {
        match WebhookEvent::parse(e.trim()) {
            Some(WebhookEvent::Ping) | None => {
                return Err(TicketFailure::field("events", EVENTS_REQUIRED));
            }
            Some(ev) if !events.contains(&ev) => events.push(ev),
            Some(_) => {}
        }
    }
    if events.is_empty() {
        return Err(TicketFailure::field("events", EVENTS_REQUIRED));
    }
    events.sort();
    let wid = acting.workspace_id();
    let mut project_ids = Vec::new();
    if !input.projects.iter().any(|p| p.trim() == "all") {
        for k in input.projects.iter().map(|k| k.trim()).filter(|k| !k.is_empty()) {
            let p = projects::Model::find_by_key(db, wid, k)
                .await?
                .ok_or(TicketFailure::field("projects", PROJECT_INVALID))?;
            if !project_ids.contains(&p.id) {
                project_ids.push(p.id);
            }
        }
    }
    Ok((
        url.to_string(),
        events.iter().map(|e| e.as_str()).collect(),
        project_ids,
    ))
}

async fn locked(
    txn: &sea_orm::DatabaseTransaction,
    acting: &Acting<'_>,
) -> Result<(), TicketFailure> {
    lock_as(txn, acting, |r| r.has(Permission::ManageIntegrations))
        .await?
        .ok_or(TicketFailure::NotFound)?;
    Ok(())
}

/// Buat webhook; kembalikan baris dan secret mentah (ditampilkan sekali).
///
/// # Errors
///
/// `Invalid` (URL, event, project), `NotFound` (tanpa izin), error database.
pub async fn create(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    settings: &Settings,
    input: &WebhookInput,
) -> Result<(webhooks::Model, String), TicketFailure> {
    if !acting.role.has(Permission::ManageIntegrations) {
        return Err(TicketFailure::NotFound);
    }
    let (url, events, project_ids) = validated(db, acting, settings, input).await?;
    let secret = new_secret();
    let encrypted = crypto::encrypt(&key(settings)?, secret.as_bytes())
        .map_err(|_| TicketFailure::field("secret", SECRET_UNAVAILABLE))?;
    let txn = db.begin().await?;
    locked(&txn, acting).await?;
    let hook = webhooks::ActiveModel {
        workspace_id: Set(acting.workspace_id().get()),
        url: Set(url),
        secret_encrypted: Set(encrypted),
        events: Set(events),
        project_ids: Set(project_ids),
        created_by: Set(Some(acting.user.id)),
        ..Default::default()
    }
    .insert(&txn)
    .await?;
    audit::record(
        &txn,
        acting.actor().audit(
            AuditAction::WebhookCreated,
            (AuditSubject::Webhook, hook.id),
            serde_json::json!({
                "url": hook.url,
                "events": hook.events,
                "project_ids": hook.project_ids,
            }),
        ),
    )
    .await?;
    txn.commit().await?;
    Ok((hook, secret))
}

async fn find_locked(
    txn: &sea_orm::DatabaseTransaction,
    acting: &Acting<'_>,
    id: Uuid,
) -> Result<webhooks::Model, TicketFailure> {
    locked(txn, acting).await?;
    webhooks::Model::find_in(txn, acting.workspace_id(), id)
        .await?
        .ok_or(TicketFailure::NotFound)
}

/// Ubah URL, event, dan project webhook.
///
/// # Errors
///
/// `Invalid`, `NotFound`, error database.
pub async fn update(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    settings: &Settings,
    id: Uuid,
    input: &WebhookInput,
) -> Result<webhooks::Model, TicketFailure> {
    if !acting.role.has(Permission::ManageIntegrations) {
        return Err(TicketFailure::NotFound);
    }
    let (url, events, project_ids) = validated(db, acting, settings, input).await?;
    let txn = db.begin().await?;
    let hook = find_locked(&txn, acting, id).await?;
    let changes = serde_json::json!({
        "url": audit::change(&hook.url, &url),
        "events": audit::change(&hook.events, &events),
        "project_ids": audit::change(&hook.project_ids, &project_ids),
    });
    let mut m = hook.into_active_model();
    m.url = Set(url);
    m.events = Set(events);
    m.project_ids = Set(project_ids);
    let hook = m.update(&txn).await?;
    audit::record(
        &txn,
        acting.actor().audit(
            AuditAction::WebhookUpdated,
            (AuditSubject::Webhook, hook.id),
            changes,
        ),
    )
    .await?;
    txn.commit().await?;
    Ok(hook)
}

/// Aktifkan/nonaktifkan. Mengaktifkan ulang menghapus hitungan gagal beruntun; menonaktifkan
/// menggagalkan delivery yang masih menunggu.
///
/// # Errors
///
/// `NotFound`, error database.
pub async fn set_active(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
    active: bool,
) -> Result<webhooks::Model, TicketFailure> {
    let txn = db.begin().await?;
    let hook = find_locked(&txn, acting, id).await?;
    if hook.is_active() == active {
        txn.commit().await?;
        return Ok(hook);
    }
    let mut m = hook.into_active_model();
    if active {
        m.disabled_at = Set(None);
        m.consecutive_failures = Set(0);
    } else {
        m.disabled_at = Set(Some(chrono::Utc::now().into()));
    }
    let hook = m.update(&txn).await?;
    if !active {
        webhook_deliveries::fail_pending_of(&txn, hook.id, "webhook_disabled").await?;
    }
    audit::record(
        &txn,
        acting.actor().audit(
            AuditAction::WebhookUpdated,
            (AuditSubject::Webhook, hook.id),
            serde_json::json!({ "active": { "from": !active, "to": active } }),
        ),
    )
    .await?;
    txn.commit().await?;
    Ok(hook)
}

/// Hapus webhook beserta log pengirimannya.
///
/// # Errors
///
/// `NotFound`, error database.
pub async fn delete(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
) -> Result<(), TicketFailure> {
    let txn = db.begin().await?;
    let hook = find_locked(&txn, acting, id).await?;
    audit::record(
        &txn,
        acting.actor().audit(
            AuditAction::WebhookDeleted,
            (AuditSubject::Webhook, hook.id),
            serde_json::json!({ "url": hook.url, "events": hook.events }),
        ),
    )
    .await?;
    hook.delete(&txn).await?;
    txn.commit().await?;
    Ok(())
}

/// Buat secret baru; kembalikan secret mentah (ditampilkan sekali).
///
/// # Errors
///
/// `NotFound`, error database.
pub async fn rotate_secret(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    settings: &Settings,
    id: Uuid,
) -> Result<(webhooks::Model, String), TicketFailure> {
    let secret = new_secret();
    let encrypted = crypto::encrypt(&key(settings)?, secret.as_bytes())
        .map_err(|_| TicketFailure::field("secret", SECRET_UNAVAILABLE))?;
    let txn = db.begin().await?;
    let hook = find_locked(&txn, acting, id).await?;
    let mut m = hook.into_active_model();
    m.secret_encrypted = Set(encrypted);
    let hook = m.update(&txn).await?;
    audit::record(
        &txn,
        acting.actor().audit(
            AuditAction::WebhookSecretRotated,
            (AuditSubject::Webhook, hook.id),
            serde_json::json!({ "url": hook.url }),
        ),
    )
    .await?;
    txn.commit().await?;
    Ok((hook, secret))
}

/// Kirim event uji (`ping`) ke webhook aktif. Delivery dikirim worker setelah commit.
///
/// # Errors
///
/// `webhook-inactive`, `NotFound`, error database.
pub async fn ping(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    id: Uuid,
) -> Result<webhook_deliveries::Model, TicketFailure> {
    let txn = db.begin().await?;
    let hook = find_locked(&txn, acting, id).await?;
    if !hook.is_active() {
        return Err(TicketFailure::field("webhook", INACTIVE));
    }
    let body = serde_json::json!({
        "id": Uuid::now_v7(),
        "event": WebhookEvent::Ping,
        "created_at": chrono::Utc::now(),
        "workspace": { "slug": acting.workspace.slug, "name": acting.workspace.name },
        "project": null,
        "data": { "webhook_id": hook.id, "message": "Ping dari Agent Gate" },
    })
    .to_string();
    let d = webhook_deliveries::ActiveModel::enqueue(
        &txn,
        acting.workspace_id(),
        hook.id,
        None,
        WebhookEvent::Ping,
        body,
    )
    .await?;
    txn.commit().await?;
    Ok(d)
}

/// Kirim ulang satu delivery (baris baru dengan isi yang sama) ke webhook yang masih aktif.
///
/// # Errors
///
/// `webhook-inactive`, `NotFound`, error database.
pub async fn resend(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    delivery_id: Uuid,
) -> Result<webhook_deliveries::Model, TicketFailure> {
    let txn = db.begin().await?;
    locked(&txn, acting).await?;
    let wid = acting.workspace_id();
    let old = webhook_deliveries::Model::find_in(&txn, wid, delivery_id)
        .await?
        .ok_or(TicketFailure::NotFound)?;
    let hook = webhooks::Model::find_in(&txn, wid, old.webhook_id)
        .await?
        .ok_or(TicketFailure::NotFound)?;
    if !hook.is_active() {
        return Err(TicketFailure::field("webhook", INACTIVE));
    }
    let event = old.event()?;
    let d = webhook_deliveries::ActiveModel::enqueue(
        &txn,
        wid,
        hook.id,
        old.event_id,
        event,
        old.body.clone(),
    )
    .await?;
    txn.commit().await?;
    Ok(d)
}
