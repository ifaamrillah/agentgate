//! Sampah: hapus → sampah (tiket hilang dari board dan 404), pulihkan, hapus permanen (Admin
//! project atau pembuat), audit; purge otomatis 30 hari ada di `tests/workers/cleanup.rs`.

use agentgate_server::{
    app::App,
    models::{audit_events::AuditAction, project_grants::GrantSubject, roles::RoleKey, tickets},
};
use loco_rs::testing::prelude::*;
use sea_orm::EntityTrait;

use super::{create_ticket, fixture, ticket};
use crate::support::{a_user, add_member, builtin_role, grant, newest_audit_events, sign_in_as};

#[tokio::test]
async fn trash_restore_and_purge() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let wid = f.workspace.workspace_id();
        let code = create_ticket(&request, &f.cookie, &[("title", "Salah ketik")]).await;
        let t = ticket(&ctx, &f.workspace, &code).await;

        let res = request
            .post(&format!("/w/acme/t/{code}/delete"))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(res.header("location"), "/w/acme/p/KSR/board");
        assert!(ticket(&ctx, &f.workspace, &code).await.deleted_at.is_some());
        let res = request
            .get(&format!("/w/acme/t/{code}"))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 404);
        let board = request
            .get("/w/acme/p/KSR/board")
            .add_cookie(f.cookie.clone())
            .await;
        assert_css_not_exists(&board.text(), "[data-ticket]");
        let page = request
            .get("/w/acme/p/KSR/trash")
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(page.status_code(), 200);
        assert_css_exists(&page.text(), &format!("[data-trashed='{code}']"));

        let res = request
            .post(&format!("/w/acme/p/KSR/trash/{}/restore", t.id))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303);
        assert!(ticket(&ctx, &f.workspace, &code).await.deleted_at.is_none());

        // Hapus permanen hanya dari sampah.
        let res = request
            .post(&format!("/w/acme/p/KSR/trash/{}/purge", t.id))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 404);
        request
            .post(&format!("/w/acme/t/{code}/delete"))
            .add_cookie(f.cookie.clone())
            .await;
        let res = request
            .post(&format!("/w/acme/p/KSR/trash/{}/purge", t.id))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303);
        assert!(
            tickets::Entity::find_by_id(t.id)
                .one(&ctx.db)
                .await
                .unwrap()
                .is_none()
        );
        let events = newest_audit_events(&ctx.db, wid, 3).await.unwrap();
        let actions: Vec<_> = events.iter().map(|e| e.action().unwrap()).collect();
        assert_eq!(
            actions,
            [
                AuditAction::TicketPurged,
                AuditAction::TicketTrashed,
                AuditAction::TicketRestored
            ]
        );
    })
    .await;
}

#[tokio::test]
async fn only_project_admin_or_creator_can_purge() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let wid = f.workspace.workspace_id();
        let ksr = agentgate_server::models::projects::Model::find_by_key(&ctx.db, wid, "KSR")
            .await
            .unwrap()
            .unwrap();
        let dewi = a_user().create(&ctx).await;
        add_member(&ctx, &f.workspace, &dewi, RoleKey::Member).await;
        let member = builtin_role(&ctx, &f.workspace, RoleKey::Member).await;
        grant(
            &ctx,
            &f.workspace,
            &ksr,
            GrantSubject::User(dewi.id),
            &member,
        )
        .await;
        let code = create_ticket(&request, &f.cookie, &[("title", "Milik owner")]).await;
        let t = ticket(&ctx, &f.workspace, &code).await;

        // Dewi (Member project, bukan pembuat) boleh membuang dan memulihkan, tidak menghapus
        // permanen.
        let cookie = sign_in_as(&request, &dewi).await;
        let res = request
            .post(&format!("/w/acme/t/{code}/delete"))
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303);
        let page = request
            .get("/w/acme/p/KSR/trash")
            .add_cookie(cookie.clone())
            .await;
        assert_css_not_exists(&page.text(), "[data-purge]");
        let res = request
            .post(&format!("/w/acme/p/KSR/trash/{}/purge", t.id))
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(res.status_code(), 404);
        assert!(
            tickets::Entity::find_by_id(t.id)
                .one(&ctx.db)
                .await
                .unwrap()
                .is_some()
        );
        // Tiket buatan Dewi sendiri boleh dihapus permanen olehnya.
        let mine = create_ticket(&request, &cookie, &[("title", "Milik Dewi")]).await;
        let mine = ticket(&ctx, &f.workspace, &mine).await;
        request
            .post(&format!("/w/acme/t/{}/delete", mine.code))
            .add_cookie(cookie.clone())
            .await;
        let res = request
            .post(&format!("/w/acme/p/KSR/trash/{}/purge", mine.id))
            .add_cookie(cookie)
            .await;
        assert_eq!(res.status_code(), 303);
    })
    .await;
}
