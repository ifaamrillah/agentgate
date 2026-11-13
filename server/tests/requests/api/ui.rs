//! Pengaturan → API & webhook: empat panel (ManageIntegrations; selain itu 404), tautan di menu
//! Pengaturan, buat/cabut token (tampil sekali + audit), CRUD webhook (secret tampil sekali,
//! isian ditolak → 422 dengan dialog terbuka, audit), ping, rotasi secret, log + kirim ulang.

use agentgate_server::{
    app::App,
    models::{api_tokens, webhook_deliveries, webhooks},
};
use loco_rs::testing::prelude::*;
use sea_orm::EntityTrait;

use super::{new_token, raw_secret};
use crate::{
    requests::tickets::{fixture, stages::member},
    support::newest_audit_events,
};

#[tokio::test]
async fn pages_need_manage_integrations() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let res = request
            .get("/w/acme/settings/api")
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(res.header("location"), "/w/acme/settings/api/tokens");
        for panel in ["tokens", "hooks", "log", "docs"] {
            let res = request
                .get(&format!("/w/acme/settings/api/{panel}"))
                .add_cookie(f.cookie.clone())
                .await;
            assert_eq!(res.status_code(), 200, "{panel}: {}", res.text());
            let html = res.text();
            assert_css_exists(&html, &format!("[data-api-page='{panel}']"));
            assert_css_exists(&html, "[data-subnav] [data-nav='api'], [data-nav='api']");
        }
        let res = request
            .get("/w/acme/settings/api/bogus")
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 404);

        let (_, member_cookie) = member(&request, &ctx, &f, "Mira Member").await;
        for path in ["/w/acme/settings/api", "/w/acme/settings/api/tokens"] {
            let res = request.get(path).add_cookie(member_cookie.clone()).await;
            assert_eq!(res.status_code(), 404, "{path}");
        }
        let res = request
            .get("/w/acme/settings")
            .add_cookie(member_cookie.clone())
            .await;
        assert!(!res.text().contains("/w/acme/settings/api"));
    })
    .await;
}

#[tokio::test]
async fn tokens_are_shown_once_listed_and_revoked() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        // Isian ditolak: dialog terbuka lagi dengan pesan.
        let res = request
            .post("/w/acme/settings/api/tokens")
            .add_cookie(f.cookie.clone())
            .form(&[
                ("name", "Tanpa cakupan"),
                ("expiry", "90"),
                ("project", "all"),
            ])
            .await;
        assert_eq!(res.status_code(), 422);
        let html = res.text();
        assert_css_exists(&html, "dialog#token-new[data-open-on-load]");
        assert!(html.contains("Pilih minimal satu cakupan."));
        assert!(html.contains("value=\"Tanpa cakupan\""));

        let raw = new_token(&request, &f.cookie, &["tiket:baca", "laporan:baca"], "KSR").await;
        let token = api_tokens::Entity::find()
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(token.scopes, ["tiket:baca", "laporan:baca"]);
        assert!(token.expires_at.is_some());
        assert!(token.project_id.is_some());
        let audit = newest_audit_events(&ctx.db, f.workspace.workspace_id(), 1)
            .await
            .unwrap();
        assert_eq!(audit[0].action, "api_token.created");
        assert!(!audit[0].changes.to_string().contains(&raw));

        // Daftar hanya memuat prefiks, bukan token mentah.
        let res = request
            .get("/w/acme/settings/api/tokens?q=uji")
            .add_cookie(f.cookie.clone())
            .await;
        let html = res.text();
        assert_css_exists(&html, &format!("tr[data-token='{}']", token.id));
        assert!(html.contains(&token.prefix));
        assert!(!html.contains(&raw));
        let res = request
            .get("/w/acme/settings/api/tokens?q=zzz")
            .add_cookie(f.cookie.clone())
            .await;
        assert_css_not_exists(&res.text(), "tr[data-token]");

        let res = request
            .post(&format!("/w/acme/settings/api/tokens/{}/revoke", token.id))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303);
        let token = api_tokens::Entity::find()
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert!(token.revoked_at.is_some());
        let audit = newest_audit_events(&ctx.db, f.workspace.workspace_id(), 1)
            .await
            .unwrap();
        assert_eq!(audit[0].action, "api_token.revoked");
        let res = request
            .get("/w/acme/settings/api/tokens")
            .add_cookie(f.cookie.clone())
            .await;
        assert_css_not_exists(&res.text(), "tr[data-token]");
    })
    .await;
}

#[tokio::test]
async fn webhooks_are_created_edited_toggled_rotated_and_deleted() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let res = request
            .post("/w/acme/settings/api/hooks")
            .add_cookie(f.cookie.clone())
            .form(&[
                ("url", "ftp://contoh.id"),
                ("events", "ticket.created"),
                ("projects", "all"),
            ])
            .await;
        assert_eq!(res.status_code(), 422);
        let html = res.text();
        assert_css_exists(&html, "dialog#hook-new[data-open-on-load]");
        assert!(html.contains("Masukkan URL HTTPS yang valid"));

        let res = request
            .post("/w/acme/settings/api/hooks")
            .add_cookie(f.cookie.clone())
            .form(&[
                ("url", "https://contoh.id/hooks/agentgate"),
                ("events", "ticket.created"),
                ("events", "sprint.completed"),
                ("projects", "KSR"),
            ])
            .await;
        assert_eq!(res.status_code(), 200, "{}", res.text());
        let html = res.text();
        assert_css_exists(
            &html,
            "dialog#hook-secret[data-open-on-load] [data-raw-secret]",
        );
        let secret = raw_secret(&html, "whsec_");
        let hook = webhooks::Entity::find()
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(hook.project_ids.len(), 1);
        assert!(
            !hook
                .secret_encrypted
                .windows(secret.len())
                .any(|w| w == secret.as_bytes())
        );
        let key = agentgate_server::common::settings::Settings::from_ctx(&ctx)
            .unwrap()
            .auth
            .encryption_key_bytes()
            .unwrap();
        assert_eq!(hook.secret(&key).unwrap(), secret);
        let audit = newest_audit_events(&ctx.db, f.workspace.workspace_id(), 1)
            .await
            .unwrap();
        assert_eq!(audit[0].action, "webhook.created");

        let res = request
            .get("/w/acme/settings/api/hooks")
            .add_cookie(f.cookie.clone())
            .await;
        let html = res.text();
        assert_css_exists(
            &html,
            &format!("tr[data-hook='{}'][data-state='none']", hook.id),
        );
        assert!(!html.contains(&secret));

        // Ubah: event dan semua project.
        let base = format!("/w/acme/settings/api/hooks/{}", hook.id);
        let res = request
            .post(&base)
            .add_cookie(f.cookie.clone())
            .form(&[
                ("url", "https://contoh.id/hooks/v2"),
                ("events", "ticket.stage_changed"),
                ("projects", "all"),
            ])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let hook = webhooks::Entity::find()
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(hook.url, "https://contoh.id/hooks/v2");
        assert_eq!(hook.events, ["ticket.stage_changed"]);
        assert!(hook.project_ids.is_empty());
        // Isian ubah yang ditolak membuka dialog ubah.
        let res = request
            .post(&base)
            .add_cookie(f.cookie.clone())
            .form(&[("url", "https://contoh.id/x"), ("projects", "all")])
            .await;
        assert_eq!(res.status_code(), 422);
        assert_css_exists(
            &res.text(),
            &format!("dialog#hook-edit-{}[data-open-on-load]", hook.id),
        );

        // Nonaktif (switch tidak tercentang = tanpa nilai) → aktif.
        let res = request
            .post(&format!("{base}/toggle"))
            .add_cookie(f.cookie.clone())
            .form(&[("active", "")])
            .await;
        assert_eq!(res.status_code(), 303);
        let off = webhooks::Entity::find()
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert!(off.disabled_at.is_some());
        // Ping ke webhook nonaktif ditolak.
        let res = request
            .post(&format!("{base}/ping"))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303);
        assert!(
            webhook_deliveries::Entity::find()
                .one(&ctx.db)
                .await
                .unwrap()
                .is_none()
        );
        let res = request
            .post(&format!("{base}/toggle"))
            .add_cookie(f.cookie.clone())
            .form(&[("active", "1")])
            .await;
        assert_eq!(res.status_code(), 303);
        assert!(
            webhooks::Entity::find()
                .one(&ctx.db)
                .await
                .unwrap()
                .unwrap()
                .disabled_at
                .is_none()
        );

        // Rotasi: secret baru tampil sekali dan berbeda.
        let res = request
            .post(&format!("{base}/rotate"))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 200);
        let rotated = raw_secret(&res.text(), "whsec_");
        assert_ne!(rotated, secret);
        let actions: Vec<String> = newest_audit_events(&ctx.db, f.workspace.workspace_id(), 10)
            .await
            .unwrap()
            .into_iter()
            .map(|a| a.action)
            .collect();
        assert!(
            actions.contains(&"webhook.updated".to_string()),
            "{actions:?}"
        );
        assert!(
            actions.contains(&"webhook.secret_rotated".to_string()),
            "{actions:?}"
        );

        let res = request
            .post(&format!("{base}/delete"))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303);
        assert!(
            webhooks::Entity::find()
                .one(&ctx.db)
                .await
                .unwrap()
                .is_none()
        );
        let audit = newest_audit_events(&ctx.db, f.workspace.workspace_id(), 1)
            .await
            .unwrap();
        assert_eq!(audit[0].action, "webhook.deleted");
    })
    .await;
}
