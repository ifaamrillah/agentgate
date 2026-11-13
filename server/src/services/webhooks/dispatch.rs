//! Pengirim webhook keluar (efek samping "wajib terjadi", rules §5). Baris
//! `webhook_deliveries` pending ditulis di transaksi perubahan domain; pengiriman dilakukan
//! setelah commit oleh worker `WebhookDeliveryWorker` (enqueue best-effort dari layer HTTP),
//! dan sapuan berkala ([`sweep`], task `webhooks`) mengirim yang tertahan atau jatuh tempo
//! retry. Setiap baris di-claim dulu (update bersyarat) sehingga aman dijalankan bersamaan.
//!
//! Permintaan: `POST` body JSON dengan header `X-AgentGate-Event`, `X-AgentGate-Delivery`,
//! `X-AgentGate-Timestamp` (detik unix), dan `X-AgentGate-Signature: sha256=<hex>` = HMAC-SHA256
//! secret atas `"{timestamp}.{body}"`. Respons 2xx = berhasil. Gagal → diulang dengan jeda
//! bertahap sampai 5 percobaan; 5 percobaan gagal beruntun pada satu webhook menonaktifkannya
//! dan memberi tahu pemegang izin `ManageIntegrations` (Owner/Admin) lewat notifikasi in-app.

use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, DbErr, EntityTrait, FromQueryResult,
    Statement,
};
use sha2::Sha256;
use uuid::Uuid;

use super::url_guard;
use crate::{
    common::settings::Settings,
    models::{
        notifications::{self, NewNotification, NotificationKind},
        webhook_deliveries::{self, Attempt},
        webhooks::{self, MAX_CONSECUTIVE_FAILURES},
        workspaces,
    },
};

/// Tanda tangan `sha256=<hex>` atas `"{timestamp}.{body}"`.
#[must_use]
pub fn sign(secret: &str, timestamp: i64, body: &str) -> String {
    // HMAC menerima kunci sepanjang apa pun; cabang `Err` tidak terjadi.
    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(secret.as_bytes()) else {
        return String::new();
    };
    mac.update(format!("{timestamp}.{body}").as_bytes());
    format!("sha256={}", hex::encode(mac.finalize().into_bytes()))
}

/// Kirim delivery `ids` yang jatuh tempo pada `now` (yang sudah di-claim pengirim lain dilewati).
/// Satu delivery yang gagal tidak menghentikan yang lain.
///
/// # Errors
///
/// Error database saat claim.
pub async fn send_many(
    db: &DatabaseConnection,
    settings: &Settings,
    ids: &[Uuid],
    now: DateTime<Utc>,
) -> Result<u64, DbErr> {
    let mut sent = 0;
    for id in ids {
        match deliver(db, settings, *id, now).await {
            Ok(true) => sent += 1,
            Ok(false) => {}
            Err(e) => tracing::warn!(error = %e, delivery = %id, "webhook delivery failed"),
        }
    }
    Ok(sent)
}

/// Sapu delivery pending yang jatuh tempo pada `now` (maks. `limit`) dan kirim.
///
/// # Errors
///
/// Error database.
pub async fn sweep(
    db: &DatabaseConnection,
    settings: &Settings,
    now: DateTime<Utc>,
    limit: u64,
) -> Result<u64, DbErr> {
    let ids = webhook_deliveries::Model::due(db, now, limit).await?;
    send_many(db, settings, &ids, now).await
}

/// Claim lalu kirim satu delivery. `false` bila tidak di-claim (bukan pending, belum jatuh
/// tempo, atau sudah diambil pengirim lain).
///
/// # Errors
///
/// Error database.
pub async fn deliver(
    db: &DatabaseConnection,
    settings: &Settings,
    id: Uuid,
    now: DateTime<Utc>,
) -> Result<bool, DbErr> {
    if !webhook_deliveries::Model::claim(db, id, now).await? {
        return Ok(false);
    }
    let Some(delivery) = webhook_deliveries::Entity::find_by_id(id).one(db).await? else {
        return Ok(false);
    };
    let hook = webhooks::Entity::find_by_id(delivery.webhook_id).one(db).await?;
    let Some(hook) = hook.filter(webhooks::Model::is_active) else {
        let attempt = Attempt {
            error: "webhook_disabled".into(),
            ..Attempt::default()
        };
        webhook_deliveries::ActiveModel::record_attempt(db, delivery, &attempt, now, true).await?;
        return Ok(true);
    };
    let attempt = send(settings, &hook, &delivery).await;
    let ok = attempt.succeeded();
    webhook_deliveries::ActiveModel::record_attempt(db, delivery, &attempt, now, false).await?;
    record_outcome(db, &hook, &attempt, now, ok).await?;
    Ok(true)
}

/// Satu percobaan HTTP (tanpa menyentuh database).
async fn send(
    settings: &Settings,
    hook: &webhooks::Model,
    delivery: &webhook_deliveries::Model,
) -> Attempt {
    let started = Instant::now();
    let fail = |error: &str, started: Instant| Attempt {
        status: None,
        body: String::new(),
        error: error.to_string(),
        duration_ms: i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX),
    };
    let key = match settings.auth.encryption_key_bytes() {
        Ok(k) => k,
        Err(_) => return fail("secret_unavailable", started),
    };
    let Ok(secret) = hook.secret(&key) else {
        return fail("secret_unavailable", started);
    };
    let allow = settings.api.webhook_allow_loopback;
    let (host, addr) = match url_guard::resolve(&hook.url, allow).await {
        Ok(v) => v,
        Err(code) => return fail(code, started),
    };
    let timeout = Duration::from_secs(settings.api.webhook_timeout_seconds.max(1));
    let client = match reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(timeout)
        .connect_timeout(timeout)
        .resolve(&host, addr)
        .user_agent("AgentGate-Webhook/1")
        .build()
    {
        Ok(c) => c,
        Err(_) => return fail("client_error", started),
    };
    let ts = Utc::now().timestamp();
    let res = client
        .post(&hook.url)
        .header("content-type", "application/json")
        .header("x-agentgate-event", &delivery.event)
        .header("x-agentgate-delivery", delivery.id.to_string())
        .header("x-agentgate-timestamp", ts.to_string())
        .header("x-agentgate-signature", sign(&secret, ts, &delivery.body))
        .body(delivery.body.clone())
        .send()
        .await;
    match res {
        Ok(r) => {
            let status = r.status().as_u16();
            let body = r.text().await.unwrap_or_default();
            Attempt {
                status: Some(status),
                body,
                error: String::new(),
                duration_ms: i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX),
            }
        }
        Err(e) if e.is_timeout() => fail("timeout", started),
        Err(e) if e.is_connect() => fail("connect_failed", started),
        Err(_) => fail("request_failed", started),
    }
}

#[derive(Debug, FromQueryResult)]
struct Failures {
    consecutive_failures: i32,
}

/// Perbarui status terakhir webhook; gagal beruntun mencapai batas → nonaktif + notifikasi.
async fn record_outcome(
    db: &DatabaseConnection,
    hook: &webhooks::Model,
    attempt: &Attempt,
    now: DateTime<Utc>,
    ok: bool,
) -> Result<(), DbErr> {
    let status = attempt.status.map(i32::from);
    let at: sea_orm::prelude::DateTimeWithTimeZone = now.into();
    let row = Failures::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE webhooks SET
            consecutive_failures = CASE WHEN $2 THEN 0 ELSE consecutive_failures + 1 END,
            last_delivery_at = $3, last_status = $4, updated_at = now()
         WHERE id = $1 RETURNING consecutive_failures",
        [hook.id.into(), ok.into(), at.into(), status.into()],
    ))
    .one(db)
    .await?;
    let failures = row.map_or(0, |r| r.consecutive_failures);
    if ok || failures < MAX_CONSECUTIVE_FAILURES {
        return Ok(());
    }
    // Race guard: hanya pengirim yang benar-benar menonaktifkan yang memberi tahu.
    let disabled = db
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE webhooks SET disabled_at = $2, updated_at = now()
             WHERE id = $1 AND disabled_at IS NULL",
            [hook.id.into(), at.into()],
        ))
        .await?
        .rows_affected();
    if disabled != 1 {
        return Ok(());
    }
    webhook_deliveries::fail_pending_of(db, hook.id, "webhook_disabled").await?;
    notify_admins(db, hook).await
}

#[derive(Debug, FromQueryResult)]
struct Recipient {
    user_id: Uuid,
}

/// Notifikasi `webhook_disabled` untuk anggota aktif yang role-nya memegang
/// `manage_integrations` (Owner/Admin bawaan).
async fn notify_admins<C: ConnectionTrait>(db: &C, hook: &webhooks::Model) -> Result<(), DbErr> {
    let Some(ws) = workspaces::Model::find_by_id(db, hook.workspace_id).await? else {
        return Ok(());
    };
    let recipients = Recipient::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT wm.user_id FROM workspace_members wm
         JOIN roles r ON r.id = wm.role_id
         JOIN users u ON u.id = wm.user_id
         WHERE wm.workspace_id = $1 AND 'manage_integrations' = ANY (r.permissions)
           AND u.disabled_at IS NULL",
        [hook.workspace_id.into()],
    ))
    .all(db)
    .await?;
    for r in recipients {
        notifications::ActiveModel::notify(
            db,
            ws.workspace_id(),
            NewNotification {
                user_id: r.user_id,
                actor_id: None,
                kind: NotificationKind::WebhookDisabled,
                ticket_id: None,
                comment_id: None,
                project_id: None,
            },
        )
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_covers_timestamp_and_body() {
        let a = sign("whsec_x", 1_700_000_000, "{}");
        assert!(a.starts_with("sha256="));
        assert_eq!(a.len(), 7 + 64);
        assert_ne!(a, sign("whsec_x", 1_700_000_001, "{}"));
        assert_ne!(a, sign("whsec_y", 1_700_000_000, "{}"));
    }
}
