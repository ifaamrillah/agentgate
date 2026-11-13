//! Knowledge base (M8): halaman project (beranda, kelompok, editor, versi, tinjau usulan), usulan
//! AI lewat MCP (propose → approve/tolak/batal), konflik simpan, XSS, pencarian, batasan persona,
//! pemicu tiket Done.

use agentgate_server::{
    app::App,
    models::{
        knowledge_docs, knowledge_proposals, knowledge_runs, personas,
        project_grants::GrantSubject, roles::RoleKey,
    },
};
use loco_rs::{TestServer, auth::cookies::Cookie, testing::prelude::*};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter,
};
use serde_json::json;

use super::{
    mcp::call,
    tickets::{
        Fixture, create_ticket, fixture,
        stages::{ksr, member},
    },
};
use crate::support::{a_user, add_member, builtin_role, grant, oauth_flow, sign_in_as};

const BASE: &str = "/w/acme/p/KSR/knowledge";

async fn post(
    request: &TestServer,
    cookie: &Cookie<'static>,
    path: &str,
    form: &[(&str, &str)],
) -> axum_test::TestResponse {
    request
        .post(path)
        .add_cookie(cookie.clone())
        .form(&form)
        .await
}

async fn get(
    request: &TestServer,
    cookie: &Cookie<'static>,
    path: &str,
) -> axum_test::TestResponse {
    request.get(path).add_cookie(cookie.clone()).await
}

async fn doc(ctx: &loco_rs::app::AppContext, f: &Fixture, slug: &str) -> knowledge_docs::Model {
    knowledge_docs::Entity::find()
        .filter(knowledge_docs::Column::WorkspaceId.eq(f.workspace.id))
        .filter(knowledge_docs::Column::Slug.eq(slug))
        .one(&ctx.db)
        .await
        .unwrap()
        .expect("dokumen ada")
}

async fn proposals_of(
    ctx: &loco_rs::app::AppContext,
    f: &Fixture,
) -> Vec<knowledge_proposals::Model> {
    knowledge_proposals::Entity::find()
        .filter(knowledge_proposals::Column::WorkspaceId.eq(f.workspace.id))
        .all(&ctx.db)
        .await
        .unwrap()
}

#[tokio::test]
async fn owner_creates_edits_and_restores_a_doc() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;

        let res = get(&request, &f.cookie, BASE).await;
        assert_eq!(res.status_code(), 200, "{}", res.text());
        assert_css_exists(&res.text(), "[data-kb-empty]");

        let res = post(
            &request,
            &f.cookie,
            &format!("{BASE}/new"),
            &[("type_key", "overview"), ("title", "Overview Kasir")],
        )
        .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(
            res.header("location"),
            "/w/acme/p/KSR/knowledge/overview-kasir"
        );
        let d = doc(&ctx, &f, "overview-kasir").await;
        assert_eq!(d.version, 1);

        let res = get(&request, &f.cookie, &format!("{BASE}/overview-kasir")).await;
        assert_eq!(res.status_code(), 200, "{}", res.text());
        assert_css_exists(&res.text(), "[data-kb-editor]");

        // Simpan versi baru, isi berbahaya tetap ditampilkan sebagai teks.
        let evil = "# Alur\n\n<script>alert(1)</script>\n\nKasir membuka shift.";
        let res = post(
            &request,
            &f.cookie,
            &format!("{BASE}/overview-kasir"),
            &[
                ("title", "Overview Kasir"),
                ("body", evil),
                ("base_version", "1"),
                ("message", "Tambah alur"),
            ],
        )
        .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(doc(&ctx, &f, "overview-kasir").await.version, 2);
        let page = get(&request, &f.cookie, &format!("{BASE}/overview-kasir"))
            .await
            .text();
        assert!(!page.contains("<script>alert(1)"), "XSS lolos");

        // Konflik: versi dasar lama → 409, isian tetap ada.
        let res = post(
            &request,
            &f.cookie,
            &format!("{BASE}/overview-kasir"),
            &[
                ("title", "Overview Kasir"),
                ("body", "Isian saya yang tidak boleh hilang"),
                ("base_version", "1"),
            ],
        )
        .await;
        assert_eq!(res.status_code(), 409);
        assert!(res.text().contains("Isian saya yang tidak boleh hilang"));
        assert_eq!(doc(&ctx, &f, "overview-kasir").await.version, 2);

        // Riwayat + pulihkan v1 = versi baru 3.
        let res = get(
            &request,
            &f.cookie,
            &format!("{BASE}/overview-kasir/versions?v=1"),
        )
        .await;
        assert_eq!(res.status_code(), 200, "{}", res.text());
        assert_css_exists(&res.text(), "[data-kb-sbs]");
        let res = post(
            &request,
            &f.cookie,
            &format!("{BASE}/overview-kasir/versions/1/restore"),
            &[],
        )
        .await;
        assert_eq!(res.status_code(), 303);
        let d = doc(&ctx, &f, "overview-kasir").await;
        assert_eq!(d.version, 3);
        assert!(!d.body.contains("script"));

        // Beranda, kelompok, unduh, cari.
        let res = get(&request, &f.cookie, BASE).await;
        assert_eq!(res.status_code(), 200);
        assert_css_exists(&res.text(), "[data-group='gambaran-umum']");
        let res = get(&request, &f.cookie, &format!("{BASE}/g/gambaran-umum")).await;
        assert_eq!(res.status_code(), 200, "{}", res.text());
        assert_css_exists(&res.text(), "[data-doc='overview-kasir']");
        let res = get(&request, &f.cookie, &format!("{BASE}/overview-kasir/raw")).await;
        assert_eq!(res.status_code(), 200);
        assert_eq!(res.text(), d.body);
        let res = get(&request, &f.cookie, &format!("{BASE}?q=Overview")).await;
        assert_css_exists(&res.text(), "[data-kb-results] .app-kb-hit");

        // Arsipkan → 404.
        let res = post(
            &request,
            &f.cookie,
            &format!("{BASE}/overview-kasir/archive"),
            &[],
        )
        .await;
        assert_eq!(res.status_code(), 303);
        let res = get(&request, &f.cookie, &format!("{BASE}/overview-kasir")).await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}

#[tokio::test]
async fn ai_proposals_are_reviewed_by_the_owner() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let (_, mcp, _) =
            oauth_flow::connect(&request, &f.cookie, &f.workspace.id.to_string()).await;

        // Usulan dokumen baru.
        let (err, value) = call(
            &request,
            &mcp,
            "ag_knowledge",
            json!({
                "action": "propose", "project": "KSR", "title": "Alur Refund",
                "type": "overview", "body": "# Refund\n\nSupervisor menyetujui.",
                "reason": "Dari tiket refund",
            }),
        )
        .await;
        assert!(!err, "{value}");
        assert_eq!(value["new_doc"], true);
        let id = value["proposal"].as_str().unwrap().to_string();

        let res = get(&request, &f.cookie, &format!("{BASE}/proposals")).await;
        assert_eq!(res.status_code(), 200, "{}", res.text());
        let html = res.text();
        assert_css_exists(&html, "[data-kb-review]");
        assert_css_exists(&html, "#kb-approve");
        assert!(html.contains("Supervisor menyetujui."));

        let res = post(&request, &f.cookie, &format!("{BASE}/proposals/{id}/approve"), &[]).await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let d = doc(&ctx, &f, "alur-refund").await;
        assert_eq!(d.version, 1);

        // Usulan perubahan → tolak tanpa alasan gagal, dengan alasan berhasil, lalu batal.
        let (err, value) = call(
            &request,
            &mcp,
            "ag_knowledge",
            json!({ "action": "propose", "project": "KSR", "slug": "alur-refund", "body": "# Refund\n\nManajer menyetujui." }),
        )
        .await;
        assert!(!err, "{value}");
        let id2 = value["proposal"].as_str().unwrap().to_string();
        let res = post(&request, &f.cookie, &format!("{BASE}/proposals/{id2}/reject"), &[("note", "")]).await;
        assert_eq!(res.status_code(), 303);
        assert!(proposals_of(&ctx, &f).await.iter().any(|p| p.id.to_string() == id2 && p.status == "pending"));
        let res = post(
            &request,
            &f.cookie,
            &format!("{BASE}/proposals/{id2}/reject"),
            &[("note", "Masih supervisor")],
        )
        .await;
        assert_eq!(res.status_code(), 303);
        assert!(proposals_of(&ctx, &f).await.iter().any(|p| p.id.to_string() == id2 && p.status == "rejected"));
        let res = post(&request, &f.cookie, &format!("{BASE}/proposals/{id2}/undo"), &[]).await;
        assert_eq!(res.status_code(), 303);
        assert!(proposals_of(&ctx, &f).await.iter().any(|p| p.id.to_string() == id2 && p.status == "pending"));

        // "Edit dulu": approve dengan isi yang diedit.
        let res = get(&request, &f.cookie, &format!("{BASE}/alur-refund?proposal={id2}")).await;
        assert_eq!(res.status_code(), 200);
        assert!(res.text().contains("Manajer menyetujui."));
        let res = post(
            &request,
            &f.cookie,
            &format!("{BASE}/proposals/{id2}/approve"),
            &[("body", "# Refund\n\nManajer toko menyetujui."), ("base_version", "1")],
        )
        .await;
        assert_eq!(res.status_code(), 303);
        let d = doc(&ctx, &f, "alur-refund").await;
        assert_eq!(d.version, 2);
        assert!(d.body.contains("Manajer toko"));

        // Agent membaca + mencari lewat MCP.
        let (err, value) = call(&request, &mcp, "ag_knowledge", json!({ "action": "read", "project": "KSR", "slug": "alur-refund" })).await;
        assert!(!err, "{value}");
        assert_eq!(value["version"], 2);
        let (err, value) = call(&request, &mcp, "ag_knowledge", json!({ "action": "list", "project": "KSR" })).await;
        assert!(!err, "{value}");
        assert_eq!(value["docs"][0]["slug"], "alur-refund");
    })
    .await;
}

#[tokio::test]
async fn members_and_viewers_get_limited_actions() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let (rina, rina_cookie) = member(&request, &ctx, &f, "Rina Member").await;
        let viewer = a_user().name("Vino Viewer").create(&ctx).await;
        add_member(&ctx, &f.workspace, &viewer, RoleKey::Viewer).await;
        let role = builtin_role(&ctx, &f.workspace, RoleKey::Viewer).await;
        grant(&ctx, &f.workspace, &ksr(&ctx, &f).await, GrantSubject::User(viewer.id), &role).await;
        let viewer_cookie = sign_in_as(&request, &viewer).await;

        post(&request, &f.cookie, &format!("{BASE}/new"), &[("type_key", "overview")]).await;
        let (_, mcp, _) =
            oauth_flow::connect(&request, &f.cookie, &f.workspace.id.to_string()).await;
        let (err, value) = call(
            &request,
            &mcp,
            "ag_knowledge",
            json!({ "action": "propose", "project": "KSR", "slug": "overview-project", "body": "Baru" }),
        )
        .await;
        assert!(!err, "{value}");
        let id = value["proposal"].as_str().unwrap().to_string();

        // Viewer: boleh baca, tidak boleh menyimpan.
        let res = get(&request, &viewer_cookie, &format!("{BASE}/overview-project")).await;
        assert_eq!(res.status_code(), 200, "{}", res.text());
        assert_css_not_exists(&res.text(), "[data-kb-editor]");
        let res = post(
            &request,
            &viewer_cookie,
            &format!("{BASE}/overview-project"),
            &[("title", "x"), ("body", "x"), ("base_version", "1")],
        )
        .await;
        assert_eq!(res.status_code(), 404);

        // Member bukan pemilik knowledge: tidak bisa memutuskan usulan.
        let res = get(&request, &rina_cookie, &format!("{BASE}/proposals")).await;
        assert_css_not_exists(&res.text(), "#kb-approve");
        post(&request, &rina_cookie, &format!("{BASE}/proposals/{id}/approve"), &[]).await;
        assert!(proposals_of(&ctx, &f).await.iter().all(|p| p.status == "pending"));

        // Owner menunjuk Rina sebagai pemilik knowledge → Rina bisa approve.
        let res = post(
            &request,
            &f.cookie,
            &format!("{BASE}/owner"),
            &[("owner", &rina.to_string())],
        )
        .await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(ksr(&ctx, &f).await.knowledge_owner_id, Some(rina));
        let res = post(&request, &rina_cookie, &format!("{BASE}/proposals/{id}/approve"), &[]).await;
        assert_eq!(res.status_code(), 303);
        assert!(proposals_of(&ctx, &f).await.iter().all(|p| p.status == "approved"));
    })
    .await;
}

#[tokio::test]
async fn persona_knowledge_types_limit_what_agents_read() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        post(&request, &f.cookie, &format!("{BASE}/new"), &[("type_key", "overview")]).await;
        let qa = personas::Entity::find()
            .filter(personas::Column::WorkspaceId.eq(f.workspace.id))
            .filter(personas::Column::Key.eq("qa"))
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        let mut m = qa.into_active_model();
        m.knowledge_types = Set(vec!["tech-stack".to_string()]);
        m.update(&ctx.db).await.unwrap();

        let (_, mcp, _) =
            oauth_flow::connect(&request, &f.cookie, &f.workspace.id.to_string()).await;
        let (err, value) = call(
            &request,
            &mcp,
            "ag_knowledge",
            json!({ "action": "read", "project": "KSR", "slug": "overview-project", "persona": "qa" }),
        )
        .await;
        assert!(err, "{value}");
        let (err, value) = call(
            &request,
            &mcp,
            "ag_knowledge",
            json!({ "action": "list", "project": "KSR", "persona": "qa" }),
        )
        .await;
        assert!(!err);
        assert_eq!(value["docs"].as_array().unwrap().len(), 0, "{value}");
        let (err, value) = call(
            &request,
            &mcp,
            "ag_knowledge",
            json!({ "action": "read", "project": "KSR", "slug": "overview-project", "persona": "dev" }),
        )
        .await;
        assert!(!err, "{value}");
    })
    .await;
}

#[tokio::test]
async fn ticket_done_requests_a_knowledge_update() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let code = create_ticket(
            &request,
            &f.cookie,
            &[("title", "Refund parsial"), ("type", "bug")],
        )
        .await;
        // Tanpa dokumen → tidak ada run.
        let runs = || async {
            knowledge_runs::Entity::find()
                .filter(knowledge_runs::Column::WorkspaceId.eq(f.workspace.id))
                .all(&ctx.db)
                .await
                .unwrap()
        };
        post(
            &request,
            &f.cookie,
            &format!("{BASE}/new"),
            &[("type_key", "overview")],
        )
        .await;
        let res = post(
            &request,
            &f.cookie,
            &format!("/w/acme/t/{code}/status"),
            &[("status", "done")],
        )
        .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let list = runs().await;
        assert_eq!(list.len(), 1, "{list:?}");
        assert_eq!(list[0].trigger, "done");
        assert_eq!(list[0].kind, "update");
        assert!(list[0].ticket_id.is_some());

        // Beranda menampilkan run yang sedang antre.
        let res = get(&request, &f.cookie, BASE).await;
        assert_css_exists(&res.text(), "[data-kb-run]");
    })
    .await;
}

#[tokio::test]
async fn workspace_knowledge_settings_manage_docs_types_and_triggers() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let ws = "/w/acme/settings/knowledge";
        let res = get(&request, &f.cookie, ws).await;
        assert_eq!(res.status_code(), 303);
        for page in ["docs", "types", "auto", "docs/new", "types/new"] {
            let res = get(&request, &f.cookie, &format!("{ws}/{page}")).await;
            assert_eq!(res.status_code(), 200, "{page}: {}", res.text());
        }

        // Dokumen workspace: buat → ubah meta (persona qa saja).
        let res = post(
            &request,
            &f.cookie,
            &format!("{ws}/docs/new"),
            &[("title", "Standar coding"), ("body", "## Penamaan\n\ncamelCase")],
        )
        .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(res.header("location"), "/w/acme/settings/knowledge/docs/standar-coding");
        let res = post(
            &request,
            &f.cookie,
            &format!("{ws}/docs/standar-coding"),
            &[
                ("title", "Standar coding"),
                ("body", "## Penamaan\n\ncamelCase"),
                ("base_version", "1"),
                ("personas", "qa"),
            ],
        )
        .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let d = doc(&ctx, &f, "standar-coding").await;
        assert!(d.project_id.is_none());
        assert_eq!(d.personas, vec!["qa".to_string()]);
        let res = get(&request, &f.cookie, &format!("{ws}/docs/standar-coding")).await;
        assert_eq!(res.status_code(), 200, "{}", res.text());

        // Agent: dev tidak membaca dokumen untuk qa.
        let (_, mcp, _) =
            oauth_flow::connect(&request, &f.cookie, &f.workspace.id.to_string()).await;
        let (err, _) = call(
            &request,
            &mcp,
            "ag_knowledge",
            json!({ "action": "read", "project": "KSR", "slug": "standar-coding", "persona": "dev" }),
        )
        .await;
        assert!(err);
        let (err, value) = call(
            &request,
            &mcp,
            "ag_knowledge",
            json!({ "action": "read", "project": "KSR", "slug": "standar-coding", "persona": "qa" }),
        )
        .await;
        assert!(!err, "{value}");

        // Jenis custom → muncul di katalog project.
        let res = post(
            &request,
            &f.cookie,
            &format!("{ws}/types/new"),
            &[("name", "SOP tutup kasir"), ("group_key", "operasional"), ("template", "## Langkah"), ("ai_scan", "on")],
        )
        .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let res = post(&request, &f.cookie, &format!("{ws}/types/new"), &[("name", ""), ("group_key", "operasional")]).await;
        assert_eq!(res.status_code(), 422);
        let res = post(
            &request,
            &f.cookie,
            &format!("{BASE}/new"),
            &[("type_key", "x-sop-tutup-kasir")],
        )
        .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert!(res.header("location").to_str().unwrap().contains("/knowledge/sop-tutup-kasir"));

        // Update otomatis: matikan Done.
        let res = post(
            &request,
            &f.cookie,
            &format!("{ws}/auto"),
            &[("on_merge", "on"), ("audit", "on"), ("audit_day", "1"), ("default_depth", "cepat")],
        )
        .await;
        assert_eq!(res.status_code(), 303);
        let s = agentgate_server::models::knowledge_settings::Model::of(&ctx.db, f.workspace.workspace_id())
            .await
            .unwrap();
        assert!(!s.on_done && s.audit);
        assert_eq!(s.default_depth, "cepat");

        // Member tanpa izin: 404.
        let (_, rina_cookie) = member(&request, &ctx, &f, "Rina").await;
        let res = get(&request, &rina_cookie, &format!("{ws}/docs")).await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}
