//! Bukti gate manual (R1): anggota mengunggah screenshot/log/teks berlabel Manual; gate evidence
//! baru terpenuhi setelah approver stage menyetujui; semua langkah di audit dan riwayat tiket.

use agentgate_server::{app::App, models::manual_evidence};
use axum_test::multipart::{MultipartForm, Part};
use loco_rs::{TestServer, auth::cookies::Cookie, testing::prelude::*};
use sea_orm::EntityTrait;

use super::{
    create_ticket, fixture,
    stages::{member, use_workflow},
    ticket,
};
use crate::support::newest_audit_events;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDRfake";

fn flow(approver: &str) -> String {
    format!(
        r"workflow: manual
nama: Alur Bukti Manual
stages:
  - id: todo
    nama: To do
    kategori: todo
  - id: qa
    nama: QA
    kategori: in_progress
    mode: manual
    gate: [tests_passed, screenshot_exists]
    transisi: tunggu_approve
    penugasan:
      approver: [{approver}]
  - id: done
    nama: Selesai
    kategori: done
"
    )
}

fn upload(gate: &str, kind: &str, file: Option<(&[u8], &str, &str)>, text: &str) -> MultipartForm {
    let mut form = MultipartForm::new()
        .add_text("gate", gate.to_string())
        .add_text("kind", kind.to_string())
        .add_text("text", text.to_string())
        .add_text("note", "Diuji manual di staging".to_string());
    if let Some((bytes, name, mime)) = file {
        form = form.add_part(
            "file",
            Part::bytes(bytes.to_vec()).file_name(name).mime_type(mime),
        );
    }
    form
}

async fn page(request: &TestServer, cookie: &Cookie<'static>) -> String {
    request
        .get("/w/acme/t/KSR-1")
        .add_cookie(cookie.clone())
        .await
        .text()
}

async fn rows(ctx: &loco_rs::app::AppContext) -> Vec<manual_evidence::Model> {
    manual_evidence::Entity::find().all(&ctx.db).await.unwrap()
}

#[tokio::test]
async fn manual_evidence_satisfies_gate_only_after_stage_approver_approves() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        use_workflow(&request, &ctx, &f, &flow(&f.owner.email)).await;
        create_ticket(&request, &f.cookie, &[("title", "Checkout QRIS")]).await;
        let res = request
            .post("/w/acme/t/KSR-1/stage/complete")
            .add_cookie(f.cookie.clone())
            .form(&[("x", "")])
            .await;
        assert_eq!(res.status_code(), 303);
        let (_, mia) = member(&request, &ctx, &f, "Mia Member").await;
        let html = page(&request, &mia).await;
        assert_css_exists(&html, "[data-manual-evidence] [data-manual-upload]");
        assert_css_exists(&html, "[data-gate='tests_passed'][data-ok='false']");

        // Ditolak: screenshot_exists hanya menerima screenshot; berkas bukan gambar.
        for form in [
            upload(
                "screenshot_exists",
                "log",
                Some((b"ok", "a.log", "text/plain")),
                "",
            ),
            upload(
                "screenshot_exists",
                "screenshot",
                Some((b"GIF89a", "a.gif", "image/gif")),
                "",
            ),
            upload("tests_passed", "text", None, "   "),
            upload("prs_merged", "text", None, "lulus"),
        ] {
            let res = request
                .post("/w/acme/t/KSR-1/evidence")
                .add_cookie(mia.clone())
                .multipart(form)
                .await;
            assert_eq!(res.status_code(), 303);
        }
        assert!(rows(&ctx).await.is_empty());

        // Member mengunggah screenshot dan log hasil test.
        for form in [
            upload(
                "screenshot_exists",
                "screenshot",
                Some((PNG, "checkout.png", "image/png")),
                "",
            ),
            upload(
                "tests_passed",
                "log",
                Some((b"42 passed, 0 failed\n", "test.log", "text/plain")),
                "",
            ),
        ] {
            let res = request
                .post("/w/acme/t/KSR-1/evidence")
                .add_cookie(mia.clone())
                .multipart(form)
                .await;
            assert_eq!(res.status_code(), 303, "{}", res.text());
        }
        let list = rows(&ctx).await;
        assert_eq!(list.len(), 2);
        assert!(list.iter().all(|r| r.status == "pending"));

        // Masih menunggu: gate belum terpenuhi, label Manual tampil, member tidak bisa approve.
        let html = page(&request, &mia).await;
        assert_css_exists(&html, "[data-gate='screenshot_exists'][data-ok='false']");
        assert_css_exists(
            &html,
            "[data-manual][data-status='pending'] [data-manual-label]",
        );
        assert_css_not_exists(&html, "[data-manual-approve]");
        let shot = list.iter().find(|r| r.kind == "screenshot").unwrap();
        let log = list.iter().find(|r| r.kind == "log").unwrap();
        let res = request
            .post(&format!("/w/acme/t/KSR-1/evidence/{}/approve", shot.id))
            .add_cookie(mia.clone())
            .form(&[("note", "")])
            .await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(
            manual_evidence::Entity::find_by_id(shot.id)
                .one(&ctx.db)
                .await
                .unwrap()
                .unwrap()
                .status,
            "pending"
        );

        // Berkas bisa dibuka anggota (nosniff); log disajikan sebagai teks unduhan.
        let res = request
            .get(&format!("/w/acme/t/KSR-1/evidence/{}", log.id))
            .add_cookie(mia.clone())
            .await;
        assert_eq!(res.status_code(), 200);
        assert_eq!(res.header("x-content-type-options"), "nosniff");
        assert!(
            res.header("content-type")
                .to_str()
                .unwrap()
                .starts_with("text/plain")
        );

        // Approver: tolak tanpa catatan ditolak; tolak log dengan catatan, setujui screenshot.
        let html = page(&request, &f.cookie).await;
        assert_css_exists(&html, "button[data-manual-approve][data-variant='success']");
        let res = request
            .post(&format!("/w/acme/t/KSR-1/evidence/{}/reject", log.id))
            .add_cookie(f.cookie.clone())
            .form(&[("note", "")])
            .await;
        assert_eq!(res.status_code(), 303);
        let res = request
            .post(&format!("/w/acme/t/KSR-1/evidence/{}/reject", log.id))
            .add_cookie(f.cookie.clone())
            .form(&[("note", "Log tidak memuat suite checkout")])
            .await;
        assert_eq!(res.status_code(), 303);
        let res = request
            .post(&format!("/w/acme/t/KSR-1/evidence/{}/approve", shot.id))
            .add_cookie(f.cookie.clone())
            .form(&[("note", "")])
            .await;
        assert_eq!(res.status_code(), 303);
        let html = page(&request, &f.cookie).await;
        assert_css_exists(&html, "[data-gate='screenshot_exists'][data-ok='true']");
        assert_css_exists(&html, "[data-gate='tests_passed'][data-ok='false']");
        // Sudah diputuskan: tidak bisa diputuskan ulang.
        let res = request
            .post(&format!("/w/acme/t/KSR-1/evidence/{}/approve", log.id))
            .add_cookie(f.cookie.clone())
            .form(&[("note", "")])
            .await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(
            manual_evidence::Entity::find_by_id(log.id)
                .one(&ctx.db)
                .await
                .unwrap()
                .unwrap()
                .status,
            "rejected"
        );

        // Teks hasil test → disetujui → semua gate terpenuhi, stage bisa minta review.
        let res = request
            .post("/w/acme/t/KSR-1/evidence")
            .add_cookie(mia.clone())
            .multipart(upload(
                "tests_passed",
                "text",
                None,
                "Suite checkout: 42 lulus",
            ))
            .await;
        assert_eq!(res.status_code(), 303);
        let text = rows(&ctx)
            .await
            .into_iter()
            .find(|r| r.kind == "text")
            .unwrap();
        request
            .post(&format!("/w/acme/t/KSR-1/evidence/{}/approve", text.id))
            .add_cookie(f.cookie.clone())
            .form(&[("note", "")])
            .await;
        let html = page(&request, &f.cookie).await;
        assert_css_exists(&html, "[data-gate='tests_passed'][data-ok='true']");
        assert_css_exists(&html, "[data-manual][data-status='approved'] .app-ev-text");
        let res = request
            .post("/w/acme/t/KSR-1/stage/complete")
            .add_cookie(mia.clone())
            .form(&[("x", "")])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(
            ticket(&ctx, &f.workspace, "KSR-1").await.stage_state,
            "review"
        );

        // Audit + riwayat tiket mencatat semua langkah.
        let actions: Vec<String> = newest_audit_events(&ctx.db, f.workspace.workspace_id(), 20)
            .await
            .unwrap()
            .into_iter()
            .map(|e| e.action)
            .collect();
        for a in [
            "evidence.submitted",
            "evidence.approved",
            "evidence.rejected",
        ] {
            assert!(actions.iter().any(|x| x == a), "{a}: {actions:?}");
        }
        let activity = request
            .get("/w/acme/t/KSR-1/activity")
            .add_cookie(f.cookie.clone())
            .await
            .text();
        assert!(
            activity.contains("mengunggah bukti manual"),
            "riwayat unggah"
        );
        assert!(
            activity.contains("memutuskan bukti manual"),
            "riwayat keputusan"
        );
    })
    .await;
}

#[tokio::test]
async fn viewers_cannot_upload_manual_evidence() {
    request::<App, _, _>(|request, ctx| async move {
        use agentgate_server::models::{project_grants::GrantSubject, roles::RoleKey};
        let f = fixture(&request, &ctx).await;
        use_workflow(&request, &ctx, &f, &flow(&f.owner.email)).await;
        create_ticket(&request, &f.cookie, &[("title", "Checkout")]).await;
        request
            .post("/w/acme/t/KSR-1/stage/complete")
            .add_cookie(f.cookie.clone())
            .form(&[("x", "")])
            .await;
        let viewer = crate::support::a_user().create(&ctx).await;
        crate::support::add_member(&ctx, &f.workspace, &viewer, RoleKey::Viewer).await;
        let role = crate::support::builtin_role(&ctx, &f.workspace, RoleKey::Viewer).await;
        crate::support::grant(
            &ctx,
            &f.workspace,
            &super::stages::ksr(&ctx, &f).await,
            GrantSubject::User(viewer.id),
            &role,
        )
        .await;
        let cookie = crate::support::sign_in_as(&request, &viewer).await;
        assert_css_not_exists(&page(&request, &cookie).await, "[data-manual-upload]");
        let res = request
            .post("/w/acme/t/KSR-1/evidence")
            .add_cookie(cookie)
            .multipart(upload("tests_passed", "text", None, "lulus"))
            .await;
        assert_eq!(res.status_code(), 404);
        assert!(rows(&ctx).await.is_empty());
    })
    .await;
}

/// Enum `manual_evidence` (gate, jenis, status) sepakat dengan CHECK-nya (rules §4).
#[tokio::test]
async fn manual_evidence_checks_match_enums() {
    use agentgate_server::models::manual_evidence::{ManualGate, ManualKind, ManualStatus};
    use sea_orm::{ActiveEnum, ConnectionTrait, DbBackend, Iterable, Statement, Value};
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        create_ticket(&request, &f.cookie, &[("title", "Enum")]).await;
        let t = ticket(&ctx, &f.workspace, "KSR-1").await;
        let insert = |gate: String, kind: String, status: String| {
            let sql = Statement::from_sql_and_values(
                DbBackend::Postgres,
                "INSERT INTO manual_evidence (workspace_id, ticket_id, stage_key, gate, kind, name, \
                 status) VALUES ($1, $2, 'qa', $3, $4, 'x', $5)",
                [
                    Value::from(f.workspace.id),
                    Value::from(t.id),
                    Value::from(gate),
                    Value::from(kind),
                    Value::from(status),
                ],
            );
            let db = ctx.db.clone();
            async move { db.execute_raw(sql).await.is_ok() }
        };
        for g in ManualGate::iter() {
            assert!(insert(g.to_value(), "text".into(), "pending".into()).await, "{g:?}");
        }
        for k in ManualKind::iter() {
            assert!(insert("tests_passed".into(), k.to_value(), "pending".into()).await, "{k:?}");
        }
        for s in ManualStatus::iter() {
            assert!(insert("tests_passed".into(), "text".into(), s.to_value()).await, "{s:?}");
        }
        assert!(!insert("bogus".into(), "text".into(), "pending".into()).await);
        assert!(!insert("tests_passed".into(), "bogus".into(), "pending".into()).await);
        assert!(!insert("tests_passed".into(), "text".into(), "bogus".into()).await);
    })
    .await;
}
