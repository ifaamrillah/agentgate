//! `GET|POST /w/{slug}/settings`: nama dan slug workspace (`ManageWorkspace`).

use agentgate_server::{
    app::App,
    models::{
        audit_events::{self, AuditAction},
        workspaces,
    },
};
use loco_rs::testing::prelude::*;

use crate::support::{a_user, a_workspace, sign_in_as};

#[tokio::test]
async fn owner_updates_name_and_slug_and_audit_is_written() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner)
            .name("Acme")
            .slug("acme")
            .create(&ctx)
            .await;
        let cookie = sign_in_as(&request, &owner).await;

        let form = request
            .get("/w/acme/settings")
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(form.status_code(), 200);
        let html = form.text();
        assert_css_exists(&html, "form[method=\"post\"][action=\"/w/acme/settings\"]");
        assert_css_exists(&html, "input#workspace-name[value=\"Acme\"]");

        let res = request
            .post("/w/acme/settings")
            .add_cookie(cookie.clone())
            .form(&[("name", "Acme Baru"), ("slug", "Acme-Baru")])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(res.header("location"), "/w/acme-baru/settings");

        let updated = workspaces::Model::find_by_slug(&ctx.db, "acme-baru")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.id, workspace.id);
        assert_eq!(updated.name, "Acme Baru");

        let events = audit_events::Model::newest_first(&ctx.db, workspace.workspace_id(), 10)
            .await
            .unwrap();
        assert_eq!(events[0].action().unwrap(), AuditAction::WorkspaceUpdated);
        assert_eq!(events[0].actor_id, Some(owner.id));
        assert_eq!(
            events[0].changes,
            serde_json::json!({
                "name": { "from": "Acme", "to": "Acme Baru" },
                "slug": { "from": "acme", "to": "acme-baru" },
            })
        );

        let page = request
            .get("/w/acme-baru/settings")
            .add_cookie(cookie.clone())
            .await;
        assert!(page.text().contains("Pengaturan workspace disimpan"));
        // Slug lama tidak lagi me-resolve.
        assert_eq!(
            request
                .get("/w/acme")
                .add_cookie(cookie)
                .await
                .status_code(),
            404
        );
    })
    .await;
}

#[tokio::test]
async fn settings_without_changes_writes_no_audit() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner)
            .name("Acme")
            .slug("acme")
            .create(&ctx)
            .await;
        let cookie = sign_in_as(&request, &owner).await;
        let res = request
            .post("/w/acme/settings")
            .add_cookie(cookie)
            .form(&[("name", "Acme"), ("slug", "acme")])
            .await;
        assert_eq!(res.status_code(), 303);
        let events = audit_events::Model::newest_first(&ctx.db, workspace.workspace_id(), 10)
            .await
            .unwrap();
        assert_eq!(events.len(), 1, "hanya workspace.created");
    })
    .await;
}

#[tokio::test]
async fn settings_slug_taken_or_invalid_gets_422() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        a_workspace(&owner).slug("acme").create(&ctx).await;
        a_workspace(&owner).slug("lain").create(&ctx).await;
        let cookie = sign_in_as(&request, &owner).await;
        for slug in ["LAIN", "a", "pakai spasi", "new"] {
            let res = request
                .post("/w/acme/settings")
                .add_cookie(cookie.clone())
                .form(&[("name", "Acme"), ("slug", slug)])
                .await;
            assert_eq!(res.status_code(), 422, "{slug}");
            assert_css_exists(&res.text(), "#workspace-slug-error");
        }
    })
    .await;
}
