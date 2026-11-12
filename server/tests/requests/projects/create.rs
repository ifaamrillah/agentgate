//! `/w/{slug}/projects/new` (`CreateProjects`), beranda workspace dengan daftar project, dan
//! pengaturan project (nama, deskripsi, arsip; `AdministerProject`).

use agentgate_server::{
    app::App,
    models::{
        audit_events::{AuditAction, AuditSubject},
        project_grants::{self, GrantSubject},
        projects,
        roles::RoleKey,
    },
};
use loco_rs::testing::prelude::*;

use crate::support::{a_user, a_workspace, add_member, builtin_role, sign_in_as};

#[tokio::test]
async fn project_key_unique_case_insensitive_and_normalized_upper() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let member = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let wid = workspace.workspace_id();
        add_member(&ctx, &workspace, &member, RoleKey::Member).await;
        // Member memegang CreateProjects.
        let cookie = sign_in_as(&request, &member).await;

        let form = request
            .get("/w/acme/projects/new")
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(form.status_code(), 200);
        assert_css_exists(
            &form.text(),
            "form[method=\"post\"][action=\"/w/acme/projects/new\"] input#project-key",
        );

        let res = request
            .post("/w/acme/projects/new")
            .add_cookie(cookie.clone())
            .form(&[
                ("key", " ksr "),
                ("name", " Kasir Online "),
                ("description", "Aplikasi kasir"),
            ])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(res.header("location"), "/w/acme/p/KSR");
        let project = projects::Model::find_by_key(&ctx.db, wid, "KSR")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(project.key, "KSR");
        assert_eq!(project.name, "Kasir Online");
        assert_eq!(project.created_by, Some(member.id));

        // Pembuat mendapat grant langsung dengan role bawaan Admin.
        let admin = builtin_role(&ctx, &workspace, RoleKey::Admin).await;
        let grants = project_grants::Model::of_project(&ctx.db, wid, project.id)
            .await
            .unwrap();
        assert_eq!(grants.len(), 1);
        assert_eq!(grants[0].subject().unwrap(), GrantSubject::User(member.id));
        assert_eq!(grants[0].role_id, admin.id);

        let events = crate::support::newest_audit_events(&ctx.db, wid, 10)
            .await
            .unwrap();
        let actions: Vec<_> = events
            .iter()
            .take(2)
            .map(|e| (e.action().unwrap(), e.subject().unwrap()))
            .collect();
        assert_eq!(
            actions,
            vec![
                (
                    AuditAction::ProjectAccessGranted,
                    AuditSubject::ProjectGrant
                ),
                (AuditAction::ProjectCreated, AuditSubject::Project),
            ]
        );
        assert_eq!(events[1].subject_id, project.id);
        assert_eq!(events[1].changes["key"], "KSR");

        // Key yang sama dengan huruf lain → 422 project-key-taken, form terisi kembali.
        let res = request
            .post("/w/acme/projects/new")
            .add_cookie(cookie.clone())
            .form(&[("key", "Ksr"), ("name", "Kasir Dua"), ("description", "")])
            .await;
        assert_eq!(res.status_code(), 422);
        let html = res.text();
        assert_css_exists(&html, "#project-key-error");
        assert_css_exists(&html, "input#project-key[value=\"KSR\"]");
        assert_css_exists(&html, "input#project-name[value=\"Kasir Dua\"]");
        assert!(html.contains("sudah dipakai"), "{html}");

        // Format key salah → 422.
        for bad in ["1AB", "K", "KASIR-ONLINE", "ABCDEFGHIJK"] {
            let res = request
                .post("/w/acme/projects/new")
                .add_cookie(cookie.clone())
                .form(&[("key", bad), ("name", "X"), ("description", "")])
                .await;
            assert_eq!(res.status_code(), 422, "{bad}");
            assert_css_exists(&res.text(), "#project-key-error");
        }

        // URL tidak membedakan huruf besar.
        let res = request
            .get("/w/acme/p/ksr")
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(res.status_code(), 200);
        let html = res.text();
        assert_css_exists(&html, "[data-project-home]");
        assert_css_exists(&html, "[data-board-empty]");
        // Sidebar: project aktif dengan menu "segera".
        assert_css_exists(
            &html,
            "[data-project-list] a[href=\"/w/acme/p/KSR\"][aria-current=\"page\"]",
        );
        assert_css_exists(&html, "[data-project-list] a[aria-disabled=\"true\"]");
        // Pembuat (Admin project) melihat tautan pengaturan project.
        assert_css_exists(&html, "a[href=\"/w/acme/p/KSR/settings\"]");
    })
    .await;
}

#[tokio::test]
async fn viewer_cannot_create_projects() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let viewer = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        add_member(&ctx, &workspace, &viewer, RoleKey::Viewer).await;
        let cookie = sign_in_as(&request, &viewer).await;

        let home = request.get("/w/acme").add_cookie(cookie.clone()).await;
        assert_css_not_exists(&home.text(), "[data-create-project]");
        let res = request
            .get("/w/acme/projects/new")
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(res.status_code(), 404);
        let res = request
            .post("/w/acme/projects/new")
            .add_cookie(cookie)
            .form(&[("key", "KSR"), ("name", "Kasir"), ("description", "")])
            .await;
        assert_eq!(res.status_code(), 404);
        assert!(
            projects::Model::find_by_key(&ctx.db, workspace.workspace_id(), "KSR")
                .await
                .unwrap()
                .is_none()
        );
    })
    .await;
}

#[tokio::test]
async fn project_settings_update_and_archive() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let wid = workspace.workspace_id();
        let cookie = sign_in_as(&request, &owner).await;
        let res = request
            .post("/w/acme/projects/new")
            .add_cookie(cookie.clone())
            .form(&[("key", "KSR"), ("name", "Kasir"), ("description", "")])
            .await;
        assert_eq!(res.status_code(), 303);

        let page = request
            .get("/w/acme/p/KSR/settings")
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(page.status_code(), 200);
        assert_css_exists(&page.text(), "input#project-name[value=\"Kasir\"]");

        let res = request
            .post("/w/acme/p/KSR/settings")
            .add_cookie(cookie.clone())
            .form(&[("name", " "), ("description", "x")])
            .await;
        assert_eq!(res.status_code(), 422);
        assert_css_exists(&res.text(), "#project-name-error");

        let res = request
            .post("/w/acme/p/KSR/settings")
            .add_cookie(cookie.clone())
            .form(&[("name", "Kasir Online"), ("description", "Aplikasi kasir")])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(res.header("location"), "/w/acme/p/KSR/settings");
        let project = projects::Model::find_by_key(&ctx.db, wid, "KSR")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(project.name, "Kasir Online");
        assert_eq!(project.description, "Aplikasi kasir");

        let res = request
            .post("/w/acme/p/KSR/settings/archive")
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let project = projects::Model::find_by_key(&ctx.db, wid, "KSR")
            .await
            .unwrap()
            .unwrap();
        assert!(project.is_archived());

        // Diarsipkan tetap bisa dibuka, dan ditandai di beranda.
        let res = request
            .get("/w/acme/p/KSR")
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(res.status_code(), 200);
        assert_css_exists(&res.text(), "[data-project-archived]");
        let home = request.get("/w/acme").add_cookie(cookie).await.text();
        assert_css_exists(&home, "[data-project=\"KSR\"] [data-project-archived]");

        let events = crate::support::newest_audit_events(&ctx.db, wid, 10)
            .await
            .unwrap();
        assert_eq!(events[0].action().unwrap(), AuditAction::ProjectArchived);
        assert_eq!(events[1].action().unwrap(), AuditAction::ProjectUpdated);
        assert_eq!(
            events[1].changes["name"],
            serde_json::json!({ "from": "Kasir", "to": "Kasir Online" })
        );
    })
    .await;
}
