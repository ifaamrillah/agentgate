//! Workflow per tiket (M3): stage awal, gate, minta review, approve/reject oleh approver, batas
//! gagal → Butuh keputusan, lima keputusan, lompat/paksa oleh Admin, penugasan stage, artifact
//! (versi, pending saat review, bandingkan), tab kotak masuk "Butuh keputusan", kolom board per
//! stage.

use agentgate_server::{
    app::App,
    models::{
        artifact_versions, project_grants::GrantSubject, projects, roles::RoleKey,
        ticket_artifacts, ticket_events, ticket_types, tickets,
    },
};
use loco_rs::{TestServer, app::AppContext, auth::cookies::Cookie, testing::prelude::*};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait, IntoActiveModel};

use super::{Fixture, create_ticket, fixture, ticket};
use crate::support::{a_user, add_member, builtin_role, grant, sign_in_as};

/// Workflow uji: To do → PRD (artifact wajib, approval, batas gagal 2) → Dev (gate evidence,
/// manual) → Selesai.
pub const FLOW_YAML: &str = r##"workflow: uji
nama: Alur Uji
stages:
  - id: todo
    nama: To do
    kategori: todo
  - id: prd
    nama: PRD
    kategori: in_progress
    output: prd.md
    template_output: |
      # PRD
    gate: [artifact_exists]
    transisi: tunggu_approve
    batas_gagal: 2
  - id: dev
    nama: Dev
    kategori: in_progress
    gate: [tests_passed]
  - id: done
    nama: Selesai
    kategori: done
"##;

/// Impor `yaml` lalu jadikan workflow semua tipe tiket di project KSR. Mengembalikan id workflow.
pub async fn use_workflow(
    request: &TestServer,
    ctx: &AppContext,
    f: &Fixture,
    yaml: &str,
) -> String {
    let res = request
        .post("/w/acme/settings/workflows/import")
        .add_cookie(f.cookie.clone())
        .form(&[("yaml", yaml)])
        .await;
    assert_eq!(res.status_code(), 303, "{}", res.text());
    let id = res
        .header("location")
        .to_str()
        .unwrap()
        .rsplit('/')
        .next()
        .unwrap()
        .to_string();
    let types = ticket_types::Model::all_in(&ctx.db, f.workspace.workspace_id())
        .await
        .unwrap();
    let fields: Vec<(String, String)> = types
        .iter()
        .map(|t| (format!("type_{}", t.id), id.clone()))
        .collect();
    let res = request
        .post("/w/acme/p/KSR/settings/workflow")
        .add_cookie(f.cookie.clone())
        .form(&fields)
        .await;
    assert_eq!(res.status_code(), 303, "{}", res.text());
    id
}

async fn post(
    request: &TestServer,
    cookie: &Cookie<'static>,
    path: &str,
    form: &[(&str, &str)],
) -> axum_test::TestResponse {
    request
        .post(path)
        .add_cookie(cookie.clone())
        .form(form)
        .await
}

pub async fn ksr(ctx: &AppContext, f: &Fixture) -> projects::Model {
    projects::Model::find_by_key(&ctx.db, f.workspace.workspace_id(), "KSR")
        .await
        .unwrap()
        .unwrap()
}

/// Anggota dengan role Member di project KSR.
pub async fn member(
    request: &TestServer,
    ctx: &AppContext,
    f: &Fixture,
    name: &str,
) -> (uuid::Uuid, Cookie<'static>) {
    let user = a_user().name(name).create(ctx).await;
    add_member(ctx, &f.workspace, &user, RoleKey::Member).await;
    let role = builtin_role(ctx, &f.workspace, RoleKey::Member).await;
    grant(
        ctx,
        &f.workspace,
        &ksr(ctx, f).await,
        GrantSubject::User(user.id),
        &role,
    )
    .await;
    let cookie = sign_in_as(request, &user).await;
    (user.id, cookie)
}

fn stage_error(html: &str, code: &str) {
    assert_css_exists(html, &format!("[data-stage-error='{code}']"));
}

#[tokio::test]
async fn ticket_moves_through_gates_review_and_approval() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        use_workflow(&request, &ctx, &f, FLOW_YAML).await;
        let code = create_ticket(&request, &f.cookie, &[("title", "Checkout QRIS")]).await;
        let t = ticket(&ctx, &f.workspace, &code).await;
        assert_eq!(t.stage_key, "todo");

        // Detail menampilkan stepper dan kartu stage.
        let page = request
            .get("/w/acme/t/KSR-1")
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(page.status_code(), 200);
        let html = page.text();
        assert_css_exists(&html, "[data-stepper] [data-stage='prd']");
        assert_css_exists(
            &html,
            "[data-stepper] [data-stage='todo'][aria-current='step']",
        );
        assert_css_exists(&html, "[data-stage-card]");

        // To do (manual) → PRD.
        let res = post(&request, &f.cookie, "/w/acme/t/KSR-1/stage/complete", &[]).await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(ticket(&ctx, &f.workspace, &code).await.stage_key, "prd");

        // Gate artifact belum terpenuhi → 422.
        let res = post(&request, &f.cookie, "/w/acme/t/KSR-1/stage/complete", &[]).await;
        assert_eq!(res.status_code(), 422);
        stage_error(&res.text(), "ticket-gates-unmet");
        // Langsung pindah ke Dev lewat board juga ditolak (stage butuh approval).
        let res = post(
            &request,
            &f.cookie,
            "/w/acme/t/KSR-1/status",
            &[("stage", "dev")],
        )
        .await;
        assert_eq!(res.status_code(), 422);

        // Artifact: nama kosong = output stage, isi = template.
        let res = post(
            &request,
            &f.cookie,
            "/w/acme/t/KSR-1/artifacts",
            &[("name", "")],
        )
        .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let t = ticket(&ctx, &f.workspace, &code).await;
        let wid = f.workspace.workspace_id();
        let artifact = ticket_artifacts::Model::find_named(&ctx.db, wid, t.id, "prd.md")
            .await
            .unwrap()
            .expect("prd.md dibuat");
        let v1 = artifact_versions::Model::latest(&ctx.db, wid, artifact.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(v1.body.trim(), "# PRD");
        assert_eq!(v1.status, "draft");
        let res = post(
            &request,
            &f.cookie,
            "/w/acme/t/KSR-1/artifacts",
            &[("name", "prd.md")],
        )
        .await;
        assert_eq!(res.status_code(), 422);

        // Minta review → menunggu review.
        let res = post(&request, &f.cookie, "/w/acme/t/KSR-1/stage/complete", &[]).await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(
            ticket(&ctx, &f.workspace, &code).await.stage_state,
            "review"
        );

        // Simpan versi saat review → pending; isi sama ditolak.
        let save = format!("/w/acme/t/KSR-1/artifacts/{}", artifact.id);
        let res = post(
            &request,
            &f.cookie,
            &save,
            &[("body", "# PRD\n\nBayar QRIS.")],
        )
        .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let v2 = artifact_versions::Model::latest(&ctx.db, wid, artifact.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!((v2.number, v2.status.as_str()), (2, "pending"));
        let res = post(
            &request,
            &f.cookie,
            &save,
            &[("body", "# PRD\n\nBayar QRIS.")],
        )
        .await;
        assert_eq!(res.status_code(), 422);

        // Halaman artifact, editor, dan bandingkan.
        let page = request
            .get("/w/acme/t/KSR-1/artifacts")
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(page.status_code(), 200);
        let html = page.text();
        assert_css_exists(&html, "[data-artifact='prd.md']");
        assert_css_exists(&html, "[data-artifact-preview] [data-version='2']");
        assert_css_exists(&html, "[data-stage-alert='review-mine']");
        let page = request
            .get(&format!("{save}/edit"))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(page.status_code(), 200);
        let page = request
            .get(&format!("{save}/compare?from=1&to=2"))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(page.status_code(), 200);
        let html = page.text();
        assert_css_exists(&html, "[data-diff] [data-line='add']");

        // Approver ditetapkan Rina: pemilik bukan approver → ditolak; Rina approve.
        let (rina, rina_cookie) = member(&request, &ctx, &f, "Rina Reviewer").await;
        let res = post(
            &request,
            &f.cookie,
            "/w/acme/t/KSR-1/stage/people",
            &[("approvers", &rina.to_string())],
        )
        .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let res = post(&request, &f.cookie, "/w/acme/t/KSR-1/stage/approve", &[]).await;
        assert_eq!(res.status_code(), 422);
        stage_error(&res.text(), "ticket-not-approver");

        // Kotak masuk Rina: tab Butuh keputusan memuat tiket.
        let inbox = request
            .get("/w/acme/inbox?tab=decisions")
            .add_cookie(rina_cookie.clone())
            .await;
        assert_eq!(inbox.status_code(), 200);
        assert_css_exists(&inbox.text(), "[data-decision-ticket='KSR-1']");

        let res = post(&request, &rina_cookie, "/w/acme/t/KSR-1/stage/approve", &[]).await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let t = ticket(&ctx, &f.workspace, &code).await;
        assert_eq!(
            (t.stage_key.as_str(), t.stage_state.as_str()),
            ("dev", "active")
        );
        let v2 = artifact_versions::Model::latest(&ctx.db, wid, artifact.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(v2.status, "approved");

        // Dev: gate evidence tidak pernah terpenuhi di M3.
        let res = post(&request, &f.cookie, "/w/acme/t/KSR-1/stage/complete", &[]).await;
        assert_eq!(res.status_code(), 422);
        stage_error(&res.text(), "ticket-gates-unmet");

        // Paksa transisi dimatikan per project; alasan wajib.
        let res = post(
            &request,
            &f.cookie,
            "/w/acme/t/KSR-1/stage/admin-move",
            &[("stage", "done"), ("kind", "force"), ("reason", "Hotfix")],
        )
        .await;
        assert_eq!(res.status_code(), 422);
        let res = post(
            &request,
            &f.cookie,
            "/w/acme/t/KSR-1/stage/admin-move",
            &[("stage", "done"), ("kind", "jump"), ("reason", "")],
        )
        .await;
        assert_eq!(res.status_code(), 422);
        assert_css_exists(&res.text(), "#admin-move-reason-error");
        // Member bukan Admin: rute lompat 404.
        let res = post(
            &request,
            &rina_cookie,
            "/w/acme/t/KSR-1/stage/admin-move",
            &[("stage", "done"), ("kind", "jump"), ("reason", "Cepat")],
        )
        .await;
        assert_eq!(res.status_code(), 404);

        // Aktifkan paksa transisi → Admin memaksa dengan alasan.
        let mut p = ksr(&ctx, &f).await.into_active_model();
        p.allow_force_transition = Set(true);
        p.update(&ctx.db).await.unwrap();
        let res = post(
            &request,
            &f.cookie,
            "/w/acme/t/KSR-1/stage/admin-move",
            &[
                ("stage", "done"),
                ("kind", "force"),
                ("reason", "Sudah diuji manual"),
            ],
        )
        .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let t = ticket(&ctx, &f.workspace, &code).await;
        assert_eq!(t.stage_key, "done");
        assert!(t.done_at.is_some());

        // Riwayat mencatat alasan.
        let page = request
            .get("/w/acme/t/KSR-1/activity")
            .add_cookie(f.cookie.clone())
            .await;
        assert!(page.text().contains("Sudah diuji manual"));
    })
    .await;
}

#[tokio::test]
async fn jumping_over_an_approval_stage_needs_admin_and_reason() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        use_workflow(&request, &ctx, &f, FLOW_YAML).await;
        let code = create_ticket(&request, &f.cookie, &[("title", "Lompat")]).await;
        let (_, rina) = member(&request, &ctx, &f, "Rina").await;

        // Member memindah lewat board ke Dev = melewati PRD (approval) → ditolak.
        let res = post(
            &request,
            &rina,
            "/w/acme/t/KSR-1/status",
            &[("stage", "dev")],
        )
        .await;
        assert_eq!(res.status_code(), 422);
        assert_css_exists(&res.text(), "[data-status-error]");
        // Stage tak dikenal.
        let res = post(
            &request,
            &rina,
            "/w/acme/t/KSR-1/status",
            &[("stage", "nope")],
        )
        .await;
        assert_eq!(res.status_code(), 422);

        // Admin melompat dengan alasan.
        let res = post(
            &request,
            &f.cookie,
            "/w/acme/t/KSR-1/stage/admin-move",
            &[
                ("stage", "dev"),
                ("kind", "jump"),
                ("reason", "PRD sudah ada di Notion"),
            ],
        )
        .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(ticket(&ctx, &f.workspace, &code).await.stage_key, "dev");
        // Mundur bebas untuk Member.
        let res = post(
            &request,
            &rina,
            "/w/acme/t/KSR-1/status",
            &[("stage", "todo")],
        )
        .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(ticket(&ctx, &f.workspace, &code).await.stage_key, "todo");
    })
    .await;
}

/// Bawa KSR-1 ke PRD menunggu review (artifact sudah ada).
async fn to_review(request: &TestServer, cookie: &Cookie<'static>) {
    for (path, form) in [
        ("/w/acme/t/KSR-1/stage/complete", vec![]),
        ("/w/acme/t/KSR-1/artifacts", vec![("name", "")]),
        ("/w/acme/t/KSR-1/stage/complete", vec![]),
    ] {
        let res = post(request, cookie, path, &form).await;
        assert_eq!(res.status_code(), 303, "{path}: {}", res.text());
    }
}

#[tokio::test]
async fn rejects_count_rounds_and_the_limit_asks_for_a_decision() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        use_workflow(&request, &ctx, &f, FLOW_YAML).await;
        let code = create_ticket(&request, &f.cookie, &[("title", "Gagal terus")]).await;
        to_review(&request, &f.cookie).await;

        // Reject tanpa catatan ditolak (dialog terbuka dengan error).
        let res = post(
            &request,
            &f.cookie,
            "/w/acme/t/KSR-1/stage/reject",
            &[("note", " ")],
        )
        .await;
        assert_eq!(res.status_code(), 422);
        assert_css_exists(&res.text(), "#reject-note-error");

        let res = post(
            &request,
            &f.cookie,
            "/w/acme/t/KSR-1/stage/reject",
            &[("note", "Acceptance criteria kurang")],
        )
        .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let t = ticket(&ctx, &f.workspace, &code).await;
        assert_eq!(
            (t.stage_key.as_str(), t.stage_state.as_str(), t.fail_count),
            ("prd", "active", 1)
        );

        // Putaran kedua mencapai batas (2) → Butuh keputusan.
        let res = post(&request, &f.cookie, "/w/acme/t/KSR-1/stage/complete", &[]).await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let res = post(
            &request,
            &f.cookie,
            "/w/acme/t/KSR-1/stage/reject",
            &[("note", "Masih kurang")],
        )
        .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let t = ticket(&ctx, &f.workspace, &code).await;
        assert_eq!((t.stage_state.as_str(), t.fail_count), ("decision", 2));

        let page = request
            .get("/w/acme/t/KSR-1")
            .add_cookie(f.cookie.clone())
            .await;
        let html = page.text();
        assert_css_exists(&html, "[data-stage-alert='decision']");
        assert_css_exists(&html, "#decision [data-round]");
        let inbox = request
            .get("/w/acme/inbox?tab=decisions")
            .add_cookie(f.cookie.clone())
            .await;
        assert_css_exists(&inbox.text(), "[data-decision-ticket='KSR-1']");
        // Board menandai kartu.
        let board = request
            .get("/w/acme/p/KSR/board")
            .add_cookie(f.cookie.clone())
            .await;
        assert_css_exists(
            &board.text(),
            "[data-column='prd'] [data-ticket='KSR-1'] .app-tcard-state",
        );

        // Petunjuk wajib catatan; pilihan tak dikenal ditolak.
        let res = post(
            &request,
            &f.cookie,
            "/w/acme/t/KSR-1/stage/decide",
            &[("choice", "hint"), ("note", "")],
        )
        .await;
        assert_eq!(res.status_code(), 422);
        let res = post(
            &request,
            &f.cookie,
            "/w/acme/t/KSR-1/stage/decide",
            &[("choice", "magic")],
        )
        .await;
        assert_eq!(res.status_code(), 422);

        // Terima dengan catatan: lanjut ke Dev, tiket Bug baru berelasi.
        let res = post(
            &request,
            &f.cookie,
            "/w/acme/t/KSR-1/stage/decide",
            &[("choice", "accept"), ("note", "Tambah AC refund nanti")],
        )
        .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let t = ticket(&ctx, &f.workspace, &code).await;
        assert_eq!(
            (t.stage_key.as_str(), t.stage_state.as_str(), t.fail_count),
            ("dev", "active", 0)
        );
        let bug = ticket(&ctx, &f.workspace, "KSR-2").await;
        assert!(
            bug.title.contains("Tambah AC refund nanti"),
            "{}",
            bug.title
        );
        let bug_type = ticket_types::Entity::find_by_id(bug.type_id)
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(bug_type.key, "bug");
    })
    .await;
}

#[tokio::test]
async fn decisions_hint_self_requirement_and_cancel() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        use_workflow(&request, &ctx, &f, FLOW_YAML).await;
        let code = create_ticket(&request, &f.cookie, &[("title", "Keputusan")]).await;
        to_review(&request, &f.cookie).await;
        let to_decision = || async {
            for _ in 0..2 {
                let t = ticket(&ctx, &f.workspace, &code).await;
                if t.stage_state == "active" {
                    let res =
                        post(&request, &f.cookie, "/w/acme/t/KSR-1/stage/complete", &[]).await;
                    assert_eq!(res.status_code(), 303, "{}", res.text());
                }
                let res = post(
                    &request,
                    &f.cookie,
                    "/w/acme/t/KSR-1/stage/reject",
                    &[("note", "x")],
                )
                .await;
                assert_eq!(res.status_code(), 303, "{}", res.text());
            }
            assert_eq!(
                ticket(&ctx, &f.workspace, &code).await.stage_state,
                "decision"
            );
        };
        to_decision().await;

        // Keputusan hanya saat Butuh keputusan; petunjuk mereset hitungan.
        let res = post(
            &request,
            &f.cookie,
            "/w/acme/t/KSR-1/stage/decide",
            &[("choice", "hint"), ("note", "Lihat PRD lama")],
        )
        .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let t = ticket(&ctx, &f.workspace, &code).await;
        assert_eq!(
            (t.stage_key.as_str(), t.stage_state.as_str(), t.fail_count),
            ("prd", "active", 0)
        );
        let res = post(
            &request,
            &f.cookie,
            "/w/acme/t/KSR-1/stage/decide",
            &[("choice", "self")],
        )
        .await;
        assert_eq!(res.status_code(), 422);

        // Perbaiki sendiri: penerima tugas = pengambil keputusan.
        to_decision().await;
        let res = post(
            &request,
            &f.cookie,
            "/w/acme/t/KSR-1/stage/decide",
            &[("choice", "self")],
        )
        .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(
            ticket(&ctx, &f.workspace, &code).await.assignee_id,
            Some(f.owner.id)
        );

        // Ubah requirement: kembali ke stage sebelumnya.
        to_decision().await;
        let res = post(
            &request,
            &f.cookie,
            "/w/acme/t/KSR-1/stage/decide",
            &[("choice", "requirement"), ("target", "todo")],
        )
        .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(ticket(&ctx, &f.workspace, &code).await.stage_key, "todo");

        // Batalkan: ke stage terakhir, ditandai dibatalkan.
        let res = post(&request, &f.cookie, "/w/acme/t/KSR-1/stage/complete", &[]).await;
        assert_eq!(res.status_code(), 303);
        let res = post(&request, &f.cookie, "/w/acme/t/KSR-1/stage/complete", &[]).await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        to_decision().await;
        let res = post(
            &request,
            &f.cookie,
            "/w/acme/t/KSR-1/stage/decide",
            &[("choice", "cancel")],
        )
        .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let t = ticket(&ctx, &f.workspace, &code).await;
        assert_eq!(t.stage_key, "done");
        assert!(t.cancelled_at.is_some());
        let page = request
            .get("/w/acme/t/KSR-1")
            .add_cookie(f.cookie.clone())
            .await;
        assert!(page.text().contains("Tiket dibatalkan"));

        let events = ticket_events::Model::of_ticket(&ctx.db, f.workspace.workspace_id(), t.id)
            .await
            .unwrap();
        assert!(
            events.iter().any(|e| e.kind == "decided"),
            "{:?}",
            events.iter().map(|e| &e.kind).collect::<Vec<_>>()
        );
    })
    .await;
}

#[tokio::test]
async fn board_has_a_column_per_stage_and_drag_uses_stage_keys() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        use_workflow(&request, &ctx, &f, FLOW_YAML).await;
        create_ticket(&request, &f.cookie, &[("title", "Kartu")]).await;
        let board = request
            .get("/w/acme/p/KSR/board")
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(board.status_code(), 200);
        let html = board.text();
        for key in ["todo", "prd", "dev", "done"] {
            assert_css_exists(&html, &format!("[data-column='{key}']"));
        }
        assert_css_exists(&html, "[data-column='todo'] [data-ticket='KSR-1']");

        // Tiket baru langsung di stage pilihan (kolom "+").
        let page = request
            .get("/w/acme/p/KSR/tickets/new?stage=prd")
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(page.status_code(), 200);
        let code = create_ticket(
            &request,
            &f.cookie,
            &[("title", "Di PRD"), ("stage", "prd")],
        )
        .await;
        assert_eq!(ticket(&ctx, &f.workspace, &code).await.stage_key, "prd");
        // Stage setelah stage approval tidak bisa jadi stage awal.
        let res = request
            .post("/w/acme/p/KSR/tickets/new")
            .add_cookie(f.cookie.clone())
            .form(&[("title", "Di Dev"), ("stage", "dev")])
            .await;
        assert_eq!(res.status_code(), 422);
    })
    .await;
}

#[tokio::test]
async fn artifact_routes_respect_ticket_access() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        use_workflow(&request, &ctx, &f, FLOW_YAML).await;
        create_ticket(&request, &f.cookie, &[("title", "Rahasia")]).await;
        to_review(&request, &f.cookie).await;
        let t = ticket(&ctx, &f.workspace, "KSR-1").await;
        let artifact = ticket_artifacts::Model::find_named(
            &ctx.db,
            f.workspace.workspace_id(),
            t.id,
            "prd.md",
        )
        .await
        .unwrap()
        .unwrap();

        let stranger = a_user().create(&ctx).await;
        add_member(&ctx, &f.workspace, &stranger, RoleKey::Member).await;
        let cookie = sign_in_as(&request, &stranger).await;
        let base = format!("/w/acme/t/KSR-1/artifacts/{}", artifact.id);
        for path in [
            "/w/acme/t/KSR-1/artifacts".to_string(),
            format!("{base}/edit"),
            format!("{base}/compare"),
        ] {
            let res = request.get(&path).add_cookie(cookie.clone()).await;
            assert_eq!(res.status_code(), 404, "{path}");
        }
        for path in [
            "/w/acme/t/KSR-1/stage/approve",
            "/w/acme/t/KSR-1/stage/complete",
            "/w/acme/t/KSR-1/stage/reject",
            "/w/acme/t/KSR-1/stage/people",
        ] {
            let res = post(&request, &cookie, path, &[("note", "x")]).await;
            assert_eq!(res.status_code(), 404, "{path}");
        }
        let res = post(&request, &cookie, &base, &[("body", "x")]).await;
        assert_eq!(res.status_code(), 404);
        // Artifact tiket lain/id acak → 404.
        let res = request
            .get(&format!(
                "/w/acme/t/KSR-1/artifacts/{}/edit",
                uuid::Uuid::now_v7()
            ))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 404);
        let _ = tickets::Entity::find().all(&ctx.db).await.unwrap();
    })
    .await;
}

#[tokio::test]
async fn artifact_body_renders_sanitized_markdown() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        use_workflow(&request, &ctx, &f, FLOW_YAML).await;
        create_ticket(&request, &f.cookie, &[("title", "XSS artifact")]).await;
        post(&request, &f.cookie, "/w/acme/t/KSR-1/stage/complete", &[]).await;
        let res = post(
            &request,
            &f.cookie,
            "/w/acme/t/KSR-1/artifacts",
            &[("name", "")],
        )
        .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let t = ticket(&ctx, &f.workspace, "KSR-1").await;
        let artifact = ticket_artifacts::Model::find_named(
            &ctx.db,
            f.workspace.workspace_id(),
            t.id,
            "prd.md",
        )
        .await
        .unwrap()
        .unwrap();
        let evil = "# PRD\n\n- [x] **tebal**\n\n<script>alert(1)</script>\n\n\
                    <img src=x onerror=alert(2)>\n\n[klik](javascript:alert(3))";
        let res = post(
            &request,
            &f.cookie,
            &format!("/w/acme/t/KSR-1/artifacts/{}", artifact.id),
            &[("body", evil)],
        )
        .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        for path in [
            "/w/acme/t/KSR-1/artifacts".to_string(),
            format!("/w/acme/t/KSR-1/artifacts/{}/edit", artifact.id),
        ] {
            let html = request
                .get(&path)
                .add_cookie(f.cookie.clone())
                .await
                .text();
            assert!(!html.contains("<script>alert(1)"), "{path}: script lolos");
            assert!(!html.contains("<img src=x"), "{path}: img onerror lolos");
            assert!(!html.contains("href=\"javascript"), "{path}: javascript lolos");
        }
        let html = request
            .get("/w/acme/t/KSR-1/artifacts")
            .add_cookie(f.cookie.clone())
            .await
            .text();
        assert_css_exists(&html, "[data-artifact-body].app-md h1");
        assert_css_exists(&html, "[data-artifact-body] strong");
    })
    .await;
}
