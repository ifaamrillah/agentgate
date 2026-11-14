//! Pengiriman webhook ke penerima palsu lokal (`support::fake_hook`, 127.0.0.1:0, tanpa jaringan
//! nyata): tanda tangan HMAC + header, dikirim setelah commit oleh worker, retry bertahap lewat
//! sapuan, 5 gagal beruntun → webhook nonaktif + notifikasi Owner, kirim ulang, dan SSRF ditolak
//! (saat disimpan dan setelah resolve DNS saat kirim).

use agentgate_server::{
    app::App,
    common::settings::Settings,
    models::{notifications, outbox_events::WebhookEvent, webhook_deliveries, webhooks},
    services::webhooks::{dispatch, url_guard},
};
use chrono::{Duration, Utc};
use hmac::{Hmac, Mac};
use loco_rs::{TestServer, app::AppContext, auth::cookies::Cookie, testing::prelude::*};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde_json::{Value, json};

use super::raw_secret;
use crate::{
    requests::tickets::{create_ticket, fixture},
    support::{fake_hook, request_with_settings},
};

/// Buat webhook lewat UI; kembalikan secret (tampil sekali).
async fn new_hook(
    request: &TestServer,
    cookie: &Cookie<'static>,
    url: &str,
    events: &[&str],
) -> String {
    let mut form = vec![("url", url)];
    form.extend(events.iter().map(|e| ("events", *e)));
    form.push(("projects", "all"));
    let res = request
        .post("/w/acme/settings/api/hooks")
        .add_cookie(cookie.clone())
        .form(&form)
        .await;
    assert_eq!(res.status_code(), 200, "{}", res.text());
    raw_secret(&res.text(), "whsec_")
}

async fn deliveries(ctx: &AppContext) -> Vec<webhook_deliveries::Model> {
    webhook_deliveries::Entity::find()
        .order_by_asc(webhook_deliveries::Column::CreatedAt)
        .all(&ctx.db)
        .await
        .unwrap()
}

async fn hook(ctx: &AppContext) -> webhooks::Model {
    webhooks::Entity::find()
        .one(&ctx.db)
        .await
        .unwrap()
        .unwrap()
}

fn expected_signature(secret: &str, ts: &str, body: &str) -> String {
    let mut mac = Hmac::<sha2::Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(format!("{ts}.{body}").as_bytes());
    format!("sha256={}", hex::encode(mac.finalize().into_bytes()))
}

#[tokio::test]
async fn events_are_signed_and_sent_after_commit() {
    request::<App, _, _>(|request, ctx| async move {
        let fake = fake_hook::start().await;
        let f = fixture(&request, &ctx).await;
        let secret = new_hook(
            &request,
            &f.cookie,
            &fake.url,
            &["ticket.created", "comment.created"],
        )
        .await;
        create_ticket(&request, &f.cookie, &[("title", "Checkout QRIS")]).await;

        let got = fake.received();
        assert_eq!(got.len(), 1, "{got:?}");
        let r = &got[0];
        assert_eq!(r.header("x-agentgate-event"), "ticket.created");
        assert_eq!(r.header("content-type"), "application/json");
        let ts = r.header("x-agentgate-timestamp");
        assert!((Utc::now().timestamp() - ts.parse::<i64>().unwrap()).abs() < 60);
        assert_eq!(
            r.header("x-agentgate-signature"),
            expected_signature(&secret, &ts, &r.body)
        );
        let body: Value = serde_json::from_str(&r.body).unwrap();
        assert_eq!(body["event"], "ticket.created");
        assert_eq!(body["workspace"]["slug"], "acme");
        assert_eq!(body["project"]["key"], "KSR");
        assert_eq!(body["data"]["ticket"]["code"], "KSR-1");

        let rows = deliveries(&ctx).await;
        assert_eq!(rows.len(), 1);
        assert_eq!(r.header("x-agentgate-delivery"), rows[0].id.to_string());
        assert_eq!(rows[0].status, "succeeded");
        assert_eq!(rows[0].response_status, Some(200));
        assert_eq!(rows[0].response_body, "{\"received\":true}");
        assert!(rows[0].duration_ms.is_some());
        assert!(rows[0].event_id.is_some());
        let h = hook(&ctx).await;
        assert_eq!(h.last_status, Some(200));
        assert_eq!(h.consecutive_failures, 0);

        // Komentar dilanggan; ticket.updated (tidak dilanggan) tidak dikirim. Ping selalu boleh.
        let res = request
            .post("/w/acme/t/KSR-1/comments")
            .add_cookie(f.cookie.clone())
            .form(&[("body", "Sudah dicek")])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let events: Vec<String> = fake
            .received()
            .iter()
            .map(|r| r.header("x-agentgate-event"))
            .collect();
        assert_eq!(events, ["ticket.created", "comment.created"]);
        let res = request
            .post(&format!("/w/acme/settings/api/hooks/{}/ping", h.id))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303);
        let got = fake.received();
        assert_eq!(got.len(), 3);
        assert_eq!(got[2].header("x-agentgate-event"), "ping");

        // Log menampilkan keduanya, detail memuat payload.
        let res = request
            .get("/w/acme/settings/api/log")
            .add_cookie(f.cookie.clone())
            .await;
        let html = res.text();
        assert_eq!(html.matches("data-delivery=").count(), 3, "{html}");
        assert!(html.contains("&quot;KSR-1&quot;") || html.contains("\"KSR-1\""));
        let res = request
            .get("/w/acme/settings/api/log?result=fail")
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.text().matches("data-delivery=").count(), 0);
    })
    .await;
}

#[tokio::test]
async fn failures_retry_with_backoff_then_disable_the_webhook() {
    request::<App, _, _>(|request, ctx| async move {
        let fake = fake_hook::start().await;
        fake.respond(&[500; 10]);
        let f = fixture(&request, &ctx).await;
        new_hook(&request, &f.cookie, &fake.url, &["ticket.created"]).await;
        create_ticket(&request, &f.cookie, &[("title", "Gagal terus")]).await;
        let first = deliveries(&ctx).await.remove(0);
        assert_eq!(first.attempts, 1);
        assert_eq!(first.status, "pending");
        assert_eq!(first.response_status, Some(500));
        let wait = first.next_attempt_at.unwrap().with_timezone(&Utc) - Utc::now();
        assert!(
            wait > Duration::seconds(30) && wait <= Duration::minutes(1),
            "{wait}"
        );

        let settings = Settings::from_ctx(&ctx).unwrap();
        // Belum jatuh tempo: sapuan tidak mengirim.
        assert_eq!(
            dispatch::sweep(&ctx.db, &settings, Utc::now(), 50)
                .await
                .unwrap(),
            0
        );
        let mut at = Utc::now();
        let mut gaps = Vec::new();
        for _ in 0..4 {
            let row = deliveries(&ctx).await.remove(0);
            let due = row.next_attempt_at.unwrap().with_timezone(&Utc);
            gaps.push((due - at).num_minutes());
            at = due + Duration::seconds(1);
            assert_eq!(
                dispatch::sweep(&ctx.db, &settings, at, 50).await.unwrap(),
                1
            );
        }
        // Jeda bertahap 1, 5, 30, 120 menit.
        assert!(gaps.windows(2).all(|w| w[0] < w[1]), "{gaps:?}");
        assert_eq!(fake.received().len(), 5);
        let row = deliveries(&ctx).await.remove(0);
        assert_eq!(row.attempts, 5);
        assert_eq!(row.status, "failed");
        assert!(row.next_attempt_at.is_none());

        let h = hook(&ctx).await;
        assert!(
            h.disabled_at.is_some(),
            "5 gagal beruntun menonaktifkan webhook"
        );
        assert_eq!(h.consecutive_failures, 5);
        let notes = notifications::Entity::find()
            .filter(notifications::Column::UserId.eq(f.owner.id))
            .filter(notifications::Column::Kind.eq("webhook_disabled"))
            .all(&ctx.db)
            .await
            .unwrap();
        assert_eq!(notes.len(), 1);
        // Event baru tidak lagi diantrekan untuk webhook nonaktif.
        create_ticket(&request, &f.cookie, &[("title", "Setelah nonaktif")]).await;
        assert_eq!(deliveries(&ctx).await.len(), 1);
        let res = request
            .get("/w/acme/settings/api/hooks")
            .add_cookie(f.cookie.clone())
            .await;
        assert_css_exists(
            &res.text(),
            &format!("tr[data-hook='{}'][data-state='off']", h.id),
        );

        // Aktifkan lagi lalu kirim ulang: penerima sudah pulih (antrean 500 habis).
        fake.clear();
        let res = request
            .post(&format!("/w/acme/settings/api/hooks/{}/toggle", h.id))
            .add_cookie(f.cookie.clone())
            .form(&[("active", "1")])
            .await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(hook(&ctx).await.consecutive_failures, 0);
        let res = request
            .post(&format!("/w/acme/settings/api/log/{}/resend", row.id))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303);
        let rows = deliveries(&ctx).await;
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].body, rows[0].body);
        assert_eq!(rows[1].status, "succeeded");
        assert_eq!(fake.received().len(), 6);
    })
    .await;
}

#[tokio::test]
async fn private_and_loopback_targets_are_rejected_in_production_mode() {
    request_with_settings(
        |s| s["api"]["webhook_allow_loopback"] = json!(false),
        |request, ctx| async move {
            let f = fixture(&request, &ctx).await;
            for (url, code) in [
                ("https://127.0.0.1/hook", "webhook-url-forbidden"),
                ("https://10.1.2.3/hook", "webhook-url-forbidden"),
                ("https://192.168.0.10/hook", "webhook-url-forbidden"),
                ("https://169.254.169.254/latest", "webhook-url-forbidden"),
                ("https://[::1]/hook", "webhook-url-forbidden"),
                ("http://127.0.0.1:9/hook", "webhook-url-invalid"),
                ("https://user:pw@contoh.id/hook", "webhook-url-invalid"),
            ] {
                let res = request
                    .post("/w/acme/settings/api/hooks")
                    .add_cookie(f.cookie.clone())
                    .form(&[
                        ("url", url),
                        ("events", "ticket.created"),
                        ("projects", "all"),
                    ])
                    .await;
                assert_eq!(res.status_code(), 422, "{url}");
                assert_css_exists(&res.text(), "dialog#hook-new[data-open-on-load]");
                let text = res.text();
                let msg = if code == "webhook-url-forbidden" {
                    "jaringan internal"
                } else {
                    "URL HTTPS yang valid"
                };
                assert!(text.contains(msg), "{url}");
            }
            assert!(
                webhooks::Entity::find()
                    .one(&ctx.db)
                    .await
                    .unwrap()
                    .is_none()
            );
            // Nama host yang resolve ke loopback ditolak setelah resolve DNS.
            assert_eq!(
                url_guard::resolve("https://localhost:9/hook", false)
                    .await
                    .unwrap_err(),
                "url_forbidden"
            );
        },
    )
    .await;
}

#[tokio::test]
async fn sending_rechecks_the_target_after_dns_resolution() {
    request::<App, _, _>(|request, ctx| async move {
        let fake = fake_hook::start().await;
        let f = fixture(&request, &ctx).await;
        // Disimpan saat loopback diizinkan (dev/test), lalu dikirim dengan aturan produksi.
        let url = fake.url.replace("http://127.0.0.1", "https://localhost");
        new_hook(&request, &f.cookie, &url, &["ticket.created"]).await;
        let mut strict = Settings::from_ctx(&ctx).unwrap();
        strict.api.webhook_allow_loopback = false;
        let h = hook(&ctx).await;
        let delivery = webhook_deliveries::ActiveModel::enqueue(
            &ctx.db,
            f.workspace.workspace_id(),
            h.id,
            None,
            WebhookEvent::Ping,
            "{}".into(),
        )
        .await
        .unwrap();
        assert!(
            dispatch::deliver(&ctx.db, &strict, delivery.id, Utc::now())
                .await
                .unwrap()
        );
        assert!(fake.received().is_empty());
        let row = webhook_deliveries::Entity::find_by_id(delivery.id)
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.error, "url_forbidden");
        assert!(row.response_status.is_none());
    })
    .await;
}
