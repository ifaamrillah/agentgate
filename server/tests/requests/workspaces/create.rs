//! `GET|POST /w/new`: membuat workspace (M1 Task 9).

use agentgate_server::{
    app::App,
    models::{
        audit_events::{self, AuditAction},
        roles::RoleKey,
        workspace_members, workspaces,
    },
};
use loco_rs::testing::prelude::*;

use crate::support::{a_user, a_workspace, sign_in_as};

#[tokio::test]
async fn signup_then_create_workspace_makes_user_owner() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().create(&ctx).await;
        let cookie = sign_in_as(&request, &user).await;

        let form = request.get("/w/new").add_cookie(cookie.clone()).await;
        assert_eq!(form.status_code(), 200);
        assert_css_exists(&form.text(), "form[method=\"post\"][action=\"/w/new\"]");

        let res = request
            .post("/w/new")
            .add_cookie(cookie.clone())
            .form(&[("name", "  Acme Studio  "), ("slug", "")])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(res.header("location"), "/w/acme-studio");

        let workspace = workspaces::Model::find_by_slug(&ctx.db, "acme-studio")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(workspace.name, "Acme Studio");
        assert_eq!(workspace.created_by, Some(user.id));
        let (_, role) =
            workspace_members::Model::find_with_role(&ctx.db, workspace.workspace_id(), user.id)
                .await
                .unwrap()
                .unwrap();
        assert_eq!(role.role_key().unwrap(), Some(RoleKey::Owner));

        let events = audit_events::Model::newest_first(&ctx.db, workspace.workspace_id(), 10)
            .await
            .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].action().unwrap(), AuditAction::WorkspaceCreated);
        assert_eq!(events[0].actor_id, Some(user.id));
        assert_eq!(events[0].subject_id, workspace.id);
        assert!(events[0].request_id.is_some(), "request id dari request");

        let home = request.get("/w/acme-studio").add_cookie(cookie).await;
        assert_eq!(home.status_code(), 200);
    })
    .await;
}

#[tokio::test]
async fn workspace_slug_unique_case_insensitive() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        a_workspace(&owner).slug("acme").create(&ctx).await;
        let user = a_user().create(&ctx).await;
        let cookie = sign_in_as(&request, &user).await;

        let res = request
            .post("/w/new")
            .add_cookie(cookie)
            .form(&[("name", "Acme Lain"), ("slug", "ACME")])
            .await;
        assert_eq!(res.status_code(), 422);
        let html = res.text();
        assert_css_exists(&html, "#workspace-slug-error");
        // Isian dipertahankan (setelah normalisasi huruf kecil).
        assert_css_exists(&html, "input#workspace-name[value=\"Acme Lain\"]");
        assert_css_exists(&html, "input#workspace-slug[value=\"acme\"]");
    })
    .await;
}

#[tokio::test]
async fn reserved_slug_rejected() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().create(&ctx).await;
        let cookie = sign_in_as(&request, &user).await;
        for slug in ["new", "Settings", "admin", "api", "auth"] {
            let res = request
                .post("/w/new")
                .add_cookie(cookie.clone())
                .form(&[("name", "Apa Saja"), ("slug", slug)])
                .await;
            assert_eq!(res.status_code(), 422, "{slug}");
            assert_css_exists(&res.text(), "#workspace-slug-error");
        }
        // Slug default dari nama juga dicek.
        let res = request
            .post("/w/new")
            .add_cookie(cookie)
            .form(&[("name", "New"), ("slug", "")])
            .await;
        assert_eq!(res.status_code(), 422);
        assert!(
            workspaces::Model::find_by_slug(&ctx.db, "new")
                .await
                .unwrap()
                .is_none()
        );
    })
    .await;
}

#[tokio::test]
async fn invalid_name_rerenders_form_with_422() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().create(&ctx).await;
        let cookie = sign_in_as(&request, &user).await;
        let res = request
            .post("/w/new")
            .add_cookie(cookie)
            .form(&[("name", "   "), ("slug", "kosong")])
            .await;
        assert_eq!(res.status_code(), 422);
        let html = res.text();
        assert_css_exists(&html, "#workspace-name-error");
        assert_css_exists(&html, "input#workspace-slug[value=\"kosong\"]");
    })
    .await;
}
