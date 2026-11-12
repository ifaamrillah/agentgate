//! Izin per project dan isolasi workspace: tanpa akses project, workspace lain, atau izin kurang
//! → 404 (bukan 403), termasuk POST.

use agentgate_server::{
    app::App,
    models::{project_grants::GrantSubject, roles::RoleKey},
};
use loco_rs::testing::prelude::*;

use super::{create_ticket, fixture, ticket};
use crate::support::{a_project, a_user, a_workspace, add_member, builtin_role, grant, sign_in_as};

#[tokio::test]
async fn project_permissions_gate_every_ticket_route() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let code = create_ticket(&request, &f.cookie, &[("title", "Rahasia")]).await;
        let project = a_project(&ctx, &f.workspace, "GDG").await;
        let _ = project;

        // Anggota tanpa grant di KSR.
        let stranger = a_user().create(&ctx).await;
        add_member(&ctx, &f.workspace, &stranger, RoleKey::Member).await;
        // Viewer di KSR.
        let viewer = a_user().create(&ctx).await;
        add_member(&ctx, &f.workspace, &viewer, RoleKey::Member).await;
        let ksr = agentgate_server::models::projects::Model::find_by_key(
            &ctx.db,
            f.workspace.workspace_id(),
            "KSR",
        )
        .await
        .unwrap()
        .unwrap();
        let viewer_role = builtin_role(&ctx, &f.workspace, RoleKey::Viewer).await;
        grant(
            &ctx,
            &f.workspace,
            &ksr,
            GrantSubject::User(viewer.id),
            &viewer_role,
        )
        .await;

        let other = sign_in_as(&request, &stranger).await;
        for path in [
            "/w/acme/p/KSR/board",
            "/w/acme/p/KSR/list",
            "/w/acme/p/KSR/tickets/new",
            "/w/acme/p/KSR/trash",
            "/w/acme/t/KSR-1",
            "/w/acme/t/KSR-1/activity",
        ] {
            let res = request.get(path).add_cookie(other.clone()).await;
            assert_eq!(res.status_code(), 404, "{path}");
        }
        for path in [
            "/w/acme/t/KSR-1",
            "/w/acme/t/KSR-1/status",
            "/w/acme/t/KSR-1/comments",
            "/w/acme/t/KSR-1/delete",
            "/w/acme/t/KSR-1/watch",
        ] {
            let res = request
                .post(path)
                .add_cookie(other.clone())
                .form(&[("status", "done"), ("body", "x"), ("title", "x")])
                .await;
            assert_eq!(res.status_code(), 404, "{path}");
        }
        // Pencarian dan Tiket saya tidak membocorkan tiket project tanpa akses.
        let res = request
            .get("/w/acme/search?q=Rahasia")
            .add_cookie(other.clone())
            .await;
        assert_eq!(res.status_code(), 200);
        assert_css_not_exists(&res.text(), "[data-ticket-row]");

        // Viewer: boleh lihat dan ikuti, tidak boleh mengubah.
        let view = sign_in_as(&request, &viewer).await;
        let res = request
            .get("/w/acme/t/KSR-1")
            .add_cookie(view.clone())
            .await;
        assert_eq!(res.status_code(), 200);
        assert_css_not_exists(&res.text(), "form[data-ticket-edit]");
        let res = request
            .post("/w/acme/t/KSR-1/watch")
            .add_cookie(view.clone())
            .form(&[("watch", "1")])
            .await;
        assert_eq!(res.status_code(), 303);
        for path in [
            "/w/acme/t/KSR-1",
            "/w/acme/t/KSR-1/status",
            "/w/acme/t/KSR-1/comments",
            "/w/acme/t/KSR-1/delete",
        ] {
            let res = request
                .post(path)
                .add_cookie(view.clone())
                .form(&[("status", "done"), ("body", "x"), ("title", "x")])
                .await;
            assert_eq!(res.status_code(), 404, "{path}");
        }
        let res = request
            .get("/w/acme/p/KSR/tickets/new")
            .add_cookie(view.clone())
            .await;
        assert_eq!(res.status_code(), 404);
        assert_eq!(ticket(&ctx, &f.workspace, &code).await.title, "Rahasia");
    })
    .await;
}

#[tokio::test]
async fn other_workspace_cannot_reach_tickets() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        create_ticket(&request, &f.cookie, &[("title", "Milik acme")]).await;
        let bob = a_user().create(&ctx).await;
        let bobs = a_workspace(&bob).slug("bob").create(&ctx).await;
        a_project(&ctx, &bobs, "KSR").await;
        let cookie = sign_in_as(&request, &bob).await;
        // Kode yang sama di workspace sendiri tidak ada; workspace acme bukan miliknya.
        for path in ["/w/bob/t/KSR-1", "/w/acme/t/KSR-1", "/w/acme/p/KSR/board"] {
            let res = request.get(path).add_cookie(cookie.clone()).await;
            assert_eq!(res.status_code(), 404, "{path}");
        }
        let res = request
            .post("/w/bob/t/KSR-1/status")
            .add_cookie(cookie)
            .form(&[("status", "done")])
            .await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}
