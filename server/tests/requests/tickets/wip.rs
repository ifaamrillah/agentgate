//! Batas WIP ditegakkan (R3): kolom yang penuh menolak tiket baru lewat board (drag/menu), form
//! detail, API `/api/v1`, dan MCP, dengan pesan yang jelas; Admin project bisa memaksa dengan
//! alasan (audit `ticket.wip_overridden`), anggota biasa tidak.

use agentgate_server::{
    app::App,
    models::{audit_events, ticket_events},
};
use axum::http::Method;
use loco_rs::testing::prelude::*;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;

use super::{
    create_ticket, fixture,
    stages::{member, use_workflow},
    ticket,
};
use crate::{
    requests::{
        api::{json as body_json, new_token, send},
        mcp::call,
    },
    support::oauth_flow,
};

/// To do → Dev (batas WIP 1) → Selesai, tanpa gate.
const WIP_YAML: &str = r##"workflow: wip
nama: Alur WIP
stages:
  - id: todo
    nama: To do
    kategori: todo
  - id: dev
    nama: Dev
    kategori: in_progress
    batas_wip: 1
  - id: done
    nama: Selesai
    kategori: done
"##;

const BOARD: &str = "/w/acme/p/KSR/board";

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn full_column_rejects_moves_everywhere_and_admin_can_force() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        use_workflow(&request, &ctx, &f, WIP_YAML).await;
        let a = create_ticket(&request, &f.cookie, &[("title", "Bayar QRIS")]).await;
        let b = create_ticket(&request, &f.cookie, &[("title", "Struk digital")]).await;
        let move_card = |code: String, return_to: &'static str| {
            let request = &request;
            let cookie = f.cookie.clone();
            async move {
                request
                    .post(&format!("/w/acme/t/{code}/status"))
                    .add_cookie(cookie)
                    .form(&[("stage", "dev"), ("before", ""), ("return_to", return_to)])
                    .await
            }
        };

        // A masuk Dev: kolom jadi penuh (1/1) dan ditandai di board.
        let res = move_card(a.clone(), BOARD).await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let html = request.get(BOARD).add_cookie(f.cookie.clone()).await.text();
        assert_css_exists(&html, "[data-column='dev'][data-wip-full]");
        assert!(
            html.contains("Kolom Dev sudah penuh (batas WIP 1 tiket)"),
            "{html}"
        );

        // Drag/menu dari board: kembali ke board dengan pesan jelas, B tetap di To do.
        let res = move_card(b.clone(), BOARD).await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(res.header("location").to_str().unwrap(), BOARD);
        assert_eq!(ticket(&ctx, &f.workspace, &b).await.stage_key, "todo");
        let html = request.get(BOARD).add_cookie(f.cookie.clone()).await.text();
        assert_css_exists(&html, "[data-flash-key='ticket-wip-full']");
        assert!(html.contains("Kolom Dev sudah penuh"), "{html}");
        // Kartu B tetap di kolom To do.
        let todo = html.find("data-column=\"todo\"").unwrap();
        let dev = html.find("data-column=\"dev\"").unwrap();
        let card_b = html.find(&format!("data-ticket=\"{b}\"")).unwrap();
        assert!(todo < card_b && card_b < dev, "kartu B di kolom To do");

        // Dari halaman tiket (tanpa return_to): 422, pesan di kartu stage.
        let res = move_card(b.clone(), "").await;
        assert_eq!(res.status_code(), 422);
        assert!(res.text().contains("batas WIP"), "{}", res.text());
        // Form "Ubah detail" mengganti status → stage Dev penuh juga ditolak.
        let res = request
            .post(&format!("/w/acme/t/{b}"))
            .add_cookie(f.cookie.clone())
            .form(&[
                ("title", "Struk digital"),
                ("status", "in_progress"),
                ("priority", "medium"),
            ])
            .await;
        assert_eq!(res.status_code(), 422, "{}", res.text());
        assert_eq!(ticket(&ctx, &f.workspace, &b).await.stage_key, "todo");

        // API: 422 + kode + kalimat.
        let token = new_token(&request, &f.cookie, &["tiket:baca", "tiket:tulis"], "all").await;
        let res = send(
            &request,
            &token,
            Method::POST,
            &format!("/api/v1/tickets/{b}/transition"),
            &json!({ "to": "dev" }),
        )
        .await;
        assert_eq!(res.status_code(), 422, "{}", res.text());
        let body = body_json(&res);
        assert_eq!(body["error"], "ticket-wip-full");
        assert!(
            body["message"]
                .as_str()
                .unwrap()
                .contains("Kolom Dev sudah penuh")
        );

        // MCP: tool error berisi alasan.
        let (_, mcp_token, _) =
            oauth_flow::connect(&request, &f.cookie, &f.workspace.id.to_string()).await;
        let (err, value) = call(
            &request,
            &mcp_token,
            "ag_transition",
            json!({ "ticket": b, "to": "dev" }),
        )
        .await;
        assert!(err, "{value}");
        assert!(value.to_string().contains("sudah penuh"), "{value}");
        assert_eq!(ticket(&ctx, &f.workspace, &b).await.stage_key, "todo");

        // Mundur dan urut ulang di kolom yang sama tetap boleh.
        let res = request
            .post(&format!("/w/acme/t/{a}/status"))
            .add_cookie(f.cookie.clone())
            .form(&[("stage", "dev"), ("before", ""), ("return_to", BOARD)])
            .await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(ticket(&ctx, &f.workspace, &a).await.stage_key, "dev");

        // Anggota biasa tidak bisa memaksa (404, tanpa membocorkan aksi Admin).
        let (_, member_cookie) = member(&request, &ctx, &f, "Dewi Dev").await;
        let res = request
            .post(&format!("/w/acme/t/{b}/move-over-wip"))
            .add_cookie(member_cookie)
            .form(&[("stage", "dev"), ("reason", "Hotfix"), ("return_to", BOARD)])
            .await;
        assert_eq!(res.status_code(), 404);

        // Admin tanpa alasan: kembali ke board dengan pesan, tidak dipindah.
        let res = request
            .post(&format!("/w/acme/t/{b}/move-over-wip"))
            .add_cookie(f.cookie.clone())
            .form(&[("stage", "dev"), ("reason", " "), ("return_to", BOARD)])
            .await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(ticket(&ctx, &f.workspace, &b).await.stage_key, "todo");

        // Admin dengan alasan: pindah, audit + riwayat mencatat alasannya.
        let res = request
            .post(&format!("/w/acme/t/{b}/move-over-wip"))
            .add_cookie(f.cookie.clone())
            .form(&[
                ("stage", "dev"),
                ("reason", "Hotfix produksi"),
                ("return_to", BOARD),
            ])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(res.header("location").to_str().unwrap(), BOARD);
        let tb = ticket(&ctx, &f.workspace, &b).await;
        assert_eq!(tb.stage_key, "dev");
        let audit = audit_events::Entity::find()
            .filter(audit_events::Column::Action.eq("ticket.wip_overridden"))
            .one(&ctx.db)
            .await
            .unwrap()
            .expect("audit WIP");
        assert_eq!(audit.changes["wip_override"]["reason"], "Hotfix produksi");
        assert_eq!(audit.changes["code"], b.as_str());
        let event = ticket_events::Entity::find()
            .filter(ticket_events::Column::TicketId.eq(tb.id))
            .all(&ctx.db)
            .await
            .unwrap()
            .into_iter()
            .find(|e| !e.changes["wip_override"].is_null())
            .expect("riwayat WIP");
        assert_eq!(event.changes["wip_override"]["wip"], 1);
        // Board: beban 2/1 (melebihi) ditandai.
        let html = request.get(BOARD).add_cookie(f.cookie.clone()).await.text();
        assert_css_exists(&html, "[data-column='dev'][data-over]");
    })
    .await;
}
