//! Pengaturan workflow (M3): daftar, template bawaan, impor/ekspor YAML, builder (stage, urutan,
//! YAML), terbitkan versi + migrasi tiket di stage yang dihapus, default per tipe, workflow per
//! project, dan izin.

use agentgate_server::{
    app::App,
    models::{
        project_grants::GrantSubject, projects, roles::RoleKey, ticket_events, ticket_types,
        workflows,
    },
};
use loco_rs::{TestServer, app::AppContext, auth::cookies::Cookie, testing::prelude::*};
use uuid::Uuid;

use super::{
    Fixture, create_ticket, fixture,
    stages::{FLOW_YAML, use_workflow},
    ticket,
};
use crate::support::{a_user, add_member, builtin_role, grant, sign_in_as};

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

async fn get(
    request: &TestServer,
    cookie: &Cookie<'static>,
    path: &str,
) -> axum_test::TestResponse {
    request.get(path).add_cookie(cookie.clone()).await
}

async fn workflow(ctx: &AppContext, f: &Fixture, id: &str) -> workflows::Model {
    workflows::Model::find_in(
        &ctx.db,
        f.workspace.workspace_id(),
        Uuid::parse_str(id).unwrap(),
    )
    .await
    .unwrap()
    .expect("workflow ada")
}

fn id_from(res: &axum_test::TestResponse) -> String {
    res.header("location")
        .to_str()
        .unwrap()
        .rsplit('/')
        .next()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn workflow_is_built_from_a_template_edited_and_published() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let c = &f.cookie;

        let page = get(&request, c, "/w/acme/settings/workflows").await;
        assert_eq!(page.status_code(), 200);
        assert_css_exists(&page.text(), "[data-workflow='Kanban']");
        let page = get(&request, c, "/w/acme/settings/workflows?tab=templates").await;
        assert_eq!(page.status_code(), 200);
        let html = page.text();
        assert_eq!(
            html.matches("data-template ").count() + html.matches("data-template>").count(),
            9,
            "9 template"
        );
        let page = get(&request, c, "/w/acme/settings/workflows?tab=import").await;
        assert_css_exists(&page.text(), "[data-workflow-import]");

        // Dari template: nama kosong = nama template.
        let res = post(
            &request,
            c,
            "/w/acme/settings/workflows",
            &[("source", "template:superpowers"), ("name", "")],
        )
        .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let id = id_from(&res);
        let base = format!("/w/acme/settings/workflows/{id}");
        let w = workflow(&ctx, &f, &id).await;
        assert_eq!((w.name.as_str(), w.version), ("Superpowers-style", 1));
        // Nama duplikat / sumber tak dikenal → 422.
        let res = post(
            &request,
            c,
            "/w/acme/settings/workflows",
            &[
                ("source", "template:superpowers"),
                ("name", "Superpowers-style"),
            ],
        )
        .await;
        assert_eq!(res.status_code(), 422);
        let res = post(
            &request,
            c,
            "/w/acme/settings/workflows",
            &[("source", "template:nope"), ("name", "X")],
        )
        .await;
        assert_eq!(res.status_code(), 422);

        // Builder, konfigurasi stage, YAML, riwayat versi.
        let page = get(&request, c, &base).await;
        assert_eq!(page.status_code(), 200);
        assert_css_exists(&page.text(), "[data-flow] [data-stage='brainstorm']");
        let page = get(&request, c, &format!("{base}?stage=brainstorm")).await;
        assert_css_exists(&page.text(), "[data-stage-form]");
        assert_eq!(
            get(&request, c, &format!("{base}?tab=yaml"))
                .await
                .status_code(),
            200
        );
        assert_eq!(
            get(&request, c, &format!("{base}?tab=versions"))
                .await
                .status_code(),
            200
        );

        // Ubah stage → masuk draf; versi aktif tetap.
        let res = post(
            &request,
            c,
            &format!("{base}/stage"),
            &[
                ("key", "brainstorm"),
                ("name", "Brainstorm ide"),
                ("category", "in_progress"),
                ("persona", "analyst"),
                ("mode", "interactive"),
                ("exit", "approval"),
                ("gates", "artifact_exists"),
                ("output", "brainstorm.md"),
                ("fail_limit", "3"),
                ("fail_to", ""),
                ("wip", "4"),
            ],
        )
        .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let w = workflow(&ctx, &f, &id).await;
        assert_eq!(w.version, 1);
        let draft = w.draft().expect("draf ada");
        assert_eq!(draft.stage("brainstorm").unwrap().name, "Brainstorm ide");
        assert_eq!(
            draft.stage("brainstorm").unwrap().wip,
            Some(4),
            "batas WIP tersimpan"
        );
        assert_css_exists(&get(&request, c, &base).await.text(), "[data-draft-bar]");

        // Persona yang tidak ada di workspace → 422, draf tidak berubah.
        let res = post(
            &request,
            c,
            &format!("{base}/stage"),
            &[
                ("key", "brainstorm"),
                ("name", "Brainstorm ide"),
                ("category", "in_progress"),
                ("persona", "hantu"),
                ("mode", "interactive"),
                ("exit", "approval"),
                ("fail_limit", "3"),
            ],
        )
        .await;
        assert_eq!(res.status_code(), 422);
        assert!(
            res.text().contains("Persona tidak dikenal"),
            "{}",
            res.text()
        );
        let draft = workflow(&ctx, &f, &id).await.draft().expect("draf ada");
        assert_eq!(
            draft.stage("brainstorm").unwrap().persona.as_deref(),
            Some("analyst")
        );

        // Nama kosong → 422 di form stage.
        let res = post(
            &request,
            c,
            &format!("{base}/stage"),
            &[
                ("key", "brainstorm"),
                ("name", ""),
                ("category", "in_progress"),
            ],
        )
        .await;
        assert_eq!(res.status_code(), 422);
        assert_css_exists(&res.text(), "[data-stage-form]");

        // Tambah, pindah, hapus stage.
        let res = post(
            &request,
            c,
            &format!("{base}/stages"),
            &[("name", "Desain UI"), ("category", "in_progress")],
        )
        .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let res = post(
            &request,
            c,
            &format!("{base}/stages/move"),
            &[("key", "plan"), ("by", "-1")],
        )
        .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let res = post(
            &request,
            c,
            &format!("{base}/stages/remove"),
            &[("key", "backlog")],
        )
        .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let draft = workflow(&ctx, &f, &id).await.draft().unwrap();
        let keys: Vec<&str> = draft.stages.iter().map(|s| s.key.as_str()).collect();
        assert_eq!(&keys[..2], ["plan", "brainstorm"]);
        assert!(draft.stages.iter().any(|s| s.name == "Desain UI"));
        assert!(draft.stage("backlog").is_none());

        // YAML rusak → 422 dengan detail.
        let res = post(
            &request,
            c,
            &format!("{base}/yaml"),
            &[("yaml", "stages: [")],
        )
        .await;
        assert_eq!(res.status_code(), 422);

        // Terbitkan (tak ada tiket di stage yang dihapus) → v2, draf kosong.
        let res = post(&request, c, &format!("{base}/publish"), &[]).await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let w = workflow(&ctx, &f, &id).await;
        assert_eq!(w.version, 2);
        assert!(w.draft.is_none());
        let export = get(&request, c, &format!("{base}/export")).await;
        assert_eq!(export.status_code(), 200);
        assert!(export.text().contains("Brainstorm ide"));

        // Buang draf.
        post(
            &request,
            c,
            &format!("{base}/stages"),
            &[("name", "Sementara"), ("category", "todo")],
        )
        .await;
        assert!(workflow(&ctx, &f, &id).await.draft.is_some());
        let res = post(&request, c, &format!("{base}/discard"), &[]).await;
        assert_eq!(res.status_code(), 303);
        assert!(workflow(&ctx, &f, &id).await.draft.is_none());

        // Ubah nama.
        let res = post(
            &request,
            c,
            &format!("{base}/identity"),
            &[("name", ""), ("description", "")],
        )
        .await;
        assert_eq!(res.status_code(), 422);
        let res = post(
            &request,
            c,
            &format!("{base}/identity"),
            &[
                ("name", "Superpowers KSR"),
                ("description", "Untuk tim kasir"),
            ],
        )
        .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(workflow(&ctx, &f, &id).await.name, "Superpowers KSR");

        // Hapus: workflow tak terpakai bisa; Kanban (default tipe) tidak.
        let res = post(&request, c, &format!("{base}/delete"), &[]).await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(get(&request, c, &base).await.status_code(), 404);
        let kanban = workflows::Model::all_in(&ctx.db, f.workspace.workspace_id())
            .await
            .unwrap()
            .into_iter()
            .find(|w| w.name == "Kanban")
            .unwrap();
        let res = post(
            &request,
            c,
            &format!("/w/acme/settings/workflows/{}/delete", kanban.id),
            &[],
        )
        .await;
        assert_eq!(res.status_code(), 422);
    })
    .await;
}

#[tokio::test]
async fn yaml_import_round_trips_and_rejects_bad_documents() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let c = &f.cookie;
        let res = post(
            &request,
            c,
            "/w/acme/settings/workflows/import",
            &[("yaml", FLOW_YAML)],
        )
        .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let id = id_from(&res);
        let w = workflow(&ctx, &f, &id).await;
        assert_eq!(w.name, "Alur Uji");
        let def = w.definition(&ctx.db).await.unwrap();
        assert_eq!(def.stage("prd").unwrap().fail_limit, 2);

        // Ekspor lalu impor lagi (nama diganti) menghasilkan definisi sama.
        let yaml = get(
            &request,
            c,
            &format!("/w/acme/settings/workflows/{id}/export"),
        )
        .await
        .text();
        let copy = yaml.replace("nama: Alur Uji", "nama: Alur Uji 2");
        let res = post(
            &request,
            c,
            "/w/acme/settings/workflows/import",
            &[("yaml", &copy)],
        )
        .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let again = workflow(&ctx, &f, &id_from(&res))
            .await
            .definition(&ctx.db)
            .await
            .unwrap();
        assert_eq!(again.stages, def.stages);

        for bad in [
            "bukan: [yaml",
            "workflow: x\nnama: X\nstages: []\n",
            "workflow: x\nnama: X\nstages:\n  - id: a\n    nama: A\n    kategori: entah\n",
        ] {
            let res = post(
                &request,
                c,
                "/w/acme/settings/workflows/import",
                &[("yaml", bad)],
            )
            .await;
            assert_eq!(res.status_code(), 422, "{bad}");
            assert_css_exists(&res.text(), "#workflow-yaml-error");
        }
    })
    .await;
}

#[tokio::test]
async fn publishing_a_removed_stage_maps_its_tickets() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let c = &f.cookie;
        let id = use_workflow(&request, &ctx, &f, FLOW_YAML).await;
        let base = format!("/w/acme/settings/workflows/{id}");
        let code = create_ticket(&request, c, &[("title", "Di PRD")]).await;
        let res = post(&request, c, "/w/acme/t/KSR-1/stage/complete", &[]).await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(ticket(&ctx, &f.workspace, &code).await.stage_key, "prd");

        let res = post(
            &request,
            c,
            &format!("{base}/stages/remove"),
            &[("key", "prd")],
        )
        .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_css_exists(&get(&request, c, &base).await.text(), "[data-draft-bar]");
        // Terbitkan → halaman migrasi.
        let res = post(&request, c, &format!("{base}/publish"), &[]).await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(res.header("location"), format!("{base}/migrate"));
        let page = get(&request, c, &format!("{base}/migrate")).await;
        assert_eq!(page.status_code(), 200);
        assert_css_exists(&page.text(), "[data-removed-stage]");

        // Tujuan tak dikenal → 422; lalu petakan ke Dev.
        let res = post(
            &request,
            c,
            &format!("{base}/migrate"),
            &[("map_prd", "nope")],
        )
        .await;
        assert_eq!(res.status_code(), 422);
        let res = post(
            &request,
            c,
            &format!("{base}/migrate"),
            &[("map_prd", "dev")],
        )
        .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let t = ticket(&ctx, &f.workspace, &code).await;
        assert_eq!((t.stage_key.as_str(), t.stage_version), ("dev", 2));
        assert_eq!(workflow(&ctx, &f, &id).await.version, 2);
        let events = ticket_events::Model::of_ticket(&ctx.db, f.workspace.workspace_id(), t.id)
            .await
            .unwrap();
        assert!(events.iter().any(|e| e.kind == "workflow_migrated"));
    })
    .await;
}

#[tokio::test]
async fn type_defaults_and_project_overrides_move_tickets() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let c = &f.cookie;
        let wid = f.workspace.workspace_id();
        let code = create_ticket(&request, c, &[("title", "Ikut workflow")]).await;
        let t = ticket(&ctx, &f.workspace, &code).await;
        assert_eq!(t.stage_key, "todo");

        // Default tipe tiket ini → Alur Uji: tiket pindah ke stage sekunci (todo).
        let res = post(
            &request,
            c,
            "/w/acme/settings/workflows/import",
            &[("yaml", FLOW_YAML)],
        )
        .await;
        let id = id_from(&res);
        let field = format!("type_{}", t.type_id);
        let res = post(
            &request,
            c,
            "/w/acme/settings/workflows/defaults",
            &[(&field, &id)],
        )
        .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let ty = ticket_types::Model::find_in(&ctx.db, wid, t.type_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(ty.workflow_id.to_string(), id);
        let t = ticket(&ctx, &f.workspace, &code).await;
        assert_eq!(
            (t.workflow_id.to_string(), t.stage_key.as_str()),
            (id.clone(), "todo")
        );

        // Halaman workflow project; override kembali ke Kanban.
        let page = get(&request, c, "/w/acme/p/KSR/settings/workflow").await;
        assert_eq!(page.status_code(), 200);
        assert_css_exists(&page.text(), "[data-type-workflow]");
        let kanban = workflows::Model::all_in(&ctx.db, wid)
            .await
            .unwrap()
            .into_iter()
            .find(|w| w.name == "Kanban")
            .unwrap();
        let res = post(
            &request,
            c,
            "/w/acme/p/KSR/settings/workflow",
            &[
                (&field, &kanban.id.to_string()),
                ("allow_force_transition", "on"),
            ],
        )
        .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let t = ticket(&ctx, &f.workspace, &code).await;
        assert_eq!(t.workflow_id, kanban.id);
        let p = projects::Model::find_by_key(&ctx.db, wid, "KSR")
            .await
            .unwrap()
            .unwrap();
        assert!(p.allow_force_transition);
    })
    .await;
}

#[tokio::test]
async fn workflow_settings_need_permissions() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let member = a_user().create(&ctx).await;
        add_member(&ctx, &f.workspace, &member, RoleKey::Member).await;
        let ksr = projects::Model::find_by_key(&ctx.db, f.workspace.workspace_id(), "KSR")
            .await
            .unwrap()
            .unwrap();
        let role = builtin_role(&ctx, &f.workspace, RoleKey::Member).await;
        grant(
            &ctx,
            &f.workspace,
            &ksr,
            GrantSubject::User(member.id),
            &role,
        )
        .await;
        let cookie = sign_in_as(&request, &member).await;
        let kanban = workflows::Model::all_in(&ctx.db, f.workspace.workspace_id())
            .await
            .unwrap()
            .remove(0);

        for path in [
            "/w/acme/settings/workflows".to_string(),
            format!("/w/acme/settings/workflows/{}", kanban.id),
            format!("/w/acme/settings/workflows/{}/export", kanban.id),
            "/w/acme/p/KSR/settings/workflow".to_string(),
        ] {
            assert_eq!(
                get(&request, &cookie, &path).await.status_code(),
                404,
                "{path}"
            );
        }
        for path in [
            "/w/acme/settings/workflows".to_string(),
            "/w/acme/settings/workflows/import".to_string(),
            "/w/acme/settings/workflows/defaults".to_string(),
            format!("/w/acme/settings/workflows/{}/publish", kanban.id),
            format!("/w/acme/settings/workflows/{}/delete", kanban.id),
            "/w/acme/p/KSR/settings/workflow".to_string(),
        ] {
            let res = post(&request, &cookie, &path, &[("name", "x")]).await;
            assert_eq!(res.status_code(), 404, "{path}");
        }
        // Workflow workspace lain → 404 untuk pemilik acme.
        let res = get(
            &request,
            &f.cookie,
            &format!("/w/acme/settings/workflows/{}", Uuid::now_v7()),
        )
        .await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}
